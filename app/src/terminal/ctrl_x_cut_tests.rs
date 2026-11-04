use std::sync::atomic::Ordering;

use warp_core::features::FeatureFlag;
use warpui::App;
use warpui::platform::OperatingSystem;

use super::{CTRL_X_CUT_ENABLED, CtrlXCutState, ctrl_x_cut_enabled, set_enabled};
use crate::test_util::settings::initialize_settings_for_tests;

fn set_ctrl_x_cut(app: &mut App, enabled: bool) {
    app.update(|ctx| set_enabled(enabled, ctx));
}

#[test]
fn ctrl_x_cut_follows_setting_when_flag_enabled() {
    let _flag = FeatureFlag::EditCommandInEditor.override_enabled(true);
    CTRL_X_CUT_ENABLED.store(true, Ordering::Relaxed);
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        app.add_singleton_model(CtrlXCutState::new);
        let is_mac = OperatingSystem::get().is_mac();

        assert!(ctrl_x_cut_enabled());

        set_ctrl_x_cut(&mut app, false);
        assert_eq!(ctrl_x_cut_enabled(), is_mac);

        set_ctrl_x_cut(&mut app, true);
        assert!(ctrl_x_cut_enabled());
    });
    CTRL_X_CUT_ENABLED.store(true, Ordering::Relaxed);
}

#[test]
fn ctrl_x_cut_loads_disabled_setting_at_construction() {
    let _flag = FeatureFlag::EditCommandInEditor.override_enabled(true);
    CTRL_X_CUT_ENABLED.store(true, Ordering::Relaxed);
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        set_ctrl_x_cut(&mut app, false);
        app.add_singleton_model(CtrlXCutState::new);

        assert_eq!(ctrl_x_cut_enabled(), OperatingSystem::get().is_mac());
    });
    CTRL_X_CUT_ENABLED.store(true, Ordering::Relaxed);
}

#[test]
fn ctrl_x_cut_stays_enabled_when_flag_disabled() {
    let _flag = FeatureFlag::EditCommandInEditor.override_enabled(false);
    CTRL_X_CUT_ENABLED.store(true, Ordering::Relaxed);
    App::test((), |mut app| async move {
        initialize_settings_for_tests(&mut app);
        app.add_singleton_model(CtrlXCutState::new);

        set_ctrl_x_cut(&mut app, false);
        assert!(ctrl_x_cut_enabled());
    });
    CTRL_X_CUT_ENABLED.store(true, Ordering::Relaxed);
}
