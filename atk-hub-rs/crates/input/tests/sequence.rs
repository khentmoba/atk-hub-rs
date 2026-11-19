//! Tests for macro key parsing (no input events are injected).

use atk_input::{Key, KeyInjector, KeySequence};
use std::str::FromStr;

#[test]
fn parses_modifiers_and_letters() {
    let sequence = KeySequence::from_str("Ctrl+K Enter").expect("sequence");
    assert_eq!(
        sequence.keys,
        vec![Key::Ctrl, Key::Letter(b'K'), Key::Enter]
    );
    assert_eq!(Key::Ctrl.vk(), 0x11);
    assert_eq!(Key::Letter(b'K').vk(), 0x4b);
    assert_eq!(Key::Function(5).vk(), 0x74);
}

#[test]
fn rejects_unknown_keys() {
    assert!(Key::from_str("F13").is_err());
    assert!(Key::from_str("??").is_err());
}

#[test]
fn injector_is_available_without_sending_events() {
    let _injector = KeyInjector;
    let sequence = KeySequence::from_str("F1").expect("sequence");
    assert_eq!(sequence.keys.len(), 1);
}
