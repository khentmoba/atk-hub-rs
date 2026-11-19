//! Tests for the recovered IPC channel inventory.

use atk_ipc::{AGENT_METHODS, Category, all, by_category, category, find, is_agent_method};
use std::collections::BTreeSet;

#[test]
fn registry_has_all_observed_literal_channels_in_order() {
    assert_eq!(all().len(), 169);
    assert_eq!(all()[0].as_str(), "dotScreen:status");
    assert_eq!(all()[168].as_str(), "quit-app");
    assert_eq!(
        find("plugin-install-bundle").map(|channel| channel.as_str()),
        Some("plugin-install-bundle")
    );
    assert!(find("not-a-channel").is_none());
}

#[test]
fn channels_are_unique() {
    let unique: BTreeSet<&str> = all().iter().map(|channel| channel.as_str()).collect();
    assert_eq!(unique.len(), all().len(), "duplicate channel names");
}

#[test]
fn categories_cover_the_registry() {
    let groups = by_category();
    assert_eq!(groups.values().map(Vec::len).sum::<usize>(), 169);
    assert!(
        groups[&Category::Wallpaper]
            .iter()
            .any(|c| c.as_str() == "wallpaper:list")
    );
    assert!(
        groups[&Category::Widget]
            .iter()
            .any(|c| c.as_str() == "woodenFish:tap")
    );
    assert!(
        groups[&Category::Automation]
            .iter()
            .any(|c| c.as_str() == "pc-script:start")
    );
    assert!(
        groups[&Category::Device]
            .iter()
            .any(|c| c.as_str() == "scanDevice")
    );
    assert!(
        groups[&Category::Game]
            .iter()
            .any(|c| c.as_str() == "gameAuto_getStatus")
    );
    assert!(
        groups[&Category::Agent]
            .iter()
            .any(|c| c.as_str() == "atk-rust-agent:request")
    );
    assert_eq!(category("some-unknown-channel"), Category::System);
}

#[test]
fn agent_allowlist_is_exact() {
    assert_eq!(AGENT_METHODS.len(), 9);
    assert_eq!(AGENT_METHODS[0], "device.list");
    assert_eq!(AGENT_METHODS[8], "bindings.set");
    assert!(is_agent_method("device.write"));
    assert!(!is_agent_method("shell.execute"));
    assert!(!is_agent_method(""));
}
