//! TLS gRPC client for the THX `ThxAudioProperties` service.
//!
//! The connection file and RPC set mirror the shipped Electron implementation.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tonic::Request;
use tonic::transport::{Certificate, Channel, ClientTlsConfig};

/// Generated protobuf types and service client.
#[allow(missing_docs, clippy::all)]
pub mod proto {
    tonic::include_proto!("thx.proto");
}

use proto::thx_audio_properties_client::ThxAudioPropertiesClient;
use proto::{
    GetEndpointListRequest, GetPresetListRequest, GetPropertyListRequest, GetPropertyRequest,
    GetSelectedPresetRequest, Property, PropertyType, SelectPresetRequest, SetPropertyRequest,
};

/// Endpoint id used by the original hub for headphone effects.
pub const HEADPHONES_ENDPOINT: &str = "headphones";
/// Property controlling spatial audio.
pub const SPATIAL_ENABLED: &str = "spatial_enabled";
/// Property controlling effect rendering.
pub const RENDER_ENABLED: &str = "render_enabled";
/// Preset group used by the hub.
pub const PROFILE_PRESETS: &str = "profile_presets";
/// Default THX product identifier.
pub const DEFAULT_PRODUCT: &str = "atk_n9v2_ultimate";
/// Deadline applied to each RPC by the original client.
pub const RPC_TIMEOUT: Duration = Duration::from_secs(3);

/// Failures while reading connection data or calling the THX service.
#[derive(Debug, Error)]
pub enum AudioError {
    /// The connection file was not valid JSON.
    #[error("THX connection file JSON is invalid")]
    Json(#[from] serde_json::Error),
    /// The product id was not a safe path component.
    #[error("invalid THX product id: {product}")]
    InvalidProduct {
        /// Rejected product id.
        product: String,
    },
    /// The connection address was not a loopback host with a port.
    #[error("invalid THX address: {address}")]
    InvalidAddress {
        /// Rejected address.
        address: String,
    },
    /// The connection file had no TLS certificate.
    #[error("THX connection file has no TLS certificate")]
    MissingCertificate,
    /// The connection file could not be read.
    #[error("THX connection file could not be read")]
    Io(#[from] std::io::Error),
    /// The gRPC channel could not be built.
    #[error("THX transport configuration failed")]
    Transport(#[from] tonic::transport::Error),
    /// The loopback address was not a valid URI authority.
    #[error("invalid THX URI")]
    InvalidUri(#[from] tonic::codegen::http::uri::InvalidUri),
    /// The gRPC call failed.
    #[error("THX RPC failed: {0}")]
    Grpc(Box<tonic::Status>),
    /// The RPC did not finish within the deadline.
    #[error("THX RPC timed out")]
    Timeout,
}

impl From<tonic::Status> for AudioError {
    fn from(status: tonic::Status) -> Self {
        Self::Grpc(Box::new(status))
    }
}

/// Contents of `thx_properties.tcp`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointConfig {
    /// Loopback host and port of the local THX service.
    pub address: String,
    /// PEM-encoded server certificate.
    pub cert: String,
}

impl EndpointConfig {
    /// Parses a connection file.
    ///
    /// # Errors
    ///
    /// Returns an error when the text is not a valid connection document.
    pub fn parse(text: &str) -> Result<Self, AudioError> {
        Ok(serde_json::from_str(text)?)
    }

    /// Validates the loopback address and certificate, as the original does.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let mut parts = self.address.split(':');
        let host = parts.next().unwrap_or_default();
        let port = parts.next().unwrap_or_default();
        let host_ok = matches!(host, "localhost" | "127.0.0.1" | "[::1]");
        let port_ok = !port.is_empty()
            && parts.next().is_none()
            && port.bytes().all(|byte| byte.is_ascii_digit());
        host_ok && port_ok && !self.cert.is_empty()
    }
}

/// Returns the connection file path for a THX product id.
///
/// # Errors
///
/// Returns [`AudioError::InvalidProduct`] when `product` contains characters
/// that are unsafe as a directory name.
pub fn endpoint_file(product: &str) -> Result<PathBuf, AudioError> {
    let safe = !product.is_empty()
        && product.len() <= 64
        && product
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    if !safe {
        return Err(AudioError::InvalidProduct {
            product: product.to_owned(),
        });
    }
    let program_data =
        std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_owned());
    Ok(PathBuf::from(program_data)
        .join("thx")
        .join(product)
        .join("thx_properties.tcp"))
}

/// Loads and validates the connection file for a product.
///
/// # Errors
///
/// Returns an error for a bad product id, a missing file, malformed JSON, or
/// an invalid address/certificate.
pub fn load_endpoint_config(product: &str) -> Result<EndpointConfig, AudioError> {
    let path = endpoint_file(product)?;
    let text = std::fs::read_to_string(path)?;
    let config = EndpointConfig::parse(&text)?;
    if !config.is_valid() {
        if config.cert.is_empty() {
            return Err(AudioError::MissingCertificate);
        }
        return Err(AudioError::InvalidAddress {
            address: config.address.clone(),
        });
    }
    Ok(config)
}

/// A property value formatted for display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyView {
    /// Property id.
    pub id: String,
    /// Protobuf property type name.
    pub kind: String,
    /// Value rendered as text.
    pub value: String,
}

/// Aggregated THX status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThxStatus {
    /// Endpoint ids reported by the service.
    pub endpoints: Vec<String>,
    /// Spatial audio toggle, when available.
    pub spatial_enabled: Option<bool>,
    /// Effect render toggle, when available.
    pub render_enabled: Option<bool>,
    /// Known properties with their values.
    pub properties: Vec<PropertyView>,
    /// Presets in the hub's preset group.
    pub presets: Vec<String>,
    /// Currently selected preset.
    pub selected_preset: String,
    /// Service version string, when available.
    pub version: Option<String>,
}

/// Connected THX gRPC client.
#[derive(Debug, Clone)]
pub struct ThxClient {
    inner: ThxAudioPropertiesClient<Channel>,
}

impl ThxClient {
    /// Connects to the local THX service over TLS.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid connection file or a failed channel.
    pub async fn connect(config: &EndpointConfig) -> Result<Self, AudioError> {
        if config.cert.is_empty() {
            return Err(AudioError::MissingCertificate);
        }
        if !config.is_valid() {
            return Err(AudioError::InvalidAddress {
                address: config.address.clone(),
            });
        }
        let certificate = Certificate::from_pem(config.cert.as_bytes());
        let tls = ClientTlsConfig::new()
            .ca_certificate(certificate)
            .domain_name("localhost");
        let channel = Channel::from_shared(format!("https://{}", config.address))?
            .tls_config(tls)?
            .connect()
            .await?;
        Ok(Self {
            inner: ThxAudioPropertiesClient::new(channel),
        })
    }

    /// Connects without TLS, for local mock-server validation only.
    ///
    /// # Errors
    ///
    /// Returns an error if the address is invalid or the channel cannot connect.
    #[doc(hidden)]
    pub async fn connect_insecure(address: &str) -> Result<Self, AudioError> {
        let channel = Channel::from_shared(format!("http://{address}"))?
            .connect()
            .await?;
        Ok(Self {
            inner: ThxAudioPropertiesClient::new(channel),
        })
    }

    /// Returns the service version.
    ///
    /// # Errors
    ///
    /// Returns an error when the RPC fails or times out.
    pub async fn version(&mut self) -> Result<String, AudioError> {
        let response = tokio::time::timeout(
            RPC_TIMEOUT,
            self.inner
                .get_version(Request::new(proto::GetVersionRequest {})),
        )
        .await
        .map_err(|_| AudioError::Timeout)??;
        Ok(response.into_inner().version)
    }

    /// Lists endpoint ids.
    ///
    /// # Errors
    ///
    /// Returns an error when the RPC fails or times out.
    pub async fn endpoints(&mut self) -> Result<Vec<String>, AudioError> {
        let response = tokio::time::timeout(
            RPC_TIMEOUT,
            self.inner
                .get_endpoint_list(Request::new(GetEndpointListRequest {
                    r#type: String::new(),
                })),
        )
        .await
        .map_err(|_| AudioError::Timeout)??;
        Ok(response.into_inner().endpoint_ids)
    }

    /// Lists property ids for an endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error when the RPC fails or times out.
    pub async fn properties(&mut self, endpoint: &str) -> Result<Vec<String>, AudioError> {
        let response = tokio::time::timeout(
            RPC_TIMEOUT,
            self.inner
                .get_property_list(Request::new(GetPropertyListRequest {
                    endpoint_id: Some(endpoint.to_owned()),
                })),
        )
        .await
        .map_err(|_| AudioError::Timeout)??;
        Ok(response.into_inner().properties)
    }

    /// Reads one property.
    ///
    /// # Errors
    ///
    /// Returns an error when the RPC fails or times out.
    pub async fn property(&mut self, endpoint: &str, id: &str) -> Result<Property, AudioError> {
        let response = tokio::time::timeout(
            RPC_TIMEOUT,
            self.inner.get_property(Request::new(GetPropertyRequest {
                id: id.to_owned(),
                endpoint_id: Some(endpoint.to_owned()),
            })),
        )
        .await
        .map_err(|_| AudioError::Timeout)??;
        Ok(response.into_inner())
    }

    /// Writes a boolean property.
    ///
    /// # Errors
    ///
    /// Returns an error when the RPC fails or times out.
    pub async fn set_bool(
        &mut self,
        endpoint: &str,
        id: &str,
        value: bool,
    ) -> Result<(), AudioError> {
        let property = Property {
            id: id.to_owned(),
            r#type: PropertyType::Bool as i32,
            float_value: None,
            int_value: None,
            bool_value: Some(value),
            string_value: None,
            string_array_value: Vec::new(),
            float_array_value: Vec::new(),
            any_value: None,
        };
        tokio::time::timeout(
            RPC_TIMEOUT,
            self.inner.set_property(Request::new(SetPropertyRequest {
                property: Some(property),
                endpoint_id: Some(endpoint.to_owned()),
            })),
        )
        .await
        .map_err(|_| AudioError::Timeout)??;
        Ok(())
    }

    /// Lists presets in the hub's preset group.
    ///
    /// # Errors
    ///
    /// Returns an error when the RPC fails or times out.
    pub async fn presets(&mut self, endpoint: &str) -> Result<Vec<String>, AudioError> {
        let response = tokio::time::timeout(
            RPC_TIMEOUT,
            self.inner
                .get_preset_list(Request::new(GetPresetListRequest {
                    preset_group_id: PROFILE_PRESETS.to_owned(),
                    endpoint_id: Some(endpoint.to_owned()),
                })),
        )
        .await
        .map_err(|_| AudioError::Timeout)??;
        Ok(response.into_inner().preset_ids)
    }

    /// Returns the selected preset in the hub's preset group.
    ///
    /// # Errors
    ///
    /// Returns an error when the RPC fails or times out.
    pub async fn selected_preset(&mut self, endpoint: &str) -> Result<String, AudioError> {
        let response = tokio::time::timeout(
            RPC_TIMEOUT,
            self.inner
                .get_selected_preset(Request::new(GetSelectedPresetRequest {
                    endpoint_id: Some(endpoint.to_owned()),
                    preset_group_id: PROFILE_PRESETS.to_owned(),
                })),
        )
        .await
        .map_err(|_| AudioError::Timeout)??;
        Ok(response.into_inner().preset_id)
    }

    /// Selects a preset.
    ///
    /// # Errors
    ///
    /// Returns an error when the RPC fails or times out.
    pub async fn select_preset(
        &mut self,
        endpoint: &str,
        preset: &str,
    ) -> Result<Vec<String>, AudioError> {
        let response = tokio::time::timeout(
            RPC_TIMEOUT,
            self.inner.select_preset(Request::new(SelectPresetRequest {
                preset_group_id: PROFILE_PRESETS.to_owned(),
                preset_id: preset.to_owned(),
                endpoint_id: Some(endpoint.to_owned()),
            })),
        )
        .await
        .map_err(|_| AudioError::Timeout)??;
        Ok(response.into_inner().warnings)
    }

    /// Reads the same status fields the Electron UI displays.
    ///
    /// # Errors
    ///
    /// Returns an error when any required RPC fails or times out.
    pub async fn status(&mut self) -> Result<ThxStatus, AudioError> {
        let endpoints = self.endpoints().await?;
        if !endpoints.iter().any(|id| id == HEADPHONES_ENDPOINT) {
            return Ok(ThxStatus {
                endpoints,
                spatial_enabled: None,
                render_enabled: None,
                properties: Vec::new(),
                presets: Vec::new(),
                selected_preset: String::new(),
                version: None,
            });
        }

        let spatial = self
            .property(HEADPHONES_ENDPOINT, SPATIAL_ENABLED)
            .await
            .ok();
        let render = self
            .property(HEADPHONES_ENDPOINT, RENDER_ENABLED)
            .await
            .ok();
        let ids = self.properties(HEADPHONES_ENDPOINT).await?;
        let mut values = Vec::new();
        for id in &ids {
            if let Ok(property) = self.property(HEADPHONES_ENDPOINT, id).await {
                let value = format_property(&property);
                values.push(PropertyView {
                    kind: property.r#type.to_string(),
                    id: property.id,
                    value,
                });
            }
        }
        let presets = self.presets(HEADPHONES_ENDPOINT).await.unwrap_or_default();
        let selected_preset = self
            .selected_preset(HEADPHONES_ENDPOINT)
            .await
            .unwrap_or_default();
        let version = self.version().await.ok();

        Ok(ThxStatus {
            endpoints,
            spatial_enabled: spatial.and_then(|property| property.bool_value),
            render_enabled: render.and_then(|property| property.bool_value),
            properties: values,
            presets,
            selected_preset,
            version,
        })
    }
}

fn format_property(property: &Property) -> String {
    if let Some(value) = property.bool_value {
        value.to_string()
    } else if let Some(value) = property.float_value {
        value.to_string()
    } else if let Some(value) = property.int_value {
        value.to_string()
    } else if let Some(value) = &property.string_value {
        value.clone()
    } else {
        "unset".to_owned()
    }
}
