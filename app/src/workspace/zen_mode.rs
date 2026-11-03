#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ZenPanels {
    pub vertical_tabs: bool,
    pub left: bool,
    pub right: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZenModeOptions {
    pub enter_fullscreen: bool,
    pub hide_panels: bool,
    pub follow_window_fullscreen: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZenModeTransition {
    Enter {
        request_fullscreen: bool,
        close_panels: ZenPanels,
    },
    Exit {
        leave_fullscreen: bool,
        reopen_panels: ZenPanels,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActiveZenMode {
    entered_fullscreen: bool,
    hidden_panels: ZenPanels,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ZenModeState {
    active: Option<ActiveZenMode>,
    window_fullscreen: bool,
}

impl ZenModeState {
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn toggle(
        &mut self,
        options: ZenModeOptions,
        is_fullscreen: bool,
        open_panels: ZenPanels,
    ) -> ZenModeTransition {
        if self.is_active() {
            self.exit(is_fullscreen)
        } else {
            self.enter(
                options.enter_fullscreen && !is_fullscreen,
                options,
                open_panels,
            )
        }
    }

    pub fn window_fullscreen_changed(
        &mut self,
        is_fullscreen: bool,
        options: ZenModeOptions,
        open_panels: ZenPanels,
    ) -> Option<ZenModeTransition> {
        let was_fullscreen = std::mem::replace(&mut self.window_fullscreen, is_fullscreen);
        if !options.follow_window_fullscreen || was_fullscreen == is_fullscreen {
            return None;
        }
        match (is_fullscreen, self.is_active()) {
            (true, false) => Some(self.enter(false, options, open_panels)),
            (false, true) => Some(self.exit(false)),
            (true, true) | (false, false) => None,
        }
    }

    fn enter(
        &mut self,
        request_fullscreen: bool,
        options: ZenModeOptions,
        open_panels: ZenPanels,
    ) -> ZenModeTransition {
        let close_panels = if options.hide_panels {
            open_panels
        } else {
            ZenPanels::default()
        };
        self.active = Some(ActiveZenMode {
            entered_fullscreen: request_fullscreen,
            hidden_panels: close_panels,
        });
        ZenModeTransition::Enter {
            request_fullscreen,
            close_panels,
        }
    }

    fn exit(&mut self, is_fullscreen: bool) -> ZenModeTransition {
        let active = self.active.take();
        ZenModeTransition::Exit {
            leave_fullscreen: is_fullscreen && active.is_some_and(|zen| zen.entered_fullscreen),
            reopen_panels: active.map(|zen| zen.hidden_panels).unwrap_or_default(),
        }
    }
}

pub(crate) fn zen_mode_shows_tab_bar(
    reveal_on_hover: bool,
    is_hovered: bool,
    tab_bar_needed: bool,
) -> bool {
    tab_bar_needed || (reveal_on_hover && is_hovered)
}

#[cfg(test)]
#[path = "zen_mode_tests.rs"]
mod tests;
