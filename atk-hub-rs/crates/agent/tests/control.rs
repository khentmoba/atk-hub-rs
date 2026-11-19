//! End-to-end tests for typed HID control over the agent pipe.

use atk_agent::{AgentClient, AgentError, DeviceId, PipeName, Rule, RuleAction};
use named_pipe::PipeOptions;
use serde_json::Value;
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;

fn pipe_name(seed: u32) -> String {
    format!("atk-hub-agent-session-{}{}", std::process::id(), seed)
}

fn pipe_path(name: &str) -> String {
    format!(r"\\.\pipe\{name}")
}

/// Runs one operation against a local pipe server and returns the request frame.
fn one_request<F>(seed: u32, operation: F) -> Value
where
    F: FnOnce(&AgentClient) -> Result<Value, AgentError>,
{
    let name = pipe_name(seed);
    let server = PipeOptions::new(pipe_path(&name))
        .first(false)
        .single()
        .expect("create control pipe");
    let server_thread = thread::spawn(move || {
        let mut server = server.wait().expect("client connection");
        server.set_read_timeout(Some(Duration::from_millis(200)));
        let mut chunk = [0_u8; 4096];
        loop {
            match server.read(&mut chunk) {
                Ok(0) => continue,
                Ok(count) => {
                    let request: Value =
                        serde_json::from_slice(&chunk[..count]).expect("request JSON");
                    let id = request["id"].as_u64().expect("request id");
                    let response = format!("{{\"id\":{id},\"ok\":true,\"result\":{{}}}}\n");
                    server
                        .write_all(response.as_bytes())
                        .expect("response frame");
                    return request;
                }
                Err(_) => continue,
            }
        }
    });

    let pipe = PipeName::parse(&name).expect("valid pipe");
    let client = AgentClient::connect(&pipe).expect("connect");
    operation(&client).expect("control operation");
    server_thread.join().expect("server thread")
}

#[test]
fn connect_device_sends_vid_pid_and_usage() {
    let device = DeviceId::new(0x3554, 0xf505).with_usage(1, 6);
    let request = one_request(1, |client| client.connect_device(&device));
    assert_eq!(request["method"], "device.connect");
    assert_eq!(request["params"]["vid"], 0x3554);
    assert_eq!(request["params"]["pid"], 0xf505);
    assert_eq!(request["params"]["usage"], 6);
}

#[test]
fn disconnect_and_write_report_send_expected_frames() {
    let disconnect = one_request(2, |client| client.disconnect_device());
    assert_eq!(disconnect["method"], "device.disconnect");

    let report = vec![0_u8, 170, 1];
    let written = one_request(3, |client| client.write_report(&report));
    assert_eq!(written["method"], "device.write");
    assert_eq!(written["params"], serde_json::json!([0, 170, 1]));
}

#[test]
fn rules_and_bindings_send_expected_frames() {
    let rule = Rule::new("open-notepad", "A1", RuleAction::open("notepad.exe"));
    let rules = one_request(4, |client| client.set_rules(std::slice::from_ref(&rule)));
    assert_eq!(rules["method"], "rules.set");
    assert_eq!(rules["params"][0]["action"]["type"], "open");

    let device = DeviceId::new(0x3554, 0xf505);
    let bindings = one_request(5, |client| client.set_bindings(&device, "rgb", &[rule]));
    assert_eq!(bindings["method"], "bindings.set");
    assert_eq!(bindings["params"]["scope"], "rgb");
    assert_eq!(bindings["params"]["device"]["vid"], 0x3554);
}

#[test]
fn validates_report_length_and_scope_locally() {
    let name = pipe_name(6);
    let server = PipeOptions::new(pipe_path(&name))
        .first(false)
        .single()
        .expect("create control pipe");
    let server_thread = thread::spawn(move || {
        let _ = server.wait_ms(2_000);
    });
    let client =
        AgentClient::connect(&PipeName::parse(&name).expect("valid pipe")).expect("connect");
    assert!(client.write_report(&[]).is_err());
    assert!(client.set_bindings(&DeviceId::new(1, 1), "", &[]).is_err());
    assert!(
        client
            .set_bindings(&DeviceId::new(1, 1), "bad scope", &[])
            .is_err()
    );
    server_thread.join().expect("server thread");
}
