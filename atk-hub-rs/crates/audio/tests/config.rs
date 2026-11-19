//! Tests for THX connection data and generated protobuf messages.

use atk_audio::{DEFAULT_PRODUCT, EndpointConfig, HEADPHONES_ENDPOINT, endpoint_file, proto};
use prost::Message;

const VALID: &str = r#"{"address":"127.0.0.1:50051","cert":"-----BEGIN CERTIFICATE-----x"}"#;

#[test]
fn parses_and_validates_the_connection_file_shape() {
    let config = EndpointConfig::parse(VALID).expect("valid connection file");
    assert_eq!(config.address, "127.0.0.1:50051");
    assert!(config.is_valid());

    for bad in [
        r#"{"address":"example.com:50051","cert":"x"}"#,
        r#"{"address":"127.0.0.1","cert":"x"}"#,
        r#"{"address":"127.0.0.1:abc","cert":"x"}"#,
        r#"{"address":"127.0.0.1:50051","cert":""}"#,
    ] {
        assert!(
            !EndpointConfig::parse(bad).expect("json").is_valid(),
            "{bad}"
        );
    }
}

#[test]
fn endpoint_file_uses_program_data_and_rejects_unsafe_products() {
    let path = endpoint_file(DEFAULT_PRODUCT).expect("valid product");
    assert!(
        path.ends_with("thx/atk_n9v2_ultimate/thx_properties.tcp")
            || path.ends_with(r"thx\atk_n9v2_ultimate\thx_properties.tcp")
    );

    for bad in ["", "../evil", "a/b", "with space"] {
        assert!(endpoint_file(bad).is_err(), "{bad}");
    }
}

#[test]
fn generated_request_messages_round_trip() {
    let request = proto::GetPropertyRequest {
        id: "spatial_enabled".to_owned(),
        endpoint_id: Some(HEADPHONES_ENDPOINT.to_owned()),
    };
    let bytes = request.encode_to_vec();
    let decoded = proto::GetPropertyRequest::decode(bytes.as_slice()).expect("decode");
    assert_eq!(decoded.id, "spatial_enabled");
    assert_eq!(decoded.endpoint_id.as_deref(), Some(HEADPHONES_ENDPOINT));
}

#[test]
fn invalid_config_is_rejected_before_connecting() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let bad = EndpointConfig {
        address: "remote.example:1234".to_owned(),
        cert: "x".to_owned(),
    };
    runtime.block_on(async {
        assert!(atk_audio::ThxClient::connect(&bad).await.is_err());
    });
}
