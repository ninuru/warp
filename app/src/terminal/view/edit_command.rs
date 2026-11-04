use warpui::ViewContext;

use super::{Event, TerminalView};
use crate::features::FeatureFlag;

impl TerminalView {
    pub(super) fn edit_command_in_editor(&mut self, ctx: &mut ViewContext<Self>) {
        if !FeatureFlag::EditCommandInEditor.is_enabled() {
            return;
        }
        let text = self.input.as_ref(ctx).buffer_text(ctx);
        let editor = self
            .active_block_session_id()
            .and_then(|id| self.sessions.as_ref(ctx).get(id))
            .and_then(|session| session.editor().map(str::to_owned));
        ctx.emit(Event::EditCommandInEditor { text, editor });
    }

    pub(crate) fn apply_edited_command(&mut self, text: &str, ctx: &mut ViewContext<Self>) {
        self.input.update(ctx, |input, ctx| {
            input.replace_buffer_content(text, ctx);
            ctx.focus_self();
        });
    }
}

#[cfg(test)]
#[path = "edit_command_tests.rs"]
mod tests;
