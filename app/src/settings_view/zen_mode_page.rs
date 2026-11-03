use std::cell::RefCell;
use std::collections::HashMap;

use settings::{Setting as _, SyncToCloud, ToggleableSetting as _};
use warp_errors::report_if_error;
use warpui::elements::{Element, MouseStateHandle};
use warpui::keymap::ContextPredicate;
use warpui::ui_components::components::UiComponent;
use warpui::ui_components::switch::SwitchStateHandle;
use warpui::{
    Action, AppContext, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use super::settings_page::{
    LocalOnlyIconState, MatchData, PageTitle, PageType, SettingsPageMeta, SettingsPageViewHandle,
    SettingsWidget, render_body_item,
};
use super::{SettingsAction, SettingsSection, ToggleSettingActionPair, ToggleState, flags};
use crate::appearance::Appearance;
use crate::features::FeatureFlag;
use crate::settings::{
    ZenModeEnterFullscreen, ZenModeFollowWindowFullscreen, ZenModeHidePanels,
    ZenModeRevealTabBarOnHover, ZenModeSettings,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZenModePageAction {
    EnterFullscreen,
    RevealTabBarOnHover,
    HidePanels,
    FollowWindowFullscreen,
}

impl ZenModePageAction {
    const ALL: [Self; 4] = [
        Self::EnterFullscreen,
        Self::RevealTabBarOnHover,
        Self::HidePanels,
        Self::FollowWindowFullscreen,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::EnterFullscreen => "Make the window fullscreen",
            Self::RevealTabBarOnHover => "Reveal the tab bar on hover",
            Self::HidePanels => "Hide side panels",
            Self::FollowWindowFullscreen => {
                "Enter Zen Mode when the window manager makes the window fullscreen"
            }
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::EnterFullscreen => {
                "Entering Zen Mode makes the window fullscreen, and exiting restores it."
            }
            Self::RevealTabBarOnHover => {
                "Show the hidden tab bar while the mouse is at the top of the window."
            }
            Self::HidePanels => "Close the open side panels on entry and reopen them on exit.",
            Self::FollowWindowFullscreen => {
                "For example Hyprland fullscreen mode 0. Leaving fullscreen exits Zen Mode."
            }
        }
    }

    fn command_palette_suffix(self) -> &'static str {
        match self {
            Self::EnterFullscreen => "fullscreen when entering Zen Mode",
            Self::RevealTabBarOnHover => "revealing the tab bar on hover in Zen Mode",
            Self::HidePanels => "hiding side panels in Zen Mode",
            Self::FollowWindowFullscreen => "Zen Mode for window manager fullscreen",
        }
    }

    fn search_terms(self) -> &'static str {
        match self {
            Self::EnterFullscreen => "zen mode fullscreen full screen enter",
            Self::RevealTabBarOnHover => "zen mode tab bar hover reveal show",
            Self::HidePanels => "zen mode hide side panels vertical tabs left right",
            Self::FollowWindowFullscreen => {
                "zen mode window manager fullscreen hyprland wayland follow"
            }
        }
    }

    fn widget_id(self) -> &'static str {
        match self {
            Self::EnterFullscreen => "ZenModeEnterFullscreenWidget",
            Self::RevealTabBarOnHover => "ZenModeRevealTabBarOnHoverWidget",
            Self::HidePanels => "ZenModeHidePanelsWidget",
            Self::FollowWindowFullscreen => "ZenModeFollowWindowFullscreenWidget",
        }
    }

    fn context_flag(self) -> &'static str {
        match self {
            Self::EnterFullscreen => flags::ZEN_MODE_ENTER_FULLSCREEN_FLAG,
            Self::RevealTabBarOnHover => flags::ZEN_MODE_REVEAL_TAB_BAR_ON_HOVER_FLAG,
            Self::HidePanels => flags::ZEN_MODE_HIDE_PANELS_FLAG,
            Self::FollowWindowFullscreen => flags::ZEN_MODE_FOLLOW_WINDOW_FULLSCREEN_FLAG,
        }
    }

    fn is_enabled(self, app: &AppContext) -> bool {
        let settings = ZenModeSettings::as_ref(app);
        match self {
            Self::EnterFullscreen => *settings.enter_fullscreen,
            Self::RevealTabBarOnHover => *settings.reveal_tab_bar_on_hover,
            Self::HidePanels => *settings.hide_panels,
            Self::FollowWindowFullscreen => *settings.follow_window_fullscreen,
        }
    }

    fn storage_key(self) -> &'static str {
        match self {
            Self::EnterFullscreen => ZenModeEnterFullscreen::storage_key(),
            Self::RevealTabBarOnHover => ZenModeRevealTabBarOnHover::storage_key(),
            Self::HidePanels => ZenModeHidePanels::storage_key(),
            Self::FollowWindowFullscreen => ZenModeFollowWindowFullscreen::storage_key(),
        }
    }

    fn sync_to_cloud(self) -> SyncToCloud {
        match self {
            Self::EnterFullscreen => ZenModeEnterFullscreen::sync_to_cloud(),
            Self::RevealTabBarOnHover => ZenModeRevealTabBarOnHover::sync_to_cloud(),
            Self::HidePanels => ZenModeHidePanels::sync_to_cloud(),
            Self::FollowWindowFullscreen => ZenModeFollowWindowFullscreen::sync_to_cloud(),
        }
    }
}

pub fn init_actions_from_parent_view<T: Action + Clone>(
    app: &mut AppContext,
    context: &ContextPredicate,
    builder: fn(SettingsAction) -> T,
) {
    let pairs: Vec<_> = ZenModePageAction::ALL
        .into_iter()
        .map(|action| {
            ToggleSettingActionPair::new(
                action.command_palette_suffix(),
                builder(SettingsAction::ZenModePageToggle(action)),
                context,
                action.context_flag(),
            )
            .with_enabled(|| FeatureFlag::FullScreenZenMode.is_enabled())
        })
        .collect();
    ToggleSettingActionPair::add_toggle_setting_action_pairs_as_bindings(pairs, app);
}

pub struct ZenModeSettingsPageView {
    page: PageType<Self>,
    local_only_icon_tooltip_states: RefCell<HashMap<String, MouseStateHandle>>,
}

impl ZenModeSettingsPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        ctx.subscribe_to_model(&ZenModeSettings::handle(ctx), |_, _, _, ctx| {
            ctx.notify();
        });

        let widgets: Vec<Box<dyn SettingsWidget<View = Self>>> = ZenModePageAction::ALL
            .into_iter()
            .map(|action| {
                Box::new(ZenModeToggleWidget {
                    action,
                    switch_state: SwitchStateHandle::default(),
                }) as Box<dyn SettingsWidget<View = Self>>
            })
            .collect();

        Self {
            page: PageType::new_uncategorized(widgets, Some(PageTitle::new("Zen Mode"))),
            local_only_icon_tooltip_states: RefCell::new(HashMap::new()),
        }
    }
}

impl Entity for ZenModeSettingsPageView {
    type Event = ();
}

impl TypedActionView for ZenModeSettingsPageView {
    type Action = ZenModePageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        ZenModeSettings::handle(ctx).update(ctx, |settings, ctx| {
            let result = match action {
                ZenModePageAction::EnterFullscreen => {
                    settings.enter_fullscreen.toggle_and_save_value(ctx)
                }
                ZenModePageAction::RevealTabBarOnHover => {
                    settings.reveal_tab_bar_on_hover.toggle_and_save_value(ctx)
                }
                ZenModePageAction::HidePanels => settings.hide_panels.toggle_and_save_value(ctx),
                ZenModePageAction::FollowWindowFullscreen => {
                    settings.follow_window_fullscreen.toggle_and_save_value(ctx)
                }
            };
            report_if_error!(result);
        });
        ctx.notify();
    }
}

impl View for ZenModeSettingsPageView {
    fn ui_name() -> &'static str {
        "ZenModeSettingsPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

impl SettingsPageMeta for ZenModeSettingsPageView {
    fn section() -> SettingsSection {
        SettingsSection::ZenMode
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        FeatureFlag::FullScreenZenMode.is_enabled()
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id)
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<ZenModeSettingsPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<ZenModeSettingsPageView>) -> Self {
        SettingsPageViewHandle::ZenMode(view_handle)
    }
}

struct ZenModeToggleWidget {
    action: ZenModePageAction,
    switch_state: SwitchStateHandle,
}

impl SettingsWidget for ZenModeToggleWidget {
    type View = ZenModeSettingsPageView;

    fn widget_id(&self) -> &'static str {
        self.action.widget_id()
    }

    fn search_terms(&self) -> &str {
        self.action.search_terms()
    }

    fn render(
        &self,
        view: &Self::View,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let action = self.action;
        render_body_item::<ZenModePageAction>(
            action.label().into(),
            None,
            LocalOnlyIconState::for_setting(
                action.storage_key(),
                action.sync_to_cloud(),
                &mut view.local_only_icon_tooltip_states.borrow_mut(),
                app,
            ),
            ToggleState::Enabled,
            appearance,
            appearance
                .ui_builder()
                .switch(self.switch_state.clone())
                .check(action.is_enabled(app))
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action);
                })
                .finish(),
            Some(action.description().to_owned()),
        )
    }
}
