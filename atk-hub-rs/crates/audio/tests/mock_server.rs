//! Mock THX gRPC server used to validate the generated client.
#![allow(missing_docs, unused_variables)]
use atk_audio::proto::thx_audio_properties_server::ThxAudioProperties;
use atk_audio::proto::thx_audio_properties_server::ThxAudioPropertiesServer;
use atk_audio::proto::*;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

#[derive(Debug, Default)]
struct MockThx;

#[tonic::async_trait]
impl ThxAudioProperties for MockThx {
    type SubscribeToPropertyChangesStream = tokio_stream::Empty<Result<Property, Status>>;
    type StreamPoseStream = tokio_stream::Empty<Result<StreamPoseResponse, Status>>;
    type SubscribeToPresetChangesStream = tokio_stream::Empty<Result<PresetChange, Status>>;
    type SubscribeToEndpointChangesStream = tokio_stream::Empty<Result<EndpointChange, Status>>;
    type SubscribeToApplicationListChangesStream =
        tokio_stream::Empty<Result<ApplicationListChange, Status>>;
    type SubscribeToApplicationPropertyChangesStream =
        tokio_stream::Empty<Result<ApplicationPropertyChange, Status>>;
    async fn get_endpoint_list(
        &self,
        request: Request<GetEndpointListRequest>,
    ) -> Result<Response<GetEndpointListResponse>, Status> {
        Ok(tonic::Response::new(GetEndpointListResponse {
            endpoint_ids: vec!["headphones".into()],
        }))
    }
    async fn get_endpoint(
        &self,
        request: Request<GetEndpointRequest>,
    ) -> Result<Response<Endpoint>, Status> {
        Err(Status::unimplemented("get_endpoint"))
    }
    async fn get_property_list(
        &self,
        request: Request<GetPropertyListRequest>,
    ) -> Result<Response<GetPropertyListResponse>, Status> {
        Ok(tonic::Response::new(GetPropertyListResponse {
            properties: vec!["spatial_enabled".into()],
        }))
    }
    async fn get_property_definition(
        &self,
        request: Request<GetPropertyDefinitionRequest>,
    ) -> Result<Response<PropertyDefinition>, Status> {
        Err(Status::unimplemented("get_property_definition"))
    }
    async fn get_property(
        &self,
        request: Request<GetPropertyRequest>,
    ) -> Result<Response<Property>, Status> {
        Ok(tonic::Response::new(Property {
            id: request.into_inner().id,
            r#type: PropertyType::Bool as i32,
            float_value: None,
            int_value: None,
            bool_value: Some(true),
            string_value: None,
            string_array_value: vec![],
            float_array_value: vec![],
            any_value: None,
        }))
    }
    async fn set_property(
        &self,
        request: Request<SetPropertyRequest>,
    ) -> Result<Response<()>, Status> {
        Err(Status::unimplemented("set_property"))
    }
    async fn subscribe_to_property_changes(
        &self,
        request: Request<SubscribeToPropertyChangesRequest>,
    ) -> Result<Response<Self::SubscribeToPropertyChangesStream>, Status> {
        Err(Status::unimplemented("subscribe_to_property_changes"))
    }
    async fn stream_pose(
        &self,
        request: Request<StreamPoseRequest>,
    ) -> Result<Response<Self::StreamPoseStream>, Status> {
        Err(Status::unimplemented("stream_pose"))
    }
    async fn get_preset_list(
        &self,
        request: Request<GetPresetListRequest>,
    ) -> Result<Response<GetPresetListResponse>, Status> {
        Ok(tonic::Response::new(GetPresetListResponse {
            preset_ids: vec!["neutral".into()],
            preset_group_id: "profile_presets".into(),
        }))
    }
    async fn get_preset_group_list(
        &self,
        request: Request<GetPresetGroupListRequest>,
    ) -> Result<Response<GetPresetGroupListResponse>, Status> {
        Err(Status::unimplemented("get_preset_group_list"))
    }
    async fn get_preset_group_definition(
        &self,
        request: Request<GetPresetGroupDefinitionRequest>,
    ) -> Result<Response<GetPresetGroupDefinitionResponse>, Status> {
        Err(Status::unimplemented("get_preset_group_definition"))
    }
    async fn select_preset(
        &self,
        request: Request<SelectPresetRequest>,
    ) -> Result<Response<SelectPresetResponse>, Status> {
        Err(Status::unimplemented("select_preset"))
    }
    async fn get_selected_preset(
        &self,
        request: Request<GetSelectedPresetRequest>,
    ) -> Result<Response<GetSelectedPresetResponse>, Status> {
        Ok(tonic::Response::new(GetSelectedPresetResponse {
            preset_group_id: "profile_presets".into(),
            preset_id: "neutral".into(),
        }))
    }
    async fn save_preset(
        &self,
        request: Request<SavePresetRequest>,
    ) -> Result<Response<()>, Status> {
        Err(Status::unimplemented("save_preset"))
    }
    async fn reset_preset(
        &self,
        request: Request<ResetPresetRequest>,
    ) -> Result<Response<()>, Status> {
        Err(Status::unimplemented("reset_preset"))
    }
    async fn add_preset(
        &self,
        request: Request<AddPresetRequest>,
    ) -> Result<Response<AddPresetResponse>, Status> {
        Err(Status::unimplemented("add_preset"))
    }
    async fn remove_preset(
        &self,
        request: Request<RemovePresetRequest>,
    ) -> Result<Response<RemovePresetResponse>, Status> {
        Err(Status::unimplemented("remove_preset"))
    }
    async fn get_preset_description(
        &self,
        request: Request<GetPresetDescriptionRequest>,
    ) -> Result<Response<GetPresetDescriptionResponse>, Status> {
        Err(Status::unimplemented("get_preset_description"))
    }
    async fn get_preset_name(
        &self,
        request: Request<GetPresetNameRequest>,
    ) -> Result<Response<GetPresetNameResponse>, Status> {
        Err(Status::unimplemented("get_preset_name"))
    }
    async fn parse_preset(
        &self,
        request: Request<ParsePresetRequest>,
    ) -> Result<Response<ParsePresetResponse>, Status> {
        Err(Status::unimplemented("parse_preset"))
    }
    async fn export_preset(
        &self,
        request: Request<ExportPresetRequest>,
    ) -> Result<Response<ExportPresetResponse>, Status> {
        Err(Status::unimplemented("export_preset"))
    }
    async fn import_preset(
        &self,
        request: Request<ImportPresetRequest>,
    ) -> Result<Response<ImportPresetResponse>, Status> {
        Err(Status::unimplemented("import_preset"))
    }
    async fn reset_preset_allowed(
        &self,
        request: Request<PresetResetAllowedRequest>,
    ) -> Result<Response<PresetResetAllowedResponse>, Status> {
        Err(Status::unimplemented("reset_preset_allowed"))
    }
    async fn subscribe_to_preset_changes(
        &self,
        request: Request<SubscribeToPresetChangesRequest>,
    ) -> Result<Response<Self::SubscribeToPresetChangesStream>, Status> {
        Err(Status::unimplemented("subscribe_to_preset_changes"))
    }
    async fn subscribe_to_endpoint_changes(
        &self,
        request: Request<SubscribeToEndpointChangesRequest>,
    ) -> Result<Response<Self::SubscribeToEndpointChangesStream>, Status> {
        Err(Status::unimplemented("subscribe_to_endpoint_changes"))
    }
    async fn get_version(
        &self,
        request: Request<GetVersionRequest>,
    ) -> Result<Response<GetVersionResponse>, Status> {
        Ok(tonic::Response::new(GetVersionResponse {
            version: "mock-1".into(),
        }))
    }
    async fn get_application_list(
        &self,
        request: Request<GetApplicationListRequest>,
    ) -> Result<Response<GetApplicationListResponse>, Status> {
        Err(Status::unimplemented("get_application_list"))
    }
    async fn get_application_info(
        &self,
        request: Request<GetApplicationInfoRequest>,
    ) -> Result<Response<ApplicationInfo>, Status> {
        Err(Status::unimplemented("get_application_info"))
    }
    async fn get_application_property(
        &self,
        request: Request<GetApplicationPropertyRequest>,
    ) -> Result<Response<Property>, Status> {
        Err(Status::unimplemented("get_application_property"))
    }
    async fn subscribe_to_application_list_changes(
        &self,
        request: Request<SubscribeToApplicationListChangesRequest>,
    ) -> Result<Response<Self::SubscribeToApplicationListChangesStream>, Status> {
        Err(Status::unimplemented(
            "subscribe_to_application_list_changes",
        ))
    }
    async fn subscribe_to_application_property_changes(
        &self,
        request: Request<SubscribeToApplicationPropertyChangesRequest>,
    ) -> Result<Response<Self::SubscribeToApplicationPropertyChangesStream>, Status> {
        Err(Status::unimplemented(
            "subscribe_to_application_property_changes",
        ))
    }
}

#[tokio::test]
async fn client_reads_status_from_mock_server() {
    let address: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    let local = listener.local_addr().unwrap();
    tokio::spawn(async move {
        Server::builder()
            .add_service(ThxAudioPropertiesServer::new(MockThx))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    let mut client = atk_audio::ThxClient::connect_insecure(&local.to_string())
        .await
        .unwrap();
    let status = client.status().await.unwrap();
    assert_eq!(status.endpoints, vec!["headphones"]);
    assert_eq!(status.spatial_enabled, Some(true));
    assert_eq!(status.selected_preset, "neutral");
    assert_eq!(status.version.as_deref(), Some("mock-1"));
}
