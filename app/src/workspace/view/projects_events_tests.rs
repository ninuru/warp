use tempfile::TempDir;
use warpui::{App, SingletonEntity, TypedActionView, ViewContext};

use super::ProjectRef;
use super::tests::{add_three_tabs_with_mru, tab_ids};
use crate::features::FeatureFlag;
use crate::pane_group::Event;
use crate::tab::NewSessionMenuItem;
use crate::undo_close::UndoCloseStack;
use crate::workspace::view::Workspace;
use crate::workspace::view::tests::{initialize_app, mock_workspace};
use crate::workspace::{PaneViewLocator, WorkspaceAction};

fn exit_tab(workspace: &mut Workspace, pane_group_index: usize, ctx: &mut ViewContext<Workspace>) {
    let pane_group = workspace
        .tab_views()
        .nth(pane_group_index)
        .cloned()
        .expect("pane group exists");
    workspace.handle_file_tree_event(
        pane_group,
        &Event::Exited {
            add_to_undo_stack: false,
        },
        ctx,
    );
}

fn first_pane_locator(
    workspace: &Workspace,
    tab_view_index: usize,
    ctx: &ViewContext<Workspace>,
) -> PaneViewLocator {
    let pane_group = workspace
        .tab_views()
        .nth(tab_view_index)
        .expect("pane group exists");
    PaneViewLocator {
        pane_group_id: pane_group.id(),
        pane_id: pane_group
            .as_ref(ctx)
            .pane_id_by_index(0)
            .expect("pane exists"),
    }
}

#[test]
fn test_hidden_tab_exit_removes_only_that_tab() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();
        let home = ProjectRef::home().root;

        workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            let ids_a = tab_ids(workspace);
            workspace.switch_to_project(root.clone(), ctx);
            let ids_b = tab_ids(workspace);

            exit_tab(workspace, ids_b.len() + 1, ctx);

            assert_eq!(tab_ids(workspace), ids_b);
            assert_eq!(workspace.tab_views().count(), ids_b.len() + 2);
            assert_eq!(workspace.open_projects().len(), 2);

            workspace.switch_to_project(home.clone(), ctx);

            assert_eq!(tab_ids(workspace), vec![ids_a[0], ids_a[2]]);
            assert_eq!(workspace.active_tab_index, 1);
            assert!(!workspace.tab_mru_order().contains(&ids_a[1]));
            assert_eq!(workspace.tab_mru_order().len(), 2);
        });
    });
}

#[test]
fn test_hidden_project_last_tab_exit_drops_project() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();

        workspace.update(&mut app, |workspace, ctx| {
            workspace.switch_to_project(root.clone(), ctx);
            let ids_b = tab_ids(workspace);

            exit_tab(workspace, ids_b.len(), ctx);

            assert_eq!(
                workspace.open_projects(),
                vec![ProjectRef::for_root(root.clone())]
            );
            assert_eq!(workspace.tab_views().count(), ids_b.len());

            workspace.switch_to_previous_project(ctx);
            assert_eq!(workspace.active_project().root, root);
            assert_eq!(tab_ids(workspace), ids_b);
        });
    });
}

#[test]
fn test_closing_last_visible_tab_switches_to_previous_project() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_b = TempDir::new().expect("failed to create temp dir");
        let temp_c = TempDir::new().expect("failed to create temp dir");
        let root_b = temp_b.path().to_path_buf();
        let root_c = temp_c.path().to_path_buf();
        let home = ProjectRef::home().root;

        workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            workspace.switch_to_project(root_b.clone(), ctx);
            let ids_b = tab_ids(workspace);
            workspace.switch_to_project(root_c.clone(), ctx);
            assert_eq!(workspace.tab_count(), 1);

            exit_tab(workspace, 0, ctx);

            assert_eq!(workspace.active_project().root, root_b);
            assert_eq!(tab_ids(workspace), ids_b);
            let open_roots: Vec<_> = workspace
                .open_projects()
                .into_iter()
                .map(|project| project.root)
                .collect();
            assert_eq!(open_roots, vec![root_b.clone(), home.clone()]);

            workspace.close_tab(0, true, false, ctx);

            assert_eq!(workspace.active_project().root, home);
            assert_eq!(workspace.tab_count(), 3);
            assert_eq!(workspace.open_projects(), vec![ProjectRef::home()]);
        });
    });
}

#[test]
fn test_activate_tab_by_pane_group_id_reveals_hidden_project() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();
        let home = ProjectRef::home().root;

        workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            let ids_a = tab_ids(workspace);
            workspace.switch_to_project(root.clone(), ctx);

            workspace.activate_tab_by_pane_group_id(ids_a[2], ctx);

            assert_eq!(workspace.active_project().root, home);
            assert_eq!(tab_ids(workspace), ids_a);
            assert_eq!(workspace.active_tab_index, 2);
        });
    });
}

#[test]
fn test_focus_pane_reveals_hidden_project() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();
        let home = ProjectRef::home().root;

        workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            workspace.switch_to_project(root.clone(), ctx);
            let locator = first_pane_locator(workspace, 1, ctx);

            workspace.focus_pane(locator, ctx);

            assert_eq!(workspace.active_project().root, home);
            assert_eq!(workspace.active_tab_index, 0);
            assert_eq!(
                workspace.active_tab_pane_group().id(),
                locator.pane_group_id
            );
        });
    });
}

#[test]
fn test_undo_close_restores_tab_into_its_project_and_group() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    let _grouped_tabs_guard = FeatureFlag::GroupedTabs.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();
        let home = ProjectRef::home().root;

        let (closed_id, group_id) = workspace.update(&mut app, |workspace, ctx| {
            workspace.add_terminal_tab(false, ctx);
            workspace.handle_action(
                &WorkspaceAction::SelectNewSessionMenuItem(NewSessionMenuItem::CreateNewTabGroup),
                ctx,
            );
            let group_id = workspace.tabs[0].group_id.expect("new tab is grouped");
            workspace.assign_tab_to_group(1, Some(group_id), ctx);
            let closed_id = workspace.tabs[0].pane_group.id();

            workspace.close_tab(0, true, true, ctx);
            assert!(!tab_ids(workspace).contains(&closed_id));
            workspace.switch_to_project(root.clone(), ctx);
            (closed_id, group_id)
        });

        app.update(|ctx| {
            UndoCloseStack::handle(ctx).update(ctx, |stack, ctx| stack.undo_close(ctx));
        });

        workspace.read(&app, |workspace, _| {
            assert_eq!(workspace.active_project().root, home);
            let restored = workspace
                .tabs
                .iter()
                .find(|tab| tab.pane_group.id() == closed_id)
                .expect("tab restored into its project");
            assert_eq!(restored.group_id, Some(group_id));
            assert!(workspace.tab_groups.contains_key(&group_id));
            assert_eq!(workspace.active_tab_pane_group().id(), closed_id);
            let hidden_roots: Vec<_> = workspace
                .open_projects()
                .into_iter()
                .skip(1)
                .map(|project| project.root)
                .collect();
            assert_eq!(hidden_roots, vec![root.clone()]);
        });
    });
}

#[test]
fn test_workspace_sessions_include_hidden_tabs() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();

        workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            let ids_a = tab_ids(workspace);
            workspace.switch_to_project(root.clone(), ctx);
            let ids_b = tab_ids(workspace);

            let session_groups: Vec<_> = workspace
                .workspace_sessions(ctx.window_id(), ctx)
                .map(|session| session.pane_view_locator().pane_group_id)
                .collect();

            for id in ids_a.iter().chain(&ids_b) {
                assert!(session_groups.contains(id));
            }
        });
    });
}

#[test]
fn test_modal_event_from_hidden_tab_is_ignored() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();

        workspace.update(&mut app, |workspace, ctx| {
            workspace.switch_to_project(root.clone(), ctx);
            let hidden = workspace
                .tab_views()
                .nth(1)
                .cloned()
                .expect("hidden pane group exists");

            workspace.handle_file_tree_event(
                hidden,
                &Event::OpenAutoReloadModal {
                    purchased_credits: 100,
                },
                ctx,
            );
            assert!(
                !workspace
                    .current_workspace_state
                    .is_enable_auto_reload_modal_open
            );
            assert_eq!(workspace.active_project().root, root);

            let visible = workspace.active_tab_pane_group().clone();
            workspace.handle_file_tree_event(
                visible,
                &Event::OpenAutoReloadModal {
                    purchased_credits: 100,
                },
                ctx,
            );
            assert!(
                workspace
                    .current_workspace_state
                    .is_enable_auto_reload_modal_open
            );
        });
    });
}

#[test]
fn test_exit_close_jump_and_undo_with_flag_off_unchanged() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(false);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);

        let closed_id = workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            let ids = tab_ids(workspace);

            exit_tab(workspace, 1, ctx);
            assert_eq!(tab_ids(workspace), vec![ids[0], ids[2]]);

            workspace.activate_tab_by_pane_group_id(ids[2], ctx);
            assert_eq!(workspace.active_tab_index, 1);

            let session_groups: Vec<_> = workspace
                .workspace_sessions(ctx.window_id(), ctx)
                .map(|session| session.pane_view_locator().pane_group_id)
                .collect();
            assert_eq!(session_groups, vec![ids[0], ids[2]]);

            workspace.close_tab(0, true, true, ctx);
            assert_eq!(tab_ids(workspace), vec![ids[2]]);
            assert_eq!(workspace.open_projects(), vec![ProjectRef::home()]);
            ids[0]
        });

        app.update(|ctx| {
            UndoCloseStack::handle(ctx).update(ctx, |stack, ctx| stack.undo_close(ctx));
        });

        workspace.read(&app, |workspace, _| {
            assert_eq!(tab_ids(workspace).len(), 2);
            assert!(tab_ids(workspace).contains(&closed_id));
            assert_eq!(workspace.open_projects(), vec![ProjectRef::home()]);
        });
    });
}
