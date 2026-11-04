use std::sync::atomic::{AtomicBool, Ordering};

use warp_core::features::FeatureFlag;
use warpui::platform::OperatingSystem;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

use crate::terminal::keys_settings::KeysSettings;

static CTRL_X_CUT_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn ctrl_x_cut_enabled() -> bool {
    OperatingSystem::get().is_mac()
        || !FeatureFlag::EditCommandInEditor.is_enabled()
        || CTRL_X_CUT_ENABLED.load(Ordering::Relaxed)
}

pub fn init(ctx: &mut AppContext) {
    ctx.add_singleton_model(CtrlXCutState::new);
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
