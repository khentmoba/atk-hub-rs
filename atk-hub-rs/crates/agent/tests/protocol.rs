//! Tests for the named-pipe wire protocol.

use atk_agent::{
    AgentError, Frame, FramedBuffer, MAX_FRAME_BYTES, MAX_PENDING, PIPE_NAME_PREFIX, PipeName,
    REQUEST_TIMEOUT, Request,
};
use serde_json::{Value, json};
use std::time::Duration;

#[test]
fn request_bytes_match_the_original_frame_shape() {
    let request = Request::new(7, "device.list", Value::Null);
    assert_eq!(
        request.encode().expect("encode"),
        b"{\"id\":7,\"method\":\"device.list\",\"params\":null}\n"
    );

    let with_params = Request::new(8, "rules.set", json!({"scope": "rgb", "enabled": true}));
    let text = String::from_utf8(with_params.encode().expect("encode")).expect("utf8");
    assert!(text.starts_with("{\"id\":8,\"method\":\"rules.set\",\"params\":{"));
    assert!(text.ends_with("}\n"));
}

#[test]
fn decodes_success_error_and_event_frames() {
    let ok =
        Frame::decode(&json!({"id": 3, "ok": true, "result": {"devices": []}})).expect("decode ok");
    assert_eq!(
        ok,
        Frame::Response {
            id: 3,
            ok: true,
            result: json!({"devices": []}),
            error: None
        }
    );

    let err = Frame::decode(&json!({"id": 4, "ok": false, "error": "boom"})).expect("decode error");
    assert!(
        matches!(err, Frame::Response { id: 4, ok: false, error: Some(ref e), .. } if e == "boom")
    );

    let event = Frame::decode(&json!({"event": "agent.status", "data": {"connected": true}}))
        .expect("decode event");
    assert!(matches!(event, Frame::Event { ref event, .. } if event == "agent.status"));

    assert!(matches!(
        Frame::decode(&json!({"ok": true})),
        Err(AgentError::MissingId)
    ));
}

#[test]
fn framed_buffer_handles_split_frames_and_multiple_frames() {
    let mut buffer = FramedBuffer::default();
    let first = b"{\"id\":1,\"ok\":true,\"result\":null}\n{\"event\":\"x\"";
    let frames = buffer.push(first).expect("first chunk");
    assert_eq!(frames.len(), 1);
    assert_eq!(buffer.pending_len(), 12);

    let more = buffer.push(b",\"data\":2}\n").expect("second chunk");
    assert_eq!(more.len(), 1);
    assert!(matches!(more[0], Frame::Event { ref event, .. } if event == "x"));
    assert_eq!(buffer.pending_len(), 0);
}

#[test]
fn rejects_oversized_and_malformed_frames() {
    let mut buffer = FramedBuffer::default();
    let too_big = vec![b'a'; MAX_FRAME_BYTES + 1];
    assert!(matches!(
        buffer.push(&too_big),
        Err(AgentError::OversizedFrame { .. })
    ));

    let mut bad = FramedBuffer::default();
    assert!(matches!(bad.push(b"{oops\n"), Err(AgentError::Json(_))));

    let mut bad_utf8 = FramedBuffer::default();
    assert!(matches!(
        bad_utf8.push(&[0xff, b'\n']),
        Err(AgentError::InvalidUtf8(_))
    ));
}

#[test]
fn pipe_name_validation_matches_agent_discovery() {
    let name = PipeName::parse(&format!("{PIPE_NAME_PREFIX}1")).expect("valid name");
    assert_eq!(name.as_str(), "atk-hub-agent-session-1");
    assert_eq!(name.windows_path(), r"\\.\pipe\atk-hub-agent-session-1");

    for bad in [
        "",
        "atk-hub-agent-session-",
        "atk-hub-agent-session-x",
        "other-1",
    ] {
        assert!(
            matches!(
                PipeName::parse(bad),
                Err(AgentError::InvalidPipeName { .. })
            ),
            "{bad}"
        );
    }
}

#[test]
fn protocol_limits_are_stable() {
    assert_eq!(MAX_FRAME_BYTES, 262_144);
    assert_eq!(MAX_PENDING, 64);
    assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(10));
}
