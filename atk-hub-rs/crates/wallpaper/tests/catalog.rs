//! Tests for the wallpaper catalogue parser.

use atk_wallpaper::Catalog;

const SAMPLE: &str = r#"{"wallpaper":[{"id":5000,"value":"sonic-terrain","title":"Sonic","sortOrder":-1,"previewUrl":"https://wallpaper.atkgear.com/preview/sonic.png","urlPath":"https://wallpaper.atkgear.com/preview/sonic.png","downloadUrl":"https://wallpaper.atkgear.com/sonic.zip","version":"0"}]}"#;

#[test]
fn parses_the_observed_catalogue_shape() {
    let catalog = Catalog::parse(SAMPLE).expect("valid catalogue");
    assert_eq!(catalog.wallpaper.len(), 1);
    let entry = &catalog.wallpaper[0];
    assert_eq!(entry.id.as_str(), "5000");
    assert_eq!(entry.value, "sonic-terrain");
    assert_eq!(entry.sort_order, -1);
    assert!(entry.download_url.ends_with(".zip"));
}

#[test]
fn search_matches_value_and_title_and_handles_empty_needles() {
    let catalog = Catalog::parse(SAMPLE).expect("valid catalogue");
    assert_eq!(catalog.search("sonic").len(), 1);
    assert_eq!(catalog.search("Sonic").len(), 1);
    assert_eq!(catalog.search("").len(), 1);
    assert!(catalog.search("missing").is_empty());
}

#[test]
fn accepts_numeric_versions_and_string_ids_from_live_data() {
    let json = r#"{"wallpaper":[{"id":"DUCKBILL","value":"duck","title":"Duck","version":1,"sortOrder":2}]}"#;
    let catalog = Catalog::parse(json).expect("live-shaped catalogue");
    assert_eq!(catalog.wallpaper[0].id.as_str(), "DUCKBILL");
    assert_eq!(catalog.wallpaper[0].version.as_str(), "1");
}

#[test]
fn rejects_malformed_catalogues() {
    assert!(Catalog::parse("{}").is_err());
    assert!(Catalog::parse("not json").is_err());
}
