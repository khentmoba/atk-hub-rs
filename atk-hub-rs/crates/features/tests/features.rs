//! Tests for the feature layer behind the GUI pages.

use atk_features::{Lighting, Macro, MacroStep, UpdateInfo, version_newer};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

const LATEST: &str = "version: 3.2.33\nfiles:\n  - url: https://example.installer\n    sha512: digest\n    size: 123\npath: https://example.installer\nsha512: digest\nsize: 123\n";

#[test]
fn parses_latest_yml_and_compares_versions() {
    let info = UpdateInfo::parse(LATEST).expect("latest.yml");
    assert_eq!(info.version, "3.2.33");
    assert_eq!(info.url, "https://example.installer");
    assert_eq!(info.size, 123);
    assert!(version_newer("3.2.31", "3.2.33"));
    assert!(!version_newer("3.2.33", "3.2.31"));
    assert!(!version_newer("3.2.33", "3.2.33"));
    assert!(UpdateInfo::parse("version: 1").is_err());
}

#[test]
fn lighting_report_has_expected_byte_layout() {
    let lighting = Lighting {
        keymap: "a87.json".to_owned(),
        red: 1,
        green: 2,
        blue: 3,
        brightness: 4,
    };
    assert_eq!(lighting.report(), vec![0, 170, 1, 1, 2, 3, 4]);
}

#[test]
fn macros_round_trip_through_disk() {
    let dir = std::env::temp_dir().join(format!("atk-macros-{}", std::process::id()));
    let path = dir.join("macros.json");
    let macros = vec![Macro {
        name: "combo".to_owned(),
        steps: vec![MacroStep::press("Ctrl+K", 40)],
        repeat_mode: atk_features::RepeatMode::Count,
        repeat_count: 2,
    }];
    atk_features::save_macros(&path, &macros).expect("save");
    assert_eq!(atk_features::load_macros(&path).expect("load"), macros);
    let _ = fs::remove_file(path);
    let _ = fs::remove_dir(dir);
}

#[test]
fn pet_settings_toggle_a_boolean_key() {
    let mut config =
        atk_config::Config::parse(&json!({"woodenFish":{"enabled":false}}).to_string())
            .expect("settings");
    assert!(!atk_features::pet_enabled(&config, "woodenFish.enabled"));
    atk_features::set_pet_enabled(&mut config, "woodenFish.enabled", true).expect("set");
    assert!(atk_features::pet_enabled(&config, "woodenFish.enabled"));
}

#[test]
fn plugin_bundle_install_verifies_sha256_and_extracts_safely() {
    use std::io::{Cursor, Write};
    let mut builder = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    builder.start_file("index.js", options).expect("start file");
    builder
        .write_all(b"console.log('plugin')")
        .expect("write file");
    let bytes = builder.finish().expect("finish");
    let bytes = bytes.into_inner();
    let digest = atk_features::sha256_hex(&bytes);

    let root = std::env::temp_dir().join(format!("atk-plugin-test-{}", std::process::id()));
    let installed = atk_features::install_bundle_bytes("demo", "1.0", &bytes, &digest, &root)
        .expect("install verified bundle");
    assert_eq!(
        std::fs::read_to_string(installed.join("index.js")).expect("installed file"),
        "console.log('plugin')"
    );
    assert!(atk_features::install_bundle_bytes("bad", "1.0", &bytes, "deadbeef", &root).is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rejects_unsafe_zip_paths() {
    use std::io::{Cursor, Write};
    let mut builder = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    builder
        .start_file("../escape.js", options)
        .expect("start file");
    builder.write_all(b"bad").expect("write");
    let bytes = builder.finish().expect("finish").into_inner();
    let root = std::env::temp_dir().join(format!("atk-plugin-unsafe-{}", std::process::id()));
    let digest = atk_features::sha256_hex(&bytes);
    assert!(atk_features::install_bundle_bytes("x", "1", &bytes, &digest, &root).is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn plugin_listing_is_empty_for_a_missing_directory() {
    let missing = PathBuf::from("C:\\definitely-not-an-atk-plugin-directory");
    assert!(
        atk_features::list_plugins(&missing)
            .expect("missing directory")
            .is_empty()
    );
}

#[test]
fn pet_settings_read_progress_keys() {
    let config = atk_config::Config::parse(
        r#"{"progress":{"enabled":true,"petVisible":false,"sound":true,"skin":"gold","period":"day","days":{"a":1}}}"#,
    )
    .unwrap();
    let pet = atk_features::PetSettings::from_config(&config);
    assert!(pet.enabled && pet.sound && !pet.pet_visible);
    assert_eq!(pet.skin, "gold");
}

#[test]
fn hub_settings_round_trip_under_setting() {
    let mut config =
        atk_config::Config::parse(r#"{"setting":{"closeMinimizeTray":false}}"#).unwrap();
    let mut settings = atk_features::HubSettings::from_config(&config);
    assert!(!settings.close_minimize_tray);
    settings.open_at_login = true;
    settings.apply(&mut config).unwrap();
    assert_eq!(
        config.get("setting.openAtLogin"),
        Some(&serde_json::json!(true))
    );
}

#[test]
fn sha512_base64_matches_known_digest() {
    assert_eq!(
        atk_features::sha512_base64(b"abc"),
        "3a81oZNherrMQXNJriBBMRLm+k6JqX6iCp7u5ktV05ohkpkqJ0/BqDa6PCOj/uu9RU1EI2Q86A4qmslPpUyknw=="
    );
}

#[test]
fn game_rule_matching_ignores_case() {
    let rules = vec![atk_features::GameRule {
        exe: "Game.exe".into(),
        profile: 2,
        keyboard_profile: None,
    }];
    assert_eq!(
        atk_features::match_game_rule(&rules, "game.EXE").map(|rule| rule.profile),
        Some(2)
    );
    assert_eq!(
        atk_features::url_file_name("https://x/y/a%20b.exe?x=1"),
        "a%20b.exe"
    );
}

#[test]
fn plugins_report_renderers_and_can_be_removed() {
    let root = std::env::temp_dir().join(format!("atk-plugins-{}", std::process::id()));
    let dir = root.join("plugins").join("pc-script").join("1.0.0");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("pc-script.js"), "export default {}").unwrap();
    let plugins = atk_features::list_plugins(&root).unwrap();
    assert!(plugins[0].pc_script && !plugins[0].floating);
    assert!(atk_features::remove_plugin(&root, "../evil").is_err());
    atk_features::remove_plugin(&root, "pc-script").unwrap();
    assert!(atk_features::list_plugins(&root).unwrap().is_empty());
    let _ = fs::remove_dir_all(root);
}
