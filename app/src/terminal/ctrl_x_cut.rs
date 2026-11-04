use std::sync::atomic::{AtomicBool, Ordering};

use settings::Setting as _;
use warp_core::features::FeatureFlag;
use warp_errors::report_if_error;
use warpui::keymap::FixedBinding;
use warpui::platform::OperatingSystem;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity, id};

use crate::terminal::keys_settings::KeysSettings;
use crate::terminal::view::TerminalAction;

pub const CTRL_X_CUT_CONTEXT: &str = "CtrlXCutEnabled";

static CTRL_X_CUT_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn ctrl_x_cut_enabled() -> bool {
    OperatingSystem::get().is_mac()
        || !FeatureFlag::EditCommandInEditor.is_enabled()
        || CTRL_X_CUT_ENABLED.load(Ordering::Relaxed)
}

pub fn init(ctx: &mut AppContext) {
    ctx.add_singleton_model(CtrlXCutState::new);
    ctx.register_fixed_bindings([
        FixedBinding::empty(
            "Enable Ctrl-X as Cut",
            TerminalAction::SetCtrlXCut(true),
            id!("Terminal") & !id!(CTRL_X_CUT_CONTEXT),
        )
        .with_enabled(|| !OperatingSystem::get().is_mac()),
        FixedBinding::empty(
            "Disable Ctrl-X as Cut",
            TerminalAction::SetCtrlXCut(false),
            id!("Terminal") & id!(CTRL_X_CUT_CONTEXT),
        )
        .with_enabled(|| !OperatingSystem::get().is_mac()),
    ]);
}

pub fn set_enabled(enabled: bool, ctx: &mut AppContext) {
    KeysSettings::handle(ctx).update(ctx, |settings, ctx| {
        report_if_error!(settings.ctrl_x_cut.set_value(enabled, ctx));
    });
}

pub struct CtrlXCutState;

impl CtrlXCutState {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        ctx.subscribe_to_model(&KeysSettings::handle(ctx), |_, _, _, ctx| load(ctx));
        load(ctx);
        Self
    }
}

impl Entity for CtrlXCutState {
    type Event = ();
}

impl SingletonEntity for CtrlXCutState {}

fn load(ctx: &AppContext) {
    let enabled = *KeysSettings::as_ref(ctx).ctrl_x_cut;
    CTRL_X_CUT_ENABLED.store(enabled, Ordering::Relaxed);
}

#[cfg(test)]
#[path = "ctrl_x_cut_tests.rs"]
mod tests;
