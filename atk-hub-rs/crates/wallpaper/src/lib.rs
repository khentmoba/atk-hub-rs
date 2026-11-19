//! Client for the ATK HUB animated-wallpaper catalogue.

use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use thiserror::Error;

/// A catalogue scalar that may be serialized as either a JSON string or number.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct FlexibleText(String);

impl FlexibleText {
    /// Returns the scalar as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FlexibleText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for FlexibleText {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let text = match &value {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Number(number) => number.to_string(),
            _ => {
                return Err(serde::de::Error::custom(
                    "catalogue scalar is not a string or number",
                ));
            }
        };
        Ok(Self(text))
    }
}

/// One entry of `list-v2.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wallpaper {
    /// Catalogue id, serialized as a string or number.
    pub id: FlexibleText,
    /// Internal asset name.
    pub value: String,
    /// Display title.
    pub title: String,
    /// Sort order from the catalogue.
    #[serde(rename = "sortOrder", default)]
    pub sort_order: i32,
    /// Preview image URL.
    #[serde(rename = "previewUrl", default)]
    pub preview_url: String,
    /// Alternate preview URL used by the original UI.
    #[serde(rename = "urlPath", default)]
    pub url_path: String,
    /// Downloadable bundle URL.
    #[serde(rename = "downloadUrl", default)]
    pub download_url: String,
    /// Asset version, serialized as a string or number.
    #[serde(default)]
    pub version: FlexibleText,
}

/// Parsed wallpaper catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    /// Catalogue entries in server order.
    pub wallpaper: Vec<Wallpaper>,
}

impl Catalog {
    /// Parses a catalogue from JSON text.
    ///
    /// # Errors
    ///
    /// Returns an error when the text is not a valid catalogue document.
    pub fn parse(text: &str) -> Result<Self, WallpaperError> {
        Ok(serde_json::from_str(text)?)
    }

    /// Returns entries whose value or title contains `needle`.
    #[must_use]
    pub fn search(&self, needle: &str) -> Vec<&Wallpaper> {
        if needle.is_empty() {
            return self.wallpaper.iter().collect();
        }
        self.wallpaper
            .iter()
            .filter(|entry| entry.value.contains(needle) || entry.title.contains(needle))
            .collect()
    }
}

/// Failures while loading the wallpaper catalogue.
#[derive(Debug, Error)]
pub enum WallpaperError {
    /// The catalogue JSON was malformed.
    #[error("wallpaper catalogue JSON is invalid")]
    Json(#[from] serde_json::Error),
    /// The catalogue could not be downloaded.
    #[error("wallpaper catalogue request failed")]
    Request(#[from] Box<ureq::Error>),
    /// The response body could not be read.
    #[error("wallpaper catalogue response could not be read")]
    Io(#[from] std::io::Error),
}

/// Downloads and parses the live catalogue.
///
/// # Errors
///
/// Returns an error for a transport failure, non-success status, unreadable
/// body, or malformed JSON.
pub fn fetch_catalog() -> Result<Catalog, WallpaperError> {
    let response = ureq::get(atk_endpoints::wallpaper_catalog_url())
        .call()
        .map_err(|error| WallpaperError::Request(Box::new(error)))?;
    let text = response.into_string()?;
    Catalog::parse(&text)
}
