use warpui::{App, AppContext, SingletonEntity, ViewContext, ViewHandle};

use super::{editor_command, strip_trailing_newlines};
use crate::features::FeatureFlag;
use crate::pane_group::tests::{initialize_app, mock_pane_group};
use crate::pane_group::{PaneGroup, PaneId, TerminalPaneId};
use crate::terminal::history::History;
use crate::terminal::model::bootstrap::BootstrapStage;
use crate::test_util::assert_eventually;

#[test]
fn strip_trailing_newlines_removes_only_trailing_line_breaks() {
    assert_eq!(strip_trailing_newlines("a\n"), "a");
    assert_eq!(strip_trailing_newlines("a\r\n\n"), "a");
    assert_eq!(strip_trailing_newlines(""), "");
    assert_eq!(strip_trailing_newlines("a\n b"), "a\n b");
    assert_eq!(strip_trailing_newlines("  a"), "  a");
}

#[test]
fn editor_command_quotes_path_and_falls_back_to_vi() {
    let path = std::path::Path::new("/tmp/my cmd.sh");
    assert_eq!(
        editor_command(path, Some("nvim -u NONE")),
        " nvim -u NONE '/tmp/my cmd.sh'"
    );
    assert_eq!(editor_command(path, None), " vi '/tmp/my cmd.sh'");
}

async fn ready_pane_group(app: &mut App) -> ViewHandle<PaneGroup> {
    let pane_group = mock_pane_group(app, Default::default());
    assert_eventually!(
        2000 => {
            pane_group.read(app, |panes, ctx| {
                let origin = origin_pane(panes, ctx);
                let view = panes.terminal_view_from_pane_id(origin, ctx).unwrap();
                let model = view.as_ref(ctx).model.lock();
                let block = model.block_list().active_block();
                block.bootstrap_stage() == BootstrapStage::PostBootstrapPrecmd
                    && !block.is_active_and_long_running()
                    && block.session_id().is_some_and(|id| History::as_ref(ctx).is_appendable(&id))
            })
        },
        "terminal did not finish bootstrapping"
    );
    pane_group
}

fn origin_pane(panes: &PaneGroup, ctx: &AppContext) -> TerminalPaneId {
    panes.active_session_id(ctx).expect("origin terminal pane")
}

fn buffer_text(panes: &PaneGroup, pane: TerminalPaneId, ctx: &AppContext) -> String {
    panes
        .terminal_view_from_pane_id(pane, ctx)
        .expect("terminal view")
        .as_ref(ctx)
        .input()
        .as_ref(ctx)
        .buffer_text(ctx)
}

fn set_buffer_text(
    panes: &PaneGroup,
    pane: TerminalPaneId,
    text: &str,
    ctx: &mut ViewContext<PaneGroup>,
) {
    let view = panes
        .terminal_view_from_pane_id(pane, ctx)
        .expect("terminal view");
    view.update(ctx, |view, ctx| view.apply_edited_command(text, ctx));
}

fn visible_panes(panes: &PaneGroup) -> usize {
    panes.visible_pane_count()
}

fn only_session(panes: &PaneGroup) -> (TerminalPaneId, std::path::PathBuf) {
    assert_eq!(panes.command_editor_sessions.len(), 1);
    let (editor, session) = panes
        .command_editor_sessions
        .iter()
        .next()
        .expect("command editor session");
    (*editor, session.temp_path.to_path_buf())
}

#[test]
fn successful_edit_replaces_input_in_the_same_pane() {
    let _flag = FeatureFlag::EditCommandInEditor.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let pane_group = ready_pane_group(&mut app).await;

        pane_group.update(&mut app, |panes, ctx| {
            let origin = origin_pane(panes, ctx);
            set_buffer_text(panes, origin, "echo old", ctx);

            panes.open_command_editor(origin, "echo old", None, ctx);

            assert_eq!(visible_panes(panes), 1);
            let (editor, path) = only_session(panes);
            assert_eq!(editor, origin);
            assert_eq!(buffer_text(panes, origin, ctx), "echo old");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "echo old");

            std::fs::write(&path, "echo new\n").unwrap();
            panes.finish_command_editor(editor, true, ctx);

            assert_eq!(buffer_text(panes, origin, ctx), "echo new");
            assert_eq!(visible_panes(panes), 1);
            assert!(panes.command_editor_sessions.is_empty());
            assert!(!path.exists());
        });
    })
}

#[test]
fn failed_edit_restores_original_input_in_the_same_pane() {
    let _flag = FeatureFlag::EditCommandInEditor.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let pane_group = ready_pane_group(&mut app).await;

        pane_group.update(&mut app, |panes, ctx| {
            let origin = origin_pane(panes, ctx);
            set_buffer_text(panes, origin, "echo old", ctx);

            panes.open_command_editor(origin, "echo old", None, ctx);
            let (editor, path) = only_session(panes);

            std::fs::write(&path, "echo new\n").unwrap();
            set_buffer_text(panes, origin, "", ctx);
            panes.finish_command_editor(editor, false, ctx);

            assert_eq!(buffer_text(panes, origin, ctx), "echo old");
            assert_eq!(visible_panes(panes), 1);
            assert!(panes.command_editor_sessions.is_empty());
            assert!(!path.exists());
        });
    })
}

#[test]
fn finishing_unrelated_pane_is_ignored() {
    let _flag = FeatureFlag::EditCommandInEditor.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let pane_group = ready_pane_group(&mut app).await;

        pane_group.update(&mut app, |panes, ctx| {
            let origin = origin_pane(panes, ctx);
            set_buffer_text(panes, origin, "echo old", ctx);

            panes.finish_command_editor(origin, true, ctx);

            assert_eq!(buffer_text(panes, origin, ctx), "echo old");
            assert_eq!(visible_panes(panes), 1);
        });
    })
}

#[test]
fn second_trigger_from_same_origin_reuses_editor_pane() {
    let _flag = FeatureFlag::EditCommandInEditor.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let pane_group = ready_pane_group(&mut app).await;

        pane_group.update(&mut app, |panes, ctx| {
            let origin = origin_pane(panes, ctx);

            panes.open_command_editor(origin, "echo one", None, ctx);
            panes.open_command_editor(origin, "echo two", None, ctx);

            assert_eq!(visible_panes(panes), 1);
            let (editor, path) = only_session(panes);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "echo one");
            assert_eq!(panes.focused_pane_id(ctx), PaneId::from(editor));
        });
    })
}

#[test]
fn abandoned_editor_session_is_dropped_and_origin_refocused() {
    let _flag = FeatureFlag::EditCommandInEditor.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let pane_group = ready_pane_group(&mut app).await;

        pane_group.update(&mut app, |panes, ctx| {
            let origin = origin_pane(panes, ctx);

            panes.open_command_editor(origin, "echo one", None, ctx);
            let (editor, path) = only_session(panes);
            panes.abandon_command_editor(editor);

            assert!(panes.command_editor_sessions.is_empty());
            assert!(!path.exists());
            assert_eq!(panes.focused_pane_id(ctx), PaneId::from(origin));
        });
    })
}

#[test]
fn unreadable_edited_file_restores_original_input() {
    let _flag = FeatureFlag::EditCommandInEditor.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let pane_group = ready_pane_group(&mut app).await;
        pane_group.update(&mut app, |panes, ctx| {
            let origin = origin_pane(panes, ctx);
            set_buffer_text(panes, origin, "echo old", ctx);
            panes.open_command_editor(origin, "echo old", None, ctx);
            let (editor, path) = only_session(panes);
            std::fs::remove_file(path).unwrap();
            set_buffer_text(panes, origin, "", ctx);
            panes.finish_command_editor(editor, true, ctx);
            assert_eq!(buffer_text(panes, origin, ctx), "echo old");
            assert_eq!(visible_panes(panes), 1);
        });
    })
}

#[test]
fn pending_command_is_not_replaced_by_command_editor() {
    let _flag = FeatureFlag::EditCommandInEditor.override_enabled(true);
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let pane_group = ready_pane_group(&mut app).await;
        pane_group.update(&mut app, |panes, ctx| {
            let origin = origin_pane(panes, ctx);
            set_buffer_text(panes, origin, "echo queued", ctx);
            let view = panes.terminal_view_from_pane_id(origin, ctx).unwrap();
            view.update(ctx, |view, ctx| view.set_pending_command("", ctx));
            panes.open_command_editor(origin, "echo queued", None, ctx);
            assert!(panes.command_editor_sessions.is_empty());
            assert_eq!(buffer_text(panes, origin, ctx), "echo queued");
            assert!(
                view.as_ref(ctx)
                    .has_pending_command_or_awaiting_completion(ctx)
            );
            assert_eq!(visible_panes(panes), 1);
        });
    })
}
