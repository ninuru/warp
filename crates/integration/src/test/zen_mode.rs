use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::terminal::wait_until_bootstrapped_single_pane_for_tab;
use warp::integration_testing::view_getters::workspace_view;
use warp::workspace::WorkspaceAction;
use warpui_core::integration::{AssertionCallback, TestStep};
use warpui_core::{TypedActionView, async_assert};

use crate::Builder;

fn toggle_zen_mode() -> TestStep {
    new_step_with_default_assertions("Toggle Zen Mode").with_action(|app, window_id, _| {
        workspace_view(app, window_id).update(app, |workspace, ctx| {
            workspace.handle_action(&WorkspaceAction::ToggleZenMode, ctx);
        });
    })
}

fn assert_zen_mode(active: bool) -> AssertionCallback {
    Box::new(move |app, window_id| {
        workspace_view(app, window_id).read(app, |workspace, ctx| {
            let zen_mode_active = workspace.is_zen_mode_active();
            let tab_bar_shown = workspace.is_tab_bar_shown(ctx);
            async_assert!(
                zen_mode_active == active && tab_bar_shown != active,
                "Expected zen mode {active}, got zen mode {zen_mode_active} with tab bar shown \
                 {tab_bar_shown}"
            )
        })
    })
}

pub fn test_toggle_zen_mode_hides_and_restores_tab_bar() -> Builder {
    Builder::new()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Tab bar is shown before Zen Mode")
                .add_assertion(assert_zen_mode(false)),
        )
        .with_step(toggle_zen_mode())
        .with_step(
            new_step_with_default_assertions("Zen Mode hides the tab bar")
                .add_assertion(assert_zen_mode(true)),
        )
        .with_step(toggle_zen_mode())
        .with_step(
            new_step_with_default_assertions("Leaving Zen Mode restores the tab bar")
                .add_assertion(assert_zen_mode(false)),
        )
}
