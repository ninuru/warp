use std::path::{Path, PathBuf};

use tempfile::TempDir;
use warpui::{App, EntityId, SingletonEntity, TypedActionView, ViewContext};

use super::{ProjectRef, project_name};
use crate::app_state::{
    LeafContents, LeafSnapshot, PaneNodeSnapshot, ProjectSnapshot, TabSnapshot,
    TerminalPaneSnapshot, WindowProjectsSnapshot,
};
use crate::features::FeatureFlag;
use crate::settings::import::model::ImportedConfigModel;
use crate::tab::{NewSessionMenuItem, SelectedTabColor};
use crate::terminal::session_settings::{NewSessionSource, SessionSettings, WorkingDirectoryMode};
use crate::workspace::view::Workspace;
use crate::workspace::view::tests::{initialize_app, mock_workspace, restored_workspace};
use crate::workspace::{WorkspaceAction, project_bindings};

pub(super) fn tab_ids(workspace: &Workspace) -> Vec<EntityId> {
    workspace
        .tabs
        .iter()
        .map(|tab| tab.pane_group.id())
        .collect()
}

fn group_states(
    workspace: &Workspace,
) -> Vec<(
    uuid::Uuid,
    Option<String>,
    crate::tab::SelectedTabColor,
    bool,
)> {
    let mut states: Vec<_> = workspace
        .tab_groups
        .values()
        .map(|group| (group.id.0, group.name.clone(), group.color, group.collapsed))
        .collect();
    states.sort_by_key(|state| state.0);
    states
}

pub(super) fn add_three_tabs_with_mru(workspace: &mut Workspace, ctx: &mut ViewContext<Workspace>) {
    workspace.add_terminal_tab(false, ctx);
    workspace.add_terminal_tab(false, ctx);
    workspace.handle_action(&WorkspaceAction::ActivateTab(2), ctx);
    workspace.handle_action(&WorkspaceAction::ActivateTab(0), ctx);
    workspace.handle_action(&WorkspaceAction::ActivateTab(1), ctx);
}

#[test]
fn test_project_name_replaces_dots_and_names_home() {
    assert_eq!(project_name(Path::new("/tmp/foo.bar")), "foo_bar");
    assert_eq!(project_name(Path::new("/tmp/warp")), "warp");
    let home = dirs::home_dir().expect("home dir");
    assert_eq!(project_name(&home), "~");
    assert_eq!(ProjectRef::home().root, home);
    assert_eq!(ProjectRef::home().name, "~");
}

#[test]
fn test_switch_to_new_project_hides_tabs_and_creates_one_tab_in_root() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();

        workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            let old_ids = tab_ids(workspace);

            workspace.switch_to_project(root.clone(), ctx);

            assert_eq!(workspace.tab_count(), 1);
            assert_eq!(workspace.active_tab_index, 0);
            assert!(!old_ids.contains(&workspace.tabs[0].pane_group.id()));
            assert_eq!(workspace.active_project().root, root);
            assert_eq!(
                workspace.tab_mru_order(),
                &[workspace.tabs[0].pane_group.id()]
            );
            assert_eq!(
                workspace.open_projects(),
                vec![ProjectRef::for_root(root.clone()), ProjectRef::home()]
            );

            let pane_group = workspace.tabs[0].pane_group.as_ref(ctx);
            let startup_path =
                pane_group.startup_path_for_new_session(pane_group.active_session_id(ctx), ctx);
            assert_eq!(startup_path, Some(root.clone()));
        });
    });
}

#[test]
fn test_switch_back_restores_tabs_index_mru_and_groups() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    let _grouped_tabs_guard = FeatureFlag::GroupedTabs.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();
        let home = ProjectRef::home().root;

        workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            workspace.handle_action(
                &WorkspaceAction::SelectNewSessionMenuItem(NewSessionMenuItem::CreateNewTabGroup),
                ctx,
            );
            let ids = tab_ids(workspace);
            let active_tab_index = workspace.active_tab_index;
            let mru = workspace.tab_mru_order().to_vec();
            let group_ids: Vec<_> = workspace.tabs.iter().map(|tab| tab.group_id).collect();
            for group in workspace.tab_groups.values_mut() {
                group.name = Some("api".to_string());
                group.collapsed = true;
            }
            let groups = group_states(workspace);
            assert_eq!(groups.len(), 1);

            workspace.switch_to_project(root.clone(), ctx);
            assert!(workspace.tab_groups.is_empty());

            workspace.switch_to_project(home.clone(), ctx);

            assert_eq!(tab_ids(workspace), ids);
            assert_eq!(workspace.active_tab_index, active_tab_index);
            assert_eq!(workspace.tab_mru_order(), mru.as_slice());
            assert_eq!(
                workspace
                    .tabs
                    .iter()
                    .map(|tab| tab.group_id)
                    .collect::<Vec<_>>(),
                group_ids
            );
            assert_eq!(group_states(workspace), groups);
            assert_eq!(workspace.active_project().root, home);
        });
    });
}

#[test]
fn test_hidden_pane_groups_stay_alive_across_switch() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();

        let (weak_pane_groups, weak_terminals) = workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            let weak_pane_groups: Vec<_> = workspace
                .tabs
                .iter()
                .map(|tab| tab.pane_group.downgrade())
                .collect();
            let weak_terminals: Vec<_> = workspace
                .tabs
                .iter()
                .flat_map(|tab| tab.pane_group.as_ref(ctx).terminal_views(ctx))
                .map(|terminal| terminal.downgrade())
                .collect();
            workspace.switch_to_project(root.clone(), ctx);
            (weak_pane_groups, weak_terminals)
        });
        assert_eq!(weak_terminals.len(), 3);

        app.read(|ctx| {
            for handle in &weak_pane_groups {
                assert!(handle.upgrade(ctx).is_some());
            }
            for handle in &weak_terminals {
                assert!(handle.upgrade(ctx).is_some());
            }
        });

        workspace.update(&mut app, |workspace, ctx| {
            workspace.switch_to_project(ProjectRef::home().root, ctx);
        });

        app.read(|ctx| {
            for handle in &weak_pane_groups {
                assert!(handle.upgrade(ctx).is_some());
            }
            for handle in &weak_terminals {
                assert!(handle.upgrade(ctx).is_some());
            }
        });
    });
}

#[test]
fn test_tab_views_lists_hidden_tabs_after_visible_ones() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();

        workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            let hidden_ids = tab_ids(workspace);

            workspace.switch_to_project(root.clone(), ctx);
            let visible_ids = tab_ids(workspace);

            let all: Vec<_> = workspace.tab_views().map(|view| view.id()).collect();
            let expected: Vec<_> = visible_ids.iter().chain(&hidden_ids).copied().collect();
            assert_eq!(all, expected);

            let visible: Vec<_> = workspace
                .visible_tab_views()
                .map(|view| view.id())
                .collect();
            assert_eq!(visible, visible_ids);
        });
    });
}

#[test]
fn test_switch_to_current_project_is_noop() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);

        workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            let ids = tab_ids(workspace);
            let active_tab_index = workspace.active_tab_index;
            let mru = workspace.tab_mru_order().to_vec();

            workspace.switch_to_project(ProjectRef::home().root, ctx);

            assert_eq!(tab_ids(workspace), ids);
            assert_eq!(workspace.active_tab_index, active_tab_index);
            assert_eq!(workspace.tab_mru_order(), mru.as_slice());
            assert_eq!(workspace.open_projects(), vec![ProjectRef::home()]);
        });
    });
}

#[test]
fn test_switch_with_flag_off_changes_nothing() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(false);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();

        workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            let ids = tab_ids(workspace);
            let active_tab_index = workspace.active_tab_index;
            let mru = workspace.tab_mru_order().to_vec();

            workspace.switch_to_project(root.clone(), ctx);
            workspace.switch_to_previous_project(ctx);

            assert_eq!(tab_ids(workspace), ids);
            assert_eq!(workspace.active_tab_index, active_tab_index);
            assert_eq!(workspace.tab_mru_order(), mru.as_slice());
            assert_eq!(workspace.tab_views().count(), ids.len());
            assert_eq!(
                workspace.project_startup_directory(NewSessionSource::Tab, ctx),
                None
            );
        });
    });
}

#[test]
fn test_switch_to_previous_project_toggles() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();
        let home = ProjectRef::home().root;

        workspace.update(&mut app, |workspace, ctx| {
            workspace.switch_to_previous_project(ctx);
            assert_eq!(workspace.active_project().root, home);

            workspace.switch_to_project(root.clone(), ctx);
            workspace.switch_to_previous_project(ctx);
            assert_eq!(workspace.active_project().root, home);
            workspace.switch_to_previous_project(ctx);
            assert_eq!(workspace.active_project().root, root);
            assert_eq!(workspace.open_projects().len(), 2);
        });
    });
}

#[test]
fn test_switch_to_previous_project_binding_switches() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.add_singleton_model(ImportedConfigModel::new);
        app.update(|ctx| ctx.register_editable_bindings(project_bindings()));
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();
        let home = ProjectRef::home().root;
        workspace.update(&mut app, |workspace, ctx| {
            workspace.switch_to_project(root.clone(), ctx)
        });

        let window_id = app.read(|ctx| workspace.window_id(ctx));
        let handled = app.update(|ctx| {
            ctx.dispatch_binding_by_name(window_id, "workspace:switch_to_previous_project")
        });

        assert!(handled.expect("binding dispatch failed"));
        workspace.read(&app, |workspace, _| {
            assert_eq!(workspace.active_project().root, home)
        });
    });
}

#[test]
fn test_new_tab_starts_in_project_root() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root: PathBuf = temp_root.path().to_path_buf();

        workspace.update(&mut app, |workspace, ctx| {
            workspace.switch_to_project(root.clone(), ctx);
            let window_id = ctx.window_id();
            let directory = workspace.get_new_tab_startup_directory(
                NewSessionSource::Tab,
                Some(window_id),
                None,
                ctx,
            );
            assert_eq!(directory, Some(root.clone()));
        });
    });
}

#[test]
fn test_switch_between_three_projects_keeps_others_hidden() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_b = TempDir::new().expect("failed to create temp dir");
        let temp_c = TempDir::new().expect("failed to create temp dir");
        let root_a = ProjectRef::home().root;
        let root_b = temp_b.path().to_path_buf();
        let root_c = temp_c.path().to_path_buf();

        workspace.update(&mut app, |workspace, ctx| {
            add_three_tabs_with_mru(workspace, ctx);
            let ids_a = tab_ids(workspace);
            workspace.switch_to_project(root_b.clone(), ctx);
            let ids_b = tab_ids(workspace);
            workspace.switch_to_project(root_c.clone(), ctx);
            let ids_c = tab_ids(workspace);

            workspace.switch_to_previous_project(ctx);
            assert_eq!(workspace.active_project().root, root_b);
            assert_eq!(tab_ids(workspace), ids_b);

            workspace.switch_to_project(root_a.clone(), ctx);
            assert_eq!(workspace.active_project().root, root_a);
            assert_eq!(tab_ids(workspace), ids_a);

            let hidden_roots: Vec<_> = workspace
                .open_projects()
                .into_iter()
                .skip(1)
                .map(|project| project.root)
                .collect();
            assert_eq!(hidden_roots.len(), 2);
            assert!(hidden_roots.contains(&root_b));
            assert!(hidden_roots.contains(&root_c));

            let hidden_ids: Vec<_> = workspace
                .tab_views()
                .skip(ids_a.len())
                .map(|view| view.id())
                .collect();
            assert_eq!(hidden_ids.len(), ids_b.len() + ids_c.len());
            assert!(ids_b.iter().chain(&ids_c).all(|id| hidden_ids.contains(id)));
        });
    });
}

#[test]
fn test_custom_dir_setting_wins_over_project_root() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let temp_custom = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();
        let custom = temp_custom.path().to_path_buf();
        let custom_string = custom.to_string_lossy().into_owned();

        app.update(|ctx| {
            SessionSettings::handle(ctx).update(ctx, |settings, ctx| {
                settings
                    .working_directory_config
                    .update_and_save_value(
                        |config| {
                            config.advanced_mode = false;
                            config.global.mode = WorkingDirectoryMode::CustomDir;
                            config.global.custom_dir = custom_string;
                        },
                        ctx,
                    )
                    .expect("working directory setting saves");
            });
        });

        workspace.update(&mut app, |workspace, ctx| {
            workspace.switch_to_project(root.clone(), ctx);
            assert_eq!(
                workspace.project_startup_directory(NewSessionSource::Tab, ctx),
                None
            );
            let window_id = ctx.window_id();
            let directory = workspace.get_new_tab_startup_directory(
                NewSessionSource::Tab,
                Some(window_id),
                None,
                ctx,
            );
            assert_eq!(directory, Some(custom.clone()));
        });
    });
}

#[test]
fn test_split_pane_and_home_project_keep_default_directory() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();

        workspace.update(&mut app, |workspace, ctx| {
            let window_id = ctx.window_id();
            let without_flag = workspace.get_new_tab_startup_directory(
                NewSessionSource::Tab,
                Some(window_id),
                None,
                ctx,
            );
            let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
            assert_eq!(
                workspace.project_startup_directory(NewSessionSource::Tab, ctx),
                None
            );
            let with_flag = workspace.get_new_tab_startup_directory(
                NewSessionSource::Tab,
                Some(window_id),
                None,
                ctx,
            );
            assert_eq!(with_flag, without_flag);

            workspace.switch_to_project(root.clone(), ctx);
            assert_eq!(
                workspace.project_startup_directory(NewSessionSource::SplitPane, ctx),
                None
            );
            assert_eq!(
                workspace.project_startup_directory(NewSessionSource::Tab, ctx),
                Some(root.clone())
            );
        });
    });
}

fn restored_terminal_tab(uuid: u8, cwd: &Path) -> TabSnapshot {
    TabSnapshot {
        custom_title: Some(cwd.to_string_lossy().into_owned()),
        root: PaneNodeSnapshot::Leaf(LeafSnapshot {
            is_focused: true,
            custom_vertical_tabs_title: None,
            contents: LeafContents::Terminal(TerminalPaneSnapshot {
                uuid: vec![uuid],
                cwd: Some(cwd.to_string_lossy().into_owned()),
                shell_launch_data: None,
                is_active: true,
                is_read_only: false,
                input_config: None,
                llm_model_override: None,
                active_profile_id: None,
                conversation_ids_to_restore: vec![],
                active_conversation_id: None,
            }),
        }),
        default_directory_color: None,
        selected_color: SelectedTabColor::default(),
        left_panel: None,
        right_panel: None,
        group_id: None,
        pinned: false,
    }
}

#[test]
fn test_restore_keeps_hidden_project_pending_until_first_switch() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let active_root = TempDir::new().expect("failed to create temp dir");
        let hidden_root = TempDir::new().expect("failed to create temp dir");
        let active_dir = active_root.path().to_path_buf();
        let hidden_dir = hidden_root.path().to_path_buf();
        let hidden_sub_dir = hidden_dir.join("sub");
        std::fs::create_dir(&hidden_sub_dir).expect("failed to create sub dir");

        let base = mock_workspace(&mut app);
        let mut snapshot = base.update(&mut app, |workspace, ctx| {
            workspace.snapshot(ctx.window_id(), false, ctx)
        });
        snapshot.tabs = vec![restored_terminal_tab(1, &active_dir)];
        snapshot.active_tab_index = 0;
        snapshot.tab_groups = vec![];
        snapshot.projects = Some(Box::new(WindowProjectsSnapshot {
            active: ProjectRef::for_root(active_dir.clone()),
            previous: Some(hidden_dir.clone()),
            hidden: vec![ProjectSnapshot {
                root: hidden_dir.clone(),
                name: project_name(&hidden_dir),
                tabs: vec![
                    restored_terminal_tab(2, &hidden_dir),
                    restored_terminal_tab(3, &hidden_sub_dir),
                ],
                tab_groups: vec![],
                active_tab_index: 1,
            }],
        }));

        let workspace = restored_workspace(&mut app, snapshot);
        workspace.update(&mut app, |workspace, ctx| {
            assert_eq!(workspace.tab_count(), 1);
            assert_eq!(workspace.tab_views().count(), 1);
            assert_eq!(workspace.active_project().root, active_dir);
            assert_eq!(
                workspace.open_projects(),
                vec![
                    ProjectRef::for_root(active_dir.clone()),
                    ProjectRef::for_root(hidden_dir.clone())
                ]
            );

            workspace.switch_to_previous_project(ctx);

            assert_eq!(workspace.active_project().root, hidden_dir);
            assert_eq!(workspace.tab_count(), 2);
            assert_eq!(workspace.tab_views().count(), 3);
            assert_eq!(workspace.active_tab_index, 1);
            let directories: Vec<_> = workspace
                .tabs
                .iter()
                .map(|tab| {
                    let pane_group = tab.pane_group.as_ref(ctx);
                    pane_group.startup_path_for_new_session(pane_group.active_session_id(ctx), ctx)
                })
                .collect();
            assert_eq!(
                directories,
                vec![Some(hidden_dir.clone()), Some(hidden_sub_dir.clone())]
            );
        });
    });
}
