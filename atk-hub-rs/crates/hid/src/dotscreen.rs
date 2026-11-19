//! 6 x 6 RGB dot-matrix screens on ATK keyboards (`dotScreen:*` channels).
//!
//! Packets are `[0xAA, cmd, offset_lo, offset_hi, len, seq, 0, 0, data...]`
//! on report ID 0; replies echo `0xAA` and carry the sequence number as a
//! little-endian `u16` at bytes 5-6 (`0xFF` in byte 2 means rejected).

use crate::{HidError, HidTransport, Transport};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Vendor ID of the supported boards.
pub const VENDOR_ID: u16 = 0x373B;
/// Product IDs with a dot screen.
pub const PRODUCT_IDS: [u16; 2] = [4507, 4607];
const USAGE_PAGE: u16 = 0xFF60;
const USAGE: u16 = 0x61;

/// Screen size.
pub const SIDE: usize = 6;
/// Bytes in one frame (`6 x 6 x RGB`).
pub const FRAME_BYTES: usize = SIDE * SIDE * 3;
/// Config byte selecting the display mode.
pub const MODE_BYTE: usize = 30;
/// Mode that shows host-streamed frames.
pub const MODE_STREAM: u8 = 6;

mod cmd {
    pub const HANDSHAKE: u8 = 16;
    pub const READ_CONFIG: u8 = 20;
    pub const WRITE_CONFIG: u8 = 21;
    pub const BEGIN_CONFIG: u8 = 47;
    pub const FRAME: u8 = 59;
    pub const PREPARE: u8 = 61;
}

const CONFIG_CHUNKS: [(u16, u8); 3] = [(0, 24), (24, 24), (48, 16)];

/// One frame: rows of RGB pixels.
pub type Frame = [[[u8; 3]; SIDE]; SIDE];

/// An animation (same shape as the original's artwork JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artwork {
    /// Always 1.
    pub version: u8,
    /// Display name (1-40 characters).
    pub name: String,
    /// Frames per second (1-6).
    pub frame_rate: u8,
    /// 1-20 frames.
    pub frames: Vec<Frame>,
}

impl Artwork {
    /// Checks the limits the original enforces.
    ///
    /// # Errors
    ///
    /// Returns [`HidError::Invalid`] describing the first problem.
    pub fn validate(&self) -> Result<(), HidError> {
        let name = self.name.trim();
        if self.version != 1 {
            return Err(HidError::Invalid("unsupported artwork version".into()));
        }
        if name.is_empty() || name.chars().count() > 40 {
            return Err(HidError::Invalid("name must be 1-40 characters".into()));
        }
        if !(1..=6).contains(&self.frame_rate) {
            return Err(HidError::Invalid("frame rate must be 1-6".into()));
        }
        if !(1..=20).contains(&self.frames.len()) {
            return Err(HidError::Invalid("animations need 1-20 frames".into()));
        }
        Ok(())
    }
}

/// A connected dot-screen interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DotTarget {
    /// OS path.
    pub path: String,
    /// Vendor ID.
    pub vendor_id: u16,
    /// Product ID.
    pub product_id: u16,
    /// Product string.
    pub product: String,
}

/// Lists connected keyboards with a dot screen.
///
/// # Errors
///
/// Returns an error when hidapi cannot initialise.
pub fn list_dot_screens() -> Result<Vec<DotTarget>, HidError> {
    let api = hidapi::HidApi::new()?;
    Ok(api
        .device_list()
        .filter(|d| {
            d.vendor_id() == VENDOR_ID
                && PRODUCT_IDS.contains(&d.product_id())
                && d.usage_page() == USAGE_PAGE
                && d.usage() == USAGE
        })
        .map(|d| DotTarget {
            path: d.path().to_string_lossy().into_owned(),
            vendor_id: d.vendor_id(),
            product_id: d.product_id(),
            product: d.product_string().unwrap_or("ATK keyboard").to_owned(),
        })
        .collect())
}

/// Builds one command packet (without the report ID).
#[must_use]
pub fn encode(command: u8, offset: u16, data: &[u8], len: u8, seq: u8) -> Vec<u8> {
    let [lo, hi] = offset.to_le_bytes();
    let mut packet = vec![0xAA, command, lo, hi, len, seq, 0, 0];
    packet.extend_from_slice(data);
    packet.resize(packet.len().max(32), 0);
    packet
}

/// Strips an echoed report ID and checks that `reply` answers `seq`.
fn answer(reply: &[u8], seq: u8) -> Option<&[u8]> {
    let reply = if reply.first() == Some(&0) && reply.get(1) == Some(&0xAA) {
        &reply[1..]
    } else {
        reply
    };
    (reply.len() >= 8
        && reply[0] == 0xAA
        && u16::from_le_bytes([reply[5], reply[6]]) == u16::from(seq))
    .then_some(reply)
}

/// Flattens a frame to the 108 wire bytes.
#[must_use]
pub fn frame_bytes(frame: &Frame) -> Vec<u8> {
    frame.iter().flatten().flatten().copied().collect()
}

/// Driver for one dot screen.
#[derive(Debug)]
pub struct DotScreen<T: Transport> {
    transport: T,
    seq: u8,
    handshaken: Option<std::time::Instant>,
}

impl DotScreen<HidTransport> {
    /// Opens a dot-screen interface.
    ///
    /// # Errors
    ///
    /// Returns an error when the device cannot be opened.
    pub fn open(path: &str) -> Result<Self, HidError> {
        let mut transport = HidTransport::open(path)?;
        transport.set_timeout(Duration::from_millis(1200));
        Ok(Self::new(transport))
    }
}

impl<T: Transport> DotScreen<T> {
    /// Wraps a transport.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            seq: 0,
            handshaken: None,
        }
    }

    fn next_seq(&mut self) -> u8 {
        self.seq = self.seq % 250 + 1;
        self.seq
    }

    fn command(
        &mut self,
        command: u8,
        offset: u16,
        data: &[u8],
        len: u8,
    ) -> Result<Vec<u8>, HidError> {
        let seq = self.next_seq();
        let packet = encode(command, offset, data, len, seq);
        let reply = self
            .transport
            .transfer_where(0, &packet, &|reply: &[u8]| answer(reply, seq).is_some())?;
        let reply = answer(&reply, seq).map(<[u8]>::to_vec).unwrap_or(reply);
        if reply.get(2) == Some(&0xFF) {
            return Err(HidError::Invalid("dot screen rejected the command".into()));
        }
        Ok(reply)
    }

    /// Keeps the session alive (the screen expects one every few seconds).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn handshake(&mut self) -> Result<(), HidError> {
        if self
            .handshaken
            .is_some_and(|at| at.elapsed() < Duration::from_secs(4))
        {
            return Ok(());
        }
        self.command(cmd::HANDSHAKE, 0, &[], 0)?;
        self.handshaken = Some(std::time::Instant::now());
        Ok(())
    }

    /// Reads the 64-byte screen configuration.
    ///
    /// # Errors
    ///
    /// Returns a transport error or a short reply.
    pub fn read_config(&mut self) -> Result<[u8; 64], HidError> {
        self.handshake()?;
        let mut config = [0u8; 64];
        for (offset, len) in CONFIG_CHUNKS {
            let reply = self.command(cmd::READ_CONFIG, offset, &[], len)?;
            let end = 8 + usize::from(len);
            if reply.len() < end {
                return Err(HidError::Invalid(
                    "dot screen config reply too short".into(),
                ));
            }
            let at = usize::from(offset);
            config[at..at + usize::from(len)].copy_from_slice(&reply[8..end]);
        }
        Ok(config)
    }

    /// Writes the 64-byte screen configuration.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn write_config(&mut self, config: &[u8; 64]) -> Result<(), HidError> {
        self.handshake()?;
        self.command(cmd::BEGIN_CONFIG, 0, &[], 0)?;
        for (offset, len) in CONFIG_CHUNKS {
            let at = usize::from(offset);
            self.command(
                cmd::WRITE_CONFIG,
                offset,
                &config[at..at + usize::from(len)],
                len,
            )?;
        }
        Ok(())
    }

    /// Switches the screen to streamed frames and returns the previous
    /// configuration for [`DotScreen::end_stream`].
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn begin_stream(&mut self) -> Result<[u8; 64], HidError> {
        let previous = self.read_config()?;
        let mut config = previous;
        config[29] = 0;
        config[MODE_BYTE] = MODE_STREAM;
        if config[31] == 0 {
            config[31] = 75;
        }
        self.command(cmd::PREPARE, 0, &[], 0)?;
        self.write_config(&config)?;
        Ok(previous)
    }

    /// Sends one frame in 24-byte chunks (no replies expected).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn write_frame(&mut self, frame: &Frame) -> Result<(), HidError> {
        self.handshake()?;
        let bytes = frame_bytes(frame);
        for (index, chunk) in bytes.chunks(24).enumerate() {
            let seq = self.next_seq();
            let offset = u16::try_from(index * 24).unwrap_or(0);
            let mut data = chunk.to_vec();
            data.resize(24, 0);
            self.transport
                .write_only(0, &encode(cmd::FRAME, offset, &data, 24, seq))?;
        }
        Ok(())
    }

    /// Restores the effect that was showing before [`DotScreen::begin_stream`].
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn end_stream(&mut self, previous: &[u8; 64]) -> Result<(), HidError> {
        let mut config = self.read_config()?;
        if config[MODE_BYTE] != MODE_STREAM {
            return Ok(());
        }
        config[29..38].copy_from_slice(&previous[29..38]);
        config[29] = 0;
        if config[MODE_BYTE] == MODE_STREAM {
            config[MODE_BYTE] = 1;
        }
        if config[31] == 0 {
            config[31] = 75;
        }
        self.command(cmd::PREPARE, 0, &[], 0)?;
        self.write_config(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct Fake {
        sent: Vec<Vec<u8>>,
        config: [u8; 64],
    }

    impl Default for Fake {
        fn default() -> Self {
            Self {
                sent: Vec::new(),
                config: [0; 64],
            }
        }
    }

    impl Transport for Fake {
        fn transfer(&mut self, _: u8, data: &[u8], _: u8) -> Result<Vec<u8>, HidError> {
            self.sent.push(data.to_vec());
            let mut reply = vec![0xAA, data[1], 0, 0, 0, data[5], 0, 0];
            if data[1] == cmd::READ_CONFIG {
                let at = usize::from(u16::from_le_bytes([data[2], data[3]]));
                reply.extend_from_slice(&self.config[at..at + usize::from(data[4])]);
            }
            if data[1] == cmd::WRITE_CONFIG {
                let at = usize::from(u16::from_le_bytes([data[2], data[3]]));
                let len = usize::from(data[4]);
                self.config[at..at + len].copy_from_slice(&data[8..8 + len]);
            }
            Ok(reply)
        }

        fn write_only(&mut self, _: u8, data: &[u8]) -> Result<(), HidError> {
            self.sent.push(data.to_vec());
            Ok(())
        }
    }

    #[test]
    fn encodes_header() {
        let packet = encode(59, 48, &[1, 2, 3], 24, 7);
        assert_eq!(&packet[..11], &[0xAA, 59, 48, 0, 24, 7, 0, 0, 1, 2, 3]);
        assert_eq!(packet.len(), 32);
        assert!(answer(&[0, 0xAA, 1, 0, 0, 0, 7, 0, 0], 7).is_some());
        assert!(answer(&[0xAA, 1, 0, 0, 0, 8, 0, 0], 7).is_none());
    }

    #[test]
    fn stream_round_trip_restores_mode() {
        let mut fake = Fake::default();
        fake.config[MODE_BYTE] = 3;
        fake.config[31] = 0;
        let mut screen = DotScreen::new(fake);
        let previous = screen.begin_stream().unwrap();
        assert_eq!(previous[MODE_BYTE], 3);
        assert_eq!(screen.transport.config[MODE_BYTE], MODE_STREAM);
        assert_eq!(screen.transport.config[31], 75);

        let mut frame: Frame = [[[0; 3]; SIDE]; SIDE];
        frame[5][5] = [9, 8, 7];
        screen.write_frame(&frame).unwrap();
        let frames: VecDeque<_> = screen
            .transport
            .sent
            .iter()
            .filter(|p| p[1] == cmd::FRAME)
            .collect();
        assert_eq!(frames.len(), 5);
        assert_eq!(&frames[4][2..5], &[96, 0, 24]);
        assert_eq!(&frames[4][8 + 9..8 + 12], &[9, 8, 7]);

        screen.end_stream(&previous).unwrap();
        assert_eq!(screen.transport.config[MODE_BYTE], 3);
    }

    #[test]
    fn validates_artwork() {
        let art = Artwork {
            version: 1,
            name: "Heart".into(),
            frame_rate: 2,
            frames: vec![[[[0; 3]; SIDE]; SIDE]],
        };
        assert!(art.validate().is_ok());
        assert!(
            Artwork {
                frame_rate: 7,
                ..art.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Artwork {
                frames: vec![],
                ..art
            }
            .validate()
            .is_err()
        );
    }
}
