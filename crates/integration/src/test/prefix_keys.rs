use std::path::PathBuf;
use std::time::Duration;

use warp::features::FeatureFlag;
use warp::integration_testing::pane_group::assert_num_panes_in_tab;
use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::terminal::{
    assert_alt_grid_active, assert_input_editor_contents, assert_no_block_executing,
    execute_command_successfully, execute_long_running_command,
    wait_until_bootstrapped_single_pane_for_tab,
};
use warp::integration_testing::view_getters::{
    command_palette_view, single_terminal_view_for_tab, workspace_view,
};
use warp::integration_testing::workspace::{assert_focused_tab_index, assert_tab_count};
use warp::search::command_palette::mixer::CommandPaletteItemAction;
use warp::workspace::WorkspaceAction;
use warpui_core::event::KeyEventDetails;
use warpui_core::integration::{AssertionCallback, AssertionOutcome, TestStep};
use warpui_core::keymap::Keystroke;
use warpui_core::{App, Event, WindowId, async_assert};

use super::new_builder;
use crate::Builder;

const PAST_PREFIX_TIMEOUT: Duration = Duration::from_millis(1700);
const CHIP_RENDER_PAUSE: Duration = Duration::from_millis(300);
const PREFIX_CHIP_SCREENSHOT: &str = "prefix-chip.png";
const PREFIX_CHIP_CANCELLED_SCREENSHOT: &str = "prefix-chip-cancelled.png";
const COPY_MODE_CURSOR_SCREENSHOT: &str = "copy-mode-cursor.png";
const COPY_MODE_SELECTION_SCREENSHOT: &str = "copy-mode-selection.png";
const COPY_MODE_EXITED_SCREENSHOT: &str = "copy-mode-exited.png";
const COPY_MODE_ALT_SCREEN_SCREENSHOT: &str = "copy-mode-alt-screen.png";
const COPY_MODE_COMMAND: &str = "printf 'first line\\nsecond line\\n'";
const ALT_SCREEN_COPY_MODE_COMMAND: &str =
    "(printf 'alpha one\\nbeta two\\ngamma three\\n'; seq 1 200) | less";
const ALT_SCREEN_FIRST_LINE: &str = "alpha one";

fn ctrl_a() -> Event {
    Event::KeyDown {
        keystroke: Keystroke::parse("ctrl-a").expect("ctrl-a is a valid keystroke"),
        chars: "\u{1}".to_owned(),
        details: KeyEventDetails::default(),
        is_composing: false,
    }
}

fn ctrl_a_bracket() -> TestStep {
    new_step_with_default_assertions("ctrl-a [ enters copy mode")
        .with_event(ctrl_a())
        .with_keystrokes(&["["])
        .add_named_assertion("copy mode is active", assert_copy_mode_row(Some(1)))
        .add_named_assertion("COPY badge is shown", assert_copy_mode_badge_rendered(true))
        .add_named_assertion(
            "input editor did not receive `[`",
            assert_input_editor_contents(0, ""),
        )
}

fn assert_copy_mode_row(expected_row: Option<usize>) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let terminal_view = single_terminal_view_for_tab(app, window_id, 0);
        terminal_view.read(app, |view, _| {
            let row = view
                .model
                .lock()
                .block_list()
                .copy_mode()
                .map(|state| state.cursor.get().row);
            async_assert!(
                row == expected_row,
                "Expected copy mode cursor row {expected_row:?}, got {row:?}"
            )
        })
    })
}

fn assert_copy_mode_badge_rendered(expected: bool) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let terminal_view = single_terminal_view_for_tab(app, window_id, 0);
        terminal_view.read(app, |view, ctx| {
            let rendered = ctx
                .element_position_by_id_at_last_frame(window_id, view.copy_mode_badge_position_id())
                .is_some();
            async_assert!(
                rendered == expected,
                "Expected copy mode badge rendered {expected}, got {rendered}"
            )
        })
    })
}

fn assert_alt_screen_copy_mode_row(expected_row: Option<usize>) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let terminal_view = single_terminal_view_for_tab(app, window_id, 0);
        terminal_view.read(app, |view, _| {
            let row = view
                .model
                .lock()
                .alt_screen()
                .copy_mode()
                .map(|state| state.cursor.row);
            async_assert!(
                row == expected_row,
                "Expected alt screen copy mode cursor row {expected_row:?}, got {row:?}"
            )
        })
    })
}

fn assert_alt_screen_first_line(expected: &'static str) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let terminal_view = single_terminal_view_for_tab(app, window_id, 0);
        terminal_view.read(app, |view, _| {
            let output = view.model.lock().alt_screen().output_to_string();
            let first_line = output.lines().next().unwrap_or_default().trim_end();
            async_assert!(
                first_line == expected,
                "Expected alt screen first line {expected:?}, got {first_line:?}"
            )
        })
    })
}

fn assert_clipboard_text(expected: &'static str) -> AssertionCallback {
    Box::new(move |app, _window_id| {
        let text = app.update(|ctx| ctx.clipboard().read()).plain_text;
        async_assert!(
            text == expected,
            "Expected clipboard {expected:?}, got {text:?}"
        )
    })
}

fn prefix_builder() -> Builder {
    FeatureFlag::PrefixKeys.set_enabled(true);
    new_builder().with_step(wait_until_bootstrapped_single_pane_for_tab(0))
}

fn assert_output_has_single_ctrl_a(app: &mut App, window_id: WindowId) -> AssertionOutcome {
    let terminal_view = single_terminal_view_for_tab(app, window_id, 0);
    terminal_view.read(app, |view, _| {
        let output = view
            .model
            .lock()
            .block_list()
            .active_block()
            .output_to_string();
        async_assert!(
            output.contains("^A") && !output.contains("^A^A"),
            "Expected `cat -v` to print exactly one ^A per line, but output was: {output:?}"
        )
    })
}

pub fn test_prefix_pending_indicator() -> Builder {
    prefix_builder()
        .with_real_display()
        .with_step(
            new_step_with_default_assertions("ctrl-a shows the ^A pending chip")
                .with_event(ctrl_a())
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot(PREFIX_CHIP_SCREENSHOT),
        )
        .with_step(
            new_step_with_default_assertions("escape hides the pending chip")
                .with_keystrokes(&["escape"])
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot(PREFIX_CHIP_CANCELLED_SCREENSHOT)
                .add_named_assertion("workspace still has one tab", assert_tab_count(1)),
        )
}

pub fn test_prefix_new_tab() -> Builder {
    prefix_builder().with_step(
        new_step_with_default_assertions("ctrl-a c opens a new tab")
            .with_event(ctrl_a())
            .with_keystrokes(&["c"])
            .add_named_assertion("workspace has two tabs", assert_tab_count(2))
            .add_named_assertion(
                "first tab input editor did not receive `c`",
                assert_input_editor_contents(0, ""),
            )
            .add_named_assertion(
                "new tab input editor did not receive `c`",
                assert_input_editor_contents(1, ""),
            ),
    )
}

pub fn test_prefix_split_right() -> Builder {
    prefix_builder().with_step(
        new_step_with_default_assertions("ctrl-a % splits the pane to the right")
            .with_event(ctrl_a())
            .with_keystrokes(&["shift-%"])
            .add_named_assertion("workspace still has one tab", assert_tab_count(1))
            .add_named_assertion("active tab has two panes", assert_num_panes_in_tab(0, 2)),
    )
}

pub fn test_prefix_twice_passes_through() -> Builder {
    prefix_builder()
        .with_step(execute_long_running_command(0, "cat -v".to_owned()))
        .with_step(
            TestStep::new("ctrl-a ctrl-a enter writes a single ^A to `cat -v`")
                .with_event(ctrl_a())
                .with_event(ctrl_a())
                .with_keystrokes(&["enter"])
                .set_timeout(Duration::from_secs(5))
                .add_named_assertion(
                    "terminal output shows exactly one ^A",
                    assert_output_has_single_ctrl_a,
                ),
        )
        .with_step(
            TestStep::new("ctrl-d ends `cat -v`")
                .with_keystrokes(&["ctrl-d"])
                .add_named_assertion("`cat -v` exited", assert_no_block_executing(0, 0)),
        )
}

pub fn test_prefix_timeout_cancels() -> Builder {
    prefix_builder()
        .with_step(
            new_step_with_default_assertions("ctrl-a, then wait past the prefix timeout")
                .with_event(ctrl_a())
                .set_post_step_pause(PAST_PREFIX_TIMEOUT),
        )
        .with_step(
            new_step_with_default_assertions("`c` after the timeout is typed into the input")
                .with_keystrokes(&["c"])
                .add_named_assertion("workspace still has one tab", assert_tab_count(1))
                .add_named_assertion(
                    "input editor contains `c`",
                    assert_input_editor_contents(0, "c"),
                ),
        )
}

pub fn test_prefix_escape_cancels() -> Builder {
    prefix_builder().with_step(
        new_step_with_default_assertions("ctrl-a escape cancels, then `c` is typed into the input")
            .with_event(ctrl_a())
            .with_keystrokes(&["escape", "c"])
            .add_named_assertion("workspace still has one tab", assert_tab_count(1))
            .add_named_assertion(
                "input editor contains `c`",
                assert_input_editor_contents(0, "c"),
            ),
    )
}

pub fn test_root_alt_number_switches_tab() -> Builder {
    prefix_builder()
        .with_step(
            new_step_with_default_assertions("ctrl-a c opens a second tab")
                .with_event(ctrl_a())
                .with_keystrokes(&["c"])
                .add_named_assertion("workspace has two tabs", assert_tab_count(2))
                .add_named_assertion("second tab is active", assert_focused_tab_index(1)),
        )
        .with_step(wait_until_bootstrapped_single_pane_for_tab(1))
        .with_step(
            new_step_with_default_assertions("alt-1 activates the first tab")
                .with_keystrokes(&["alt-1"])
                .add_named_assertion("first tab is active", assert_focused_tab_index(0))
                .add_named_assertion(
                    "first tab input editor did not receive `1`",
                    assert_input_editor_contents(0, ""),
                ),
        )
        .with_step(
            new_step_with_default_assertions("alt-2 activates the second tab")
                .with_keystrokes(&["alt-2"])
                .add_named_assertion("second tab is active", assert_focused_tab_index(1))
                .add_named_assertion(
                    "second tab input editor did not receive `2`",
                    assert_input_editor_contents(1, ""),
                ),
        )
}

pub fn test_root_ctrl_shift_enter_splits_right() -> Builder {
    prefix_builder().with_step(
        new_step_with_default_assertions("ctrl-shift-enter splits the pane to the right")
            .with_keystrokes(&["ctrl-shift-enter"])
            .add_named_assertion("workspace still has one tab", assert_tab_count(1))
            .add_named_assertion("active tab has two panes", assert_num_panes_in_tab(0, 2)),
    )
}

pub fn test_root_alt_shift_enter_splits_down() -> Builder {
    prefix_builder().with_step(
        new_step_with_default_assertions("alt-shift-enter splits the pane down")
            .with_keystrokes(&["alt-shift-enter"])
            .add_named_assertion("workspace still has one tab", assert_tab_count(1))
            .add_named_assertion("active tab has two panes", assert_num_panes_in_tab(0, 2)),
    )
}

pub fn test_copy_mode_yanks_selection() -> Builder {
    prefix_builder()
        .with_step(execute_command_successfully(COPY_MODE_COMMAND))
        .with_step(ctrl_a_bracket())
        .with_step(
            new_step_with_default_assertions("k and j move the cursor without typing")
                .with_keystrokes(&["k"])
                .add_named_assertion("cursor moved up", assert_copy_mode_row(Some(0)))
                .add_named_assertion(
                    "input editor did not receive `k`",
                    assert_input_editor_contents(0, ""),
                ),
        )
        .with_step(
            new_step_with_default_assertions("j moves the cursor back down")
                .with_keystrokes(&["j"])
                .add_named_assertion("cursor moved down", assert_copy_mode_row(Some(1)))
                .add_named_assertion(
                    "input editor did not receive `j`",
                    assert_input_editor_contents(0, ""),
                )
                .add_named_assertion("no command started", assert_no_block_executing(0, 0)),
        )
        .with_step(
            new_step_with_default_assertions("v e y yanks the word under the cursor")
                .with_keystrokes(&["v", "e", "y"])
                .add_named_assertion("clipboard holds the word", assert_clipboard_text("second"))
                .add_named_assertion("yank exits copy mode", assert_copy_mode_row(None))
                .add_named_assertion("COPY badge is gone", assert_copy_mode_badge_rendered(false))
                .add_named_assertion(
                    "input editor did not receive copy mode keys",
                    assert_input_editor_contents(0, ""),
                ),
        )
        .with_step(ctrl_a_bracket())
        .with_step(
            new_step_with_default_assertions("q exits copy mode")
                .with_keystrokes(&["q"])
                .add_named_assertion("copy mode is inactive", assert_copy_mode_row(None))
                .add_named_assertion("COPY badge is gone", assert_copy_mode_badge_rendered(false))
                .add_named_assertion(
                    "input editor did not receive `q`",
                    assert_input_editor_contents(0, ""),
                ),
        )
        .with_step(ctrl_a_bracket())
        .with_step(
            new_step_with_default_assertions("escape exits copy mode")
                .with_keystrokes(&["escape"])
                .add_named_assertion("copy mode is inactive", assert_copy_mode_row(None))
                .add_named_assertion("COPY badge is gone", assert_copy_mode_badge_rendered(false)),
        )
}

fn ctrl_a_bracket_on_alt_screen() -> TestStep {
    TestStep::new("ctrl-a [ enters copy mode on the alt screen")
        .with_event(ctrl_a())
        .with_keystrokes(&["["])
        .add_named_assertion(
            "copy mode is active",
            assert_alt_screen_copy_mode_row_is_some(),
        )
        .add_named_assertion("COPY badge is shown", assert_copy_mode_badge_rendered(true))
        .add_named_assertion(
            "less did not receive `[`",
            assert_alt_screen_first_line(ALT_SCREEN_FIRST_LINE),
        )
}

fn assert_alt_screen_copy_mode_row_is_some() -> AssertionCallback {
    Box::new(|app, window_id| {
        let terminal_view = single_terminal_view_for_tab(app, window_id, 0);
        terminal_view.read(app, |view, _| {
            let is_active = view.model.lock().alt_screen().copy_mode().is_some();
            async_assert!(is_active, "Expected alt screen copy mode to be active")
        })
    })
}

pub fn test_copy_mode_alt_screen_yanks_selection() -> Builder {
    prefix_builder()
        .with_step(
            execute_long_running_command(0, ALT_SCREEN_COPY_MODE_COMMAND.to_owned())
                .add_named_assertion(
                    "less is on the alt screen",
                    assert_alt_grid_active(0, 0, true),
                )
                .add_named_assertion(
                    "less shows the first page",
                    assert_alt_screen_first_line(ALT_SCREEN_FIRST_LINE),
                ),
        )
        .with_step(ctrl_a_bracket_on_alt_screen())
        .with_step(
            TestStep::new("g and j move the cursor to beta")
                .with_keystrokes(&["g", "j"])
                .add_named_assertion(
                    "cursor is on the beta row",
                    assert_alt_screen_copy_mode_row(Some(1)),
                )
                .add_named_assertion(
                    "less did not scroll on `j`",
                    assert_alt_screen_first_line(ALT_SCREEN_FIRST_LINE),
                ),
        )
        .with_step(
            TestStep::new("v e y yanks beta")
                .with_keystrokes(&["v", "e", "y"])
                .add_named_assertion("clipboard holds the word", assert_clipboard_text("beta"))
                .add_named_assertion(
                    "yank exits copy mode",
                    assert_alt_screen_copy_mode_row(None),
                )
                .add_named_assertion("COPY badge is gone", assert_copy_mode_badge_rendered(false))
                .add_named_assertion("less is still running", assert_alt_grid_active(0, 0, true))
                .add_named_assertion(
                    "less did not receive copy mode keys",
                    assert_alt_screen_first_line(ALT_SCREEN_FIRST_LINE),
                ),
        )
        .with_step(ctrl_a_bracket_on_alt_screen())
        .with_step(
            TestStep::new("q exits copy mode without quitting less")
                .with_keystrokes(&["q"])
                .add_named_assertion(
                    "copy mode is inactive",
                    assert_alt_screen_copy_mode_row(None),
                )
                .add_named_assertion("COPY badge is gone", assert_copy_mode_badge_rendered(false))
                .add_named_assertion("less is still running", assert_alt_grid_active(0, 0, true))
                .add_named_assertion(
                    "less still shows the first page",
                    assert_alt_screen_first_line(ALT_SCREEN_FIRST_LINE),
                ),
        )
        .with_step(
            TestStep::new("a second q quits less")
                .with_keystrokes(&["q"])
                .add_named_assertion("alt screen is gone", assert_alt_grid_active(0, 0, false)),
        )
}

pub fn test_copy_mode_cursor_screenshot() -> Builder {
    prefix_builder()
        .with_real_display()
        .with_step(execute_command_successfully(COPY_MODE_COMMAND))
        .with_step(ctrl_a_bracket())
        .with_step(
            new_step_with_default_assertions("copy mode paints the cursor")
                .with_keystrokes(&["k", "l", "l"])
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot(COPY_MODE_CURSOR_SCREENSHOT),
        )
        .with_step(
            new_step_with_default_assertions("visual mode paints the selection and the cursor")
                .with_keystrokes(&["v", "j", "e"])
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot(COPY_MODE_SELECTION_SCREENSHOT),
        )
        .with_step(
            new_step_with_default_assertions("escape exits copy mode and hides the badge")
                .with_keystrokes(&["escape"])
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot(COPY_MODE_EXITED_SCREENSHOT)
                .add_named_assertion("COPY badge is gone", assert_copy_mode_badge_rendered(false)),
        )
        .with_step(execute_long_running_command(
            0,
            ALT_SCREEN_COPY_MODE_COMMAND.to_owned(),
        ))
        .with_step(
            ctrl_a_bracket_on_alt_screen()
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot(COPY_MODE_ALT_SCREEN_SCREENSHOT),
        )
        .with_step(
            TestStep::new("q q leaves copy mode and less")
                .with_keystrokes(&["q", "q"])
                .add_named_assertion("alt screen is gone", assert_alt_grid_active(0, 0, false)),
        )
}

fn project_picker_paths(app: &App, window_id: WindowId) -> Vec<PathBuf> {
    command_palette_view(app, window_id).read(app, |palette, ctx| {
        palette
            .search_results(ctx)
            .filter_map(|result| match result.accept_result() {
                CommandPaletteItemAction::SwitchProject { path } => Some(path),
                _ => None,
            })
            .collect()
    })
}

pub fn test_prefix_lists_only_open_projects() -> Builder {
    FeatureFlag::ProjectSessions.set_enabled(true);
    prefix_builder()
        // The harness only isolates HOME on Unix.
        .set_should_run_test(|| cfg!(unix))
        .with_setup(|utils| {
            for name in ["Nominet", "DNSBelgium", "Unopened"] {
                std::fs::create_dir_all(utils.test_dir().join("projects").join(name))
                    .expect("create project fixture");
            }
        })
        .with_step(
            new_step_with_default_assertions("open two project sessions").with_action(
                |app, window_id, _| {
                    let workspace = workspace_view(app, window_id);
                    for name in ["Nominet", "DNSBelgium"] {
                        app.update(|ctx| {
                            ctx.dispatch_typed_action_for_view(
                                window_id,
                                workspace.id(),
                                &WorkspaceAction::SwitchProject {
                                    path: PathBuf::from(
                                        std::env::var_os("HOME").expect("test home"),
                                    )
                                    .join("projects")
                                    .join(name),
                                },
                            );
                        });
                    }
                },
            ),
        )
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            TestStep::new("ctrl-a s lists only other open sessions")
                .set_timeout(Duration::from_secs(15))
                .with_event(ctrl_a())
                .with_keystrokes(&["s"])
                .add_assertion(|app, window_id| {
                    let home = PathBuf::from(std::env::var_os("HOME").expect("test home"));
                    let paths = project_picker_paths(app, window_id);
                    async_assert!(
                        paths == vec![home.join("projects/Nominet"), home],
                        "Expected only open sessions, got {paths:?}"
                    )
                })
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot("open-project-sessions.png"),
        )
        .with_step(
            TestStep::new("ctrl-a f still discovers unopened projects")
                .with_keystrokes(&["escape"])
                .with_event(ctrl_a())
                .with_keystrokes(&["f"])
                .add_assertion(|app, window_id| {
                    let unopened = PathBuf::from(std::env::var_os("HOME").expect("test home"))
                        .join("projects/Unopened");
                    let paths = project_picker_paths(app, window_id);
                    async_assert!(
                        paths.contains(&unopened),
                        "Discovery should include {unopened:?}, got {paths:?}"
                    )
                }),
        )
        .with_step(
            TestStep::new("ctrl-a s resets discovery and switches to an open session")
                .with_keystrokes(&["escape"])
                .with_event(ctrl_a())
                .with_keystrokes(&["s"])
                .add_assertion(|app, window_id| {
                    let home = PathBuf::from(std::env::var_os("HOME").expect("test home"));
                    let paths = project_picker_paths(app, window_id);
                    async_assert!(
                        paths == vec![home.join("projects/Nominet"), home],
                        "Expected only open sessions, got {paths:?}"
                    )
                }),
        )
        .with_step(TestStep::new("select the open project").with_keystrokes(&["enter"]))
        .with_step(execute_command_successfully(
            r#"test "$PWD" = "$HOME/projects/Nominet""#,
        ))
}
