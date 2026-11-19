//! Tests for the recovered remote URL templates.

use atk_endpoints::{
    FirmwareService, device_update_version_url, firmware_service_url, firmware_url,
    music_service_exit_url, official_site_url, updater_feed_url, wallpaper_catalog_url,
};

#[test]
fn service_metadata_urls_match_observed_literals() {
    assert_eq!(
        firmware_service_url(FirmwareService::Bykb),
        "https://api-hub-cloud.atk.pro/firmware/0/2/service"
    );
    assert_eq!(
        firmware_service_url(FirmwareService::GamePower),
        "https://api-hub-cloud.atk.pro/firmware/0/0/service"
    );
}

#[test]
fn firmware_urls_interpolate_decimal_ids() {
    assert_eq!(
        firmware_url(0x1234, 0x5678, "service"),
        "https://api-hub-cloud.atk.pro/firmware/4660/22136/service"
    );
    assert_eq!(
        firmware_url(1, 2, "MODEL_TAG"),
        "https://api-hub-cloud.atk.pro/firmware/1/2/MODEL_TAG"
    );
}

#[test]
fn updater_and_static_urls_are_exact() {
    assert_eq!(
        wallpaper_catalog_url(),
        "https://wallpaper.atkgear.com/list-v2.json"
    );
    assert_eq!(official_site_url(), "https://desktop.vxe.com/v3hub");
    assert_eq!(music_service_exit_url(), "http://127.0.0.1:5577/exit");
    assert_eq!(
        updater_feed_url(),
        "https://bpcdn.atkgear.com/device/update-v3/win32"
    );
    assert_eq!(
        device_update_version_url("update.host", "win32"),
        "https://update.host/v1/device/update/win32/version"
    );
}
