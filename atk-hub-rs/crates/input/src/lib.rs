//! Safe wrapper around Windows key injection for recorded macros.
//!
//! Injection is only performed when the user explicitly runs a macro; tests
//! cover parsing and sequencing, never actual input events.

use std::str::FromStr;
use thiserror::Error;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, keybd_event,
};

/// Failures while building a key sequence.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InputError {
    /// A key name was not recognized.
    #[error("unknown macro key: {name}")]
    UnknownKey {
        /// Unrecognized key name.
        name: String,
    },
}

/// A named virtual key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Ctrl modifier.
    Ctrl,
    /// Shift modifier.
    Shift,
    /// Alt modifier.
    Alt,
    /// Enter key.
    Enter,
    /// Space bar.
    Space,
    /// Tab key.
    Tab,
    /// Escape key.
    Escape,
    /// Backspace key.
    Backspace,
    /// Function key 1 through 12.
    Function(u8),
    /// An ASCII letter.
    Letter(u8),
    /// An ASCII digit.
    Digit(u8),
}

impl Key {
    /// Returns the Windows virtual-key code.
    #[must_use]
    pub const fn vk(self) -> u8 {
        match self {
            Self::Ctrl => 0x11,
            Self::Shift => 0x10,
            Self::Alt => 0x12,
            Self::Enter => 0x0d,
            Self::Space => 0x20,
            Self::Tab => 0x09,
            Self::Escape => 0x1b,
            Self::Backspace => 0x08,
            Self::Function(number) => 0x70 + number.saturating_sub(1),
            Self::Letter(letter) => 0x41 + (letter.wrapping_sub(b'A') % 26),
            Self::Digit(digit) => 0x30 + (digit.wrapping_sub(b'0') % 10),
        }
    }
}

impl FromStr for Key {
    type Err = InputError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let upper = name.to_ascii_uppercase();
        let key = match upper.as_str() {
            "CTRL" | "CONTROL" => Self::Ctrl,
            "SHIFT" => Self::Shift,
            "ALT" => Self::Alt,
            "ENTER" | "RETURN" => Self::Enter,
            "SPACE" | " " => Self::Space,
            "TAB" => Self::Tab,
            "ESC" | "ESCAPE" => Self::Escape,
            "BACKSPACE" => Self::Backspace,
            _ if upper.len() == 1 && upper.as_bytes()[0].is_ascii_uppercase() => {
                Self::Letter(upper.as_bytes()[0])
            }
            _ if upper.len() == 1 && upper.as_bytes()[0].is_ascii_digit() => {
                Self::Digit(upper.as_bytes()[0])
            }
            _ if upper.starts_with('F') => {
                let number: u8 = upper[1..].parse().unwrap_or(0);
                if !(1..=12).contains(&number) {
                    return Err(InputError::UnknownKey {
                        name: name.to_owned(),
                    });
                }
                Self::Function(number)
            }
            _ => {
                return Err(InputError::UnknownKey {
                    name: name.to_owned(),
                });
            }
        };
        Ok(key)
    }
}

/// A sequence of keys pressed in order, e.g. `Ctrl+K Enter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySequence {
    /// Keys in press order.
    pub keys: Vec<Key>,
}

impl FromStr for KeySequence {
    type Err = InputError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.split(|character: char| character == '+' || character.is_ascii_whitespace())
            .filter(|part| !part.is_empty())
            .map(Key::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map(|keys| Self { keys })
    }
}

/// Injects key sequences into the active window.
#[derive(Debug, Clone, Copy)]
pub struct KeyInjector;

impl KeyInjector {
    /// Sends a key down followed by a key up for every key in `sequence`.
    ///
    /// # Errors
    ///
    /// Returns an error only when building the sequence fails; Windows key
    /// injection itself has no return value.
    ///
    /// # Safety contract
    ///
    /// The unsafe call below passes initialized virtual-key bytes and no scan
    /// code to the Win32 API. The public API takes typed [`Key`] values only,
    /// so callers cannot supply arbitrary scan codes or flags.
    pub fn press(&self, sequence: &KeySequence) -> Result<(), InputError> {
        for key in &sequence.keys {
            // SAFETY: `vk` is a typed virtual-key byte and the scan code and
            // extra info are zero, which Win32 accepts for synthesized keys.
            unsafe {
                keybd_event(key.vk(), 0, KEYBD_EVENT_FLAGS::default(), 0);
                keybd_event(key.vk(), 0, KEYEVENTF_KEYUP, 0);
            }
        }
        Ok(())
    }
}
