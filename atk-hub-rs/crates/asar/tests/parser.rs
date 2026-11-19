//! Characterization tests for the ASAR manifest parser.

use atk_asar::{AsarError, Manifest, Node};

fn archive(manifest: &str) -> Vec<u8> {
    let json_len = manifest.len();
    let header_size = 8 + json_len + 1;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&(header_size as u32).to_le_bytes());
    bytes.extend_from_slice(&((header_size - 4) as u32).to_le_bytes());
    bytes.extend_from_slice(&(json_len as u32).to_le_bytes());
    bytes.extend_from_slice(manifest.as_bytes());
    bytes.push(0);
    bytes
}

const MANIFEST: &str = r#"{"files":{"dist":{"files":{"index.js":{"size":"120","offset":"0","executable":true},"empty.js":{"size":0,"offset":"120"}}},"node_modules":{"files":{"helper.exe":{"size":22,"offset":"120","unpacked":true}}},"package.json":{"size":64,"offset":"142"}}}"#;

#[test]
fn parses_manifest_order_and_data_offset() {
    let bytes = archive(MANIFEST);
    let manifest = Manifest::parse(&bytes).expect("valid archive");

    assert_eq!(manifest.data_offset, (16 + MANIFEST.len() + 1) as u64);
    let names: Vec<&str> = manifest
        .files
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(
        names,
        ["dist", "node_modules", "package.json"],
        "manifest order is observable"
    );

    let dist = manifest.files[0]
        .1
        .as_directory()
        .expect("dist is a directory");
    let child_names: Vec<&str> = dist.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(child_names, ["index.js", "empty.js"]);
}

#[test]
fn reads_file_flags_and_numeric_or_string_fields() {
    let manifest = Manifest::parse(&archive(MANIFEST)).expect("valid archive");
    let entry = manifest.lookup("dist/index.js").expect("file exists");
    assert_eq!(entry.size, 120);
    assert_eq!(entry.offset, 0);
    assert!(entry.executable);
    assert!(!entry.unpacked);

    let unpacked = manifest
        .lookup("node_modules/helper.exe")
        .expect("file exists");
    assert!(unpacked.unpacked);
}

#[test]
fn lookup_handles_prefix_separators_and_missing_paths() {
    let manifest = Manifest::parse(&archive(MANIFEST)).expect("valid archive");
    assert!(manifest.lookup("./dist/index.js").is_some());
    assert!(manifest.lookup("/dist/index.js").is_some());
    assert!(
        manifest.lookup("dist").is_none(),
        "directories are not files"
    );
    assert!(manifest.lookup("dist/missing.js").is_none());
}

#[test]
fn counts_files_and_returns_depth_first_paths() {
    let manifest = Manifest::parse(&archive(MANIFEST)).expect("valid archive");
    assert_eq!(manifest.file_count(), 4);
    assert_eq!(
        manifest.paths(),
        [
            "dist/index.js",
            "dist/empty.js",
            "node_modules/helper.exe",
            "package.json",
        ]
    );
}

#[test]
fn rejects_truncated_header() {
    let err = Manifest::parse(&[0, 1, 2]).expect_err("short input");
    assert!(matches!(err, AsarError::TruncatedHeader { len: 3 }));
}

#[test]
fn rejects_header_beyond_input() {
    let mut bytes = archive(MANIFEST);
    bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        Manifest::parse(&bytes),
        Err(AsarError::HeaderOutOfBounds { .. })
    ));
}

#[test]
fn rejects_json_beyond_data_offset() {
    let mut bytes = archive(MANIFEST);
    bytes[12..16].copy_from_slice(&(MANIFEST.len() as u32 + 50).to_le_bytes());
    assert!(matches!(
        Manifest::parse(&bytes),
        Err(AsarError::JsonOutOfBounds { .. })
    ));
}

#[test]
fn rejects_missing_files_and_invalid_nodes() {
    let no_files = archive(r#"{"header":{"size":1}}"#);
    assert!(matches!(
        Manifest::parse(&no_files),
        Err(AsarError::MissingFiles)
    ));

    let bad_node = archive(r#"{"files":{"weird":"not-an-object"}}"#);
    assert!(matches!(
        Manifest::parse(&bad_node),
        Err(AsarError::Json(_))
    ));

    let files_not_object = archive(r#"{"files":[]}"#);
    assert!(matches!(
        Manifest::parse(&files_not_object),
        Err(AsarError::InvalidFiles)
    ));
}

#[test]
fn unpacked_entries_without_size_or_offset_are_accepted() {
    let manifest_json =
        r#"{"files":{"helper.exe":{"unpacked":true,"integrity":{"algorithm":"SHA256"}}}}"#;
    let manifest = Manifest::parse(&archive(manifest_json)).expect("real-world unpacked entry");
    let entry = manifest.lookup("helper.exe").expect("file exists");
    assert_eq!(entry.size, 0);
    assert_eq!(entry.offset, 0);
    assert!(entry.unpacked);
}

#[test]
fn node_accessors_distinguish_files_from_directories() {
    let manifest = Manifest::parse(&archive(MANIFEST)).expect("valid archive");
    assert!(matches!(manifest.files[0].1, Node::Directory(_)));
    assert!(matches!(manifest.files[2].1, Node::File(_)));
    assert!(manifest.files[0].1.as_file().is_none());
    assert!(manifest.files[2].1.as_directory().is_none());
}
