use fuzzy_match::FuzzyMatchResult;
use ordered_float::OrderedFloat;
use warp_core::ui::theme::Fill;
use warpui::elements::{
    Align, ConstrainedBox, Container, Flex, Highlight, ParentElement, Shrinkable, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::{AppContext, Element, SingletonEntity};

use super::ProjectCandidate;
use crate::appearance::Appearance;
use crate::search::action::search_item::styles;
use crate::search::command_palette::mixer::CommandPaletteItemAction;
use crate::search::command_palette::render_util;
use crate::search::item::SearchItem;
use crate::search::result_renderer::ItemHighlightState;
use crate::ui_components::icons::Icon as UiIcon;

const OPEN_BADGE: &str = "open";
const LABEL_SPACING: f32 = 8.;

#[derive(Debug)]
pub struct ProjectSearchItem {
    pub candidate: ProjectCandidate,
    pub match_result: FuzzyMatchResult,
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
            .with_style(Properties::default().weight(Weight::Bold))
            .finish();
        let path = Container::new(self.render_path(item_highlight_state, appearance))
            .with_margin_left(LABEL_SPACING)
            .finish();

        let mut row = Flex::row();
        row.add_child(name);
        row.add_child(Shrinkable::new(1., Align::new(path).left().finish()).finish());
        if self.candidate.is_open {
            row.add_child(
                Container::new(label_text(OPEN_BADGE, item_highlight_state, appearance).finish())
                    .with_margin_left(LABEL_SPACING)
                    .finish(),
            );
        }

        ConstrainedBox::new(row.finish())
            .with_height(styles::SEARCH_ITEM_HEIGHT)
            .finish()
    }
}

impl SearchItem for ProjectSearchItem {
    type Action = CommandPaletteItemAction;

    fn render_icon(
        &self,
        highlight_state: ItemHighlightState,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let icon_color: Fill = appearance.theme().terminal_colors().normal.cyan.into();

        render_util::render_search_item_icon(
            appearance,
            UiIcon::Folder,
            icon_color.into_solid(),
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
        format!("Project: {}", self.candidate.display_path)
    }
}
