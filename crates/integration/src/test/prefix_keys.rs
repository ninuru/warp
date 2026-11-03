use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use warp::appearance::AppearanceManager;
use warp::features::FeatureFlag;
use warp::integration_testing::pane_group::assert_num_panes_in_tab;
use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::terminal::{
    assert_alt_grid_active, assert_input_editor_contents, assert_no_block_executing,
    execute_command_successfully, execute_long_running_command, wait_until_bootstrapped_pane,
    wait_until_bootstrapped_single_pane_for_tab,
};
use warp::integration_testing::view_getters::{
    command_palette_view, single_terminal_view_for_tab, workspace_view,
};
use warp::integration_testing::workspace::{assert_focused_tab_index, assert_tab_count};
use warp::search::command_palette::mixer::CommandPaletteItemAction;
use warp::themes::theme::{CustomTheme, ThemeKind};
use warp::workspace::{PaneViewLocator, WorkspaceAction};
use warpui_core::event::KeyEventDetails;
use warpui_core::geometry::rect::RectF;
use warpui_core::geometry::vector::vec2f;
use warpui_core::integration::{AssertionCallback, AssertionOutcome, TestStep};
use warpui_core::keymap::Keystroke;
use warpui_core::{App, Event, SingletonEntity, WindowId, async_assert};

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

fn switch_project(name: &'static str) -> TestStep {
    new_step_with_default_assertions(&format!("open project {name}")).with_action(
        move |app, window_id, _| {
            let workspace = workspace_view(app, window_id);
            app.update(|ctx| {
                ctx.dispatch_typed_action_for_view(
                    window_id,
                    workspace.id(),
                    &WorkspaceAction::SwitchProject {
                        path: PathBuf::from(std::env::var_os("HOME").expect("test home"))
                            .join("projects")
                            .join(name),
                    },
                )
            });
        },
    )
}

fn selected_session(app: &App, window_id: WindowId) -> Option<PaneViewLocator> {
    command_palette_view(app, window_id).read(app, |palette, ctx| {
        match palette.selected_search_result(ctx)?.accept_result() {
            CommandPaletteItemAction::NavigateToSession {
                pane_view_locator, ..
            } => Some(pane_view_locator),
            _ => None,
        }
    })
}

pub fn test_prefix_session_and_project_pickers() -> Builder {
    FeatureFlag::ProjectSessions.set_enabled(true);
    let target = Rc::new(Cell::new(None));
    let remember_target = target.clone();
    let refreshed_target = target.clone();
    let activated_target = target.clone();
    prefix_builder()
        // The harness only isolates HOME on Unix.
        .set_should_run_test(|| cfg!(unix))
        .with_setup(|utils| {
            for name in [
                "Nominet",
                "DNSBelgium",
                "Unopened",
                "api",
                "開発 — very-long-international-project-name-for-path-truncation",
            ] {
                std::fs::create_dir_all(utils.test_dir().join("projects").join(name))
                    .expect("create project fixture");
            }
            std::fs::create_dir_all(utils.test_dir().join("IdeaProjects/api"))
                .expect("create duplicate project fixture");
        })
        .with_step(
            TestStep::new("apply optional visual verification theme").with_action(
                |app, window_id, _| {
                    if std::env::var_os("WARP_PICKER_NARROW").is_some() {
                        let workspace = workspace_view(app, window_id);
                        app.update(|ctx| {
                            for _ in 0..3 {
                                ctx.dispatch_typed_action_for_view(
                                    window_id,
                                    workspace.id(),
                                    &WorkspaceAction::IncreaseFontSize,
                                );
                            }
                            ctx.windows().set_window_bounds(
                                window_id,
                                RectF::new(vec2f(0., 0.), vec2f(520., 760.)),
                            );
                        });
                    }
                    if let Some(path) = std::env::var_os("WARP_PICKER_THEME_PATH") {
                        app.update(|ctx| {
                            AppearanceManager::handle(ctx).update(ctx, |appearance, ctx| {
                                appearance.set_transient_theme(
                                    ThemeKind::Custom(CustomTheme::new(
                                        "Picker verification".into(),
                                        PathBuf::from(path),
                                    )),
                                    ctx,
                                );
                            })
                        });
                    }
                },
            ),
        )
        .with_step(switch_project("Nominet"))
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("split the first project session")
                .with_event(ctrl_a())
                .with_keystrokes(&["%"])
                .add_assertion(assert_num_panes_in_tab(0, 2)),
        )
        .with_step(wait_until_bootstrapped_pane(0, 1))
        .with_step(switch_project("DNSBelgium"))
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            TestStep::new("ctrl-a s lists individual sessions across projects")
                .with_event(ctrl_a())
                .with_keystrokes(&["s"])
                .add_assertion(|app, window_id| {
                    command_palette_view(app, window_id).read(app, |palette, ctx| {
                        let actions: Vec<_> = palette
                            .search_results(ctx)
                            .map(|result| result.accept_result())
                            .collect();
                        async_assert!(
                            actions.len() == 4
                                && actions.iter().all(|action| matches!(
                                    action,
                                    CommandPaletteItemAction::NavigateToSession { .. }
                                )),
                            "Expected four terminal sessions, got {actions:?}"
                        )
                    })
                })
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot("sessions-picker.png"),
        )
        .with_step(
            TestStep::new("filter to the hidden project's split sessions")
                .with_typed_characters(&["Nominet"])
                .add_assertion(|app, window_id| {
                    command_palette_view(app, window_id).read(app, |palette, ctx| {
                        async_assert!(
                            palette.search_results(ctx).count() == 2,
                            "Expected both hidden split sessions"
                        )
                    })
                }),
        )
        .with_step(TestStep::new("choose the second session").with_keystrokes(&["down"]))
        .with_step(
            TestStep::new("refresh preserves the selected session")
                .with_action(move |app, window_id, _| {
                    remember_target.set(selected_session(app, window_id));
                    command_palette_view(app, window_id).update(app, |palette, ctx| {
                        palette
                            .search_bar
                            .update(ctx, |bar, ctx| bar.run_query(ctx));
                    });
                })
                .add_assertion(move |app, window_id| {
                    async_assert!(
                        selected_session(app, window_id) == refreshed_target.get(),
                        "Refresh changed the selected session"
                    )
                }),
        )
        .with_step(
            TestStep::new("enter reveals the hidden project and exact split pane")
                .with_keystrokes(&["enter"])
                .add_assertion(move |app, window_id| {
                    workspace_view(app, window_id).read(app, |workspace, ctx| {
                        let group = workspace.active_tab_pane_group();
                        let actual = PaneViewLocator {
                            pane_group_id: group.id(),
                            pane_id: group.as_ref(ctx).focused_pane_id(ctx),
                        };
                        async_assert!(
                            Some(actual) == activated_target.get(),
                            "Did not focus the selected hidden pane"
                        )
                    })
                }),
        )
        .with_step(
            TestStep::new("unmatched query has no selectable session")
                .with_event(ctrl_a())
                .with_keystrokes(&["s"])
                .with_typed_characters(&["no-such-session-9384"])
                .add_assertion(|app, window_id| {
                    command_palette_view(app, window_id).read(app, |palette, ctx| {
                        async_assert!(
                            palette.search_results(ctx).next().is_none()
                                && palette.selected_search_result(ctx).is_none(),
                            "Empty state selected an action"
                        )
                    })
                })
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot("sessions-empty.png"),
        )
        .with_step(
            TestStep::new("enter on empty results keeps the picker open")
                .with_keystrokes(&["enter"])
                .add_assertion(|app, window_id| {
                    workspace_view(app, window_id).read(app, |workspace, _| {
                        async_assert!(
                            workspace.is_palette_open(),
                            "Empty results activated an action"
                        )
                    })
                }),
        )
        .with_step(
            TestStep::new("escape restores terminal input focus")
                .with_keystrokes(&["escape"])
                .with_typed_characters(&["picker-focus-restored"])
                .add_assertion(|app, window_id| {
                    workspace_view(app, window_id).read(app, |workspace, ctx| {
                        let terminal = workspace
                            .active_tab_pane_group()
                            .as_ref(ctx)
                            .focused_session_view(ctx)
                            .expect("focused terminal");
                        let contents = terminal.as_ref(ctx).input().as_ref(ctx).buffer_text(ctx);
                        async_assert!(
                            contents == "picker-focus-restored",
                            "Escape did not restore input focus: {contents:?}"
                        )
                    })
                }),
        )
        .with_step(TestStep::new("clear focus probe").with_keystrokes(&["ctrl-u"]))
        .with_step(
            TestStep::new("ctrl-a f discovers projects separately")
                .with_event(ctrl_a())
                .with_keystrokes(&["f"])
                .add_assertion(|app, window_id| {
                    let unopened = PathBuf::from(std::env::var_os("HOME").expect("test home"))
                        .join("projects/Unopened");
                    async_assert!(
                        project_picker_paths(app, window_id).contains(&unopened),
                        "Project discovery omitted unopened folders"
                    )
                })
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot("projects-picker.png"),
        )
        .with_step(
            TestStep::new("duplicate basenames retain distinct paths")
                .with_typed_characters(&["api"])
                .add_assertion(|app, window_id| {
                    let home = PathBuf::from(std::env::var_os("HOME").expect("test home"));
                    let paths = project_picker_paths(app, window_id);
                    async_assert!(
                        paths.contains(&home.join("projects/api"))
                            && paths.contains(&home.join("IdeaProjects/api")),
                        "Duplicate basenames lost their identity: {paths:?}"
                    )
                })
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot("projects-duplicates.png"),
        )
        .with_step(
            TestStep::new("long Unicode project is searchable")
                .with_keystrokes(&["ctrl-u"])
                .with_typed_characters(&["開発"])
                .add_assertion(|app, window_id| {
                    async_assert!(
                        project_picker_paths(app, window_id).len() == 1,
                        "Unicode project did not match"
                    )
                })
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot("project-long-path.png"),
        )
        .with_step(
            TestStep::new("project empty state keeps query focus")
                .with_keystrokes(&["ctrl-u"])
                .with_typed_characters(&["no-such-project-9384"])
                .add_assertion(|app, window_id| {
                    command_palette_view(app, window_id).read(app, |palette, ctx| {
                        async_assert!(
                            palette.search_results(ctx).next().is_none(),
                            "Expected empty project results"
                        )
                    })
                })
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot("projects-empty.png"),
        )
        .with_step(TestStep::new("close project picker").with_keystrokes(&["escape"]))
        .with_step(
            TestStep::new("rename project with a long Unicode name")
                .with_event(ctrl_a())
                .with_keystrokes(&["$", "ctrl-u"])
                .with_typed_characters(&[
                    "開発 — International Customer Portal with a long project name",
                ])
                .with_keystrokes(&["enter"]),
        )
        .with_step(
            TestStep::new("project chip retains status and exposes full details")
                .with_hover_over_saved_position("workspace:project_chip")
                .set_post_step_pause(Duration::from_millis(600))
                .with_take_screenshot("project-chip-details.png"),
        )
        .with_step(
            TestStep::new("prefix entry point remains responsive")
                .with_event(ctrl_a())
                .set_post_step_pause(CHIP_RENDER_PAUSE)
                .with_take_screenshot("prefix-entry.png"),
        )
        .with_step(TestStep::new("cancel prefix").with_keystrokes(&["escape"]))
        .with_step(switch_project("DNSBelgium"))
        .with_step(
            TestStep::new("renamed project matches its visible name")
                .with_event(ctrl_a())
                .with_keystrokes(&["f"])
                .with_typed_characters(&["Customer Portal"])
                .add_assertion(|app, window_id| {
                    let root = PathBuf::from(std::env::var_os("HOME").expect("test home"))
                        .join("projects/Nominet");
                    async_assert!(
                        project_picker_paths(app, window_id) == vec![root],
                        "Renamed project did not match its visible name"
                    )
                }),
        )
        .with_step(TestStep::new("close renamed project results").with_keystrokes(&["escape"]))
}
