use std::time::Duration;

use settings::Setting as _;
use warp_core::features::FeatureFlag;
use warpui::r#async::Timer;
use warpui::event::KeyEventDetails;
use warpui::keymap::Keystroke;
use warpui::{App, Event, SingletonEntity, ViewHandle, WindowId};

use crate::appearance::Appearance;
use crate::prefix_keys::{PrefixKeysConfig, PrefixKeysState};
use crate::terminal::keys_settings::KeysSettings;
use crate::workspace::Workspace;

use super::prefix_label;
use crate::workspace::view::tests::{initialize_app, mock_workspace};

fn key_down(source: &str) -> Event {
    Event::KeyDown {
        keystroke: Keystroke::parse(source).expect("valid keystroke"),
        chars: String::new(),
        details: KeyEventDetails::default(),
        is_composing: false,
    }
}

fn setup(app: &mut App, timeout_ms: u64) -> ViewHandle<Workspace> {
    initialize_app(app);
    app.update(|ctx| {
        KeysSettings::handle(ctx).update(ctx, |settings, ctx| {
            let config = PrefixKeysConfig {
                timeout_ms,
                ..PrefixKeysConfig::default()
            };
            settings
                .prefix_keys
                .set_value(config, ctx)
                .expect("prefix config is stored");
        });
    });
    app.add_singleton_model(PrefixKeysState::new);
    mock_workspace(app)
}

fn send(app: &mut App, window_id: WindowId, source: &str) {
    let event = key_down(source);
    app.update(|ctx| {
        PrefixKeysState::handle(ctx).update(ctx, |state, ctx| {
            state.handle_event(window_id, &event, ctx);
        });
    });
}

fn chip_shown(app: &App, workspace: &ViewHandle<Workspace>) -> bool {
    workspace.read(app, |workspace, ctx| {
        workspace
            .render_prefix_pending_chip(Appearance::as_ref(ctx), ctx)
            .is_some()
    })
}

fn window_id(app: &App, workspace: &ViewHandle<Workspace>) -> WindowId {
    workspace.read(app, |workspace, _| workspace.window_id)
}

#[test]
fn chip_shows_while_pending_and_hides_after_dispatch() {
    let _flag = FeatureFlag::PrefixKeys.override_enabled(true);
    App::test((), |mut app| async move {
        let workspace = setup(&mut app, 1500);
        let window_id = window_id(&app, &workspace);
        assert!(!chip_shown(&app, &workspace));

        send(&mut app, window_id, "ctrl-a");
        assert!(chip_shown(&app, &workspace));

        send(&mut app, window_id, "c");
        assert!(!chip_shown(&app, &workspace));
    });
}

#[test]
fn chip_hides_after_cancel() {
    let _flag = FeatureFlag::PrefixKeys.override_enabled(true);
    App::test((), |mut app| async move {
        let workspace = setup(&mut app, 1500);
        let window_id = window_id(&app, &workspace);

        send(&mut app, window_id, "ctrl-a");
        assert!(chip_shown(&app, &workspace));

        send(&mut app, window_id, "escape");
        assert!(!chip_shown(&app, &workspace));
    });
}

#[test]
fn chip_hides_after_timeout() {
    let _flag = FeatureFlag::PrefixKeys.override_enabled(true);
    App::test((), |mut app| async move {
        let workspace = setup(&mut app, 10);
        let window_id = window_id(&app, &workspace);

        send(&mut app, window_id, "ctrl-a");
        assert!(chip_shown(&app, &workspace));

        Timer::after(Duration::from_millis(100)).await;
        assert!(!chip_shown(&app, &workspace));
    });
}

#[test]
fn chip_ignores_other_windows_pending_state() {
    let _flag = FeatureFlag::PrefixKeys.override_enabled(true);
    App::test((), |mut app| async move {
        let workspace = setup(&mut app, 1500);

        let other_window_id = WindowId::new();
        send(&mut app, other_window_id, "ctrl-a");

        assert!(app.read(|ctx| PrefixKeysState::as_ref(ctx).is_pending(other_window_id)));
        assert!(!chip_shown(&app, &workspace));
    });
}

#[test]
fn prefix_label_uses_caret_for_ctrl_letter() {
    let keystroke = Keystroke::parse("ctrl-a").expect("valid keystroke");
    assert_eq!(prefix_label(&keystroke), "^A");
}

#[test]
fn prefix_label_falls_back_to_displayed_for_other_modifiers() {
    let keystroke = Keystroke::parse("alt-a").expect("valid keystroke");
    assert_eq!(prefix_label(&keystroke), keystroke.displayed());
}
