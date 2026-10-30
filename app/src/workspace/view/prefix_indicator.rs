use warp_core::features::FeatureFlag;
use warpui::elements::{
    ChildAnchor, Container, CornerRadius, OffsetPositioning, ParentAnchor, ParentOffsetBounds,
    Radius, Stack, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::geometry::vector::Vector2F;
use warpui::keymap::Keystroke;
use warpui::{AppContext, Element, SingletonEntity, ViewContext};

use super::{TAB_BAR_HOVER_HEIGHT, Workspace};
use crate::appearance::Appearance;
use crate::prefix_keys::{PrefixKeysEvent, PrefixKeysState};

const CHIP_FONT_SIZE: f32 = 12.;

impl Workspace {
    pub(super) fn subscribe_to_prefix_keys(ctx: &mut ViewContext<Self>) {
        if FeatureFlag::PrefixKeys.is_enabled() {
            ctx.subscribe_to_model(&PrefixKeysState::handle(ctx), |me, _, event, ctx| {
                me.on_prefix_keys_event(event, ctx);
            });
        }
    }

    pub(super) fn on_prefix_keys_event(
        &mut self,
        event: &PrefixKeysEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            PrefixKeysEvent::PendingChanged { window_id } => {
                if *window_id == self.window_id {
                    ctx.notify();
                }
            }
        }
    }

    pub(super) fn render_prefix_pending_chip(
        &self,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Option<Box<dyn Element>> {
        if !FeatureFlag::PrefixKeys.is_enabled() {
            return None;
        }
        let state = PrefixKeysState::as_ref(ctx);
        if !state.is_pending(self.window_id) {
            return None;
        }
        let theme = appearance.theme();
        let background = theme.accent();
        let label = Text::new_inline(
            prefix_label(state.prefix_key()),
            appearance.ui_font_family(),
            CHIP_FONT_SIZE,
        )
        .with_color(theme.active_highlighted_text_color().into_solid())
        .with_style(Properties::default().weight(Weight::Semibold))
        .finish();
        Some(
            Container::new(label)
                .with_horizontal_padding(6.)
                .with_vertical_padding(2.)
                .with_margin_left(4.)
                .with_margin_right(4.)
                .with_background(background)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .finish(),
        )
    }

    pub(super) fn add_prefix_pending_chip_overlay(
        &self,
        stack: &mut Stack,
        appearance: &Appearance,
        ctx: &AppContext,
    ) {
        if let Some(chip) = self.render_prefix_pending_chip(appearance, ctx) {
            stack.add_positioned_child(
                Container::new(chip)
                    .with_margin_top(TAB_BAR_HOVER_HEIGHT)
                    .finish(),
                OffsetPositioning::offset_from_parent(
                    Vector2F::zero(),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopRight,
                    ChildAnchor::TopRight,
                ),
            );
        }
    }
}

fn prefix_label(prefix: &Keystroke) -> String {
    let ctrl_only = prefix.ctrl && !prefix.alt && !prefix.shift && !prefix.cmd && !prefix.meta;
    if ctrl_only && prefix.key.chars().count() == 1 {
        format!("^{}", prefix.key.to_uppercase())
    } else {
        prefix.displayed()
    }
}

#[cfg(test)]
#[path = "prefix_indicator_tests.rs"]
mod tests;
