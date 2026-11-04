use warpui::App;

use crate::test_util::terminal::{add_window_with_terminal, initialize_app_for_terminal_view};

#[test]
fn apply_edited_command_replaces_buffer_without_executing() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        terminal.update(&mut app, |view, ctx| {
            let blocks_before = view.model.lock().block_list().blocks().len();
            view.input().update(ctx, |input, ctx| {
                input.replace_buffer_content("echo old", ctx);
            });

            view.apply_edited_command("  for f in *; do\n  echo \"$f\"\ndone", ctx);

            assert_eq!(
                view.input().as_ref(ctx).buffer_text(ctx),
                "  for f in *; do\n  echo \"$f\"\ndone"
            );
            assert_eq!(view.model.lock().block_list().blocks().len(), blocks_before);
        });
    })
}
