use std::io::Write;
use std::path::Path;

use tempfile::TempPath;
use warp_errors::report_error;
use warpui::ViewContext;

use super::{PaneGroup, TerminalPaneId};

pub(super) struct CommandEditorSession {
    original_text: String,
    temp_path: TempPath,
}

fn strip_trailing_newlines(text: &str) -> &str {
    text.trim_end_matches(['\n', '\r'])
}

fn editor_command(path: &Path, editor: Option<&str>) -> String {
    let editor = editor.unwrap_or("vi");
    let path = path.to_string_lossy();
    let quoted_path = shell_words::quote(&path);
    format!(" {editor} {quoted_path}")
}

fn write_temp_file(text: &str) -> std::io::Result<TempPath> {
    let mut file = tempfile::Builder::new()
        .prefix("warp-command-")
        .suffix(".sh")
        .tempfile()?;
    file.write_all(text.as_bytes())?;
    file.flush()?;
    Ok(file.into_temp_path())
}

impl PaneGroup {
    fn is_live_pane(&self, pane: TerminalPaneId) -> bool {
        let pane_id = pane.into();
        self.pane_contents.contains_key(&pane_id) && !self.is_pane_hidden_for_close(pane_id)
    }

    pub(super) fn open_command_editor(
        &mut self,
        origin: TerminalPaneId,
        text: &str,
        editor: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) {
        let closed_editor_panes: Vec<TerminalPaneId> = self
            .command_editor_sessions
            .keys()
            .copied()
            .filter(|pane| !self.is_live_pane(*pane))
            .collect();
        for pane in closed_editor_panes {
            self.command_editor_sessions.remove(&pane);
        }

        if self.command_editor_sessions.contains_key(&origin) {
            self.focus_pane(origin.into(), true, ctx);
            return;
        }

        let temp_path = match write_temp_file(text) {
            Ok(temp_path) => temp_path,
            Err(error) => {
                log::warn!("Could not write command to a temp file for editing: {error}");
                return;
            }
        };

        let Some(editor_view) = self.terminal_view_from_pane_id(origin, ctx) else {
            report_error!("Could not get terminal view for the command editor pane");
            return;
        };
        let command = editor_command(&temp_path, editor);
        self.command_editor_sessions.insert(
            origin,
            CommandEditorSession {
                original_text: text.to_owned(),
                temp_path,
            },
        );
        let started = editor_view.update(ctx, |view, ctx| view.run_command_editor(&command, ctx));
        if !started {
            self.command_editor_sessions.remove(&origin);
        }
    }

    pub(super) fn finish_command_editor(
        &mut self,
        editor_pane: TerminalPaneId,
        succeeded: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(session) = self.command_editor_sessions.remove(&editor_pane) else {
            return;
        };

        let text = if succeeded {
            match std::fs::read_to_string(&session.temp_path) {
                Ok(content) => strip_trailing_newlines(&content).to_owned(),
                Err(error) => {
                    log::warn!("Could not read the edited command back: {error}");
                    session.original_text
                }
            }
        } else {
            session.original_text
        };
        if let Some(view) = self.terminal_view_from_pane_id(editor_pane, ctx) {
            view.update(ctx, |view, ctx| view.apply_edited_command(&text, ctx));
        }
    }

    pub(super) fn abandon_command_editor(&mut self, editor_pane: TerminalPaneId) {
        self.command_editor_sessions.remove(&editor_pane);
    }
}

#[cfg(test)]
#[path = "command_editor_tests.rs"]
mod tests;
