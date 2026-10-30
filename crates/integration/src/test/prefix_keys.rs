use std::time::Duration;

use warp::features::FeatureFlag;
use warp::integration_testing::pane_group::assert_num_panes_in_tab;
use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::terminal::{
    assert_input_editor_contents, assert_no_block_executing, execute_command_successfully,
    execute_long_running_command, wait_until_bootstrapped_single_pane_for_tab,
};
use warp::integration_testing::view_getters::single_terminal_view_for_tab;
use warp::integration_testing::workspace::{assert_focused_tab_index, assert_tab_count};
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
const COPY_MODE_COMMAND: &str = "printf 'first line\\nsecond line\\n'";

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
                .add_named_assertion(
                    "input editor did not receive `q`",
                    assert_input_editor_contents(0, ""),
                ),
        )
        .with_step(ctrl_a_bracket())
        .with_step(
            new_step_with_default_assertions("escape exits copy mode")
                .with_keystrokes(&["escape"])
                .add_named_assertion("copy mode is inactive", assert_copy_mode_row(None)),
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
}
