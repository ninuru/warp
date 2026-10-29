use std::collections::HashMap;
use std::time::Duration;

use warpui::keymap::Keystroke;

use super::{PrefixKeymap, PrefixKeysConfig, key_matches};

fn key(source: &str) -> Keystroke {
    Keystroke::parse(source).expect("valid keystroke")
}

fn config_with_bindings(bindings: &[(&str, &str)]) -> PrefixKeysConfig {
    PrefixKeysConfig {
        bindings: bindings
            .iter()
            .map(|(key, name)| ((*key).to_owned(), (*name).to_owned()))
            .collect(),
        ..PrefixKeysConfig::default()
    }
}

#[test]
fn defaults_parse_without_errors() {
    let config = PrefixKeysConfig::default();
    let (keymap, errors) = PrefixKeymap::parse(&config);

    assert!(errors.is_empty(), "{errors:?}");
    assert!(config.enabled);
    assert!(config.root_bindings.is_empty());
    assert!(keymap.is_prefix(&key("ctrl-a")));
    assert_eq!(keymap.timeout(), Duration::from_millis(1500));
    assert_eq!(keymap.bindings.len(), 20);
    assert!(keymap.root.is_empty());
}

#[test]
fn defaults_map_tmux_keys() {
    let (keymap, _) = PrefixKeymap::parse(&PrefixKeysConfig::default());

    assert_eq!(
        keymap.binding_for(&key("h")),
        Some("pane_group:navigate_left")
    );
    assert_eq!(
        keymap.binding_for(&key("l")),
        Some("pane_group:navigate_right")
    );
    assert_eq!(
        keymap.binding_for(&key("shift-\"")),
        Some("pane_group:add_down")
    );
    assert_eq!(keymap.binding_for(&key("c")), Some("workspace:new_tab"));
    assert_eq!(
        keymap.binding_for(&key("1")),
        Some("workspace:activate_first_tab")
    );
    assert_eq!(
        keymap.binding_for(&key("8")),
        Some("workspace:activate_eighth_tab")
    );
    assert_eq!(
        keymap.binding_for(&key("9")),
        Some("workspace:activate_last_tab")
    );
    assert_eq!(
        keymap.binding_for(&key("shift-^")),
        Some("workspace:activate_prev_tab")
    );
    assert_eq!(
        keymap.binding_for(&key(",")),
        Some("workspace:rename_active_tab")
    );
    assert_eq!(keymap.binding_for(&key("r")), None);
}

#[test]
fn invalid_key_is_reported_and_skipped() {
    let config = config_with_bindings(&[
        ("ctrl-nope", "workspace:new_tab"),
        ("c", "workspace:new_tab"),
    ]);
    let (keymap, errors) = PrefixKeymap::parse(&config);

    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("ctrl-nope"), "{errors:?}");
    assert_eq!(keymap.bindings.len(), 1);
    assert_eq!(keymap.binding_for(&key("c")), Some("workspace:new_tab"));
}

#[test]
fn letter_case_mismatch_is_reported_without_panicking() {
    let config = config_with_bindings(&[
        ("C", "workspace:new_tab"),
        ("shift-c", "workspace:new_tab"),
        ("shift-C", "workspace:new_tab"),
    ]);
    let (keymap, errors) = PrefixKeymap::parse(&config);

    assert_eq!(errors.len(), 2, "{errors:?}");
    assert_eq!(keymap.bindings.len(), 1);
    assert_eq!(
        keymap.binding_for(&key("shift-C")),
        Some("workspace:new_tab")
    );
    assert_eq!(keymap.binding_for(&key("c")), None);
}

#[test]
fn invalid_binding_name_is_reported_and_skipped() {
    let config = config_with_bindings(&[("c", ""), ("x", "no_namespace")]);
    let (keymap, errors) = PrefixKeymap::parse(&config);

    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(keymap.bindings.is_empty());
}

#[test]
fn invalid_prefix_falls_back_to_default() {
    let config = PrefixKeysConfig {
        key: "ctrl-nope".to_owned(),
        ..PrefixKeysConfig::default()
    };
    let (keymap, errors) = PrefixKeymap::parse(&config);

    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(keymap.is_prefix(&key("ctrl-a")));
}

#[test]
fn custom_prefix_and_timeout_are_used() {
    let config = PrefixKeysConfig {
        key: "ctrl-b".to_owned(),
        timeout_ms: 250,
        ..PrefixKeysConfig::default()
    };
    let (keymap, errors) = PrefixKeymap::parse(&config);

    assert!(errors.is_empty(), "{errors:?}");
    assert!(keymap.is_prefix(&key("ctrl-b")));
    assert!(!keymap.is_prefix(&key("ctrl-a")));
    assert_eq!(keymap.timeout(), Duration::from_millis(250));
}

#[test]
fn root_bindings_are_parsed() {
    let config = PrefixKeysConfig {
        root_bindings: HashMap::from([
            (
                "alt-1".to_owned(),
                "workspace:activate_first_tab".to_owned(),
            ),
            ("alt-bad-key".to_owned(), "workspace:new_tab".to_owned()),
        ]),
        ..PrefixKeysConfig::default()
    };
    let (keymap, errors) = PrefixKeymap::parse(&config);

    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        keymap.root_binding_for(&key("alt-1")),
        Some("workspace:activate_first_tab")
    );
    assert_eq!(keymap.root_binding_for(&key("1")), None);
}

#[test]
fn punctuation_ignores_shift() {
    assert!(key_matches(&key("%"), &key("shift-%")));
    assert!(key_matches(&key("shift-%"), &key("%")));
    assert!(key_matches(&key("%"), &key("%")));
    assert!(key_matches(&key("^"), &key("shift-^")));
    assert!(!key_matches(&key("%"), &key("ctrl-shift-%")));
    assert!(!key_matches(&key("%"), &key("shift-&")));
}

#[test]
fn letters_respect_shift_and_modifiers() {
    assert!(key_matches(&key("h"), &key("h")));
    assert!(!key_matches(&key("h"), &key("shift-H")));
    assert!(!key_matches(&key("h"), &key("ctrl-h")));
    assert!(!key_matches(&key("ctrl-a"), &key("a")));
    assert!(key_matches(&key("escape"), &key("escape")));
    assert!(!key_matches(&key("escape"), &key("shift-escape")));
}
