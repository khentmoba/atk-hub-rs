//! Generates the THX gRPC client from the shipped protobuf contract.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    let mut config = prost_build::Config::new();
    config.protoc_executable(&protoc);
    tonic_build::configure().compile_protos_with_config(
        config,
        &["proto/thx_audio_properties.proto"],
        &["proto"],
    )?;
    println!("cargo:rerun-if-changed=proto/thx_audio_properties.proto");
    Ok(())
}
