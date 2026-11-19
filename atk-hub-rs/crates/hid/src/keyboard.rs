//! VIA / BITYUAN magnetic keyboard driver.
//!
//! ATK HE keyboards (EDGE, RS, VELOTA ...) expose a VIA raw-HID interface
//! (usage page `0xFF60`, usage `0x61`) with 32-byte reports on report ID 0.
//! Byte 0 is the VIA command; vendor settings travel through
//! `get/set_keyboard_value` with the custom channel `0x96` in byte 1 and a
//! sub-command in byte 2, exactly as the original `BityuanKeyboardApi`.

use crate::{HidError, HidTransport, Transport, VXE_VENDOR_ID, kb_models::KEYBOARDS};
use hidapi::HidApi;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// VIA raw-HID usage page.
pub const USAGE_PAGE: u16 = 0xFF60;
/// VIA raw-HID usage.
pub const USAGE: u16 = 0x61;
const REPORT: usize = 32;
const CUSTOM: u8 = 150;

mod cmd {
    pub const PROTOCOL_VERSION: u8 = 1;
    pub const GET_VALUE: u8 = 2;
    pub const SET_VALUE: u8 = 3;
    pub const GET_KEYCODE: u8 = 4;
    pub const SET_KEYCODE: u8 = 5;
    pub const KEYMAP_RESET: u8 = 6;
    pub const LIGHTING_SET: u8 = 7;
    pub const LIGHTING_GET: u8 = 8;
    pub const LAYER_COUNT: u8 = 17;
    pub const GET_BUFFER: u8 = 18;
    pub const MACRO_BUFFER_SIZE: u8 = 13;
    pub const MACRO_GET_BUFFER: u8 = 14;
    pub const MACRO_SET_BUFFER: u8 = 15;
}

mod sub {
    pub const BUFFER_TRAVEL: u8 = 13;
    pub const BUFFER_RT: u8 = 14;
    pub const DEAD_BAND: u8 = 11;
    pub const LIGHT_INFO: u8 = 65;
    pub const FEATURE: u8 = 17;
    pub const GAME_MODE: u8 = 30;
    pub const PROFILE: u8 = 35;
    pub const TRAVEL: u8 = 36;
    pub const LIGHT_OFF: u8 = 38;
    pub const REPORT_RATE: u8 = 70;
    pub const RESTART: u8 = 78;
    pub const MACRO: u8 = 77;
    pub const DKS: u8 = 15;
    pub const ADVANCED: u8 = 32;
}

/// Vial prefix command carrying tap-dance (MT/TGL) slots.
const VIAL_PREFIX: u8 = 254;
const ENTRY_OP: u8 = 13;
const TAP_DANCE_GET: u8 = 1;
const TAP_DANCE_SET: u8 = 2;
const SLOT_GET: u8 = 1;
const SLOT_SET: u8 = 3;

/// Advanced-key slots per kind on BITYUAN magnetic keyboards.
pub const ADVANCED_SLOTS: u8 = 20;
/// Keycode base that points a key at DKS slot `n` (`0x7E05 + n`).
pub const DKS_KEYCODE: u16 = 0x7E05;
/// Keycode base that points a key at MT slot `n` (`0x5700 + n`).
pub const MT_KEYCODE: u16 = 0x5700;
/// Keycode base that points a key at TGL slot `n` (`0x5714 + n`).
pub const TGL_KEYCODE: u16 = 0x5714;

/// One of the four DKS bindings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DksAction {
    /// 10-bit stage mask: bits 0, 3, 6, 9 fire at press, bottom, lift and
    /// release; bits between two stages keep the key held across them.
    pub status: u16,
    /// Keycode sent.
    pub code: u16,
}

/// A dynamic keystroke slot.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DksSlot {
    /// Four bindings.
    pub actions: Vec<DksAction>,
    /// Upper trigger point in millimetres.
    pub top: f32,
    /// Lower trigger point in millimetres.
    pub bottom: f32,
}

/// Rapid snap / SOCD / OKS / AKS slot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedKey {
    /// 0 none, 1 RS, 2 SOCD last input, 3 SOCD key 1, 4 SOCD key 2,
    /// 5 SOCD neutral, 6 OKS, 7 AKS key 1, 8 AKS key 2, 9 AKS balance.
    pub mode: u8,
    /// Trigger point in millimetres.
    pub point: f32,
    /// Follow the global trigger instead of `point`.
    pub with_global: bool,
    /// Rapid trigger enabled.
    pub rt: bool,
    /// First key position.
    pub key1: [u8; 2],
    /// Second key position.
    pub key2: [u8; 2],
    /// Layer.
    pub layer: u8,
    /// AKS trigger delay in milliseconds.
    pub trigger_delay: u16,
    /// AKS key delay in milliseconds.
    pub key_delay: u16,
    /// AKS randomness percent.
    pub random: u8,
    /// AKS trigger switch.
    pub trigger_switch: u8,
}

/// Mod-tap or toggle slot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TapDance {
    /// Keycode on tap (or the toggled key for TGL).
    pub tap: u16,
    /// Keycode while held (MT only).
    pub hold: u16,
    /// Hold threshold in milliseconds.
    pub delay: u16,
}

fn word(bytes: &[u8], index: usize) -> u16 {
    u16::from_be_bytes([
        bytes.get(index).copied().unwrap_or(0),
        bytes.get(index + 1).copied().unwrap_or(0),
    ])
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn micrometres(mm: f32) -> [u8; 2] {
    ((mm * 1000.0).round().clamp(0.0, 65535.0) as u16).to_be_bytes()
}

/// Encodes the payload after `[cmd, 150, 15]` for a DKS write.
#[must_use]
pub fn encode_dks(index: u8, slot: &DksSlot) -> Vec<u8> {
    let mut out = vec![0, 0, SLOT_SET, index];
    for n in 0..4 {
        let action = slot.actions.get(n).copied().unwrap_or_default();
        out.extend_from_slice(&(action.status & 0x3FF).to_be_bytes());
        out.extend_from_slice(&action.code.to_be_bytes());
    }
    let (top, bottom) = (slot.top.min(slot.bottom), slot.top.max(slot.bottom));
    out.extend_from_slice(&micrometres(top));
    out.extend_from_slice(&micrometres(bottom));
    out
}

/// Decodes a DKS reply (including the `[cmd, 150, 15]` echo).
#[must_use]
pub fn decode_dks(reply: &[u8]) -> DksSlot {
    DksSlot {
        actions: (0..4)
            .map(|n| DksAction {
                status: word(reply, 7 + 4 * n) & 0x3FF,
                code: word(reply, 9 + 4 * n),
            })
            .collect(),
        top: f32::from(word(reply, 23)) / 1000.0,
        bottom: f32::from(word(reply, 25)) / 1000.0,
    }
}

/// Encodes the payload after `[cmd, 150, 32]` for an advanced-key write.
#[must_use]
pub fn encode_advanced(index: u8, key: &AdvancedKey) -> Vec<u8> {
    let mut out = vec![0, 0, SLOT_SET, index, key.mode, index % 26];
    out.extend_from_slice(&micrometres(key.point));
    out.extend_from_slice(&[
        u8::from(key.with_global),
        u8::from(key.rt),
        key.key1[0],
        key.key1[1],
        key.key2[0],
        key.key2[1],
        key.layer,
    ]);
    out.extend_from_slice(&key.trigger_delay.to_be_bytes());
    out.extend_from_slice(&key.key_delay.to_be_bytes());
    out.extend_from_slice(&[key.random, key.trigger_switch]);
    out
}

/// Decodes an advanced-key reply (including the `[cmd, 150, 32]` echo).
#[must_use]
pub fn decode_advanced(reply: &[u8]) -> AdvancedKey {
    let at = |index: usize| reply.get(index).copied().unwrap_or(0);
    AdvancedKey {
        mode: at(7),
        point: f32::from(word(reply, 9)) / 1000.0,
        with_global: at(11) != 0,
        rt: at(12) != 0,
        key1: [at(13), at(14)],
        key2: [at(15), at(16)],
        layer: at(17),
        trigger_delay: word(reply, 18),
        key_delay: word(reply, 20),
        random: at(22),
        trigger_switch: at(23),
    }
}

/// Onboard macro slots on BITYUAN keyboards.
pub const MACRO_SLOTS: usize = 8;

/// One key event of an onboard keyboard macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroAction {
    /// Press (`true`) or release.
    pub down: bool,
    /// Basic HID keycode, or 209-213 for left/right/middle/back/forward mouse buttons.
    pub code: u8,
    /// Delay after this event in milliseconds.
    pub delay_ms: u16,
}

/// One onboard macro slot with its playback mode.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardMacro {
    /// Key events in order.
    pub actions: Vec<MacroAction>,
    /// Playback mode 1-6: run N times and stop at once, continue, stop,
    /// finish the round, loop until next press, loop with no stop condition.
    pub mode: u8,
    /// Repeat count.
    pub count: u8,
}

/// Encodes macros into the VIA dynamic macro buffer: `[1, 2|3, code]` key
/// events each followed by `[1, 4, lo+1, hi+1]` delays, macros separated by 0.
#[must_use]
pub fn encode_macros(macros: &[KeyboardMacro]) -> Vec<u8> {
    let mut out = Vec::new();
    for slot in 0..MACRO_SLOTS {
        if let Some(entry) = macros.get(slot) {
            for action in &entry.actions {
                out.extend_from_slice(&[1, if action.down { 2 } else { 3 }, action.code]);
                let [lo, hi] = action.delay_ms.to_le_bytes();
                out.extend_from_slice(&[1, 4, lo.saturating_add(1), hi.saturating_add(1)]);
            }
        }
        out.push(0);
    }
    out
}

/// Decodes the VIA dynamic macro buffer into [`MACRO_SLOTS`] macros.
#[must_use]
pub fn decode_macros(buffer: &[u8]) -> Vec<Vec<MacroAction>> {
    let mut macros = Vec::new();
    let mut current = Vec::new();
    let mut index = 0;
    while index < buffer.len() && macros.len() < MACRO_SLOTS {
        match buffer[index] {
            0 => {
                macros.push(std::mem::take(&mut current));
                index += 1;
            }
            1 => match buffer.get(index + 1).copied() {
                Some(code @ 1..=3) => {
                    let key = buffer.get(index + 2).copied().unwrap_or(0);
                    if code == 1 {
                        current.push(MacroAction {
                            down: true,
                            code: key,
                            delay_ms: 0,
                        });
                        current.push(MacroAction {
                            down: false,
                            code: key,
                            delay_ms: 0,
                        });
                    } else {
                        current.push(MacroAction {
                            down: code == 2,
                            code: key,
                            delay_ms: 0,
                        });
                    }
                    index += 3;
                }
                Some(4) => {
                    let lo = buffer
                        .get(index + 2)
                        .copied()
                        .unwrap_or(1)
                        .saturating_sub(1);
                    let hi = buffer
                        .get(index + 3)
                        .copied()
                        .unwrap_or(1)
                        .saturating_sub(1);
                    if let Some(last) = current.last_mut() {
                        last.delay_ms = u16::from_le_bytes([lo, hi]);
                    }
                    index += 4;
                }
                Some(5..=7) => index += 4,
                _ => index += 2,
            },
            _ => index += 1,
        }
    }
    while macros.len() < MACRO_SLOTS {
        macros.push(Vec::new());
    }
    macros
}

/// Flash operation flag of chunked travel / rapid-trigger writes.
const FLASH_NORMAL: u8 = 0;
const FLASH_ERASE: u8 = 1;
const FLASH_WRITE: u8 = 2;

/// A known keyboard model.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardModel {
    /// Marketing name.
    pub name: String,
    /// Dynamic keymap layers.
    pub layers: u8,
    /// Switch-matrix rows.
    pub rows: u8,
    /// Switch-matrix columns.
    pub cols: u8,
}

/// Looks up a model by vendor and product ID.
#[must_use]
pub fn model(vendor_id: u16, product_id: u16) -> Option<KeyboardModel> {
    KEYBOARDS
        .iter()
        .find(|(vid, pid, ..)| *vid == vendor_id && *pid == product_id)
        .map(|(_, _, layers, rows, cols, name)| KeyboardModel {
            name: (*name).to_owned(),
            layers: *layers,
            rows: *rows,
            cols: *cols,
        })
}

/// A connected keyboard configuration interface.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardInfo {
    /// OS device path.
    pub path: String,
    /// Vendor ID.
    pub vendor_id: u16,
    /// Product ID.
    pub product_id: u16,
    /// Product string reported by the device.
    pub product: String,
    /// Known model, when the product is in the registry.
    pub model: Option<KeyboardModel>,
}

/// Lists connected VIA keyboards; ATK models first.
///
/// # Errors
///
/// Returns an error when hidapi cannot initialise.
pub fn list_keyboards() -> Result<Vec<KeyboardInfo>, HidError> {
    let api = HidApi::new()?;
    let mut list: Vec<KeyboardInfo> = api
        .device_list()
        .filter(|info| info.usage_page() == USAGE_PAGE && info.usage() == USAGE)
        .map(|info| KeyboardInfo {
            path: info.path().to_string_lossy().into_owned(),
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            product: info.product_string().unwrap_or("Keyboard").to_owned(),
            model: model(info.vendor_id(), info.product_id()),
        })
        .collect();
    list.dedup_by(|a, b| a.path == b.path);
    list.sort_by_key(|info| (info.model.is_none(), info.vendor_id != VXE_VENDOR_ID));
    Ok(list)
}

/// Main backlight settings (`lightInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lighting {
    /// Effect index.
    pub mode: u8,
    /// Effect speed (0-255).
    pub speed: u8,
    /// Brightness (0-100).
    pub brightness: u8,
    /// Colour.
    pub color: [u8; 3],
    /// Rainbow colours instead of `color`.
    pub colorful: u8,
}

/// Toggle features (`feature` sub-command).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    /// Lighting area selection.
    pub rgb_area: u8,
    /// Swap WASD with the arrow keys.
    pub wasd_swap: u8,
    /// macOS layout.
    pub mac: u8,
    /// Windows-key lock.
    pub win_lock: u8,
    /// Six-key rollover / anti-ghosting flag.
    pub six_key: u8,
}

/// Switch travel limits reported by the firmware, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelInfo {
    /// Deepest actuation point.
    pub travel_max: f32,
    /// Shallowest actuation point.
    pub travel_min: f32,
    /// Actuation step.
    pub travel_step: f32,
    /// Largest rapid-trigger sensitivity.
    pub rt_max: f32,
    /// Smallest rapid-trigger sensitivity.
    pub rt_min: f32,
    /// Rapid-trigger step.
    pub rt_step: f32,
}

/// Rapid-trigger settings of one key.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RapidTrigger {
    /// Rapid trigger enabled.
    pub on: bool,
    /// Press sensitivity in millimetres.
    pub press: f32,
    /// Release sensitivity in millimetres.
    pub release: f32,
}

/// Everything the keyboard page shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardSnapshot {
    /// Model (registry entry or generic VIA geometry).
    pub model: KeyboardModel,
    /// VIA protocol version.
    pub protocol: u16,
    /// Firmware version, e.g. `1.05`.
    pub version: String,
    /// Active onboard profile (0-3).
    pub profile: u8,
    /// Polling rate in Hz.
    pub report_rate: u16,
    /// Backlight.
    pub lighting: Lighting,
    /// Backlight sleep timeout in seconds; 0 = never.
    pub sleep_secs: u32,
    /// Toggle features.
    pub features: Features,
    /// Performance (game) mode.
    pub game_mode: u8,
    /// Top / bottom dead zones in millimetres.
    pub dead_zone: [f32; 2],
    /// Firmware travel limits.
    pub travel: TravelInfo,
}

fn mm(raw: u16) -> f32 {
    f32::from(raw) / 1000.0
}

fn raw_mm(value: f32) -> u16 {
    (value.max(0.0) * 1000.0).round().min(f32::from(u16::MAX)) as u16
}

fn be(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([
        bytes.get(at).copied().unwrap_or(0),
        bytes.get(at + 1).copied().unwrap_or(0),
    ])
}

/// Report-rate code used by the firmware.
fn rate_code(hz: u16) -> Result<u8, HidError> {
    Ok(match hz {
        1000 => 3,
        2000 => 2,
        4000 => 1,
        8000 => 0,
        _ => {
            return Err(HidError::Invalid(format!(
                "unsupported polling rate {hz} Hz"
            )));
        }
    })
}

fn rate_hz(code: u8) -> u16 {
    match code {
        0 => 8000,
        1 => 4000,
        2 => 2000,
        _ => 1000,
    }
}

/// Splits per-key records into the chunked flash writes the firmware expects.
fn flash_chunks(records: &[u8], keys_per_chunk: usize, record: usize) -> Vec<(u16, u8, Vec<u8>)> {
    let chunks: Vec<&[u8]> = records.chunks(keys_per_chunk * record).collect();
    let last = chunks.len().saturating_sub(1);
    chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            let op = if index == last {
                FLASH_WRITE
            } else if index == 0 {
                FLASH_ERASE
            } else {
                FLASH_NORMAL
            };
            let offset = u16::try_from(index * keys_per_chunk).unwrap_or(u16::MAX);
            (offset, op, chunk.to_vec())
        })
        .collect()
}

/// Light-streaming command (`RGB888_0x1D`).
pub const STREAM_RGB: u8 = 0x1D;
/// Backlight effect that shows streamed per-key colours.
pub const STREAM_MODE: u8 = 46;

/// Builds per-key colour packets: `[0x1D, row, col, count, rgb...]`, each
/// starting at the matrix position of its first light (row-major order).
#[must_use]
pub fn encode_rgb_stream(colors: &[[u8; 3]], cols: u8, per_packet: usize) -> Vec<Vec<u8>> {
    let cols = usize::from(cols.max(1));
    colors
        .chunks(per_packet.max(1))
        .enumerate()
        .map(|(index, chunk)| {
            let first = index * per_packet.max(1);
            let mut packet = vec![
                STREAM_RGB,
                u8::try_from(first / cols).unwrap_or(u8::MAX),
                u8::try_from(first % cols).unwrap_or(u8::MAX),
                u8::try_from(chunk.len()).unwrap_or(u8::MAX),
            ];
            packet.extend(chunk.iter().flatten());
            packet
        })
        .collect()
}

/// A VIA / BITYUAN keyboard.
#[derive(Debug)]
pub struct Keyboard<T: Transport> {
    transport: T,
    model: KeyboardModel,
    stream_lights: usize,
}

impl Keyboard<HidTransport> {
    /// Opens the first connected VIA keyboard.
    ///
    /// # Errors
    ///
    /// Returns [`HidError::NotFound`] when none is connected.
    pub fn open_first() -> Result<(Self, KeyboardInfo), HidError> {
        let info = list_keyboards()?
            .into_iter()
            .next()
            .ok_or(HidError::NotFound)?;
        let mut transport = HidTransport::open(&info.path)?;
        transport.set_timeout(Duration::from_millis(3000));
        let mut keyboard = Self::new(transport, info.model.clone());
        if info.model.is_none() {
            keyboard.model.name.clone_from(&info.product);
        }
        Ok((keyboard, info))
    }
}

impl<T: Transport> Keyboard<T> {
    /// Wraps a transport. Unknown models use VIA's layer count and a full
    /// 6 x 21 matrix.
    pub fn new(transport: T, model: Option<KeyboardModel>) -> Self {
        Self {
            transport,
            stream_lights: 0,
            model: model.unwrap_or(KeyboardModel {
                name: "VIA Keyboard".into(),
                layers: 4,
                rows: 6,
                cols: 21,
            }),
        }
    }

    /// The model geometry in use.
    #[must_use]
    pub fn model(&self) -> &KeyboardModel {
        &self.model
    }

    fn request(&mut self, bytes: &[u8]) -> Result<Vec<u8>, HidError> {
        let mut report = [0_u8; REPORT];
        report[..bytes.len()].copy_from_slice(bytes);
        let echo: Vec<u8> = if matches!(bytes[0], cmd::GET_VALUE | cmd::SET_VALUE) {
            bytes[..bytes.len().min(3)].to_vec()
        } else {
            bytes[..1].to_vec()
        };
        let reply = self
            .transport
            .transfer_where(0, &report, &|reply: &[u8]| reply.starts_with(&echo))?;
        if reply.first() == Some(&0xFF) {
            return Err(HidError::Invalid(format!(
                "keyboard rejected command {}",
                bytes[0]
            )));
        }
        Ok(reply)
    }

    fn get(&mut self, sub: u8, extra: &[u8]) -> Result<Vec<u8>, HidError> {
        let mut bytes = vec![cmd::GET_VALUE, CUSTOM, sub];
        bytes.extend_from_slice(extra);
        self.request(&bytes)
    }

    fn set(&mut self, sub: u8, extra: &[u8]) -> Result<(), HidError> {
        let mut bytes = vec![cmd::SET_VALUE, CUSTOM, sub];
        bytes.extend_from_slice(extra);
        self.request(&bytes).map(drop)
    }

    /// Reads the VIA protocol version.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn protocol_version(&mut self) -> Result<u16, HidError> {
        Ok(be(&self.request(&[cmd::PROTOCOL_VERSION])?, 1))
    }

    /// Reads the firmware version (`major.minor`, hex digits).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn version(&mut self) -> Result<String, HidError> {
        let reply = self.request(&[cmd::GET_VALUE, CUSTOM])?;
        Ok(format!(
            "{:x}.{:02x}",
            reply.get(5).copied().unwrap_or(0),
            reply.get(6).copied().unwrap_or(0)
        ))
    }

    /// Firmware version as the original's decimal code (`1.05` -> 105).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn version_code(&mut self) -> Result<u32, HidError> {
        let text = self.version()?.replace('.', "");
        Ok(text.parse().unwrap_or(0))
    }

    /// Switches the backlight to streamed per-key colours. Firmware 2.00+
    /// accepts frames directly; older firmware needs the streaming effect.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn begin_rgb_stream(&mut self) -> Result<(), HidError> {
        let code = self.version_code().unwrap_or(0);
        self.stream_lights = if code >= 200 { 112 } else { 41 };
        if code < 200 {
            self.set_lighting(Lighting {
                mode: STREAM_MODE,
                speed: 127,
                brightness: 100,
                color: [255, 0, 255],
                colorful: 0,
            })?;
        }
        Ok(())
    }

    /// Streams one frame of per-key colours (row-major, `rows x cols`).
    /// Falls back to smaller packets when the interface rejects big reports.
    ///
    /// # Errors
    ///
    /// Returns a transport error when even 32-byte packets fail.
    pub fn stream_rgb(&mut self, colors: &[[u8; 3]]) -> Result<(), HidError> {
        if self.stream_lights == 0 {
            self.stream_lights = 41;
        }
        loop {
            let packets = encode_rgb_stream(colors, self.model.cols, self.stream_lights);
            let result = packets
                .iter()
                .try_for_each(|packet| self.transport.write_only(0, packet));
            match result {
                Ok(()) => return Ok(()),
                Err(error) => {
                    // 64-byte then 32-byte reports.
                    self.stream_lights = match self.stream_lights {
                        n if n > 19 => 19,
                        n if n > 9 => 9,
                        _ => return Err(error),
                    };
                }
            }
        }
    }

    /// Reads the number of dynamic keymap layers and updates the model.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn layer_count(&mut self) -> Result<u8, HidError> {
        let reply = self.request(&[cmd::LAYER_COUNT])?;
        let layers = reply.get(1).copied().filter(|n| (1..=16).contains(n));
        if let Some(layers) = layers {
            self.model.layers = layers;
        }
        Ok(self.model.layers)
    }

    /// Reads one keycode.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn keycode(&mut self, layer: u8, row: u8, col: u8) -> Result<u16, HidError> {
        Ok(be(&self.request(&[cmd::GET_KEYCODE, layer, row, col])?, 4))
    }

    /// Remaps one key.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_keycode(&mut self, layer: u8, row: u8, col: u8, code: u16) -> Result<(), HidError> {
        let [hi, lo] = code.to_be_bytes();
        self.request(&[cmd::SET_KEYCODE, layer, row, col, hi, lo])
            .map(drop)
    }

    /// Reads a whole layer as `rows x cols` keycodes.
    ///
    /// # Errors
    ///
    /// Returns an invalid-layer or transport error.
    pub fn keymap(&mut self, layer: u8) -> Result<Vec<Vec<u16>>, HidError> {
        if layer >= self.model.layers {
            return Err(HidError::Invalid(format!("layer {layer}")));
        }
        let (rows, cols) = (usize::from(self.model.rows), usize::from(self.model.cols));
        let size = rows * cols * 2;
        let start = usize::from(layer) * size;
        let mut bytes = Vec::with_capacity(size);
        while bytes.len() < size {
            let len = (size - bytes.len()).min(28);
            let offset = u16::try_from(start + bytes.len())
                .map_err(|_| HidError::Invalid("keymap too large".into()))?;
            let [hi, lo] = offset.to_be_bytes();
            let reply = self.request(&[cmd::GET_BUFFER, hi, lo, len as u8])?;
            bytes.extend((0..len).map(|index| reply.get(4 + index).copied().unwrap_or(0)));
        }
        Ok(bytes
            .chunks(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>()
            .chunks(cols)
            .map(<[u16]>::to_vec)
            .collect())
    }

    /// Restores the factory keymap.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn reset_keymap(&mut self) -> Result<(), HidError> {
        self.request(&[cmd::KEYMAP_RESET]).map(drop)
    }

    /// Reads a dynamic keystroke slot (0-19).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn dks(&mut self, index: u8) -> Result<DksSlot, HidError> {
        Ok(decode_dks(&self.get(sub::DKS, &[0, 0, SLOT_GET, index])?))
    }

    /// Writes a dynamic keystroke slot. Point a key at it with
    /// [`DKS_KEYCODE`] `+ index`.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_dks(&mut self, index: u8, slot: &DksSlot) -> Result<(), HidError> {
        self.set(sub::DKS, &encode_dks(index, slot))
    }

    /// Reads a rapid snap / SOCD / OKS / AKS slot (0-19).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn advanced_key(&mut self, index: u8) -> Result<AdvancedKey, HidError> {
        Ok(decode_advanced(
            &self.get(sub::ADVANCED, &[0, 0, SLOT_GET, index])?,
        ))
    }

    /// Writes a rapid snap / SOCD / OKS / AKS slot; mode 0 clears it.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_advanced_key(&mut self, index: u8, key: &AdvancedKey) -> Result<(), HidError> {
        self.set(sub::ADVANCED, &encode_advanced(index, key))
    }

    /// Reads a tap-dance slot: 0-19 are mod-tap, 20-39 toggle.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn tap_dance(&mut self, index: u8) -> Result<TapDance, HidError> {
        let reply = self.request(&[VIAL_PREFIX, ENTRY_OP, TAP_DANCE_GET, index])?;
        Ok(TapDance {
            tap: word(&reply, 4),
            hold: word(&reply, 6),
            delay: word(&reply, 8),
        })
    }

    /// Writes a tap-dance slot: 0-19 are mod-tap ([`MT_KEYCODE`]), 20-39
    /// toggle ([`TGL_KEYCODE`]).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_tap_dance(&mut self, index: u8, slot: &TapDance) -> Result<(), HidError> {
        let mut bytes = vec![VIAL_PREFIX, ENTRY_OP, TAP_DANCE_SET, index];
        bytes.extend_from_slice(&slot.tap.to_be_bytes());
        bytes.extend_from_slice(&slot.hold.to_be_bytes());
        bytes.extend_from_slice(&slot.delay.to_be_bytes());
        self.request(&bytes).map(drop)
    }

    /// Reads all onboard macros with their playback modes.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn macros(&mut self) -> Result<Vec<KeyboardMacro>, HidError> {
        let size = usize::from(be(&self.request(&[cmd::MACRO_BUFFER_SIZE])?, 1));
        let mut buffer = Vec::with_capacity(size);
        while buffer.len() < size {
            let len = (size - buffer.len()).min(28);
            let [hi, lo] = u16::try_from(buffer.len())
                .unwrap_or(u16::MAX)
                .to_be_bytes();
            #[allow(clippy::cast_possible_truncation)]
            let reply = self.request(&[cmd::MACRO_GET_BUFFER, hi, lo, len as u8])?;
            buffer.extend((0..len).map(|index| reply.get(4 + index).copied().unwrap_or(0)));
            if buffer.iter().filter(|byte| **byte == 0).count() > MACRO_SLOTS {
                break;
            }
        }
        let mut out = Vec::with_capacity(MACRO_SLOTS);
        for (slot, actions) in decode_macros(&buffer).into_iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let config = self.get(sub::MACRO, &[slot as u8]).unwrap_or_default();
            out.push(KeyboardMacro {
                actions,
                mode: config
                    .get(4)
                    .copied()
                    .filter(|m| (1..=6).contains(m))
                    .unwrap_or(1),
                count: config.get(5).copied().unwrap_or(1),
            });
        }
        Ok(out)
    }

    /// Writes all onboard macros and their playback modes.
    ///
    /// # Errors
    ///
    /// Returns an error when the macros do not fit the keyboard's buffer or
    /// the transport fails.
    pub fn set_macros(&mut self, macros: &[KeyboardMacro]) -> Result<(), HidError> {
        let bytes = encode_macros(macros);
        let size = usize::from(be(&self.request(&[cmd::MACRO_BUFFER_SIZE])?, 1));
        if size > 0 && bytes.len() > size {
            return Err(HidError::Invalid(format!(
                "macros need {} bytes but the keyboard stores {size}",
                bytes.len()
            )));
        }
        for (index, chunk) in bytes.chunks(28).enumerate() {
            let [hi, lo] = u16::try_from(index * 28).unwrap_or(u16::MAX).to_be_bytes();
            #[allow(clippy::cast_possible_truncation)]
            let mut report = vec![cmd::MACRO_SET_BUFFER, hi, lo, chunk.len() as u8];
            report.extend_from_slice(chunk);
            self.request(&report)?;
        }
        for (slot, entry) in macros.iter().enumerate().take(MACRO_SLOTS) {
            #[allow(clippy::cast_possible_truncation)]
            self.set(
                sub::MACRO,
                &[slot as u8, entry.mode.clamp(1, 6), entry.count.max(1)],
            )?;
        }
        Ok(())
    }

    /// Reads the main backlight.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn lighting(&mut self) -> Result<Lighting, HidError> {
        let r = self.request(&[cmd::LIGHTING_GET, sub::LIGHT_INFO])?;
        let at = |index: usize| r.get(index).copied().unwrap_or(0);
        Ok(Lighting {
            mode: at(2),
            speed: at(4),
            color: [at(5), at(6), at(7)],
            brightness: at(8),
            colorful: at(9),
        })
    }

    /// Writes the main backlight.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_lighting(&mut self, light: Lighting) -> Result<(), HidError> {
        let [r, g, b] = light.color;
        self.request(&[
            cmd::LIGHTING_SET,
            sub::LIGHT_INFO,
            light.mode,
            0,
            light.speed,
            r,
            g,
            b,
            light.brightness,
            light.colorful,
        ])
        .map(drop)
    }

    /// Backlight sleep timeout in seconds (0 = never).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn sleep_secs(&mut self) -> Result<u32, HidError> {
        let minutes = self.get(sub::LIGHT_OFF, &[])?.get(3).copied().unwrap_or(0);
        Ok(u32::from(minutes) * 60)
    }

    /// Sets the backlight sleep timeout in seconds (0 = never).
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_sleep_secs(&mut self, secs: u32) -> Result<(), HidError> {
        let minutes = u8::try_from(secs / 60).unwrap_or(u8::MAX);
        self.set(sub::LIGHT_OFF, &[minutes])
    }

    /// Active onboard profile.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn profile(&mut self) -> Result<u8, HidError> {
        Ok(self.get(sub::PROFILE, &[])?.get(3).copied().unwrap_or(0))
    }

    /// Switches the onboard profile.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_profile(&mut self, profile: u8) -> Result<(), HidError> {
        self.set(sub::PROFILE, &[profile.min(3)])
    }

    /// Polling rate in Hz.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn report_rate(&mut self) -> Result<u16, HidError> {
        Ok(rate_hz(
            self.get(sub::REPORT_RATE, &[])?
                .get(3)
                .copied()
                .unwrap_or(3),
        ))
    }

    /// Sets the polling rate (1000/2000/4000/8000 Hz).
    ///
    /// # Errors
    ///
    /// Returns an invalid-rate or transport error.
    pub fn set_report_rate(&mut self, hz: u16) -> Result<(), HidError> {
        let code = rate_code(hz)?;
        self.set(sub::RESTART, &[1])?;
        self.set(sub::REPORT_RATE, &[code])
    }

    /// Toggle features.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn features(&mut self) -> Result<Features, HidError> {
        let r = self.get(sub::FEATURE, &[])?;
        let at = |index: usize| r.get(index).copied().unwrap_or(0);
        Ok(Features {
            rgb_area: at(3),
            wasd_swap: at(4),
            mac: at(5),
            win_lock: at(6),
            six_key: at(7),
        })
    }

    /// Writes the toggle features.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_features(&mut self, f: Features) -> Result<(), HidError> {
        self.set(
            sub::FEATURE,
            &[f.rgb_area, f.wasd_swap, f.mac, f.win_lock, f.six_key],
        )
    }

    /// Performance (game) mode.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn game_mode(&mut self) -> Result<u8, HidError> {
        Ok(self.get(sub::GAME_MODE, &[])?.get(3).copied().unwrap_or(0))
    }

    /// Sets performance (game) mode.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_game_mode(&mut self, on: bool) -> Result<(), HidError> {
        self.set(sub::GAME_MODE, &[u8::from(on)])
    }

    /// Top and bottom dead zones in millimetres.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn dead_zone(&mut self) -> Result<[f32; 2], HidError> {
        let r = self.get(sub::DEAD_BAND, &[])?;
        Ok([mm(be(&r, 3)), mm(be(&r, 5))])
    }

    /// Sets the top and bottom dead zones in millimetres.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn set_dead_zone(&mut self, top: f32, bottom: f32) -> Result<(), HidError> {
        let [th, tl] = raw_mm(top).to_be_bytes();
        let [bh, bl] = raw_mm(bottom).to_be_bytes();
        self.set(sub::DEAD_BAND, &[th, tl, bh, bl])
    }

    /// Firmware travel limits.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn travel_info(&mut self) -> Result<TravelInfo, HidError> {
        let r = self.get(sub::TRAVEL, &[])?;
        Ok(TravelInfo {
            travel_max: mm(be(&r, 3)),
            travel_min: mm(be(&r, 5)),
            travel_step: mm(be(&r, 7)),
            rt_max: mm(be(&r, 9)),
            rt_min: mm(be(&r, 11)),
            rt_step: mm(be(&r, 13)),
        })
    }

    fn key_count(&self) -> usize {
        usize::from(self.model.rows) * usize::from(self.model.cols)
    }

    fn read_buffer(&mut self, sub: u8, per: usize, record: usize) -> Result<Vec<u8>, HidError> {
        let keys = self.key_count();
        let mut out = Vec::with_capacity(keys * record);
        for chunk in 0..keys.div_ceil(per) {
            let [hi, lo] = u16::try_from(chunk * per).unwrap_or(0).to_be_bytes();
            let reply = self.get(sub, &[0, 0, hi, lo, per as u8])?;
            out.extend((0..per * record).map(|index| reply.get(8 + index).copied().unwrap_or(0)));
        }
        out.truncate(keys * record);
        Ok(out)
    }

    fn write_buffer(
        &mut self,
        sub: u8,
        records: &[u8],
        per: usize,
        record: usize,
    ) -> Result<(), HidError> {
        let chunks = flash_chunks(records, per, record);
        let last = chunks.len().saturating_sub(1);
        for (index, (offset, op, data)) in chunks.into_iter().enumerate() {
            let [hi, lo] = offset.to_be_bytes();
            let mut extra = vec![0, 0, hi, lo, per as u8, op];
            extra.extend(data);
            self.set(sub, &extra)?;
            if index == 0 || index == last {
                std::thread::sleep(Duration::from_millis(200));
            }
        }
        Ok(())
    }

    /// Actuation point of every key, row-major, in millimetres.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn actuation(&mut self) -> Result<Vec<f32>, HidError> {
        let bytes = self.read_buffer(sub::BUFFER_TRAVEL, 12, 2)?;
        Ok(bytes.chunks(2).map(|pair| mm(be(pair, 0))).collect())
    }

    /// Writes the actuation point of every key (row-major, millimetres).
    ///
    /// # Errors
    ///
    /// Returns an invalid-length or transport error.
    pub fn set_actuation(&mut self, points: &[f32]) -> Result<(), HidError> {
        if points.len() != self.key_count() {
            return Err(HidError::Invalid(format!(
                "expected {} actuation points",
                self.key_count()
            )));
        }
        let records: Vec<u8> = points
            .iter()
            .flat_map(|point| raw_mm(*point).to_be_bytes())
            .collect();
        self.write_buffer(sub::BUFFER_TRAVEL, &records, 11, 2)
    }

    /// Rapid-trigger settings of every key, row-major.
    ///
    /// # Errors
    ///
    /// Returns a transport error.
    pub fn rapid_trigger(&mut self) -> Result<Vec<RapidTrigger>, HidError> {
        let bytes = self.read_buffer(sub::BUFFER_RT, 4, 5)?;
        Ok(bytes
            .chunks(5)
            .map(|r| RapidTrigger {
                on: r[0] != 0,
                release: mm(be(r, 1)),
                press: mm(be(r, 3)),
            })
            .collect())
    }

    /// Writes the rapid-trigger settings of every key (row-major).
    ///
    /// # Errors
    ///
    /// Returns an invalid-length or transport error.
    pub fn set_rapid_trigger(&mut self, keys: &[RapidTrigger]) -> Result<(), HidError> {
        if keys.len() != self.key_count() {
            return Err(HidError::Invalid(format!(
                "expected {} rapid-trigger entries",
                self.key_count()
            )));
        }
        let records: Vec<u8> = keys
            .iter()
            .flat_map(|key| {
                let [rh, rl] = raw_mm(key.release).to_be_bytes();
                let [ph, pl] = raw_mm(key.press).to_be_bytes();
                [u8::from(key.on), rh, rl, ph, pl]
            })
            .collect();
        self.write_buffer(sub::BUFFER_RT, &records, 4, 5)
    }

    /// Reads everything the keyboard page shows. Optional vendor settings
    /// that a firmware does not implement fall back to defaults.
    ///
    /// # Errors
    ///
    /// Returns a transport error when the keyboard does not answer at all.
    pub fn snapshot(&mut self) -> Result<KeyboardSnapshot, HidError> {
        let protocol = self.protocol_version()?;
        let _ = self.layer_count();
        Ok(KeyboardSnapshot {
            protocol,
            version: self.version().unwrap_or_default(),
            profile: self.profile().unwrap_or(0),
            report_rate: self.report_rate().unwrap_or(1000),
            lighting: self.lighting().unwrap_or_default(),
            sleep_secs: self.sleep_secs().unwrap_or(0),
            features: self.features().unwrap_or_default(),
            game_mode: self.game_mode().unwrap_or(0),
            dead_zone: self.dead_zone().unwrap_or([0.0, 0.0]),
            travel: self.travel_info().unwrap_or_default(),
            model: self.model.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_stream_packets_start_at_matrix_positions() {
        let colors: Vec<[u8; 3]> = (0..70u8).map(|i| [i, 0, 255 - i]).collect();
        let packets = encode_rgb_stream(&colors, 14, 41);
        assert_eq!(packets.len(), 2);
        assert_eq!(&packets[0][..4], &[0x1D, 0, 0, 41]);
        assert_eq!(packets[0].len(), 4 + 41 * 3);
        // Light 41 sits at row 2, column 13 on a 14-column board.
        assert_eq!(&packets[1][..4], &[0x1D, 2, 13, 29]);
        assert_eq!(&packets[1][4..7], &[41, 0, 214]);
    }
    use std::collections::VecDeque;

    #[derive(Default)]
    struct Fake {
        sent: Vec<Vec<u8>>,
        replies: VecDeque<Vec<u8>>,
    }

    impl Transport for Fake {
        fn transfer(&mut self, _: u8, data: &[u8], _: u8) -> Result<Vec<u8>, HidError> {
            self.sent.push(data.to_vec());
            Ok(self.replies.pop_front().unwrap_or_else(|| data.to_vec()))
        }
    }

    fn keyboard(rows: u8, cols: u8) -> Keyboard<Fake> {
        Keyboard::new(
            Fake::default(),
            Some(KeyboardModel {
                name: "test".into(),
                layers: 2,
                rows,
                cols,
            }),
        )
    }

    #[test]
    fn dks_round_trips() {
        let slot = DksSlot {
            actions: vec![
                DksAction {
                    status: 0b11,
                    code: 4,
                },
                DksAction {
                    status: 1 << 9,
                    code: 5,
                },
                DksAction::default(),
                DksAction::default(),
            ],
            top: 0.5,
            bottom: 3.0,
        };
        let mut reply = vec![3, 150, 15];
        reply.extend(encode_dks(2, &slot));
        assert_eq!(&reply[3..7], &[0, 0, 3, 2]);
        assert_eq!(decode_dks(&reply), slot);
    }

    #[test]
    fn advanced_key_round_trips() {
        let key = AdvancedKey {
            mode: 2,
            point: 1.5,
            rt: true,
            key1: [2, 1],
            key2: [2, 3],
            trigger_delay: 200,
            key_delay: 200,
            random: 50,
            ..AdvancedKey::default()
        };
        let mut reply = vec![3, 150, 32];
        reply.extend(encode_advanced(4, &key));
        assert_eq!(reply.len(), 24);
        assert_eq!(decode_advanced(&reply), key);
    }

    #[test]
    fn macro_buffer_round_trips() {
        let macros = vec![KeyboardMacro {
            actions: vec![
                MacroAction {
                    down: true,
                    code: 4,
                    delay_ms: 300,
                },
                MacroAction {
                    down: false,
                    code: 4,
                    delay_ms: 20,
                },
            ],
            mode: 1,
            count: 1,
        }];
        let bytes = encode_macros(&macros);
        assert_eq!(&bytes[..7], &[1, 2, 4, 1, 4, 45, 2]);
        assert_eq!(bytes.iter().filter(|b| **b == 0).count(), MACRO_SLOTS);
        let decoded = decode_macros(&bytes);
        assert_eq!(decoded[0], macros[0].actions);
        assert!(decoded[1].is_empty());
    }

    #[test]
    fn registry_knows_edge60() {
        let model = model(0x373b, 0x1082).unwrap();
        assert_eq!((model.layers, model.rows, model.cols), (4, 5, 14));
    }

    #[test]
    fn set_keycode_and_lighting_frames() {
        let mut kb = keyboard(1, 2);
        kb.set_keycode(1, 2, 3, 0x5220).unwrap();
        kb.set_lighting(Lighting {
            mode: 4,
            speed: 2,
            brightness: 3,
            color: [1, 2, 3],
            colorful: 1,
        })
        .unwrap();
        kb.set_report_rate(8000).unwrap();
        let sent = &kb.transport.sent;
        assert_eq!(&sent[0][..6], &[5, 1, 2, 3, 0x52, 0x20]);
        assert_eq!(&sent[1][..10], &[7, 65, 4, 0, 2, 1, 2, 3, 3, 1]);
        assert_eq!(&sent[2][..4], &[3, 150, 78, 1]);
        assert_eq!(&sent[3][..4], &[3, 150, 70, 0]);
        assert!(sent.iter().all(|frame| frame.len() == 32));
    }

    #[test]
    fn keymap_reads_buffer_in_chunks() {
        let mut kb = keyboard(1, 2);
        let mut reply = vec![18, 0, 4, 4, 0, 4, 0x52, 0x20];
        reply.resize(32, 0);
        kb.transport.replies.push_back(reply);
        assert_eq!(kb.keymap(1).unwrap(), vec![vec![4, 0x5220]]);
        assert_eq!(&kb.transport.sent[0][..4], &[18, 0, 4, 4]);
    }

    #[test]
    fn actuation_write_uses_erase_then_write_chunks() {
        let mut kb = keyboard(2, 12);
        kb.set_actuation(&[1.2; 24]).unwrap();
        let sent = &kb.transport.sent;
        assert_eq!(sent.len(), 3);
        assert_eq!(&sent[0][..9], &[3, 150, 13, 0, 0, 0, 0, 11, FLASH_ERASE]);
        assert_eq!(&sent[0][9..11], &1200_u16.to_be_bytes());
        assert_eq!(&sent[1][5..9], &[0, 11, 11, FLASH_NORMAL]);
        assert_eq!(&sent[2][5..9], &[0, 22, 11, FLASH_WRITE]);
        assert!(kb.set_actuation(&[1.0; 3]).is_err());
    }

    #[test]
    fn rapid_trigger_round_trip_records() {
        let mut kb = keyboard(1, 1);
        let mut reply = vec![2, 150, 14, 0, 0, 0, 0, 4, 1, 0, 100, 0, 200];
        reply.resize(32, 0);
        kb.transport.replies.push_back(reply);
        let keys = kb.rapid_trigger().unwrap();
        assert_eq!(
            keys,
            vec![RapidTrigger {
                on: true,
                release: 0.1,
                press: 0.2
            }]
        );
    }
}
