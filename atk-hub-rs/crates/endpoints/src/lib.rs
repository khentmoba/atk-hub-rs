//! URL templates recovered from the ATK HUB 3.2.31 main process.

/// Cloud firmware API root.
pub const FIRMWARE_BASE: &str = "https://api-hub-cloud.atk.pro/firmware";
/// Wallpaper catalogue used by the wallpaper manager.
pub const WALLPAPER_CATALOG: &str = "https://wallpaper.atkgear.com/list-v2.json";
/// Fallback download page shown on updater failure.
pub const OFFICIAL_SITE: &str = "https://desktop.vxe.com/v3hub";
/// Loopback shutdown endpoint of the legacy music service.
pub const MUSIC_SERVICE_EXIT: &str = "http://127.0.0.1:5577/exit";
/// electron-updater feed configured in `app-update.yml`.
pub const UPDATER_FEED: &str = "https://bpcdn.atkgear.com/device/update-v3/win32";

/// Selects which background service metadata endpoint to query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FirmwareService {
    /// BYKB lighting/music service (`0/2`).
    Bykb,
    /// Game-power service (`0/0`).
    GamePower,
}

impl FirmwareService {
    /// Returns the two numeric path segments used by the cloud API.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Bykb => "0/2",
            Self::GamePower => "0/0",
        }
    }
}

/// Builds the metadata URL for a background service.
#[must_use]
pub fn firmware_service_url(service: FirmwareService) -> String {
    format!("{FIRMWARE_BASE}/{}/service", service.path())
}

/// Builds a device firmware or audio-driver URL.
///
/// Both the model-pack and THX audio call sites use this exact template with
/// vendor id, product id and a device tag.
#[must_use]
pub fn firmware_url(vendor_id: u16, product_id: u16, tag: &str) -> String {
    format!("{FIRMWARE_BASE}/{vendor_id}/{product_id}/{tag}")
}

/// Builds the updater version-check URL for a host and platform.
#[must_use]
pub fn device_update_version_url(host: &str, platform: &str) -> String {
    format!("https://{host}/v1/device/update/{platform}/version")
}

/// Returns the wallpaper catalogue URL.
#[must_use]
pub const fn wallpaper_catalog_url() -> &'static str {
    WALLPAPER_CATALOG
}

/// Returns the official download page.
#[must_use]
pub const fn official_site_url() -> &'static str {
    OFFICIAL_SITE
}

/// Returns the loopback music-service shutdown URL.
#[must_use]
pub const fn music_service_exit_url() -> &'static str {
    MUSIC_SERVICE_EXIT
}

/// Returns the configured electron-updater feed.
#[must_use]
pub const fn updater_feed_url() -> &'static str {
    UPDATER_FEED
}
