use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use settings::Setting as _;
use warpui::r#async::Timer;
use warpui::event::{KeyEventDetails, KeyState};
use warpui::keymap::Keystroke;
use warpui::platform::keyboard::KeyCode;
use warpui::{App, Event, ModelHandle, SingletonEntity, WindowId};

use super::{PrefixKeysEvent, PrefixKeysState};
use crate::prefix_keys::config::{PrefixKeymap, PrefixKeysConfig};
use crate::prefix_keys::machine::PrefixOutcome;
use crate::terminal::keys_settings::KeysSettings;
use crate::test_util::settings::initialize_settings_for_tests;

fn key_down(source: &str) -> Event {
    Event::KeyDown {
        keystroke: Keystroke::parse(source).expect("valid keystroke"),
        chars: String::new(),
        details: KeyEventDetails::default(),
        is_composing: false,
    }
}

fn add_state(app: &mut App, config: PrefixKeysConfig) -> ModelHandle<PrefixKeysState> {
    initialize_settings_for_tests(app);
    let state = app.add_singleton_model(PrefixKeysState::new);
    state.update(app, |state, _| {
        state.enabled = config.enabled;
        state.keymap = PrefixKeymap::parse(&config).0;
    });
    state
}

fn capture_events(
    app: &mut App,
    state: &ModelHandle<PrefixKeysState>,
) -> Arc<Mutex<Vec<WindowId>>> {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = captured.clone();
    app.update(|ctx| {
        ctx.subscribe_to_model(state, move |_, event, _| match event {
            PrefixKeysEvent::PendingChanged { window_id } => captured_clone.lock().push(*window_id),
        });
    });
    captured
}

fn send(
    app: &mut App,
    state: &ModelHandle<PrefixKeysState>,
    window_id: WindowId,
    event: Event,
) -> PrefixOutcome {
    state.update(app, |state, ctx| state.handle_event(window_id, &event, ctx))
}

fn is_pending(app: &App, state: &ModelHandle<PrefixKeysState>, window_id: WindowId) -> bool {
    state.read(app, |state, _| {
        state
            .machines
            .get(&window_id)
            .is_some_and(|machine| machine.is_pending())
    })
}

#[test]
fn pending_times_out_and_emits_pending_changed() {
    App::test((), |mut app| async move {
        let state = add_state(
            &mut app,
            PrefixKeysConfig {
                timeout_ms: 10,
                ..PrefixKeysConfig::default()
            },
        );
        let captured = capture_events(&mut app, &state);
        let window_id = WindowId::new();

        let outcome = send(&mut app, &state, window_id, key_down("ctrl-a"));
        assert!(matches!(outcome, PrefixOutcome::EnterPending { .. }));
        assert!(is_pending(&app, &state, window_id));
        assert_eq!(*captured.lock(), vec![window_id]);

        Timer::after(Duration::from_millis(100)).await;

        assert!(!is_pending(&app, &state, window_id));
        assert_eq!(*captured.lock(), vec![window_id, window_id]);
    });
}

#[test]
fn dispatch_leaves_pending_and_emits_pending_changed() {
    App::test((), |mut app| async move {
        let state = add_state(&mut app, PrefixKeysConfig::default());
        let captured = capture_events(&mut app, &state);
        let window_id = WindowId::new();

        send(&mut app, &state, window_id, key_down("ctrl-a"));
        let outcome = send(&mut app, &state, window_id, key_down("c"));

        assert_eq!(
            outcome,
            PrefixOutcome::Dispatch {
                binding: "workspace:new_tab".to_owned()
            }
        );
        assert!(!is_pending(&app, &state, window_id));
        assert_eq!(*captured.lock(), vec![window_id, window_id]);
    });
}

#[test]
fn disabled_config_passes_everything_through() {
    App::test((), |mut app| async move {
        let state = add_state(
            &mut app,
            PrefixKeysConfig {
                enabled: false,
                ..PrefixKeysConfig::default()
            },
        );
        let captured = capture_events(&mut app, &state);
        let window_id = WindowId::new();

        assert_eq!(
            send(&mut app, &state, window_id, key_down("ctrl-a")),
            PrefixOutcome::PassThrough
        );
        assert!(!is_pending(&app, &state, window_id));
        assert!(captured.lock().is_empty());
    });
}

#[test]
fn modifier_and_marked_text_events_pass_through_while_pending() {
    App::test((), |mut app| async move {
        let state = add_state(&mut app, PrefixKeysConfig::default());
        let window_id = WindowId::new();
        send(&mut app, &state, window_id, key_down("ctrl-a"));

        let observed = [
            Event::ModifierKeyChanged {
                key_code: KeyCode::ControlLeft,
                state: KeyState::Released,
            },
            Event::SetMarkedText {
                marked_text: String::new(),
                selected_range: 0..0,
            },
            Event::ClearMarkedText,
        ];
        for event in observed {
            assert_eq!(
                send(&mut app, &state, window_id, event),
                PrefixOutcome::PassThrough
            );
        }
        assert!(is_pending(&app, &state, window_id));
    });
}

#[test]
fn windows_have_independent_pending_state() {
    App::test((), |mut app| async move {
        let state = add_state(&mut app, PrefixKeysConfig::default());
        let first = WindowId::new();
        let second = WindowId::new();

        send(&mut app, &state, first, key_down("ctrl-a"));

        assert!(is_pending(&app, &state, first));
        assert!(!is_pending(&app, &state, second));
        assert_eq!(
            send(&mut app, &state, second, key_down("h")),
            PrefixOutcome::PassThrough
        );
        assert!(is_pending(&app, &state, first));
    });
}

#[test]
fn settings_change_reloads_prefix_key() {
    App::test((), |mut app| async move {
        let state = add_state(&mut app, PrefixKeysConfig::default());
        let window_id = WindowId::new();

        app.update(|ctx| {
            KeysSettings::handle(ctx).update(ctx, |settings, ctx| {
                let config = PrefixKeysConfig {
                    key: "ctrl-b".to_owned(),
                    ..PrefixKeysConfig::default()
                };
                settings
                    .prefix_keys
                    .set_value(config, ctx)
                    .expect("prefix config is stored");
            });
        });

        assert_eq!(
            send(&mut app, &state, window_id, key_down("ctrl-a")),
            PrefixOutcome::PassThrough
        );
        assert!(matches!(
            send(&mut app, &state, window_id, key_down("ctrl-b")),
            PrefixOutcome::EnterPending { .. }
        ));
    });
}
