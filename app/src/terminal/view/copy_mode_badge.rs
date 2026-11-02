use warpui::Element;
use warpui::elements::{Container, CornerRadius, Radius, SavePosition, Text};
use warpui::fonts::{Properties, Weight};

use crate::appearance::Appearance;

const COPY_MODE_BADGE_LABEL: &str = "COPY";
const COPY_MODE_BADGE_FONT_SIZE: f32 = 12.;

pub(super) fn render_copy_mode_badge(
    position_id: &str,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let label = Text::new_inline(
        COPY_MODE_BADGE_LABEL,
        appearance.ui_font_family(),
        COPY_MODE_BADGE_FONT_SIZE,
    )
    .with_color(theme.active_highlighted_text_color().into_solid())
    .with_style(Properties::default().weight(Weight::Semibold))
    .finish();
    let badge = Container::new(label)
        .with_horizontal_padding(6.)
        .with_vertical_padding(2.)
        .with_background(theme.accent())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish();
    SavePosition::new(badge, position_id)
        .for_single_frame()
        .finish()
}
