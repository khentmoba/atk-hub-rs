//! Literal Electron IPC channels and Rust-agent methods used by ATK HUB.
//!
//! The registry is built from static analysis of the shipped main bundle. It
//! intentionally excludes dynamically named handlers: the original registers
//! a few channels through variables, which cannot be recovered statically.

use std::fmt;

mod channels;

pub use channels::{AGENT_METHODS, all, by_category, category, find, is_agent_method};

/// A literal IPC channel name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Channel(&'static str);

impl Channel {
    /// Creates a channel from a static literal.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Returns the channel name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Coarse feature area of an IPC channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    /// HID, pointer, audio and firmware devices.
    Device,
    /// Desktop pets and overlay widgets.
    Widget,
    /// Floating selection, plugins and Lua scripts.
    Automation,
    /// Animated wallpapers.
    Wallpaper,
    /// Game detection and automatic switching.
    Game,
    /// Agent bridge and diagnostics.
    Agent,
    /// Windows, settings, updates and tray.
    System,
}

impl fmt::Display for Category {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Device => "device",
            Self::Widget => "widget",
            Self::Automation => "automation",
            Self::Wallpaper => "wallpaper",
            Self::Game => "game",
            Self::Agent => "agent",
            Self::System => "system",
        };
        formatter.write_str(name)
    }
}
