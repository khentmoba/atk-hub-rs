//! ATK headset drivers.
//!
//! Two dongle families exist: Bolutek (N9 V2 series), which uses 63-byte
//! commands on report 0x55, and KTMicro (Horizon), a register machine on
//! report 0x4B.

use crate::{HidError, HidTransport, Transport, VXE_VENDOR_ID};
use hidapi::HidApi;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Consumer usage page shared by both headset families.
const USAGE_PAGE: u16 = 12;
/// Consumer control usage.
const USAGE: u16 = 1;

/// Bolutek product IDs and names.
const BOLUTEK: &[(u16, &str)] = &[
    (4705, "ATK N9 V2 Ultimate"),
    (4706, "ATK N9 V2 Ultimate"),
    (4819, "ATK N9 V2 PRO"),
    (4820, "ATK N9 V2 PRO"),
    (4821, "ATK N9 V2 ULTRA"),
    (4822, "ATK N9 V2 ULTRA"),
];
/// KTMicro product IDs and names.
const KTMICRO: &[(u16, &str)] = &[(4767, "ATK Horizon")];

/// Headset protocol family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    /// N9 V2 series.
    Bolutek,
    /// Horizon.
    Ktmicro,
}

/// A connected headset dongle.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadsetInfo {
    /// OS device path.
    pub path: String,
    /// Product ID.
    pub product_id: u16,
    /// Model name.
    pub name: &'static str,
    /// Protocol family.
    pub family: Family,
}

/// Lists connected headset dongles.
///
/// # Errors
///
/// Returns an error when hidapi cannot initialise.
pub fn list_headsets() -> Result<Vec<HeadsetInfo>, HidError> {
    let api = HidApi::new()?;
    let mut list: Vec<HeadsetInfo> = api
        .device_list()
        .filter(|info| {
            info.vendor_id() == VXE_VENDOR_ID
                && info.usage_page() == USAGE_PAGE
                && info.usage() == USAGE
        })
        .filter_map(|info| {
            let pid = info.product_id();
            let (family, name) = BOLUTEK
                .iter()
                .find(|(id, _)| *id == pid)
                .map(|(_, name)| (Family::Bolutek, *name))
                .or_else(|| {
                    KTMICRO
                        .iter()
                        .find(|(id, _)| *id == pid)
                        .map(|(_, name)| (Family::Ktmicro, *name))
                })?;
            Some(HeadsetInfo {
                path: info.path().to_string_lossy().into_owned(),
                product_id: pid,
                name,
                family,
            })
        })
        .collect();
    list.dedup_by(|a, b| a.path == b.path);
    Ok(list)
}

/* ------------------------------------------------------------ Bolutek -- */

mod cmd {
    pub const RGB_CONFIG: u8 = 3;
    pub const SIDETONE: u8 = 7;
    pub const SURROUND: u8 = 8;
    pub const SPATIAL: u8 = 9;
    pub const EQ: u8 = 33;
    pub const FACTORY_RESET: u8 = 34;
    pub const AUTO_POWER_OFF: u8 = 35;
    pub const PROMPT_VOLUME: u8 = 36;
    pub const BATTERY: u8 = 101;
    pub const PEQ_SET: u8 = 104;
    pub const PEQ_GET: u8 = 105;
    pub const VERSION: u8 = 170;
}

const BOLUTEK_REPORT: u8 = 0x55;
const BOLUTEK_LEN: usize = 63;
/// Parametric EQ points on Bolutek headsets.
pub const PEQ_POINTS: u8 = 10;

/// State read from a Bolutek headset.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BolutekState {
    /// Battery percent.
    pub battery: u8,
    /// `discharging`, `charging`, `full` or `offline`.
    pub status: String,
    /// EQ preset: 0 Game 1, 1 Game 2, 2 Custom.
    pub eq_mode: u8,
    /// Virtual 7.1 surround.
    pub surround: bool,
    /// Spatial sound.
    pub spatial: bool,
    /// Auto power off level, 0 off then 1-4.
    pub auto_power_off: u8,
    /// Sidetone on.
    pub sidetone: bool,
    /// Sidetone volume.
    pub sidetone_volume: u8,
    /// Voice prompt volume 0-15.
    pub prompt_volume: u8,
    /// Dongle firmware version.
    pub dongle_version: String,
    /// Headset firmware version.
    pub headset_version: String,
    /// Headset lighting on.
    pub rgb: bool,
}

/// One parametric EQ point.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeqPoint {
    /// Centre frequency in Hz.
    pub freq: u16,
    /// Gain in dB.
    pub gain: f32,
    /// Quality factor.
    pub q: f32,
    /// Filter type: Bolutek 0 off, 1 peaking, 2 high-pass, 3 low-pass, 4 low
    /// shelf, 5 high shelf; KTMicro 0 peak, 1 low-pass, 2 high-pass, 3 low
    /// shelf, 4 high shelf.
    pub kind: u8,
    /// Point enabled.
    pub enabled: bool,
}

fn version(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

/// Decodes the battery/device-info reply.
#[must_use]
pub fn decode_bolutek(reply: &[u8]) -> BolutekState {
    let at = |index: usize| reply.get(index).copied().unwrap_or(0);
    BolutekState {
        battery: at(1),
        status: match at(2) {
            2 => "discharging",
            3 => "charging",
            4 => "full",
            _ => "offline",
        }
        .to_owned(),
        eq_mode: at(3),
        surround: at(4) != 0,
        spatial: at(5) != 0,
        auto_power_off: at(6),
        sidetone: at(7) != 0,
        sidetone_volume: at(8),
        prompt_volume: 0,
        dongle_version: version(reply.get(9..13).unwrap_or_default()),
        headset_version: version(reply.get(13..17).unwrap_or_default()),
        rgb: at(17) != 0,
    }
}

/// Encodes a PEQ point write (`[104, 1, point, fc, gain, q, type, en, mask]`).
#[must_use]
pub fn encode_peq(point: u8, peq: &PeqPoint) -> Vec<u8> {
    let [fh, fl] = peq.freq.to_be_bytes();
    #[allow(clippy::cast_possible_truncation)]
    let [gh, gl] = ((peq.gain * 10.0).round() as i16).to_be_bytes();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let [qh, ql] = (((peq.q * 10.0).round() as u16).clamp(5, 50)).to_be_bytes();
    vec![
        cmd::PEQ_SET,
        1,
        point,
        fh,
        fl,
        gh,
        gl,
        qh,
        ql,
        peq.kind.min(5),
        u8::from(peq.enabled),
        31,
    ]
}

/// Decodes a PEQ point reply.
#[must_use]
pub fn decode_peq(reply: &[u8]) -> PeqPoint {
    let word = |index: usize| {
        u16::from_be_bytes([
            reply.get(index).copied().unwrap_or(0),
            reply.get(index + 1).copied().unwrap_or(0),
        ])
    };
    #[allow(clippy::cast_possible_wrap)]
    let gain = f32::from(word(4) as i16) / 10.0;
    PeqPoint {
        freq: word(2),
        gain,
        q: f32::from(word(6)) / 10.0,
        kind: reply.get(8).copied().unwrap_or(0),
        enabled: reply.get(9).copied().unwrap_or(0) != 0,
    }
}

/// Bolutek headset driver.
#[derive(Debug)]
pub struct Bolutek {
    transport: HidTransport,
}

impl Bolutek {
    /// Opens a dongle by path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be opened.
    pub fn open(path: &str) -> Result<Self, HidError> {
        let mut transport = HidTransport::open(path)?;
        transport.set_timeout(Duration::from_millis(500));
        Ok(Self { transport })
    }

    fn frame(data: &[u8]) -> Vec<u8> {
        let mut frame = vec![0_u8; BOLUTEK_LEN];
        frame[..data.len()].copy_from_slice(data);
        frame
    }

    fn request(&mut self, data: &[u8]) -> Result<Vec<u8>, HidError> {
        let frame = Self::frame(data);
        self.transport.transfer(BOLUTEK_REPORT, &frame, data[0])
    }

    fn send(&mut self, data: &[u8]) -> Result<(), HidError> {
        self.transport.send(BOLUTEK_REPORT, &Self::frame(data))?;
        std::thread::sleep(Duration::from_millis(20));
        Ok(())
    }

    /// Reads battery, settings and versions.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn state(&mut self) -> Result<BolutekState, HidError> {
        let reply = self.request(&[cmd::BATTERY, 1])?;
        let mut state = decode_bolutek(&reply);
        if state.dongle_version.chars().all(|c| c == '0' || c == '.')
            && let Ok(dongle) = self.request(&[cmd::VERSION, 0])
        {
            state.dongle_version = version(dongle.get(1..5).unwrap_or_default());
        }
        if let Ok(prompt) = self.request(&[cmd::PROMPT_VOLUME, 0]) {
            state.prompt_volume = prompt.get(2).copied().unwrap_or(0);
        }
        Ok(state)
    }

    /// Selects the EQ preset (0 Game 1, 1 Game 2, 2 Custom).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_eq_mode(&mut self, mode: u8) -> Result<(), HidError> {
        self.send(&[cmd::EQ, 1, mode.min(2)])
    }

    /// Sets the sidetone switch and volume.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_sidetone(&mut self, on: bool, volume: u8) -> Result<(), HidError> {
        self.send(&[cmd::SIDETONE, 1, u8::from(on), volume])
    }

    /// Switches virtual 7.1 surround.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_surround(&mut self, on: bool) -> Result<(), HidError> {
        self.send(&[cmd::SURROUND, u8::from(on)])
    }

    /// Switches spatial sound.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_spatial(&mut self, on: bool) -> Result<(), HidError> {
        self.send(&[cmd::SPATIAL, u8::from(on)])
    }

    /// Sets the auto power off level (0 off, 1-4).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_auto_power_off(&mut self, level: u8) -> Result<(), HidError> {
        self.send(&[cmd::AUTO_POWER_OFF, 1, level.min(4)])
    }

    /// Sets the voice prompt volume (0-15).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_prompt_volume(&mut self, volume: u8) -> Result<(), HidError> {
        self.send(&[cmd::PROMPT_VOLUME, 1, volume.min(15)])
    }

    /// Sets the headset lighting effect (1 marquee, 2 breath, 3 static,
    /// 4 blink, 5 cycle) and its speed in milliseconds (10-100).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_lighting(&mut self, mode: u8, speed_ms: u16) -> Result<(), HidError> {
        let [lo, hi] = speed_ms.clamp(10, 100).to_le_bytes();
        self.send(&[cmd::RGB_CONFIG, mode.clamp(1, 5), 1, 0, lo, hi])
    }

    /// Reads all parametric EQ points.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn peq(&mut self) -> Result<Vec<PeqPoint>, HidError> {
        (0..PEQ_POINTS)
            .map(|point| {
                self.request(&[cmd::PEQ_GET, 1, point])
                    .map(|reply| decode_peq(&reply))
            })
            .collect()
    }

    /// Writes parametric EQ points.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_peq(&mut self, points: &[PeqPoint]) -> Result<(), HidError> {
        for (index, point) in points.iter().take(usize::from(PEQ_POINTS)).enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            self.send(&encode_peq(index as u8, point))?;
        }
        Ok(())
    }

    /// Restores factory settings.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn factory_reset(&mut self) -> Result<(), HidError> {
        self.send(&[cmd::FACTORY_RESET, 1])
    }
}

/* ------------------------------------------------------------ KTMicro -- */

const KT_REPORT: u8 = 0x4B;
const KT_WRITE: u8 = 87;
const KT_READ: u8 = 82;
const KT_SAVE: u8 = 83;
const KT_CLEAR: u8 = 67;
/// Output EQ point registers.
const DAC_POINTS: [u32; 6] = [53, 55, 57, 59, 61, 63];
/// Microphone EQ point registers.
const ADC_POINTS: [u32; 6] = [66, 68, 70, 72, 74, 76];
const DAC_EQ_ENABLE: u32 = 52;
const ADC_EQ_ENABLE: u32 = 65;
const MIC_VOLUME: u32 = 86;
const OUT_VOLUME: u32 = 87;
/// Microphone gain steps in dB.
pub const MIC_GAIN_DB: [u8; 6] = [0, 6, 12, 18, 24, 30];

/// State read from a KTMicro headset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KtState {
    /// Output EQ enabled.
    pub dac_eq: bool,
    /// Microphone EQ enabled.
    pub adc_eq: bool,
    /// Output EQ points.
    pub dac_points: Vec<PeqPoint>,
    /// Microphone EQ points.
    pub adc_points: Vec<PeqPoint>,
    /// Microphone gain step index into [`MIC_GAIN_DB`].
    pub mic_gain: u8,
    /// Output volume.
    pub volume: f32,
}

/// Packs an EQ point into its two register values.
#[must_use]
pub fn encode_kt_point(point: &PeqPoint) -> (u32, u32) {
    #[allow(clippy::cast_possible_truncation)]
    let gain = ((point.gain * 10.0).round() as i16).cast_unsigned();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let q = (point.q * 1000.0).round().clamp(0.0, 65535.0) as u32;
    (
        (u32::from(point.freq) << 16) | u32::from(gain),
        (q & 0xFFFF) | (u32::from(point.kind & 7) << 16),
    )
}

/// Unpacks an EQ point from its two register values.
#[must_use]
pub fn decode_kt_point(first: u32, second: u32) -> PeqPoint {
    #[allow(clippy::cast_possible_truncation)]
    let gain = f32::from((first & 0xFFFF) as u16 as i16) / 10.0;
    #[allow(clippy::cast_possible_truncation)]
    PeqPoint {
        freq: (first >> 16) as u16,
        gain,
        q: f32::from((second & 0xFFFF) as u16) / 1000.0,
        kind: ((second >> 16) & 7) as u8,
        enabled: true,
    }
}

/// KTMicro headset driver.
#[derive(Debug)]
pub struct KtMicro {
    transport: HidTransport,
}

impl KtMicro {
    /// Opens a dongle by path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be opened.
    pub fn open(path: &str) -> Result<Self, HidError> {
        let mut transport = HidTransport::open(path)?;
        transport.set_timeout(Duration::from_millis(500));
        Ok(Self { transport })
    }

    fn exchange(&mut self, addr: u32, command: u8, data: u32) -> Result<Vec<u8>, HidError> {
        let mut frame = Vec::with_capacity(10);
        frame.extend_from_slice(&addr.to_le_bytes());
        frame.extend_from_slice(&[command, 0]);
        frame.extend_from_slice(&data.to_le_bytes());
        let address = addr.to_le_bytes();
        self.transport
            .transfer_where(KT_REPORT, &frame, &|reply: &[u8]| {
                reply.len() >= 6 && reply[..4] == address && reply[4] == command
            })
    }

    fn read(&mut self, addr: u32) -> Result<u32, HidError> {
        let reply = self.exchange(addr, KT_READ, 0)?;
        let byte = |index: usize| reply.get(index).copied().unwrap_or(0);
        Ok(u32::from_le_bytes([byte(6), byte(7), byte(8), byte(9)]))
    }

    fn write(&mut self, addr: u32, value: u32) -> Result<(), HidError> {
        let reply = self.exchange(addr, KT_WRITE, value)?;
        if reply.get(6) == Some(&3) {
            Ok(())
        } else {
            Err(HidError::Invalid(format!("register {addr} write rejected")))
        }
    }

    fn points(&mut self, base: &[u32]) -> Result<Vec<PeqPoint>, HidError> {
        base.iter()
            .map(|addr| Ok(decode_kt_point(self.read(*addr)?, self.read(addr + 1)?)))
            .collect()
    }

    /// Reads EQ, mic gain and volume.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn state(&mut self) -> Result<KtState, HidError> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        Ok(KtState {
            dac_eq: self.read(DAC_EQ_ENABLE)? != 0,
            adc_eq: self.read(ADC_EQ_ENABLE)? != 0,
            dac_points: self.points(&DAC_POINTS)?,
            adc_points: self.points(&ADC_POINTS)?,
            mic_gain: self.read(MIC_VOLUME)?.min(5) as u8,
            volume: self.read(OUT_VOLUME)? as f32 / 10.0,
        })
    }

    /// Writes EQ, mic gain and volume, then saves them to flash.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn apply(&mut self, state: &KtState) -> Result<(), HidError> {
        self.write(DAC_EQ_ENABLE, u32::from(state.dac_eq))?;
        self.write(ADC_EQ_ENABLE, u32::from(state.adc_eq))?;
        for (base, points) in [
            (&DAC_POINTS, &state.dac_points),
            (&ADC_POINTS, &state.adc_points),
        ] {
            for (addr, point) in base.iter().zip(points) {
                let (first, second) = encode_kt_point(point);
                self.write(*addr, first)?;
                self.write(addr + 1, second)?;
            }
        }
        self.write(MIC_VOLUME, u32::from(state.mic_gain.min(5)))?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        self.write(OUT_VOLUME, (state.volume * 10.0).round().max(0.0) as u32)?;
        let reply = self.exchange(0, KT_SAVE, 0)?;
        if reply.get(6) == Some(&3) {
            Ok(())
        } else {
            Err(HidError::Invalid("saving to flash failed".to_owned()))
        }
    }

    /// Clears saved registers back to factory values.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn factory_reset(&mut self) -> Result<(), HidError> {
        self.exchange(0, KT_CLEAR, 0).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peq_round_trips() {
        let point = PeqPoint {
            freq: 1000,
            gain: -3.5,
            q: 1.4,
            kind: 1,
            enabled: true,
        };
        let bytes = encode_peq(2, &point);
        assert_eq!(&bytes[..5], &[104, 1, 2, 0x03, 0xE8]);
        let mut reply = vec![105, 1];
        reply.extend_from_slice(&bytes[3..11]);
        assert_eq!(decode_peq(&reply), point);
    }

    #[test]
    fn kt_point_round_trips() {
        let point = PeqPoint {
            freq: 250,
            gain: 4.5,
            q: 0.707,
            kind: 3,
            enabled: true,
        };
        let (first, second) = encode_kt_point(&point);
        assert_eq!(decode_kt_point(first, second), point);
    }

    #[test]
    fn decodes_device_info() {
        let mut reply = vec![101, 80, 3, 2, 1, 0, 2, 1, 9];
        reply.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 1]);
        let state = decode_bolutek(&reply);
        assert_eq!(state.battery, 80);
        assert_eq!(state.status, "charging");
        assert!(state.surround && state.sidetone && state.rgb);
        assert_eq!(state.headset_version, "5.6.7.8");
    }
}
