use super::*;

const ALL_ON: ZenModeOptions = ZenModeOptions {
    enter_fullscreen: true,
    hide_panels: true,
    follow_window_fullscreen: true,
};

const ALL_OPEN: ZenPanels = ZenPanels {
    vertical_tabs: true,
    left: true,
    right: true,
};

const NONE_OPEN: ZenPanels = ZenPanels {
    vertical_tabs: false,
    left: false,
    right: false,
};

#[test]
fn zen_toggle_enters_fullscreen_and_leaves_it_on_exit() {
    let mut state = ZenModeState::default();

    assert_eq!(
        state.toggle(ALL_ON, false, ALL_OPEN),
        ZenModeTransition::Enter {
            request_fullscreen: true,
            close_panels: ALL_OPEN,
        }
    );
    assert!(state.is_active());

    assert_eq!(
        state.toggle(ALL_ON, true, NONE_OPEN),
        ZenModeTransition::Exit {
            leave_fullscreen: true,
            reopen_panels: ALL_OPEN,
        }
    );
    assert!(!state.is_active());
}

#[test]
fn zen_toggle_keeps_preexisting_fullscreen_on_exit() {
    let mut state = ZenModeState::default();

    assert_eq!(
        state.toggle(ALL_ON, true, NONE_OPEN),
        ZenModeTransition::Enter {
            request_fullscreen: false,
            close_panels: NONE_OPEN,
        }
    );
    assert_eq!(
        state.toggle(ALL_ON, true, NONE_OPEN),
        ZenModeTransition::Exit {
            leave_fullscreen: false,
            reopen_panels: NONE_OPEN,
        }
    );
}

#[test]
fn zen_toggle_respects_disabled_fullscreen_and_panel_settings() {
    let mut state = ZenModeState::default();
    let options = ZenModeOptions {
        enter_fullscreen: false,
        hide_panels: false,
        follow_window_fullscreen: true,
    };

    assert_eq!(
        state.toggle(options, false, ALL_OPEN),
        ZenModeTransition::Enter {
            request_fullscreen: false,
            close_panels: NONE_OPEN,
        }
    );
    assert_eq!(
        state.toggle(options, false, ALL_OPEN),
        ZenModeTransition::Exit {
            leave_fullscreen: false,
            reopen_panels: NONE_OPEN,
        }
    );
}

#[test]
fn zen_restores_only_panels_that_were_open() {
    let mut state = ZenModeState::default();
    let open = ZenPanels {
        vertical_tabs: false,
        left: true,
        right: false,
    };

    state.toggle(ALL_ON, false, open);
    assert_eq!(
        state.toggle(ALL_ON, false, NONE_OPEN),
        ZenModeTransition::Exit {
            leave_fullscreen: false,
            reopen_panels: open,
        }
    );
}

#[test]
fn zen_auto_enters_and_exits_on_window_manager_fullscreen() {
    let mut state = ZenModeState::default();

    assert_eq!(
        state.window_fullscreen_changed(true, ALL_ON, ALL_OPEN),
        Some(ZenModeTransition::Enter {
            request_fullscreen: false,
            close_panels: ALL_OPEN,
        })
    );
    assert!(state.is_active());
    assert_eq!(
        state.window_fullscreen_changed(true, ALL_ON, NONE_OPEN),
        None
    );

    assert_eq!(
        state.window_fullscreen_changed(false, ALL_ON, NONE_OPEN),
        Some(ZenModeTransition::Exit {
            leave_fullscreen: false,
            reopen_panels: ALL_OPEN,
        })
    );
    assert!(!state.is_active());
}

#[test]
fn zen_ignores_fullscreen_it_requested_itself() {
    let mut state = ZenModeState::default();

    state.toggle(ALL_ON, false, NONE_OPEN);
    assert_eq!(
        state.window_fullscreen_changed(true, ALL_ON, NONE_OPEN),
        None
    );
    assert!(state.is_active());

    state.toggle(ALL_ON, true, NONE_OPEN);
    assert_eq!(
        state.window_fullscreen_changed(false, ALL_ON, NONE_OPEN),
        None
    );
    assert!(!state.is_active());
}

#[test]
fn zen_does_not_reenter_after_manual_exit_while_fullscreen() {
    let mut state = ZenModeState::default();

    state.window_fullscreen_changed(true, ALL_ON, NONE_OPEN);
    state.toggle(ALL_ON, true, NONE_OPEN);
    assert_eq!(
        state.window_fullscreen_changed(true, ALL_ON, NONE_OPEN),
        None
    );
    assert!(!state.is_active());
}

#[test]
fn zen_does_not_follow_window_fullscreen_when_disabled() {
    let mut state = ZenModeState::default();
    let options = ZenModeOptions {
        follow_window_fullscreen: false,
        ..ALL_ON
    };

    assert_eq!(
        state.window_fullscreen_changed(true, options, NONE_OPEN),
        None
    );
    assert!(!state.is_active());

    state.toggle(options, true, NONE_OPEN);
    assert_eq!(
        state.window_fullscreen_changed(false, options, NONE_OPEN),
        None
    );
    assert!(state.is_active());
}

#[test]
fn zen_tab_bar_visibility() {
    assert!(!zen_mode_shows_tab_bar(true, false, false));
    assert!(zen_mode_shows_tab_bar(true, true, false));
    assert!(!zen_mode_shows_tab_bar(false, true, false));
    assert!(zen_mode_shows_tab_bar(false, false, true));
    assert!(zen_mode_shows_tab_bar(false, true, true));
}
