//! Sensor-specific DPI encodings used by COMPX mouse EEPROM stages.
//!
//! Each DPI stage is stored as `(x, y, ex)` bytes; `ex` carries the high bits
//! and multiplier flags. The formulas mirror the original `ce`/`he` helpers.

use crate::tables::{PAW3315_TABLE, PAW3395SE_TABLE};
use serde::{Deserialize, Serialize};

/// Optical sensor family; selects the DPI encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sensor {
    /// PixArt PAW3955 (MASTER), logical 16-bit stages at a separate address.
    Paw3955Master,
    /// PixArt PAW3950 Ultra; the original default.
    Paw3950Ultra,
    /// PixArt PAW3950 DM.
    Paw3950Dm,
    /// PixArt PAW3950.
    Paw3950,
    /// PixArt PAW3395 Ultra.
    Paw3395Ultra,
    /// PixArt PAW3395.
    Paw3395,
    /// PixArt PAW3315 (table based).
    Paw3315,
    /// PixArt PAW3395 SE (table based).
    Paw3395Se,
    /// PixArt PAW3311 (table based, same as PAW3395 SE).
    Paw3311,
    /// PixArt PAW3320.
    Paw3320,
    /// CORE 26K.
    Core26k,
}

const HIGH: u32 = 10_000;
const DOUBLE: u32 = 30_000;
const OFFSET: u32 = 10_050;

/// Encoded EEPROM triplet for one stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DpiBytes {
    /// Low byte of the X value.
    pub x: u8,
    /// Low byte of the Y value.
    pub y: u8,
    /// High bits and flags.
    pub ex: u8,
}

fn low(value: u32) -> u8 {
    (value & 0xff) as u8
}

fn high2(value: u32) -> u8 {
    ((value >> 8) & 3) as u8
}

fn table_decode(table: &[(u8, u32, bool)], x: u8, y: u8, ex: u8, default: u32) -> (u32, u32) {
    let lookup = |hex: u8, doubled: bool| {
        table
            .iter()
            .find(|(h, _, _)| *h == hex)
            .map_or(default, |(_, target, can_double)| {
                if doubled && *can_double {
                    target * 2
                } else if doubled {
                    0
                } else {
                    *target
                }
            })
    };
    // Bit string "00Y000X0": bit 1 doubles X, bit 5 doubles Y.
    (lookup(x, ex & 0b10 != 0), lookup(y, ex & 0b10_0000 != 0))
}

fn table_encode(table: &[(u8, u32, bool)], x: u32, y: u32) -> DpiBytes {
    let snap = |value: u32| {
        let mut snapped = 50 * ((value + 25) / 50);
        if snapped > HIGH && !snapped.is_multiple_of(100) {
            snapped += 50;
        }
        snapped
    };
    let (sx, sy) = (snap(x), snap(y));
    let find = |value: u32| {
        table
            .iter()
            .find(|(_, target, doubled)| *target == value || (*doubled && target * 2 == value))
            .map_or(0, |(hex, _, _)| *hex)
    };
    let mut ex = 0;
    if sx > HIGH {
        ex |= 0b10;
    }
    if sy > HIGH {
        ex |= 0b10_0000;
    }
    DpiBytes {
        x: find(sx),
        y: find(sy),
        ex,
    }
}

impl Sensor {
    /// Decodes stored bytes to logical `(x, y)` DPI.
    #[must_use]
    pub fn decode(self, bytes: DpiBytes) -> (u32, u32) {
        let DpiBytes { x, y, ex } = bytes;
        let (x, y, ex) = (u32::from(x), u32::from(y), u32::from(ex));
        let hx = x | (((ex & 12) >> 2) << 8);
        let hy = y | (((ex & 192) >> 6) << 8);
        match self {
            Self::Paw3955Master => (x, y),
            Self::Paw3950Ultra => {
                let mut dx = if ex & 2 != 0 {
                    50 * hx + OFFSET
                } else {
                    10 * (hx + 1)
                };
                let mut dy = if ex & 32 != 0 {
                    50 * hy + OFFSET
                } else {
                    10 * (hy + 1)
                };
                if ex & 1 != 0 {
                    dx *= 2;
                }
                if ex & 16 != 0 {
                    dy *= 2;
                }
                (dx, dy)
            }
            Self::Core26k => (
                (1 + hx) * if ex & 2 != 0 { 100 } else { 50 } * if ex & 1 != 0 { 2 } else { 1 },
                (1 + hy) * if ex & 32 != 0 { 100 } else { 50 } * if ex & 16 != 0 { 2 } else { 1 },
            ),
            Self::Paw3950 | Self::Paw3950Dm | Self::Paw3395Ultra | Self::Paw3395 => {
                let mut dx = 50 * (1 + hx);
                let mut dy = 50 * (1 + hy);
                if ex & 1 != 0 {
                    dx *= 2;
                }
                if ex & 16 != 0 {
                    dy *= 2;
                }
                (dx, dy)
            }
            Self::Paw3315 => table_decode(PAW3315_TABLE, bytes.x, bytes.y, bytes.ex, 16_000),
            Self::Paw3320 => (200 * ((39 * x + 100) / 200), 200 * ((38 * y + 100) / 200)),
            Self::Paw3395Se | Self::Paw3311 => {
                table_decode(PAW3395SE_TABLE, bytes.x, bytes.y, bytes.ex, 18_000)
            }
        }
    }

    /// Encodes logical `(x, y)` DPI to stored bytes.
    #[must_use]
    pub fn encode(self, x: u32, y: u32) -> DpiBytes {
        match self {
            Self::Paw3955Master => DpiBytes {
                x: low(x),
                y: low(y),
                ex: 0,
            },
            Self::Paw3950Ultra => {
                let step = |value: u32, high: bool| -> u32 {
                    if high {
                        if value <= DOUBLE {
                            value.saturating_sub(OFFSET) / 50
                        } else {
                            (value / 2).saturating_sub(OFFSET) / 50
                        }
                    } else {
                        (value / 10).saturating_sub(1)
                    }
                };
                let (hx, hy) = (x > HIGH, y > HIGH);
                let (rx, ry) = (step(x, hx), step(y, hy));
                let mut ex = (high2(ry) << 6) | (high2(rx) << 2);
                if hx {
                    ex |= 2;
                    if x > DOUBLE {
                        ex |= 1;
                    }
                }
                if hy {
                    ex |= 32;
                    if y > DOUBLE {
                        ex |= 16;
                    }
                }
                DpiBytes {
                    x: low(rx),
                    y: low(ry),
                    ex,
                }
            }
            Self::Core26k => {
                let (rx, ry) = ((x / 50).saturating_sub(1), (y / 50).saturating_sub(1));
                DpiBytes {
                    x: low(rx),
                    y: low(ry),
                    ex: (high2(rx) << 2) | (high2(ry) << 6),
                }
            }
            Self::Paw3950 | Self::Paw3950Dm | Self::Paw3395Ultra | Self::Paw3395 => {
                let mut flags = 0;
                if x > DOUBLE {
                    flags |= 1;
                }
                if y > DOUBLE {
                    flags |= 16;
                }
                let rx = (x / if x <= DOUBLE { 50 } else { 100 }).saturating_sub(1);
                let ry = (y / if y <= DOUBLE { 50 } else { 100 }).saturating_sub(1);
                DpiBytes {
                    x: low(rx),
                    y: low(ry),
                    ex: (high2(ry) << 6) | (high2(rx) << 2) | flags,
                }
            }
            Self::Paw3315 => table_encode(PAW3315_TABLE, x, y),
            Self::Paw3320 => DpiBytes {
                x: low((x + 19) / 39),
                y: low((y + 19) / 38),
                ex: 0,
            },
            Self::Paw3395Se | Self::Paw3311 => table_encode(PAW3395SE_TABLE, x, y),
        }
    }

    /// Highest DPI the UI should offer for this sensor.
    #[must_use]
    pub fn max_dpi(self) -> u32 {
        match self {
            Self::Paw3955Master | Self::Paw3950Ultra | Self::Paw3950 | Self::Paw3950Dm => 30_000,
            Self::Core26k => 26_000,
            Self::Paw3395Ultra | Self::Paw3395 => 26_000,
            Self::Paw3395Se | Self::Paw3311 => 18_000,
            Self::Paw3315 => 16_000,
            Self::Paw3320 => 5_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paw3950_ultra_round_trips_common_values() {
        for dpi in [400, 800, 1600, 3200, 10_000, 12_000, 26_000, 30_000] {
            let bytes = Sensor::Paw3950Ultra.encode(dpi, dpi);
            assert_eq!(Sensor::Paw3950Ultra.decode(bytes), (dpi, dpi), "{dpi}");
        }
    }

    #[test]
    fn paw3395_round_trips_common_values() {
        for dpi in [50, 400, 800, 1600, 26_000] {
            let bytes = Sensor::Paw3395.encode(dpi, dpi);
            assert_eq!(Sensor::Paw3395.decode(bytes), (dpi, dpi), "{dpi}");
        }
    }

    #[test]
    fn core26k_round_trips() {
        let bytes = Sensor::Core26k.encode(1600, 3200);
        assert_eq!(Sensor::Core26k.decode(bytes), (1600, 3200));
    }

    #[test]
    fn table_sensor_round_trips() {
        let bytes = Sensor::Paw3395Se.encode(800, 1600);
        assert_eq!(Sensor::Paw3395Se.decode(bytes), (800, 1600));
    }
}
