//! COMPX mouse protocol: 16-byte output reports on report ID 8.
//!
//! Layout: `[command, status, addr_hi, addr_lo, len, data[0..10], checksum]`.
//! Most EEPROM fields are stored as `(value, 0x55 - value)` pairs.

use crate::dpi::{DpiBytes, Sensor};
use crate::models::MODELS;
use crate::{HidError, Transport};
use serde::{Deserialize, Serialize};

/// HID report ID used for every COMPX command.
pub const REPORT_ID: u8 = 8;
/// Vendor usage page of the configuration interface.
pub const USAGE_PAGE: u16 = 0xFF04;
/// Usage page of the report-ID-8 command collection on the same interface.
pub const COMMAND_USAGE_PAGE: u16 = 0xFF02;
/// Usage of the configuration interface.
pub const USAGE: u16 = 2;
/// Packet length without the report ID.
pub const PACKET_LEN: usize = 16;
const BASE: usize = 5;

/// COMPX command identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    /// Handshake that returns the connection type.
    DownloadData = 1,
    /// Whether the wireless mouse is reachable through the dongle.
    GetWirelessMouseOnline = 3,
    /// Battery level and charge state.
    GetBatteryLevel = 4,
    /// Starts dongle pairing for a CID/MID.
    SetWirelessDonglePair = 5,
    /// Pairing status.
    GetWirelessDonglePairResult = 6,
    /// Writes EEPROM bytes.
    SetEeprom = 7,
    /// Reads EEPROM bytes.
    GetEeprom = 8,
    /// Restores factory settings.
    RestoreFactory = 9,
    /// Unsolicited status report from the device.
    ReportMouseStatus = 10,
    /// Active onboard profile.
    GetCurrentConfig = 14,
    /// Switches onboard profile.
    SetCurrentConfig = 15,
    /// Customer and model identifiers.
    GetMouseCidMid = 16,
    /// Mouse firmware version.
    GetMouseVersion = 18,
    /// Sets the long-distance (high LOD) mode.
    SetFarDistanceMode = 22,
    /// Reads the long-distance mode.
    GetFarDistanceMode = 23,
    /// Sets the dongle lighting.
    SetDongleLightMode = 24,
    /// Reads the dongle lighting.
    GetDongleLightMode = 25,
    /// Dongle firmware version.
    GetDongleVersion = 29,
}

/// EEPROM addresses.
pub mod address {
    /// Report rate, stage count, current stage, bhop, button mode.
    pub const REPORT_RATE: u16 = 0;
    /// Lift-off (silent) height.
    pub const SILENT_HEIGHT: u16 = 10;
    /// First DPI stage pair; stages pairs at +8.
    pub const DPI1: u16 = 12;
    /// First DPI colour pair; pairs at +8.
    pub const DPI1_COLOR: u16 = 44;
    /// DPI indicator lighting.
    pub const DPI_LIGHT: u16 = 76;
    /// First button mapping; each button is 4 bytes.
    pub const KEY0: u16 = 96;
    /// Debounce, motion sync, LED sleep, angle snap, ripple.
    pub const MOUSE_FUNCTION: u16 = 169;
    /// Sensor sleep enable, sleep time, sensor mode.
    pub const PERFORMANCE: u16 = 181;
    /// Sensor angle tuning.
    pub const ANGLE: u16 = 189;
    /// Wheel rolling delay.
    pub const ROLLING_DELAY: u16 = 227;
    /// First macro slot; slots every 384 bytes.
    pub const MACRO0: u16 = 768;
    /// First PAW3955 stage; stages every 6 bytes.
    pub const PAW3955_DPI1: u16 = 6912;
}

/// One 16-byte COMPX packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet(pub [u8; PACKET_LEN]);

impl Packet {
    /// Creates an empty packet for a command.
    #[must_use]
    pub fn new(command: Command) -> Self {
        let mut raw = [0; PACKET_LEN];
        raw[0] = command as u8;
        Self(raw)
    }

    /// EEPROM read request.
    #[must_use]
    pub fn get_eeprom(address: u16, len: u8) -> Self {
        let mut packet = Self::new(Command::GetEeprom);
        packet.set_address(address);
        packet.0[4] = len;
        packet
    }

    /// EEPROM write request with raw payload bytes.
    #[must_use]
    pub fn set_eeprom(address: u16, payload: &[u8]) -> Self {
        let mut packet = Self::new(Command::SetEeprom);
        packet.set_address(address);
        let len = payload.len().min(10);
        packet.0[4] = len as u8;
        packet.0[BASE..BASE + len].copy_from_slice(&payload[..len]);
        packet
    }

    /// Sets the big-endian EEPROM address.
    pub fn set_address(&mut self, address: u16) {
        self.0[2..4].copy_from_slice(&address.to_be_bytes());
    }

    /// Data byte at `offset` after the 5-byte header.
    #[must_use]
    pub fn data(&self, offset: usize) -> u8 {
        self.0.get(BASE + offset).copied().unwrap_or(0)
    }

    /// Command status byte.
    #[must_use]
    pub fn status(&self) -> u8 {
        self.0[1]
    }

    /// Bytes ready to send, with the trailing checksum filled in.
    #[must_use]
    pub fn finalize(mut self) -> [u8; PACKET_LEN] {
        let sum = self.0[..15]
            .iter()
            .fold(u32::from(REPORT_ID), |acc, byte| acc + u32::from(*byte));
        self.0[15] = 0x55_u8.wrapping_sub((sum & 0xff) as u8);
        self.0
    }
}

/// `(value, 0x55 - value)` pair encoder.
#[must_use]
pub fn pair(value: u8) -> [u8; 2] {
    [value, 0x55_u8.wrapping_sub(value)]
}

/// True when a stored pair checksum matches.
#[must_use]
pub fn pair_valid(value: u8, crc: u8) -> bool {
    crc == 0x55_u8.wrapping_sub(value)
}

/// Checksum for a 3-byte record (DPI, colour or button).
#[must_use]
pub fn triple_crc(a: u8, b: u8, c: u8) -> u8 {
    0x55_u8.wrapping_sub(a.wrapping_add(b).wrapping_add(c))
}

const REPORT_RATES: &[(u32, u8)] = &[
    (1000, 1),
    (500, 2),
    (250, 4),
    (125, 8),
    (2000, 16),
    (4000, 32),
    (8000, 64),
];

/// Converts Hz to the stored report-rate code.
#[must_use]
pub fn report_rate_code(hz: u32) -> Option<u8> {
    REPORT_RATES
        .iter()
        .find(|(rate, _)| *rate == hz)
        .map(|(_, code)| *code)
}

/// Converts a stored report-rate code to Hz.
#[must_use]
pub fn report_rate_hz(code: u8) -> Option<u32> {
    REPORT_RATES
        .iter()
        .find(|(_, value)| *value == code)
        .map(|(rate, _)| *rate)
}

/// Looks up the sensor and product name for a CID/MID pair.
#[must_use]
pub fn model(cid: u8, mid: u8) -> Option<(Sensor, &'static str)> {
    MODELS
        .iter()
        .find(|(c, m, _, _)| *c == cid && *m == mid)
        .map(|(_, _, sensor, name)| (*sensor, *name))
}

/// Report rate and DPI stage block at EEPROM address 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRateDpi {
    /// Polling rate in Hz.
    pub report_rate: u32,
    /// Number of enabled DPI stages.
    pub dpi_total: u8,
    /// Active DPI stage index (0-based).
    pub dpi_level: u8,
    /// Bhop delay in milliseconds (0 = off).
    pub bhop_ms: u32,
    /// Raw button operation mode bits (bit 7 DPI toggle, bits 0/1 fast trigger).
    pub button_operation_mode: u8,
}

impl ReportRateDpi {
    fn parse(packet: &Packet) -> Self {
        let bhop = if pair_valid(packet.data(6), packet.data(7)) {
            u32::from(packet.data(6)) * 10
        } else {
            0
        };
        let mode = if pair_valid(packet.data(8), packet.data(9)) {
            packet.data(8)
        } else {
            0
        };
        Self {
            report_rate: report_rate_hz(packet.data(0)).unwrap_or(1000),
            dpi_total: packet.data(2),
            dpi_level: packet.data(4),
            bhop_ms: bhop,
            button_operation_mode: mode,
        }
    }

    fn encode(&self) -> Result<Packet, HidError> {
        let code = report_rate_code(self.report_rate).ok_or_else(|| {
            HidError::Invalid(format!("unsupported report rate {}", self.report_rate))
        })?;
        let bhop = u8::try_from(self.bhop_ms / 10).unwrap_or(u8::MAX);
        let mut payload = Vec::with_capacity(10);
        for value in [
            code,
            self.dpi_total,
            self.dpi_level,
            bhop,
            self.button_operation_mode,
        ] {
            payload.extend_from_slice(&pair(value));
        }
        Ok(Packet::set_eeprom(address::REPORT_RATE, &payload))
    }
}

/// One DPI stage with its indicator colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DpiStage {
    /// Horizontal DPI.
    pub x: u32,
    /// Vertical DPI.
    pub y: u32,
    /// Indicator colour `[r, g, b]`.
    pub color: [u8; 3],
}

/// Debounce, motion sync, LED sleep, angle snap and ripple control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MouseFunction {
    /// Key debounce in milliseconds.
    pub debounce_ms: u8,
    /// Motion sync on/off.
    pub motion_sync: bool,
    /// LED/idle sleep time in seconds.
    pub sleep_seconds: u32,
    /// Angle snapping (linear correction) on/off.
    pub angle_snap: bool,
    /// Ripple control on/off.
    pub ripple_control: bool,
}

impl MouseFunction {
    fn parse(packet: &Packet) -> Self {
        Self {
            debounce_ms: packet.data(0),
            motion_sync: packet.data(2) != 0,
            sleep_seconds: u32::from(packet.data(4)) * 10,
            angle_snap: packet.data(6) != 0,
            ripple_control: packet.data(8) != 0,
        }
    }

    fn encode(&self) -> Packet {
        let sleep = u8::try_from(self.sleep_seconds / 10).unwrap_or(u8::MAX);
        let mut payload = Vec::with_capacity(10);
        for value in [
            self.debounce_ms,
            u8::from(self.motion_sync),
            sleep,
            u8::from(self.angle_snap),
            u8::from(self.ripple_control),
        ] {
            payload.extend_from_slice(&pair(value));
        }
        Packet::set_eeprom(address::MOUSE_FUNCTION, &payload)
    }
}

/// Sensor power settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Performance {
    /// Sensor sleep enabled.
    pub sensor_sleep_enabled: bool,
    /// Sensor sleep delay in seconds.
    pub sensor_sleep_seconds: u32,
    /// Sensor mode (0 = low power, 1 = competitive, 2 = competitive max).
    pub sensor_mode: u8,
}

impl Performance {
    fn parse(packet: &Packet) -> Self {
        Self {
            sensor_sleep_enabled: packet.data(0) != 0,
            sensor_sleep_seconds: u32::from(packet.data(2)) * 10,
            sensor_mode: packet.data(4),
        }
    }

    fn encode(&self) -> Packet {
        let sleep = u8::try_from(self.sensor_sleep_seconds / 10).unwrap_or(u8::MAX);
        let mut payload = Vec::with_capacity(6);
        for value in [u8::from(self.sensor_sleep_enabled), sleep, self.sensor_mode] {
            payload.extend_from_slice(&pair(value));
        }
        Packet::set_eeprom(address::PERFORMANCE, &payload)
    }
}

/// DPI indicator lighting effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DpiLight {
    /// 0 = off, 1 = steady, 2 = breathing.
    pub mode: u8,
    /// 0 = low, 1 = medium, 2 = high.
    pub brightness: u8,
    /// 0 = slow, 1 = medium, 2 = fast.
    pub speed: u8,
}

const LIGHT_BRIGHTNESS: [u8; 3] = [16, 128, 255];
const LIGHT_SPEED: [u8; 3] = [1, 3, 5];

impl DpiLight {
    fn parse(packet: &Packet) -> Self {
        let level = |table: &[u8; 3], value: u8| {
            table
                .iter()
                .position(|candidate| *candidate == value)
                .unwrap_or(1) as u8
        };
        let enabled = packet.data(6) != 0;
        Self {
            mode: if enabled { packet.data(0) } else { 0 },
            brightness: level(&LIGHT_BRIGHTNESS, packet.data(2)),
            speed: level(&LIGHT_SPEED, packet.data(4)),
        }
    }

    fn encode(&self) -> Packet {
        let mut payload = Vec::with_capacity(8);
        let mode = if self.mode == 0 { 1 } else { self.mode.min(2) };
        payload.extend_from_slice(&pair(mode));
        payload.extend_from_slice(&pair(LIGHT_BRIGHTNESS[usize::from(self.brightness.min(2))]));
        payload.extend_from_slice(&pair(LIGHT_SPEED[usize::from(self.speed.min(2))]));
        payload.extend_from_slice(&pair(u8::from(self.mode != 0)));
        Packet::set_eeprom(address::DPI_LIGHT, &payload)
    }
}

/// Button names in EEPROM order.
pub const BUTTONS: [&str; 6] = ["left", "right", "middle", "back", "forward", "dpi"];

/// Factory mapping per button, `[class, value1, value2]`.
pub const DEFAULT_BUTTONS: [[u8; 3]; 6] = [
    [1, 1, 0],
    [1, 2, 0],
    [1, 4, 0],
    [1, 8, 0],
    [1, 16, 0],
    [2, 1, 0],
];

/// Snapshot of everything the mouse page shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MouseSnapshot {
    /// Product name from the model table.
    pub name: String,
    /// Customer ID.
    pub cid: u8,
    /// Model ID.
    pub mid: u8,
    /// Sensor family.
    pub sensor: Sensor,
    /// Maximum DPI the sensor supports.
    pub max_dpi: u32,
    /// Mouse firmware version.
    pub version: String,
    /// Dongle firmware version.
    pub dongle_version: String,
    /// Mouse reachable over the dongle.
    pub online: bool,
    /// Battery percentage.
    pub battery: u8,
    /// Charging flag.
    pub charging: bool,
    /// Active onboard profile.
    pub profile: u8,
    /// Report rate and stage state.
    pub rate: ReportRateDpi,
    /// DPI stages (always 8 slots).
    pub stages: Vec<DpiStage>,
    /// Debounce, sync, sleep, angle snap, ripple.
    pub function: MouseFunction,
    /// Sensor power settings.
    pub performance: Performance,
    /// Lift-off height (raw level).
    pub lift_off: u8,
    /// Long-distance mode.
    pub far_distance: u8,
    /// Angle tuning value.
    pub angle: i8,
    /// Angle tuning enabled.
    pub angle_enabled: bool,
    /// DPI indicator light.
    pub light: DpiLight,
    /// Button mappings `[class, value1, value2]` in [`BUTTONS`] order.
    pub buttons: Vec<[u8; 3]>,
}

/// High-level COMPX mouse driver over any transport.
#[derive(Debug)]
pub struct CompxMouse<T: Transport> {
    transport: T,
    sensor: Sensor,
}

impl<T: Transport> CompxMouse<T> {
    /// Wraps an open configuration interface.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            sensor: Sensor::Paw3950Ultra,
        }
    }

    /// Sends one packet and waits for the response with the same command byte.
    ///
    /// # Errors
    ///
    /// Returns transport errors or a timeout.
    pub fn transfer(&mut self, packet: Packet) -> Result<Packet, HidError> {
        let bytes = packet.finalize();
        let reply = self.transport.transfer(REPORT_ID, &bytes, bytes[0])?;
        let mut raw = [0; PACKET_LEN];
        let len = reply.len().min(PACKET_LEN);
        raw[..len].copy_from_slice(&reply[..len]);
        Ok(Packet(raw))
    }

    fn read(&mut self, address: u16, len: u8) -> Result<Packet, HidError> {
        self.transfer(Packet::get_eeprom(address, len))
    }

    fn command(&mut self, command: Command) -> Result<Packet, HidError> {
        self.transfer(Packet::new(command))
    }

    /// Reads CID/MID and selects the matching sensor encoding.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn identify(&mut self) -> Result<(u8, u8, &'static str), HidError> {
        let reply = self.command(Command::GetMouseCidMid)?;
        let (cid, mid) = (reply.data(0), reply.data(1));
        let (sensor, name) = model(cid, mid).unwrap_or((Sensor::Paw3950Ultra, "ATK Mouse"));
        self.sensor = sensor;
        Ok((cid, mid, name))
    }

    /// Currently selected sensor encoding.
    #[must_use]
    pub fn sensor(&self) -> Sensor {
        self.sensor
    }

    fn version_string(packet: &Packet) -> String {
        format!("{:x}.{:02x}", packet.data(0), packet.data(1))
    }

    /// Reads all eight DPI stages with colours.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn read_stages(&mut self) -> Result<Vec<DpiStage>, HidError> {
        let mut values = Vec::with_capacity(8);
        if self.sensor == Sensor::Paw3955Master {
            for index in 0..8_u16 {
                let reply = self.read(address::PAW3955_DPI1 + index * 6, 6)?;
                let ex = reply.data(4);
                let x = (u32::from(reply.data(0)) | (u32::from(reply.data(1)) << 8)) + 1;
                let y = (u32::from(reply.data(2)) | (u32::from(reply.data(3)) << 8)) + 1;
                values.push((
                    x * if ex & 1 != 0 { 2 } else { 1 },
                    y * if ex & 16 != 0 { 2 } else { 1 },
                ));
            }
        } else {
            for index in 0..4_u16 {
                let reply = self.read(address::DPI1 + index * 8, 8)?;
                for slot in 0..2 {
                    let bytes = DpiBytes {
                        x: reply.data(slot * 4),
                        y: reply.data(slot * 4 + 1),
                        ex: reply.data(slot * 4 + 2),
                    };
                    values.push(self.sensor.decode(bytes));
                }
            }
        }
        let mut stages = Vec::with_capacity(8);
        for index in 0..4_u16 {
            let reply = self.read(address::DPI1_COLOR + index * 8, 8)?;
            for slot in 0..2 {
                let (x, y) = values[stages.len()];
                stages.push(DpiStage {
                    x,
                    y,
                    color: [
                        reply.data(slot * 4),
                        reply.data(slot * 4 + 1),
                        reply.data(slot * 4 + 2),
                    ],
                });
            }
        }
        Ok(stages)
    }

    /// Writes DPI stage values and colours (up to eight).
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn write_stages(&mut self, stages: &[DpiStage]) -> Result<(), HidError> {
        if self.sensor == Sensor::Paw3955Master {
            for (index, stage) in stages.iter().take(8).enumerate() {
                let split = |value: u32| -> (u16, bool) {
                    let value = value.max(1);
                    if value < 40_000 {
                        ((value - 1) as u16, false)
                    } else {
                        (((value & !1) / 2 - 1).min(65_535) as u16, true)
                    }
                };
                let (bx, mx) = split(stage.x);
                let (by, my) = split(stage.y);
                let flags = u8::from(mx) | if my { 16 } else { 0 };
                let [xl, xh] = bx.to_le_bytes();
                let [yl, yh] = by.to_le_bytes();
                let crc = 0x55_u8.wrapping_sub(
                    xl.wrapping_add(xh)
                        .wrapping_add(yl)
                        .wrapping_add(yh)
                        .wrapping_add(flags),
                );
                let address = address::PAW3955_DPI1 + 6 * index as u16;
                self.transfer(Packet::set_eeprom(address, &[xl, xh, yl, yh, flags, crc]))?;
            }
        } else {
            for (chunk_index, chunk) in stages.chunks(2).take(4).enumerate() {
                let mut payload = [0_u8; 8];
                for (slot, stage) in chunk.iter().enumerate() {
                    let DpiBytes { x, y, ex } = self.sensor.encode(stage.x, stage.y);
                    payload[slot * 4..slot * 4 + 4].copy_from_slice(&[
                        x,
                        y,
                        ex,
                        triple_crc(x, y, ex),
                    ]);
                }
                let len = chunk.len() * 4;
                let address = address::DPI1 + 8 * chunk_index as u16;
                self.transfer(Packet::set_eeprom(address, &payload[..len]))?;
            }
        }
        for (chunk_index, chunk) in stages.chunks(2).take(4).enumerate() {
            let mut payload = [0_u8; 8];
            for (slot, stage) in chunk.iter().enumerate() {
                let [r, g, b] = stage.color;
                payload[slot * 4..slot * 4 + 4].copy_from_slice(&[r, g, b, triple_crc(r, g, b)]);
            }
            let len = chunk.len() * 4;
            let address = address::DPI1_COLOR + 8 * chunk_index as u16;
            self.transfer(Packet::set_eeprom(address, &payload[..len]))?;
        }
        Ok(())
    }

    /// Reads the report-rate block.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn read_rate(&mut self) -> Result<ReportRateDpi, HidError> {
        Ok(ReportRateDpi::parse(&self.read(address::REPORT_RATE, 10)?))
    }

    /// Writes the report-rate block.
    ///
    /// # Errors
    ///
    /// Returns an invalid-rate or transport error.
    pub fn write_rate(&mut self, rate: &ReportRateDpi) -> Result<(), HidError> {
        self.transfer(rate.encode()?).map(drop)
    }

    /// Writes debounce, motion sync, sleep, angle snap and ripple settings.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn write_function(&mut self, function: &MouseFunction) -> Result<(), HidError> {
        self.transfer(function.encode()).map(drop)
    }

    /// Writes sensor power settings.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn write_performance(&mut self, performance: &Performance) -> Result<(), HidError> {
        self.transfer(performance.encode()).map(drop)
    }

    /// Writes the DPI indicator light.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn write_light(&mut self, light: &DpiLight) -> Result<(), HidError> {
        self.transfer(light.encode()).map(drop)
    }

    /// Writes the lift-off height level.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn write_lift_off(&mut self, level: u8) -> Result<(), HidError> {
        self.transfer(Packet::set_eeprom(address::SILENT_HEIGHT, &pair(level)))
            .map(drop)
    }

    /// Writes the long-distance mode.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn write_far_distance(&mut self, mode: u8) -> Result<(), HidError> {
        let mut packet = Packet::new(Command::SetFarDistanceMode);
        packet.0[4] = 10;
        packet.0[BASE] = mode;
        self.transfer(packet).map(drop)
    }

    /// Writes the sensor angle tuning.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn write_angle(&mut self, angle: i8, enabled: bool) -> Result<(), HidError> {
        let mut payload = pair(angle as u8).to_vec();
        payload.extend_from_slice(&pair(u8::from(enabled)));
        self.transfer(Packet::set_eeprom(address::ANGLE, &payload))
            .map(drop)
    }

    /// Remaps one button (`index` in [`BUTTONS`] order) to `[class, v1, v2]`.
    ///
    /// # Errors
    ///
    /// Returns an invalid-index or transport error.
    pub fn write_button(&mut self, index: usize, mapping: [u8; 3]) -> Result<(), HidError> {
        if index >= BUTTONS.len() {
            return Err(HidError::Invalid(format!("button index {index}")));
        }
        let [class, v1, v2] = mapping;
        let address = address::KEY0 + 4 * index as u16;
        self.transfer(Packet::set_eeprom(
            address,
            &[class, v1, v2, triple_crc(class, v1, v2)],
        ))
        .map(drop)
    }

    /// Switches the onboard profile.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn set_profile(&mut self, profile: u8) -> Result<(), HidError> {
        let mut packet = Packet::new(Command::SetCurrentConfig);
        packet.0[4] = 1;
        packet.0[BASE] = profile;
        self.transfer(packet).map(drop)
    }

    /// Starts dongle pairing for the identified CID/MID.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn start_pairing(&mut self, cid: u8, mid: u8) -> Result<(), HidError> {
        let mut packet = Packet::new(Command::SetWirelessDonglePair);
        packet.0[4] = 2;
        packet.0[BASE] = cid;
        packet.0[BASE + 1] = mid;
        self.transfer(packet).map(drop)
    }

    /// Reads pairing status `(status, seconds_left)`.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn pairing_status(&mut self) -> Result<(u8, u8), HidError> {
        let reply = self.command(Command::GetWirelessDonglePairResult)?;
        Ok((reply.data(0), reply.data(1)))
    }

    /// Restores factory settings.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn factory_reset(&mut self) -> Result<(), HidError> {
        self.command(Command::RestoreFactory).map(drop)
    }

    /// Whether the mouse is reachable (always true when wired).
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn online(&mut self) -> Result<bool, HidError> {
        Ok(self.command(Command::GetWirelessMouseOnline)?.data(0) == 1)
    }

    /// Reads battery `(percent, charging)`.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn battery(&mut self) -> Result<(u8, bool), HidError> {
        let reply = self.command(Command::GetBatteryLevel)?;
        Ok((reply.data(0), reply.data(1) != 0))
    }

    /// Reads every value the mouse page displays.
    ///
    /// # Errors
    ///
    /// Returns transport errors.
    pub fn snapshot(&mut self) -> Result<MouseSnapshot, HidError> {
        let online = self.online()?;
        if !online {
            return Err(HidError::Offline);
        }
        let (cid, mid, name) = self.identify()?;
        let version = Self::version_string(&self.command(Command::GetMouseVersion)?);
        let mut dongle = Packet::new(Command::GetDongleVersion);
        dongle.0[4] = 0;
        let dongle_version = Self::version_string(&self.transfer(dongle)?);
        let (battery, charging) = self.battery()?;
        let profile = self.command(Command::GetCurrentConfig)?.data(0);
        let rate = self.read_rate()?;
        let stages = self.read_stages()?;
        let function = MouseFunction::parse(&self.read(address::MOUSE_FUNCTION, 10)?);
        let performance = Performance::parse(&self.read(address::PERFORMANCE, 6)?);
        let lift_off = self.read(address::SILENT_HEIGHT, 2)?.data(0);
        let far_distance = self.command(Command::GetFarDistanceMode)?.data(0);
        let angle_packet = self.read(address::ANGLE, 4)?;
        let light = DpiLight::parse(&self.read(address::DPI_LIGHT, 8)?);
        let mut buttons = Vec::with_capacity(6);
        for index in 0..3_u16 {
            let reply = self.read(address::KEY0 + index * 8, 8)?;
            for slot in 0..2 {
                buttons.push([
                    reply.data(slot * 4),
                    reply.data(slot * 4 + 1),
                    reply.data(slot * 4 + 2),
                ]);
            }
        }
        Ok(MouseSnapshot {
            name: name.to_owned(),
            cid,
            mid,
            sensor: self.sensor,
            max_dpi: self.sensor.max_dpi(),
            version,
            dongle_version,
            online,
            battery,
            charging,
            profile,
            rate,
            stages,
            function,
            performance,
            lift_off,
            far_distance,
            angle: angle_packet.data(0) as i8,
            angle_enabled: angle_packet.data(2) != 0,
            light,
            buttons,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct Fake {
        sent: Vec<Vec<u8>>,
        replies: VecDeque<Vec<u8>>,
    }

    impl Transport for Fake {
        fn transfer(
            &mut self,
            report_id: u8,
            data: &[u8],
            _expect: u8,
        ) -> Result<Vec<u8>, HidError> {
            assert_eq!(report_id, REPORT_ID);
            self.sent.push(data.to_vec());
            Ok(self.replies.pop_front().unwrap_or_else(|| data.to_vec()))
        }
    }

    #[test]
    fn checksum_includes_report_id() {
        let bytes = Packet::get_eeprom(0, 10).finalize();
        let sum: u32 = 8 + bytes[..15].iter().map(|b| u32::from(*b)).sum::<u32>();
        assert_eq!(bytes[15], 0x55_u8.wrapping_sub((sum & 0xff) as u8));
        assert_eq!(&bytes[..5], &[8, 0, 0, 0, 10]);
    }

    #[test]
    fn report_rate_block_encodes_pairs() {
        let rate = ReportRateDpi {
            report_rate: 4000,
            dpi_total: 4,
            dpi_level: 1,
            bhop_ms: 0,
            button_operation_mode: 0,
        };
        let packet = rate.encode().unwrap();
        assert_eq!(packet.0[0], Command::SetEeprom as u8);
        assert_eq!(&packet.0[5..11], &[32, 0x35, 4, 0x51, 1, 0x54]);
        assert_eq!(ReportRateDpi::parse(&packet).report_rate, 4000);
    }

    #[test]
    fn identify_selects_sensor() {
        let mut reply = vec![0; 16];
        reply[0] = Command::GetMouseCidMid as u8;
        reply[5] = 2;
        reply[6] = 1;
        let fake = Fake {
            replies: VecDeque::from([reply]),
            ..Fake::default()
        };
        let mut mouse = CompxMouse::new(fake);
        let (_, _, name) = mouse.identify().unwrap();
        assert_eq!(name, "ATK Dragonfly F1 PRO");
        assert_eq!(mouse.sensor(), Sensor::Paw3395);
    }

    #[test]
    fn button_write_uses_key_address() {
        let mut mouse = CompxMouse::new(Fake::default());
        mouse.write_button(3, [2, 2, 0]).unwrap();
        let sent = &mouse.transport.sent[0];
        assert_eq!(&sent[..9], &[7, 0, 0, 108, 4, 2, 2, 0, triple_crc(2, 2, 0)]);
    }
}

/// Kind of key event stored in onboard macros and shortcut keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyKind {
    /// Modifier; `value` is the modifier bit (1 LCtrl, 2 LShift, 4 LAlt, 8 LWin, 16.. right side).
    Modifier,
    /// Keyboard key; `value` is the HID usage.
    Key,
    /// Consumer (media) key.
    Media,
    /// System power key.
    Power,
    /// Mouse button; `value` is the button bit.
    Mouse,
    /// Wheel; `value` 1 = up, 255 = down.
    Scroll,
}

impl KeyKind {
    fn bits(self) -> u8 {
        match self {
            Self::Modifier => 0,
            Self::Key => 1,
            Self::Media => 2,
            Self::Power => 3,
            Self::Mouse => 4,
            Self::Scroll => 6,
        }
    }

    fn from_bits(bits: u8) -> Self {
        match bits & 7 {
            0 => Self::Modifier,
            2 => Self::Media,
            3 => Self::Power,
            4 => Self::Mouse,
            6 => Self::Scroll,
            _ => Self::Key,
        }
    }
}

/// One onboard macro or shortcut event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyEvent {
    /// Event kind.
    pub kind: KeyKind,
    /// Press (`true`) or release.
    pub down: bool,
    /// Usage, modifier bit, mouse bit or scroll direction.
    pub value: u8,
    /// Delay after the event in milliseconds (macros only).
    #[serde(default)]
    pub delay_ms: u16,
}

impl KeyEvent {
    fn status_byte(&self) -> u8 {
        let state = if self.kind == KeyKind::Scroll {
            0
        } else if self.down {
            0x80
        } else {
            0x40
        };
        state | self.kind.bits()
    }

    fn parse(bytes: &[u8]) -> Self {
        let status = bytes[0];
        Self {
            kind: KeyKind::from_bits(status),
            down: status & 0x80 != 0,
            value: bytes[1],
            delay_ms: bytes
                .get(3..5)
                .map_or(0, |delay| u16::from_be_bytes([delay[0], delay[1]])),
        }
    }
}

/// Number of onboard macro slots.
pub const MACRO_SLOTS: u16 = 12;
/// Bytes between macro slots.
pub const MACRO_STRIDE: u16 = 384;
/// Largest macro the firmware accepts.
pub const MACRO_MAX_EVENTS: usize = 70;
/// First shortcut-key block; one per button, 32 bytes apart.
pub const KEY_SHORTCUTS0: u16 = 256;
/// Largest shortcut the firmware accepts.
pub const SHORTCUT_MAX_EVENTS: usize = 6;

/// Macro playback when bound to a button (`value2` of class 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode", content = "count")]
pub enum MacroPlayback {
    /// Play `count` times (1-252).
    Loop(u8),
    /// Stop as soon as the button is released.
    StopOnRelease,
    /// Repeat until the same button is pressed again.
    UntilSameKey,
    /// Repeat until any button is pressed.
    UntilAnyKey,
}

impl MacroPlayback {
    /// `value2` byte of the button mapping.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::Loop(count) => count.clamp(1, 252),
            Self::StopOnRelease => 254,
            Self::UntilSameKey => 253,
            Self::UntilAnyKey => 255,
        }
    }
}

/// Encodes macro events into the EEPROM image written at a slot.
///
/// Scroll events expand to a wheel tick plus a release record, as in the
/// original driver.
///
/// # Errors
///
/// Returns [`HidError::Invalid`] for too many events.
pub fn encode_macro(events: &[KeyEvent]) -> Result<Vec<u8>, HidError> {
    let mut records: Vec<[u8; 5]> = Vec::new();
    for (index, event) in events.iter().enumerate() {
        let last = index + 1 == events.len();
        let delay = if last {
            event.delay_ms
        } else {
            event.delay_ms.max(1)
        }
        .to_be_bytes();
        let mut push = |value: u8| {
            records.push([event.status_byte(), value, 0, delay[0], delay[1]]);
        };
        if event.kind == KeyKind::Scroll {
            push(if event.value == 1 { 1 } else { 255 });
            push(0);
        } else {
            push(event.value);
        }
    }
    if records.len() > MACRO_MAX_EVENTS {
        return Err(HidError::Invalid(format!(
            "macro has {} events; the mouse stores at most {MACRO_MAX_EVENTS}",
            records.len()
        )));
    }
    let mut image = vec![8_u8];
    image.extend([2_u8; 8]);
    image.extend([255_u8; 22]);
    let mut body = vec![records.len() as u8];
    for record in &records {
        body.extend_from_slice(record);
    }
    let sum: u32 = body.iter().map(|byte| u32::from(*byte)).sum();
    body.push((85_u32.wrapping_sub(sum) & 0xff) as u8);
    image.extend(body);
    image.resize(image.len().div_ceil(10) * 10, 0);
    Ok(image)
}

/// Encodes a shortcut-key block (count, 3-byte records, checksum).
///
/// # Errors
///
/// Returns [`HidError::Invalid`] for an empty or oversized shortcut.
pub fn encode_shortcut(events: &[KeyEvent]) -> Result<Vec<u8>, HidError> {
    if events.is_empty() || events.len() > SHORTCUT_MAX_EVENTS {
        return Err(HidError::Invalid(format!(
            "a shortcut needs 1-{SHORTCUT_MAX_EVENTS} events"
        )));
    }
    let mut block = vec![events.len() as u8];
    for event in events {
        block.extend_from_slice(&[event.status_byte(), event.value, 0]);
    }
    let sum: u32 = block.iter().map(|byte| u32::from(*byte)).sum();
    block.push((85_u32.wrapping_sub(sum) & 0xff) as u8);
    block.resize(if block.len() <= 10 { 10 } else { 20 }, 0);
    Ok(block)
}

impl<T: Transport> CompxMouse<T> {
    fn write_chunks(&mut self, address: u16, image: &[u8]) -> Result<(), HidError> {
        for (index, chunk) in image.chunks(10).enumerate() {
            let offset = u16::try_from(index * 10).unwrap_or(u16::MAX);
            self.transfer(Packet::set_eeprom(address + offset, chunk))?;
        }
        Ok(())
    }

    fn macro_address(slot: u16) -> Result<u16, HidError> {
        if slot >= MACRO_SLOTS {
            return Err(HidError::Invalid(format!("macro slot {slot}")));
        }
        Ok(address::MACRO0 + slot * MACRO_STRIDE)
    }

    /// Reads the events stored in an onboard macro slot (0-11).
    ///
    /// # Errors
    ///
    /// Returns an invalid-slot or transport error.
    pub fn read_macro(&mut self, slot: u16) -> Result<Vec<KeyEvent>, HidError> {
        let base = Self::macro_address(slot)? + 31;
        let count = usize::from(self.read(base, 1)?.data(0));
        if count == 0 || count > MACRO_MAX_EVENTS {
            return Ok(Vec::new());
        }
        let mut events = Vec::with_capacity(count);
        for chunk in 0..count.div_ceil(2) {
            let offset = u16::try_from(chunk * 10).unwrap_or(u16::MAX);
            let packet = self.read(base + 1 + offset, 10)?;
            let data: Vec<u8> = (0..10).map(|index| packet.data(index)).collect();
            events.push(KeyEvent::parse(&data[..5]));
            events.push(KeyEvent::parse(&data[5..]));
        }
        events.truncate(count);
        Ok(events)
    }

    /// Writes an onboard macro slot (0-11).
    ///
    /// # Errors
    ///
    /// Returns an invalid-input or transport error.
    pub fn write_macro(&mut self, slot: u16, events: &[KeyEvent]) -> Result<(), HidError> {
        let address = Self::macro_address(slot)?;
        let image = encode_macro(events)?;
        self.write_chunks(address, &image)
    }

    /// Binds a button to an onboard macro slot.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input or transport error.
    pub fn bind_macro(
        &mut self,
        button: usize,
        slot: u8,
        playback: MacroPlayback,
    ) -> Result<(), HidError> {
        self.write_button(button, [6, slot, playback.code()])
    }

    /// Reads the shortcut-key events of a button.
    ///
    /// # Errors
    ///
    /// Returns an invalid-index or transport error.
    pub fn read_shortcut(&mut self, button: usize) -> Result<Vec<KeyEvent>, HidError> {
        let address = Self::shortcut_address(button)?;
        let first = self.read(address, 10)?;
        let count = usize::from(first.data(0));
        if count == 0 || count > SHORTCUT_MAX_EVENTS {
            return Ok(Vec::new());
        }
        let second = self.read(address + 10, 10)?;
        let mut bytes: Vec<u8> = (1..10).map(|index| first.data(index)).collect();
        bytes.extend((0..10).map(|index| second.data(index)));
        Ok(bytes.chunks(3).take(count).map(KeyEvent::parse).collect())
    }

    /// Stores a key combination on a button and binds it (class 5).
    ///
    /// # Errors
    ///
    /// Returns an invalid-input or transport error.
    pub fn write_shortcut(&mut self, button: usize, events: &[KeyEvent]) -> Result<(), HidError> {
        let address = Self::shortcut_address(button)?;
        let block = encode_shortcut(events)?;
        self.write_chunks(address, &block)?;
        self.write_button(button, [5, 0, 0])
    }

    fn shortcut_address(button: usize) -> Result<u16, HidError> {
        if button >= BUTTONS.len() {
            return Err(HidError::Invalid(format!("button index {button}")));
        }
        Ok(KEY_SHORTCUTS0 + 32 * button as u16)
    }
}

#[cfg(test)]
mod onboard_tests {
    use super::*;

    #[test]
    fn macro_image_has_header_count_records_and_checksum() {
        let events = [
            KeyEvent {
                kind: KeyKind::Key,
                down: true,
                value: 4,
                delay_ms: 30,
            },
            KeyEvent {
                kind: KeyKind::Key,
                down: false,
                value: 4,
                delay_ms: 0,
            },
        ];
        let image = encode_macro(&events).unwrap();
        assert_eq!(&image[..9], &[8, 2, 2, 2, 2, 2, 2, 2, 2]);
        assert_eq!(image[31], 2);
        assert_eq!(&image[32..37], &[0x81, 4, 0, 0, 30]);
        assert_eq!(&image[37..42], &[0x41, 4, 0, 0, 0]);
        let sum: u32 = image[31..42].iter().map(|b| u32::from(*b)).sum();
        assert_eq!(image[42], (85_u32.wrapping_sub(sum) & 0xff) as u8);
        assert_eq!(image.len() % 10, 0);
        assert_eq!(KeyEvent::parse(&image[32..37]), events[0]);
    }

    #[test]
    fn shortcut_block_fits_two_packets() {
        let ctrl = KeyEvent {
            kind: KeyKind::Modifier,
            down: true,
            value: 1,
            delay_ms: 0,
        };
        let block = encode_shortcut(&[ctrl; 6]).unwrap();
        assert_eq!(block.len(), 20);
        assert_eq!(block[0], 6);
        assert_eq!(&block[1..4], &[0x80, 1, 0]);
        assert!(encode_shortcut(&[ctrl; 7]).is_err());
    }
}
