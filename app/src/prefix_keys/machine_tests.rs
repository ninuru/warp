use std::collections::HashMap;

use warpui::keymap::Keystroke;

use super::{KeyInput, PrefixMachine, PrefixOutcome};
use crate::prefix_keys::config::{PrefixKeymap, PrefixKeysConfig};

fn keymap() -> PrefixKeymap {
    let config = PrefixKeysConfig {
        root_bindings: HashMap::from([("alt-h".to_owned(), "pane_group:navigate_left".to_owned())]),
        ..PrefixKeysConfig::default()
    };
    let (keymap, errors) = PrefixKeymap::parse(&config);
    assert!(errors.is_empty(), "{errors:?}");
    keymap
}

fn key(source: &str) -> Keystroke {
    Keystroke::parse(source).expect("valid keystroke")
}

fn press(machine: &mut PrefixMachine, keymap: &PrefixKeymap, source: &str) -> PrefixOutcome {
    let keystroke = key(source);
    machine.on_input(
        KeyInput::KeyDown {
            keystroke: &keystroke,
            is_repeat: false,
            is_composing: false,
        },
        keymap,
    )
}

fn repeat(machine: &mut PrefixMachine, keymap: &PrefixKeymap, source: &str) -> PrefixOutcome {
    let keystroke = key(source);
    machine.on_input(
        KeyInput::KeyDown {
            keystroke: &keystroke,
            is_repeat: true,
            is_composing: false,
        },
        keymap,
    )
}

fn enter_pending(machine: &mut PrefixMachine, keymap: &PrefixKeymap) -> u64 {
    match press(machine, keymap, "ctrl-a") {
        PrefixOutcome::EnterPending { generation } => generation,
        outcome => panic!("expected EnterPending, got {outcome:?}"),
    }
}

#[test]
fn prefix_key_enters_pending() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();

    enter_pending(&mut machine, &keymap);

    assert!(machine.is_pending());
}

#[test]
fn idle_passes_through_other_keys() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();

    assert_eq!(
        press(&mut machine, &keymap, "h"),
        PrefixOutcome::PassThrough
    );
    assert_eq!(
        press(&mut machine, &keymap, "ctrl-b"),
        PrefixOutcome::PassThrough
    );
    assert!(!machine.is_pending());
}

#[test]
fn idle_passes_through_modifier_changes() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();

    assert_eq!(
        machine.on_input(KeyInput::Modifier, &keymap),
        PrefixOutcome::PassThrough
    );
    assert!(!machine.is_pending());
}

#[test]
fn escape_cancels_pending_and_is_consumed() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);

    assert_eq!(
        press(&mut machine, &keymap, "escape"),
        PrefixOutcome::Consume
    );
    assert!(!machine.is_pending());
}

#[test]
fn prefix_twice_passes_through_and_returns_to_idle() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);

    assert_eq!(
        press(&mut machine, &keymap, "ctrl-a"),
        PrefixOutcome::PassThrough
    );
    assert!(!machine.is_pending());
}

#[test]
fn mapped_key_dispatches_and_returns_to_idle() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);

    assert_eq!(
        press(&mut machine, &keymap, "h"),
        PrefixOutcome::Dispatch {
            binding: "pane_group:navigate_left".to_owned()
        }
    );
    assert!(!machine.is_pending());
}

#[test]
fn shifted_punctuation_dispatches() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);

    assert_eq!(
        press(&mut machine, &keymap, "shift-%"),
        PrefixOutcome::Dispatch {
            binding: "pane_group:add_right".to_owned()
        }
    );
}

#[test]
fn unmapped_key_is_consumed_and_returns_to_idle() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);

    assert_eq!(press(&mut machine, &keymap, "q"), PrefixOutcome::Consume);
    assert!(!machine.is_pending());
}

#[test]
fn modifier_changes_while_pending_pass_through_and_keep_pending() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);

    assert_eq!(
        machine.on_input(KeyInput::Modifier, &keymap),
        PrefixOutcome::PassThrough
    );
    assert!(machine.is_pending());
    assert_eq!(
        press(&mut machine, &keymap, "shift-%"),
        PrefixOutcome::Dispatch {
            binding: "pane_group:add_right".to_owned()
        }
    );
    assert_eq!(
        machine.on_input(KeyInput::Modifier, &keymap),
        PrefixOutcome::PassThrough
    );
}

#[test]
fn repeat_while_pending_is_consumed_and_keeps_pending() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);

    assert_eq!(
        repeat(&mut machine, &keymap, "ctrl-a"),
        PrefixOutcome::Consume
    );
    assert_eq!(repeat(&mut machine, &keymap, "h"), PrefixOutcome::Consume);
    assert!(machine.is_pending());
    assert_eq!(
        press(&mut machine, &keymap, "h"),
        PrefixOutcome::Dispatch {
            binding: "pane_group:navigate_left".to_owned()
        }
    );
}

#[test]
fn repeated_prefix_while_idle_does_not_enter_pending() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();

    assert_eq!(
        repeat(&mut machine, &keymap, "ctrl-a"),
        PrefixOutcome::PassThrough
    );
    assert!(!machine.is_pending());
}

#[test]
fn timeout_with_current_generation_cancels_pending() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    let generation = enter_pending(&mut machine, &keymap);

    assert!(machine.on_timeout(generation));
    assert!(!machine.is_pending());
    assert!(!machine.on_timeout(generation));
}

#[test]
fn timeout_with_stale_generation_is_ignored() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    let stale = enter_pending(&mut machine, &keymap);
    press(&mut machine, &keymap, "escape");
    let current = enter_pending(&mut machine, &keymap);

    assert_ne!(stale, current);
    assert!(!machine.on_timeout(stale));
    assert!(machine.is_pending());
    assert!(machine.on_timeout(current));
}

#[test]
fn ime_composition_blocks_prefix_entry() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();

    assert_eq!(
        machine.on_input(KeyInput::MarkedText { composing: true }, &keymap),
        PrefixOutcome::PassThrough
    );
    assert_eq!(
        press(&mut machine, &keymap, "ctrl-a"),
        PrefixOutcome::PassThrough
    );
    assert!(!machine.is_pending());

    assert_eq!(
        machine.on_input(KeyInput::MarkedText { composing: false }, &keymap),
        PrefixOutcome::PassThrough
    );
    enter_pending(&mut machine, &keymap);
}

#[test]
fn composing_key_down_blocks_prefix_entry() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    let keystroke = key("ctrl-a");

    assert_eq!(
        machine.on_input(
            KeyInput::KeyDown {
                keystroke: &keystroke,
                is_repeat: false,
                is_composing: true,
            },
            &keymap,
        ),
        PrefixOutcome::PassThrough
    );
    assert!(!machine.is_pending());
}

#[test]
fn marked_text_while_pending_passes_through() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);

    assert_eq!(
        machine.on_input(KeyInput::MarkedText { composing: true }, &keymap),
        PrefixOutcome::PassThrough
    );
    assert!(machine.is_pending());
}

#[test]
fn root_key_while_idle_dispatches() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();

    assert_eq!(
        press(&mut machine, &keymap, "alt-h"),
        PrefixOutcome::DispatchRoot {
            binding: "pane_group:navigate_left".to_owned()
        }
    );
    assert!(!machine.is_pending());
}

#[test]
fn root_key_while_pending_uses_prefix_table() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);

    assert_eq!(
        press(&mut machine, &keymap, "alt-h"),
        PrefixOutcome::Consume
    );
    assert!(!machine.is_pending());
}

#[test]
fn non_root_key_while_idle_passes_through() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();

    assert_eq!(
        press(&mut machine, &keymap, "alt-j"),
        PrefixOutcome::PassThrough
    );
}

#[test]
fn repeat_of_key_that_ended_pending_is_consumed() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);
    press(&mut machine, &keymap, "h");

    assert_eq!(repeat(&mut machine, &keymap, "h"), PrefixOutcome::Consume);
    assert_eq!(repeat(&mut machine, &keymap, "h"), PrefixOutcome::Consume);
    assert!(!machine.is_pending());

    enter_pending(&mut machine, &keymap);
    press(&mut machine, &keymap, "q");

    assert_eq!(repeat(&mut machine, &keymap, "q"), PrefixOutcome::Consume);
}

#[test]
fn non_repeat_press_clears_held_key_and_other_keys_pass_through() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);
    press(&mut machine, &keymap, "h");

    assert_eq!(
        repeat(&mut machine, &keymap, "j"),
        PrefixOutcome::PassThrough
    );
    assert_eq!(
        press(&mut machine, &keymap, "h"),
        PrefixOutcome::PassThrough
    );
    assert_eq!(
        repeat(&mut machine, &keymap, "h"),
        PrefixOutcome::PassThrough
    );
}

#[test]
fn repeat_after_prefix_twice_passes_through() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    enter_pending(&mut machine, &keymap);
    press(&mut machine, &keymap, "ctrl-a");

    assert_eq!(
        repeat(&mut machine, &keymap, "ctrl-a"),
        PrefixOutcome::PassThrough
    );
}

#[test]
fn held_prefix_after_timeout_is_consumed() {
    let keymap = keymap();
    let mut machine = PrefixMachine::default();
    let generation = enter_pending(&mut machine, &keymap);
    assert!(machine.on_timeout(generation));

    assert_eq!(
        repeat(&mut machine, &keymap, "ctrl-a"),
        PrefixOutcome::Consume
    );
    assert!(!machine.is_pending());

    enter_pending(&mut machine, &keymap);
}
