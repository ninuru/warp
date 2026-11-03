use warp_core::ui::appearance::Appearance;

use super::{ItemHighlightState, QueryResultRendererStyles};

#[test]
fn neutral_selection_stays_selected_when_hovered() {
    let appearance = Appearance::mock();
    let selected = ItemHighlightState::Selected { is_hovered: true }.with_neutral_surface();

    assert!(selected.is_selected());
    assert!(selected.is_hovered());
    assert_eq!(
        selected.container_background_fill(&appearance),
        Some(appearance.theme().surface_3())
    );
    assert_eq!(
        selected.main_text_fill(&appearance),
        appearance
            .theme()
            .main_text_color(appearance.theme().surface_3())
    );
    assert_eq!(
        selected.sub_text_fill(&appearance),
        appearance
            .theme()
            .sub_text_color(appearance.theme().surface_3())
    );
}

#[test]
fn neutral_rows_distinguish_hover_from_rest() {
    let appearance = Appearance::mock();
    let hovered = ItemHighlightState::Hovered.with_neutral_surface();
    let resting = ItemHighlightState::Default.with_neutral_surface();

    assert!(!hovered.is_selected());
    assert_eq!(
        hovered.container_background_fill(&appearance),
        Some(appearance.theme().surface_2())
    );
    assert_eq!(
        resting.container_background_fill(&appearance),
        Some(appearance.theme().surface_1())
    );
    assert_eq!(
        resting.icon_fill(&appearance),
        appearance
            .theme()
            .main_text_color(appearance.theme().surface_1())
    );
}

#[test]
fn neutral_highlights_are_opt_in() {
    let appearance = Appearance::mock();

    assert!(!QueryResultRendererStyles::default().use_neutral_highlights);
    assert_eq!(
        ItemHighlightState::Default.container_background_fill(&appearance),
        None
    );
    assert_eq!(
        ItemHighlightState::Selected { is_hovered: false }.container_background_fill(&appearance),
        Some(appearance.theme().accent().with_opacity(90))
    );
}
