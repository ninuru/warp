use std::collections::HashMap;
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use warpui::{AppContext, EntityId, SingletonEntity, ViewContext};

use super::Workspace;
use crate::features::FeatureFlag;
use crate::pane_group::{NewTerminalOptions, PanesLayout};
use crate::tab::TabData;
use crate::terminal::session_settings::{NewSessionSource, SessionSettings, WorkingDirectoryMode};
use crate::workspace::cross_window_tab_drag::CrossWindowTabDrag;
use crate::workspace::tab_group::{TabGroup, TabGroupId};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProjectRef {
    pub root: PathBuf,
    pub name: String,
}

pub(crate) struct ProjectSession {
    pub project: ProjectRef,
    pub tabs: ProjectTabs,
}

pub(crate) enum ProjectTabs {
    Live {
        tabs: Vec<TabData>,
        active_tab_index: usize,
        tab_mru_order: Vec<EntityId>,
        tab_groups: HashMap<TabGroupId, TabGroup>,
    },
}

pub(crate) fn project_name(root: &Path) -> String {
    if dirs::home_dir().as_deref() == Some(root) {
        return "~".to_string();
    }
    root.file_name()
        .map(|name| name.to_string_lossy().replace('.', "_"))
        .unwrap_or_else(|| root.to_string_lossy().into_owned())
}

impl ProjectRef {
    pub fn home() -> Self {
        Self::for_root(dirs::home_dir().unwrap_or_default())
    }

    pub fn for_root(root: PathBuf) -> Self {
        let name = project_name(&root);
        Self { root, name }
    }
}

impl ProjectSession {
    pub fn live_tabs(&self) -> impl Iterator<Item = &TabData> {
        match &self.tabs {
            ProjectTabs::Live { tabs, .. } => tabs.iter(),
        }
    }
}

impl Workspace {
    pub(crate) fn switch_to_project(&mut self, root: PathBuf, ctx: &mut ViewContext<Self>) {
        if !FeatureFlag::ProjectSessions.is_enabled()
            || root == self.active_project.root
            || CrossWindowTabDrag::as_ref(ctx).is_active()
        {
            return;
        }
        if self.is_palette_open() {
            self.close_palette(false, None, ctx);
        }
        self.reset_tab_ui_state(ctx);

        let target = self
            .hidden_projects
            .iter()
            .position(|session| session.project.root == root)
            .map(|index| self.hidden_projects.remove(index));
        let outgoing = self.take_live_project();
        self.install_project(target, root, ctx);

        self.previous_project = Some(outgoing.project.root.clone());
        self.hidden_projects.push(outgoing);

        self.update_window_title(ctx);
        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub(crate) fn switch_to_previous_project(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(previous) = self.previous_project.clone() else {
            return;
        };
        if self
            .hidden_projects
            .iter()
            .any(|session| session.project.root == previous)
        {
            self.switch_to_project(previous, ctx);
        }
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub(crate) fn active_project(&self) -> &ProjectRef {
        &self.active_project
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub(crate) fn open_projects(&self) -> Vec<ProjectRef> {
        std::iter::once(self.active_project.clone())
            .chain(
                self.hidden_projects
                    .iter()
                    .map(|session| session.project.clone()),
            )
            .collect()
    }

    pub(super) fn project_startup_directory(
        &self,
        new_session_source: NewSessionSource,
        ctx: &AppContext,
    ) -> Option<PathBuf> {
        let root = &self.active_project.root;
        let mode = SessionSettings::as_ref(ctx)
            .working_directory_config
            .config_for_source(new_session_source)
            .mode;
        let applies = FeatureFlag::ProjectSessions.is_enabled()
            && new_session_source != NewSessionSource::SplitPane
            && mode == WorkingDirectoryMode::PreviousDir
            && !root.as_os_str().is_empty()
            && Some(root) != dirs::home_dir().as_ref();
        applies.then(|| root.clone())
    }

    fn take_live_project(&mut self) -> ProjectSession {
        ProjectSession {
            project: self.active_project.clone(),
            tabs: ProjectTabs::Live {
                tabs: mem::take(&mut self.tabs),
                active_tab_index: mem::take(&mut self.active_tab_index),
                tab_mru_order: mem::take(&mut self.tab_mru_order),
                tab_groups: mem::take(&mut self.tab_groups),
            },
        }
    }

    fn install_project(
        &mut self,
        session: Option<ProjectSession>,
        root: PathBuf,
        ctx: &mut ViewContext<Self>,
    ) {
        self.active_project = session
            .as_ref()
            .map(|session| session.project.clone())
            .unwrap_or_else(|| ProjectRef::for_root(root.clone()));
        match session.map(|session| session.tabs) {
            Some(ProjectTabs::Live {
                tabs,
                active_tab_index,
                tab_mru_order,
                tab_groups,
            }) if !tabs.is_empty() => {
                self.tabs = tabs;
                self.tab_mru_order = tab_mru_order;
                self.tab_groups = tab_groups;
                self.activate_tab_internal(active_tab_index, ctx);
            }
            Some(ProjectTabs::Live { .. }) | None => {
                self.add_tab_with_pane_layout(
                    PanesLayout::SingleTerminal(Box::new(NewTerminalOptions {
                        initial_directory: Some(root),
                        hide_homepage: true,
                        ..Default::default()
                    })),
                    Arc::new(HashMap::new()),
                    None,
                    ctx,
                );
            }
        }
    }

    fn reset_tab_ui_state(&mut self, ctx: &mut ViewContext<Self>) {
        self.cancel_tab_rename(ctx);
        self.cancel_tab_group_rename(ctx);
        self.hovered_tab_index = None;
        self.tab_fixed_width = None;
        self.show_tab_right_click_menu = None;
        self.show_tab_group_right_click_menu = None;
        self.show_tab_selection_right_click_menu = None;
        self.clear_tab_multi_selection(ctx);
        for tab in &self.tabs {
            self.vertical_tabs_panel
                .clear_detail_sidecar_if_for_pane_group(tab.pane_group.id());
        }
    }
}

#[cfg(test)]
#[path = "projects_tests.rs"]
mod tests;
