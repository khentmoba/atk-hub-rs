//! End-to-end tests for the live named-pipe client.

use atk_agent::{AgentClient, PipeName};
use named_pipe::PipeOptions;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;

const TEST_PIPE: &str = r"\\.\pipe\atk-hub-agent-session-9999";
const TEST_NAME: &str = "atk-hub-agent-session-9999";
const ALLOWLIST_PIPE: &str = r"\\.\pipe\atk-hub-agent-session-9998";
const ALLOWLIST_NAME: &str = "atk-hub-agent-session-9998";

#[test]
fn correlates_responses_and_forwards_events() {
    let server = PipeOptions::new(TEST_PIPE)
        .first(false)
        .single()
        .expect("create test pipe");

    let server_thread = thread::spawn(move || {
        let mut server = server.wait().expect("client connection");
        server.set_read_timeout(Some(Duration::from_secs(10)));

        let mut chunk = [0_u8; 4096];
        let count = server.read(&mut chunk).expect("request frame");
        let request: Value = serde_json::from_slice(&chunk[..count]).expect("request JSON");
        assert_eq!(request["method"], "device.list");
        let id = request["id"].as_u64().expect("numeric id");

        server
            .write_all(b"{\"event\":\"agent.status\",\"data\":{\"connected\":true}}\n")
            .expect("event frame");
        let response = format!("{{\"id\":{id},\"ok\":true,\"result\":{{\"devices\":[]}}}}\n");
        server
            .write_all(response.as_bytes())
            .expect("response frame");
    });

    let pipe = PipeName::parse(TEST_NAME).expect("valid test pipe");
    let client = AgentClient::connect(&pipe).expect("connect to test pipe");
    let events = client.events();

    let result = client
        .request("device.list", Value::Null)
        .expect("correlated response");
    assert_eq!(result, json!({ "devices": [] }));

    let event = events
        .recv_timeout(Duration::from_secs(5))
        .expect("event delivered");
    assert_eq!(event["event"], "agent.status");
    assert_eq!(event["data"]["connected"], true);

    server_thread.join().expect("server thread");
}

#[test]
fn rejects_methods_outside_the_agent_allowlist() {
    let server = PipeOptions::new(ALLOWLIST_PIPE)
        .first(false)
        .single()
        .expect("create test pipe");
    let server_thread = thread::spawn(move || {
        // No request will be written, so do not block waiting for one.
        let _ = server.wait_ms(1_000);
    });

    let pipe = PipeName::parse(ALLOWLIST_NAME).expect("valid test pipe");
    let client = AgentClient::connect(&pipe).expect("connect to test pipe");
    assert!(client.request("shell.execute", Value::Null).is_err());

    server_thread.join().expect("server thread");
}
