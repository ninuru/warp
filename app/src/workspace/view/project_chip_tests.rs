use tempfile::TempDir;
use warpui::{App, SingletonEntity};

use crate::appearance::Appearance;
use crate::features::FeatureFlag;
use crate::workspace::view::projects::project_name;
use crate::workspace::view::tests::{initialize_app, mock_workspace};

#[test]
fn test_project_chip_hidden_with_flag_off() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(false);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        workspace.read(&app, |workspace, ctx| {
            assert!(
                workspace
                    .render_project_chip(Appearance::as_ref(ctx), ctx)
                    .is_none()
            );
            let title = workspace.window_title(ctx).expect("window title");
            assert!(!title.starts_with('['), "{title}");
        });
    });
}

#[test]
fn test_window_title_and_chip_show_active_project() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();
        workspace.update(&mut app, |workspace, ctx| {
            workspace.switch_to_project(root.clone(), ctx)
        });

        workspace.read(&app, |workspace, ctx| {
            assert!(
                workspace
                    .render_project_chip(Appearance::as_ref(ctx), ctx)
                    .is_some()
            );
            let title = workspace.window_title(ctx).expect("window title");
            let expected = format!("[{}] ", project_name(&root));
            assert!(title.starts_with(&expected), "{title}");
        });
    });
}

#[test]
fn test_rename_project_sets_name_and_empty_reverts() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let temp_root = TempDir::new().expect("failed to create temp dir");
        let root = temp_root.path().to_path_buf();
        let window_id = app.read(|ctx| workspace.window_id(ctx));

        workspace.update(&mut app, |workspace, ctx| {
            workspace.switch_to_project(root.clone(), ctx);

            workspace.start_project_rename(ctx);
            assert!(workspace.is_project_being_renamed);
            workspace.project_rename_editor.update(ctx, |editor, ctx| {
                editor.set_buffer_text("renamed", ctx);
            });
            workspace.finish_project_rename(ctx);
            assert!(!workspace.is_project_being_renamed);
            assert_eq!(workspace.active_project().name, "renamed");
            assert_eq!(
                workspace.projects_snapshot(window_id, ctx).active.name,
                "renamed"
            );
            let title = workspace.window_title(ctx).expect("window title");
            assert!(title.starts_with("[renamed] "), "{title}");

            workspace.start_project_rename(ctx);
            workspace.project_rename_editor.update(ctx, |editor, ctx| {
                editor.set_buffer_text("  ", ctx);
            });
            workspace.finish_project_rename(ctx);
            assert_eq!(workspace.active_project().name, project_name(&root));
        });
    });
}

#[test]
fn test_background_project_status_counts_running_and_finished() {
    let _projects_guard = FeatureFlag::ProjectSessions.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let first_root = TempDir::new().expect("failed to create temp dir");
        let second_root = TempDir::new().expect("failed to create temp dir");

        workspace.update(&mut app, |workspace, ctx| {
            workspace.switch_to_project(first_root.path().to_path_buf(), ctx);
            workspace.switch_to_project(second_root.path().to_path_buf(), ctx);
            assert_eq!(workspace.background_project_status(), (0, 0));

            workspace.hidden_projects[0].running = true;
            workspace.hidden_projects[1].running = true;
            assert_eq!(workspace.background_project_status(), (2, 0));

            workspace.update_hidden_project_running(1, ctx);
            assert_eq!(workspace.background_project_status(), (1, 1));
            assert!(workspace.hidden_projects[1].finished_unseen);

            let finished_root = workspace.hidden_projects[1].project.root.clone();
            workspace.switch_to_project(finished_root, ctx);
            assert_eq!(workspace.background_project_status(), (1, 0));
        });
    });
}
