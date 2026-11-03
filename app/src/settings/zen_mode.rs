use settings::macros::define_settings_group;
use settings::{SupportedPlatforms, SyncToCloud};

define_settings_group!(ZenModeSettings, settings: [
    enter_fullscreen: ZenModeEnterFullscreen {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "zen_mode.enter_fullscreen",
        description: "Whether entering Zen Mode also makes the window fullscreen.",
    },
    reveal_tab_bar_on_hover: ZenModeRevealTabBarOnHover {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "zen_mode.reveal_tab_bar_on_hover",
        description: "Whether the hidden tab bar is revealed on hover while in Zen Mode.",
    },
    hide_panels: ZenModeHidePanels {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "zen_mode.hide_panels",
        description: "Whether Zen Mode closes open side panels and reopens them on exit.",
    },
    follow_window_fullscreen: ZenModeFollowWindowFullscreen {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "zen_mode.follow_window_fullscreen",
        description: "Whether Zen Mode follows fullscreen changes made by the window manager.",
    },
]);
