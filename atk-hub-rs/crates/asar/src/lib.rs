//! Typed, execution-free reader for Electron ASAR package manifests.
//!
//! The layout follows the manifest used by ATK HUB 3.2.31: a 16-byte prefix,
//! a JSON manifest, padding, then the concatenated file payloads.

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::str;
use thiserror::Error;

/// Length of the ASAR prefix before the manifest JSON.
const PREFIX_LEN: usize = 16;
/// Offset of the little-endian header-size field in the prefix.
const HEADER_SIZE_OFFSET: usize = 4;
/// Offset of the little-endian JSON-length field in the prefix.
const JSON_LEN_OFFSET: usize = 12;
/// Offset of the header-size field relative to the data start.
const DATA_OFFSET_BASE: usize = 8;

/// Failures while reading an ASAR manifest.
#[derive(Debug, Error)]
pub enum AsarError {
    /// The input is shorter than the ASAR prefix.
    #[error("ASAR header is truncated: {len} bytes")]
    TruncatedHeader {
        /// Number of bytes available.
        len: usize,
    },
    /// The declared header size does not fit in the input.
    #[error("ASAR header size {header_size} exceeds input length {len}")]
    HeaderOutOfBounds {
        /// Declared header size.
        header_size: usize,
        /// Input length.
        len: usize,
    },
    /// The declared JSON length runs past the header.
    #[error("ASAR JSON ends at {json_end}, past data offset {data_offset}")]
    JsonOutOfBounds {
        /// End of the JSON slice.
        json_end: usize,
        /// Start of the file payloads.
        data_offset: usize,
    },
    /// The manifest JSON is not UTF-8.
    #[error("ASAR manifest is not valid UTF-8")]
    InvalidUtf8(#[from] str::Utf8Error),
    /// The manifest JSON is malformed.
    #[error("ASAR manifest JSON is invalid")]
    Json(#[from] serde_json::Error),
    /// The manifest has no `files` map.
    #[error("ASAR manifest has no files map")]
    MissingFiles,
    /// The `files` member is not an object.
    #[error("ASAR manifest files member is not an object")]
    InvalidFiles,
    /// A manifest node is neither a directory nor a file.
    #[error("ASAR node is not an object")]
    InvalidNode,
}

/// A file entry inside an ASAR manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileEntry {
    /// Declared payload size in bytes.
    pub size: u64,
    /// Offset of the payload relative to [`Manifest::data_offset`].
    pub offset: u64,
    /// Whether the payload lives next to the archive in `*.asar.unpacked`.
    pub unpacked: bool,
    /// Whether the file carries the executable flag.
    pub executable: bool,
}

/// An entry in the ASAR tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Node {
    /// A directory, with children in manifest order.
    Directory(Vec<(String, Node)>),
    /// A regular file.
    File(FileEntry),
}

impl Node {
    /// Returns the file entry when this node is a file.
    #[must_use]
    pub fn as_file(&self) -> Option<&FileEntry> {
        match self {
            Self::File(entry) => Some(entry),
            Self::Directory(_) => None,
        }
    }

    /// Returns the children when this node is a directory.
    #[must_use]
    pub fn as_directory(&self) -> Option<&[(String, Node)]> {
        match self {
            Self::Directory(children) => Some(children),
            Self::File(_) => None,
        }
    }
}

/// A parsed ASAR manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Top-level entries in manifest order.
    pub files: Vec<(String, Node)>,
    /// File offset where payload bytes begin.
    pub data_offset: u64,
}

impl Manifest {
    /// Parses a complete ASAR archive header and manifest from `bytes`.
    ///
    /// # Errors
    ///
    /// Returns an error for a truncated prefix, an out-of-bounds header or
    /// JSON region, invalid UTF-8, malformed JSON, or a missing `files` map.
    pub fn parse(bytes: &[u8]) -> Result<Self, AsarError> {
        let len = bytes.len();
        if len < PREFIX_LEN {
            return Err(AsarError::TruncatedHeader { len });
        }

        let header_size = read_u32(bytes, HEADER_SIZE_OFFSET) as usize;
        let json_len = read_u32(bytes, JSON_LEN_OFFSET) as usize;
        let data_offset = DATA_OFFSET_BASE
            .checked_add(header_size)
            .ok_or(AsarError::HeaderOutOfBounds { header_size, len })?;
        if data_offset > len {
            return Err(AsarError::HeaderOutOfBounds { header_size, len });
        }

        let json_end = PREFIX_LEN
            .checked_add(json_len)
            .ok_or(AsarError::JsonOutOfBounds {
                json_end: usize::MAX,
                data_offset,
            })?;
        if json_end > data_offset {
            return Err(AsarError::JsonOutOfBounds {
                json_end,
                data_offset,
            });
        }

        let json = str::from_utf8(&bytes[PREFIX_LEN..json_end])?;
        let value: serde_json::Value = serde_json::from_str(json)?;
        let files_value = value.get("files").ok_or(AsarError::MissingFiles)?;
        let map = files_value.as_object().ok_or(AsarError::InvalidFiles)?;

        let mut files = Vec::with_capacity(map.len());
        for (name, child) in map {
            let node: Node = serde_json::from_value(child.clone())?;
            files.push((name.clone(), node));
        }

        Ok(Self {
            files,
            data_offset: data_offset as u64,
        })
    }

    /// Looks up a file by slash-separated path.
    #[must_use]
    pub fn lookup(&self, path: &str) -> Option<&FileEntry> {
        let mut parts = path
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .peekable();
        let mut nodes: &[(String, Node)] = &self.files;

        while let Some(part) = parts.next() {
            let found = nodes
                .iter()
                .find(|(name, _)| name == part)
                .map(|(_, node)| node)?;
            if parts.peek().is_none() {
                return found.as_file();
            }
            nodes = found.as_directory()?;
        }

        None
    }

    /// Counts file nodes in the whole tree.
    #[must_use]
    pub fn file_count(&self) -> usize {
        fn count(nodes: &[(String, Node)]) -> usize {
            nodes
                .iter()
                .map(|(_, node)| match node {
                    Node::File(_) => 1,
                    Node::Directory(children) => count(children),
                })
                .sum()
        }
        count(&self.files)
    }

    /// Returns every file path in depth-first manifest order.
    #[must_use]
    pub fn paths(&self) -> Vec<String> {
        fn walk(prefix: &str, nodes: &[(String, Node)], out: &mut Vec<String>) {
            for (name, node) in nodes {
                let path = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{prefix}/{name}")
                };
                match node {
                    Node::File(_) => out.push(path),
                    Node::Directory(children) => walk(&path, children, out),
                }
            }
        }

        let mut out = Vec::new();
        walk("", &self.files, &mut out);
        out
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let raw: [u8; 4] = bytes[offset..offset + 4]
        .try_into()
        .expect("prefix field is exactly four bytes");
    u32::from_le_bytes(raw)
}

fn number_field(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<u64> {
    let value = map.get(key)?;
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let map = value
            .as_object()
            .ok_or_else(|| D::Error::custom("ASAR node is not an object"))?;

        if let Some(children) = map.get("files") {
            let children = children
                .as_object()
                .ok_or_else(|| D::Error::custom("ASAR files member is not an object"))?;
            let mut entries = Vec::with_capacity(children.len());
            for (name, child) in children {
                let node: Self = serde_json::from_value(child.clone()).map_err(D::Error::custom)?;
                entries.push((name.clone(), node));
            }
            return Ok(Self::Directory(entries));
        }

        let unpacked = map
            .get("unpacked")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        // Unpacked entries have no payload inside the archive, so electron's
        // packer omits both size and offset for them.
        let size = match number_field(map, "size") {
            Some(value) => value,
            None if unpacked => 0,
            None => return Err(D::Error::custom("ASAR file has no valid size")),
        };
        let offset = match number_field(map, "offset") {
            Some(value) => value,
            None if unpacked => 0,
            None => return Err(D::Error::custom("ASAR file has no valid offset")),
        };

        Ok(Self::File(FileEntry {
            size,
            offset,
            unpacked,
            executable: map
                .get("executable")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        }))
    }
}
