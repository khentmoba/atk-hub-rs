//! Tests for electron-store compatible config behavior.

use atk_config::{Config, ConfigError};
use serde_json::json;

const SAMPLE: &str = "{\n\t\"USERINFO\": {\n\t\t\"userId\": \"84b6697c-19cc-4381-b8ec-9190203f7450\"\n\t},\n\t\"atk_service_url\": \"https://bpcdn.atkgear.com/old.zip\",\n\t\"hub\": {\n\t\t\"viteElectronVersion\": {\n\t\t\t\"lastAcknowledged\": \"26\"\n\t\t}\n\t}\n}";

#[test]
fn reads_typed_keys_with_dot_notation() {
    let config = Config::parse(SAMPLE).expect("valid config");
    assert_eq!(
        config.user_id(),
        Some("84b6697c-19cc-4381-b8ec-9190203f7450")
    );
    assert_eq!(
        config.service_url(),
        Some("https://bpcdn.atkgear.com/old.zip")
    );
    assert_eq!(config.acknowledged_ui_version(), Some("26"));
    assert!(config.get_str("USERINFO.missing").is_none());
}

#[test]
fn set_service_url_updates_only_that_key() {
    let mut config = Config::parse(SAMPLE).expect("valid config");
    config
        .set_service_url("https://bpcdn.atkgear.com/new.zip")
        .expect("set succeeds");
    assert_eq!(
        config.service_url(),
        Some("https://bpcdn.atkgear.com/new.zip")
    );
    assert_eq!(
        config.user_id(),
        Some("84b6697c-19cc-4381-b8ec-9190203f7450")
    );
}

#[test]
fn serialization_matches_electron_store_formatting() {
    let mut config = Config::parse(SAMPLE).expect("valid config");
    config
        .set_service_url("https://bpcdn.atkgear.com/new.zip")
        .expect("set succeeds");
    let expected = SAMPLE.replace(
        "https://bpcdn.atkgear.com/old.zip",
        "https://bpcdn.atkgear.com/new.zip",
    );
    assert_eq!(config.to_json().expect("serialize"), expected);
}

#[test]
fn creates_missing_intermediate_objects() {
    let mut config = Config::new();
    config
        .set("hub.viteElectronVersion.lastAcknowledged", json!("81"))
        .expect("set succeeds");
    assert_eq!(config.acknowledged_ui_version(), Some("81"));
}

#[test]
fn rejects_bad_keys_and_non_object_segments() {
    let mut config = Config::parse(SAMPLE).expect("valid config");
    assert!(matches!(
        config.set("", json!(1)),
        Err(ConfigError::EmptyKey)
    ));
    assert!(matches!(
        config.set("hub..x", json!(1)),
        Err(ConfigError::EmptyKey)
    ));
    assert!(matches!(
        config.set("USERINFO.userId.nested", json!(1)),
        Err(ConfigError::NonObjectSegment { ref key }) if key == "USERINFO.userId.nested"
    ));
}

#[test]
fn round_trip_preserves_unknown_fields() {
    let source = "{\n\t\"extra\": [1, 2, 3],\n\t\"USERINFO\": {\"userId\": \"u\"}\n}";
    let config = Config::parse(source).expect("valid config");
    assert_eq!(config.get("extra"), Some(&json!([1, 2, 3])));
    assert!(Config::parse(&config.to_json().expect("serialize")).is_ok());
}

#[test]
fn rejects_malformed_json() {
    assert!(matches!(Config::parse("{nope"), Err(ConfigError::Json(_))));
}
