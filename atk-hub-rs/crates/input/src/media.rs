//! Windows System Media Transport Controls (SMTC) reader for live wallpapers
//! (`wallpaperRegisterMedia*Listener`).

use serde::Serialize;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};

/// Current media playback snapshot from Windows SMTC.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSnapshot {
    /// Track title.
    pub title: String,
    /// Artist name.
    pub artist: String,
    /// Album title.
    pub album: String,
    /// Playback state (`"playing"`, `"paused"`, `"stopped"`).
    pub state: String,
    /// Timeline position in seconds.
    pub position: f64,
    /// Track duration in seconds.
    pub duration: f64,
}

/// Reads the current Windows SMTC media session, if any player is active.
#[must_use]
pub fn current_media() -> Option<MediaSnapshot> {
    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .ok()?
        .get()
        .ok()?;
    let session = manager.GetCurrentSession().ok()?;
    let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
    let title = props.Title().ok()?.to_string_lossy();
    let artist = props
        .Artist()
        .ok()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let album = props
        .AlbumTitle()
        .ok()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();

    let status = session
        .GetPlaybackInfo()
        .ok()
        .and_then(|info| info.PlaybackStatus().ok())
        .unwrap_or(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped);
    let state = match status {
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => "playing",
        GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => "paused",
        _ => "stopped",
    }
    .to_owned();

    let (position, duration) = session
        .GetTimelineProperties()
        .ok()
        .map(|tl| {
            let pos = tl
                .Position()
                .map(|t| t.Duration as f64 / 10_000_000.0)
                .unwrap_or(0.0);
            let dur = tl
                .EndTime()
                .map(|t| t.Duration as f64 / 10_000_000.0)
                .unwrap_or(0.0);
            (pos.max(0.0), dur.max(0.0))
        })
        .unwrap_or((0.0, 0.0));

    Some(MediaSnapshot {
        title,
        artist,
        album,
        state,
        position,
        duration,
    })
}
