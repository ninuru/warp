use warpui::elements::{
    ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Flex, Hoverable,
    MainAxisSize, ParentElement, Radius, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::platform::Cursor;
use warpui::{AppContext, Element, SingletonEntity, ViewContext, ViewHandle};

use super::Workspace;
use super::projects::project_name;
use super::vertical_tabs::render_inline_tab_rename_editor;
use crate::appearance::Appearance;
use crate::editor::{EditorView, Event as EditorEvent, SingleLineEditorOptions, TextOptions};
use crate::features::FeatureFlag;
use crate::session_management::RunningSessionSummary;
use crate::workspace::WorkspaceAction;

const CHIP_FONT_SIZE: f32 = 12.;
const RENAME_EDITOR_MAX_WIDTH: f32 = 150.;
const RUNNING_DOT_SIZE: f32 = 6.;
const STATUS_MARGIN: f32 = 4.;

impl Workspace {
    pub(super) fn project_rename_editor(ctx: &mut ViewContext<Self>) -> ViewHandle<EditorView> {
        let editor = ctx.add_typed_action_view(|ctx| {
            let appearance = Appearance::as_ref(ctx);
            let options = SingleLineEditorOptions {
                text: TextOptions::ui_text(Some(CHIP_FONT_SIZE), appearance),
                ..Default::default()
            };
            EditorView::single_line(options, ctx)
        });
        ctx.subscribe_to_view(&editor, move |me, _, event, ctx| {
            me.handle_project_rename_editor_event(event, ctx);
        });
        editor
    }

    fn handle_project_rename_editor_event(
        &mut self,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if !self.is_project_being_renamed {
            return;
        }
        match event {
            EditorEvent::Enter => self.finish_project_rename(ctx),
            EditorEvent::Blurred | EditorEvent::Escape => self.cancel_project_rename(ctx),
            _ => {}
        }
    }

    pub(crate) fn start_project_rename(&mut self, ctx: &mut ViewContext<Self>) {
        if !FeatureFlag::ProjectSessions.is_enabled() {
            return;
        }
        let seed_text = self.active_project.name.clone();
        self.is_project_being_renamed = true;
        self.project_rename_editor.update(ctx, move |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
            editor.insert_selected_text(&seed_text, ctx);
        });
        ctx.focus(&self.project_rename_editor);
        ctx.notify();
    }

    fn finish_project_rename(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.is_project_being_renamed {
            return;
        }
        self.is_project_being_renamed = false;
        let text = self.project_rename_editor.as_ref(ctx).buffer_text(ctx);
        let trimmed = text.trim();
        self.active_project.name = if trimmed.is_empty() {
            project_name(&self.active_project.root)
        } else {
            trimmed.to_string()
        };
        self.clear_project_rename_editor(ctx);
        self.focus_active_tab(ctx);
        self.update_window_title(ctx);
        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    pub(super) fn cancel_project_rename(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.is_project_being_renamed {
            return;
        }
        self.is_project_being_renamed = false;
        self.clear_project_rename_editor(ctx);
        self.focus_active_tab(ctx);
        ctx.notify();
    }

    fn clear_project_rename_editor(&mut self, ctx: &mut ViewContext<Self>) {
        self.project_rename_editor.update(ctx, |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
        });
    }

    pub(super) fn project_window_title(&self, tab_title: String) -> String {
        if FeatureFlag::ProjectSessions.is_enabled() {
            format!("[{}] {tab_title}", self.active_project.name)
        } else {
            tab_title
        }
    }

    pub(super) fn update_hidden_project_running(
        &mut self,
        project_index: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        let window_id = ctx.window_id();
        let Some(session) = self.hidden_projects.get(project_index) else {
            return;
        };
        let sessions: Vec<_> = session
            .live_tabs()
            .flat_map(|tab| {
                tab.pane_group
                    .as_ref(ctx)
                    .pane_sessions(tab.pane_group.id(), window_id, ctx)
                    .collect::<Vec<_>>()
            })
            .collect();
        let running = !RunningSessionSummary::new(&sessions)
            .long_running_cmds
            .is_empty();
        let session = &mut self.hidden_projects[project_index];
        if session.running && !running {
            session.finished_unseen = true;
        }
        session.running = running;
    }

    fn background_project_status(&self) -> (usize, usize) {
        let running = self
            .hidden_projects
            .iter()
            .filter(|session| session.running)
            .count();
        let finished = self
            .hidden_projects
            .iter()
            .filter(|session| session.finished_unseen)
            .count();
        (running, finished)
    }

    pub(super) fn render_project_chip(
        &self,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Option<Box<dyn Element>> {
        if !FeatureFlag::ProjectSessions.is_enabled() {
            return None;
        }
        if self.is_project_being_renamed {
            let editor = ConstrainedBox::new(render_inline_tab_rename_editor(
                &self.project_rename_editor,
                appearance,
                ctx,
            ))
            .with_max_width(RENAME_EDITOR_MAX_WIDTH)
            .finish();
            return Some(self.project_chip_container(editor, false, appearance));
        }
        let mouse_state = self.mouse_states.project_chip.clone();
        let label = self.project_chip_label(appearance);
        let hoverable = Hoverable::new(mouse_state, move |hover_state| {
            self.project_chip_container(label, hover_state.is_hovered(), appearance)
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(WorkspaceAction::OpenProjectPicker);
        })
        .finish();
        Some(hoverable)
    }

    fn project_chip_label(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let font_family = appearance.ui_font_family();
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        row.add_child(
            Text::new_inline(
                self.active_project.name.clone(),
                font_family,
                CHIP_FONT_SIZE,
            )
            .with_color(theme.active_ui_text_color().into_solid())
            .with_style(Properties::default().weight(Weight::Semibold))
            .finish(),
        );
        let hidden_count = self.hidden_projects.len();
        if hidden_count > 0 {
            row.add_child(
                Container::new(
                    Text::new_inline(format!("+{hidden_count}"), font_family, CHIP_FONT_SIZE)
                        .with_color(theme.nonactive_ui_text_color().into_solid())
                        .finish(),
                )
                .with_margin_left(STATUS_MARGIN)
                .finish(),
            );
        }
        let (running, finished) = self.background_project_status();
        if running > 0 {
            let dot = ConstrainedBox::new(Empty::new().finish())
                .with_width(RUNNING_DOT_SIZE)
                .with_height(RUNNING_DOT_SIZE)
                .finish();
            row.add_child(
                Container::new(dot)
                    .with_background(theme.accent())
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                        RUNNING_DOT_SIZE / 2.,
                    )))
                    .with_margin_left(STATUS_MARGIN)
                    .finish(),
            );
        }
        if finished > 0 {
            row.add_child(
                Container::new(
                    Text::new_inline("!".to_string(), font_family, CHIP_FONT_SIZE)
                        .with_color(theme.accent().into_solid())
                        .with_style(Properties::default().weight(Weight::Bold))
                        .finish(),
                )
                .with_margin_left(STATUS_MARGIN)
                .finish(),
            );
        }
        row.finish()
    }

    fn project_chip_container(
        &self,
        content: Box<dyn Element>,
        is_hovered: bool,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let background = if is_hovered {
            theme.surface_3()
        } else {
            theme.surface_2()
        };
        Container::new(content)
            .with_horizontal_padding(6.)
            .with_vertical_padding(2.)
            .with_margin_left(4.)
            .with_margin_right(4.)
            .with_background(background)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .finish()
    }
}

#[cfg(test)]
#[path = "project_chip_tests.rs"]
mod tests;
