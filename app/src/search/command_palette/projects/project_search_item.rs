use std::time::Duration;

use fuzzy_match::FuzzyMatchResult;
use ordered_float::OrderedFloat;
use warpui::elements::{
    Align, ChildAnchor, ConstrainedBox, Container, CrossAxisAlignment, Empty, Expanded, Flex,
    Highlight, Hoverable, MouseStateHandle, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, Stack, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::geometry::vector::vec2f;
use warpui::text_layout::ClipConfig;
use warpui::ui_components::components::UiComponent;
use warpui::{AppContext, Element, SingletonEntity};

use super::ProjectCandidate;
use crate::appearance::Appearance;
use crate::search::command_palette::mixer::CommandPaletteItemAction;
use crate::search::command_palette::render_util;
use crate::search::item::{IconLocation, SearchItem};
use crate::search::result_renderer::ItemHighlightState;
use crate::ui_components::icons::Icon as UiIcon;

const OPEN_BADGE: &str = "Open";
const LABEL_SPACING: f32 = 8.;

pub struct ProjectSearchItem {
    pub candidate: ProjectCandidate,
    pub match_result: FuzzyMatchResult,
    pub(super) tooltip_state: MouseStateHandle,
}

fn label_text(
    text: &str,
    item_highlight_state: ItemHighlightState,
    appearance: &Appearance,
) -> Text {
    Text::new_inline(
        text.to_string(),
        appearance.ui_font_family(),
        appearance.monospace_font_size(),
    )
    .with_color(item_highlight_state.sub_text_fill(appearance).into_solid())
}

impl ProjectSearchItem {
    fn render_path(
        &self,
        item_highlight_state: ItemHighlightState,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        label_text(
            &self.candidate.display_path,
            item_highlight_state,
            appearance,
        )
        .with_clip(ClipConfig::start())
        .with_single_highlight(
            Highlight::new()
                .with_properties(Properties::default().weight(Weight::Bold))
                .with_foreground_color(
                    item_highlight_state.main_text_fill(appearance).into_solid(),
                ),
            self.match_result.matched_indices.clone(),
        )
        .finish()
    }

    fn render(
        &self,
        item_highlight_state: ItemHighlightState,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let name = label_text(&self.candidate.name, item_highlight_state, appearance)
            .with_color(item_highlight_state.main_text_fill(appearance).into_solid())
            .with_style(Properties::default().weight(Weight::Semibold))
            .with_clip(ClipConfig::ellipsis())
            .finish();
        let status = if self.candidate.is_open {
            Align::new(label_text(OPEN_BADGE, item_highlight_state, appearance).finish())
                .right()
                .finish()
        } else {
            Empty::new().finish()
        };
        let header = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Expanded::new(1., name).finish())
            .with_child(
                Container::new(
                    ConstrainedBox::new(status)
                        .with_width(48. * appearance.monospace_ui_scalar())
                        .finish(),
                )
                .with_margin_left(LABEL_SPACING)
                .finish(),
            )
            .finish();
        let path = Hoverable::new(self.tooltip_state.clone(), |mouse| {
            let path = self.render_path(item_highlight_state, appearance);
            if !mouse.is_hovered() {
                return path;
            }
            let tooltip = ConstrainedBox::new(
                appearance
                    .ui_builder()
                    .tool_tip(format!(
                        "{}\n{}",
                        self.candidate.name,
                        self.candidate.root.display()
                    ))
                    .build()
                    .finish(),
            )
            .with_max_width(480. * appearance.monospace_ui_scalar())
            .finish();
            let mut stack = Stack::new().with_child(path);
            stack.add_positioned_overlay_child(
                tooltip,
                OffsetPositioning::offset_from_parent(
                    vec2f(0., 4.),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::BottomLeft,
                    ChildAnchor::TopLeft,
                ),
            );
            stack.finish()
        })
        .with_hover_in_delay(Duration::from_millis(400))
        .finish();
        Flex::column()
            .with_child(header)
            .with_child(Container::new(path).with_margin_top(4.).finish())
            .finish()
    }
}

impl SearchItem for ProjectSearchItem {
    type Action = CommandPaletteItemAction;

    fn is_multiline(&self) -> bool {
        true
    }

    fn icon_location(&self, appearance: &Appearance) -> IconLocation {
        IconLocation::Top {
            margin_top: (appearance.line_height_ratio() - 1.) * appearance.monospace_font_size(),
        }
    }

    fn render_icon(
        &self,
        highlight_state: ItemHighlightState,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        render_util::render_search_item_icon(
            appearance,
            UiIcon::Folder,
            highlight_state.icon_fill(appearance).into_solid(),
            highlight_state,
        )
    }

    fn render_item(
        &self,
        highlight_state: ItemHighlightState,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        self.render(highlight_state, appearance)
    }

    fn render_details(&self, _ctx: &AppContext) -> Option<Box<dyn Element>> {
        None
    }

    fn score(&self) -> OrderedFloat<f64> {
        OrderedFloat(self.match_result.score as f64)
    }

    fn accept_result(&self) -> CommandPaletteItemAction {
        CommandPaletteItemAction::SwitchProject {
            path: self.candidate.root.clone(),
        }
    }

    fn execute_result(&self) -> CommandPaletteItemAction {
        self.accept_result()
    }

    fn accessibility_label(&self) -> String {
        format!(
            "Project: {}. {}",
            self.candidate.name,
            self.candidate.root.display()
        )
    }
}
