use warpui::{App, TypedActionView};

use crate::workspace::WorkspaceAction;
use crate::workspace::view::tests::{disable_quit_warning, initialize_app, mock_workspace};

#[test]
fn test_activate_last_active_tab_toggles_between_two_most_recent_tabs() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            workspace.add_terminal_tab(false, ctx);
            workspace.add_terminal_tab(false, ctx);

            workspace.handle_action(&WorkspaceAction::ActivateTab(0), ctx);
            workspace.handle_action(&WorkspaceAction::ActivateTab(1), ctx);
            workspace.handle_action(&WorkspaceAction::ActivateTab(2), ctx);

            workspace.handle_action(&WorkspaceAction::ActivateLastActiveTab, ctx);
            assert_eq!(workspace.active_tab_index, 1);

            workspace.handle_action(&WorkspaceAction::ActivateLastActiveTab, ctx);
            assert_eq!(workspace.active_tab_index, 2);
        });
    });
}

#[test]
fn test_activate_last_active_tab_with_single_tab_is_noop() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            assert_eq!(workspace.tab_count(), 1);

            workspace.handle_action(&WorkspaceAction::ActivateLastActiveTab, ctx);

            assert_eq!(workspace.tab_count(), 1);
            assert_eq!(workspace.active_tab_index, 0);
        });
    });
}

#[test]
fn test_activate_last_active_tab_skips_closed_tab() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.update(disable_quit_warning);

        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            workspace.add_terminal_tab(false, ctx);
            workspace.add_terminal_tab(false, ctx);

            let id_a = workspace.tabs[0].pane_group.id();
            let id_c = workspace.tabs[2].pane_group.id();

            workspace.handle_action(&WorkspaceAction::ActivateTab(0), ctx);
            workspace.handle_action(&WorkspaceAction::ActivateTab(1), ctx);
            workspace.handle_action(&WorkspaceAction::ActivateTab(2), ctx);
            workspace.handle_action(&WorkspaceAction::CloseTab(1), ctx);

            assert_eq!(workspace.tab_count(), 2);
            assert_eq!(
                workspace.tabs[workspace.active_tab_index].pane_group.id(),
                id_c
            );

            workspace.handle_action(&WorkspaceAction::ActivateLastActiveTab, ctx);
            assert_eq!(
                workspace.tabs[workspace.active_tab_index].pane_group.id(),
                id_a
            );
        });
    });
}
