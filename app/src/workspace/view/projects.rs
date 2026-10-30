use std::collections::HashMap;
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use warpui::{AppContext, EntityId, SingletonEntity, ViewContext, ViewHandle, WindowId};

use super::{Workspace, is_read_only_tab_snapshot, tab_group_snapshots, tab_groups_from_snapshots};
use crate::ai::blocklist::SerializedBlockListItem;
use crate::app_state::{
    PaneUuid, ProjectSnapshot, TabGroupSnapshot, TabSnapshot, WindowProjectsSnapshot,
};
use crate::features::FeatureFlag;
use crate::pane_group::{Event, NewTerminalOptions, PaneGroup, PanesLayout};
use crate::tab::TabData;
use crate::terminal::session_settings::{NewSessionSource, SessionSettings, WorkingDirectoryMode};
use crate::undo_close::UndoCloseStack;
use crate::workspace::cross_window_tab_drag::CrossWindowTabDrag;
use crate::workspace::sync_inputs::SyncedInputState;
use crate::workspace::tab_group::{TabGroup, TabGroupId};

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectRef {
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
    Pending {
        tabs: Vec<TabSnapshot>,
        tab_groups: Vec<TabGroupSnapshot>,
        active_tab_index: usize,
        block_lists: Arc<HashMap<PaneUuid, Vec<SerializedBlockListItem>>>,
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
            ProjectTabs::Pending { .. } => Default::default(),
        }
    }

    fn live_tabs_mut(&mut self) -> impl Iterator<Item = &mut TabData> {
        match &mut self.tabs {
            ProjectTabs::Live { tabs, .. } => tabs.iter_mut(),
            ProjectTabs::Pending { .. } => Default::default(),
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
        self.activate_project(root, ctx);
    }

    pub(crate) fn switch_to_open_project(&mut self, root: PathBuf, ctx: &mut ViewContext<Self>) {
        if self.hidden_project_index(&root).is_some() {
            self.switch_to_project(root, ctx);
        }
    }

    pub(crate) fn reveal_project_for_pane_group(
        &mut self,
        pane_group_id: EntityId,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some((project_index, _)) = self.hidden_tab_position(pane_group_id) {
            let root = self.hidden_projects[project_index].project.root.clone();
            self.switch_to_project(root, ctx);
        }
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

    pub(crate) fn active_project(&self) -> &ProjectRef {
        &self.active_project
    }

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

    pub(super) fn closed_tab_project_root(&self) -> Option<PathBuf> {
        FeatureFlag::ProjectSessions
            .is_enabled()
            .then(|| self.active_project.root.clone())
    }

    pub(super) fn is_last_tab_in_window(&self) -> bool {
        self.tabs.len() == 1 && self.hidden_projects.is_empty()
    }

    pub(super) fn all_tabs_mut(&mut self) -> impl Iterator<Item = &mut TabData> {
        self.tabs.iter_mut().chain(
            self.hidden_projects
                .iter_mut()
                .flat_map(ProjectSession::live_tabs_mut),
        )
    }

    pub(super) fn close_last_tab_of_project(
        &mut self,
        add_to_undo_stack: bool,
        detach_panes_for_close: bool,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        if !FeatureFlag::ProjectSessions.is_enabled() {
            return false;
        }
        let Some(target) = self
            .previous_project
            .clone()
            .filter(|root| self.hidden_project_index(root).is_some())
            .or_else(|| {
                self.hidden_projects
                    .last()
                    .map(|session| session.project.root.clone())
            })
        else {
            return false;
        };
        let outgoing_root = self.active_project.root.clone();
        let re_adopted =
            detach_panes_for_close && self.try_re_adopt_split_off_child_agent_tab(0, ctx);

        self.activate_project(target, ctx);

        let Some(project_index) = self.hidden_project_index(&outgoing_root) else {
            return true;
        };
        let Some(tab) = self.take_hidden_tab(project_index, 0) else {
            return true;
        };
        if detach_panes_for_close && !re_adopted {
            self.detach_pane_group_for_close(&tab.pane_group, ctx);
        }
        if add_to_undo_stack && !re_adopted {
            let handle = ctx.handle();
            UndoCloseStack::handle(ctx).update(ctx, |stack, ctx| {
                stack.handle_tab_closed(handle, 0, tab, None, ctx);
            });
        }
        true
    }

    pub(super) fn consume_hidden_pane_group_event(
        &mut self,
        pane_group: &ViewHandle<PaneGroup>,
        event: &Event,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        let Some((project_index, tab_index)) = self.hidden_tab_position(pane_group.id()) else {
            return false;
        };
        match event {
            Event::Exited { .. } => {
                if let Some(tab) = self.take_hidden_tab(project_index, tab_index) {
                    self.detach_pane_group_for_close(&tab.pane_group, ctx);
                }
                ctx.dispatch_global_action("workspace:save_app", ());
                ctx.notify();
            }
            Event::AppStateChanged => {
                ctx.dispatch_global_action("workspace:save_app", ());
                self.refresh_working_directories_for_pane_group(pane_group, ctx);
                if FeatureFlag::DirectoryTabColors.is_enabled()
                    && let Some(tab) = self.hidden_projects[project_index]
                        .live_tabs_mut()
                        .nth(tab_index)
                {
                    Self::sync_codebase_tab_color(tab, ctx);
                }
            }
            Event::SendNotification { .. } => return false,
            Event::TerminalViewStateChanged => ctx.notify(),
            Event::Escape
            | Event::LeftPanelToggled { .. }
            | Event::ExecuteCommand { .. }
            | Event::PaneTitleUpdated
            | Event::OpenSettings { .. }
            | Event::OpenAutoReloadModal { .. }
            | Event::AskAIAssistant { .. }
            | Event::SyncInput { .. }
            | Event::ShowCommandSearch { .. }
            | Event::OnboardingTutorialCompleted
            | Event::OpenWorkflowModalWithCommand { .. }
            | Event::OpenCloudWorkflowForEdit { .. }
            | Event::OpenDriveObjectShareDialog { .. }
            | Event::OpenWorkflowModalWithTemporary { .. }
            | Event::OpenPromptEditor
            | Event::OpenAgentToolbarEditor
            | Event::OpenCLIAgentToolbarEditor
            | Event::OpenFileInWarp { .. }
            | Event::OpenWarpDriveLink { .. }
            | Event::OpenCodeDiff { .. }
            | Event::OpenCodeReviewPane { .. }
            | Event::ToggleCodeReviewPane { .. }
            | Event::RunWorkflow { .. }
            | Event::InvokeEnvVarCollection { .. }
            | Event::CloseSharedSessionPaneRequested { .. }
            | Event::MaximizePaneToggled
            | Event::RemoteRepoNavigated { .. }
            | Event::ActiveSessionChanged
            | Event::FocusPaneGroup
            | Event::FocusPane { .. }
            | Event::FocusPaneInWorkspace { .. }
            | Event::ViewInWarpDrive { .. }
            | Event::MoveToSpace { .. }
            | Event::PaneFocused
            | Event::DroppedOnTabBar { .. }
            | Event::SwitchTabFocusAndMovePane { .. }
            | Event::UpdateHoveredTabIndex { .. }
            | Event::ClearHoveredTabIndex
            | Event::OpenWarpDriveObjectInPane { .. }
            | Event::OpenChildAgentInNewTab { .. }
            | Event::OpenSuggestedAgentModeWorkflowModal { .. }
            | Event::OpenSuggestedRuleModal { .. }
            | Event::OpenAIFactCollection { .. }
            | Event::AnonymousUserSignup
            | Event::OpenPalette { .. }
            | Event::FileUploadCommand { .. }
            | Event::FileUploadPasswordPending { .. }
            | Event::FileUploadFinished { .. }
            | Event::OpenFileUploadSession { .. }
            | Event::TerminateFileUploadSession { .. }
            | Event::ShowToast { .. }
            | Event::SignupAnonymousUser { .. }
            | Event::OpenThemeChooser
            | Event::InvalidatedActiveConversation
            | Event::OpenConversationHistory
            | Event::OpenMCPSettingsPage { .. }
            | Event::OpenAddPromptPane { .. }
            | Event::OpenAddRulePane
            | Event::OpenEnvironmentManagementPane
            | Event::OpenFilesPalette { .. }
            | Event::ToggleLeftPanel { .. }
            | Event::OpenAgentProfileEditor { .. }
            | Event::RepoChanged
            | Event::AttachPathAsContext { .. }
            | Event::AttachPlanAsContext { .. }
            | Event::CDToDirectory { .. }
            | Event::OpenDirectoryInNewTab { .. }
            | Event::InsertCodeReviewComments { .. }
            | Event::OpenCodeReviewPaneAndScrollToComment { .. }
            | Event::ImportAllCodeReviewComments { .. }
            | Event::RunTabConfigSkill { .. }
            | Event::OpenLspLogs { .. }
            | Event::ShowCloudAgentCapacityModal { .. } => log_ignored_hidden_event(pane_group),
            #[cfg(feature = "local_fs")]
            Event::OpenCodeInWarp { .. }
            | Event::PreviewCodeInWarp { .. }
            | Event::OpenFileWithTarget { .. }
            | Event::FileRenamed { .. }
            | Event::FileDeleted { .. } => log_ignored_hidden_event(pane_group),
            #[cfg(not(target_family = "wasm"))]
            Event::OpenPluginInstructionsPane { .. } => log_ignored_hidden_event(pane_group),
        }
        true
    }

    fn activate_project(&mut self, root: PathBuf, ctx: &mut ViewContext<Self>) {
        if self.is_palette_open() {
            self.close_palette(false, None, ctx);
        }
        self.reset_tab_ui_state(ctx);

        let target = self
            .hidden_project_index(&root)
            .map(|index| self.hidden_projects.remove(index));
        let outgoing = self.take_live_project();
        self.install_project(target, root, ctx);

        self.previous_project = Some(outgoing.project.root.clone());
        self.hidden_projects.push(outgoing);

        if SyncedInputState::as_ref(ctx).is_syncing_any_inputs(ctx.window_id()) {
            self.process_updated_sync_state(ctx);
        }
        self.update_window_title(ctx);
        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    fn hidden_project_index(&self, root: &Path) -> Option<usize> {
        self.hidden_projects
            .iter()
            .position(|session| session.project.root == root)
    }

    fn hidden_tab_position(&self, pane_group_id: EntityId) -> Option<(usize, usize)> {
        self.hidden_projects
            .iter()
            .enumerate()
            .find_map(|(project_index, session)| {
                session
                    .live_tabs()
                    .position(|tab| tab.pane_group.id() == pane_group_id)
                    .map(|tab_index| (project_index, tab_index))
            })
    }

    fn take_hidden_tab(&mut self, project_index: usize, tab_index: usize) -> Option<TabData> {
        let (tab, is_empty) = match &mut self.hidden_projects[project_index].tabs {
            ProjectTabs::Live {
                tabs,
                active_tab_index,
                tab_mru_order,
                tab_groups,
            } => {
                let tab = tabs.remove(tab_index);
                let pane_group_id = tab.pane_group.id();
                tab_mru_order.retain(|id| *id != pane_group_id);
                if let Some(group_id) = tab.group_id
                    && !tabs.iter().any(|other| other.group_id == Some(group_id))
                {
                    tab_groups.remove(&group_id);
                }
                if tab_index < *active_tab_index {
                    *active_tab_index -= 1;
                } else {
                    *active_tab_index = (*active_tab_index).min(tabs.len().saturating_sub(1));
                }
                (tab, tabs.is_empty())
            }
            ProjectTabs::Pending { .. } => return None,
        };
        if is_empty {
            let session = self.hidden_projects.remove(project_index);
            if self.previous_project.as_ref() == Some(&session.project.root) {
                self.previous_project = None;
            }
        }
        Some(tab)
    }

    pub(super) fn projects_snapshot(
        &self,
        window_id: WindowId,
        app: &AppContext,
    ) -> WindowProjectsSnapshot {
        let hidden = self
            .hidden_projects
            .iter()
            .map(|session| match &session.tabs {
                ProjectTabs::Live {
                    tabs,
                    active_tab_index,
                    tab_groups,
                    ..
                } => {
                    let tabs: Vec<TabSnapshot> = tabs
                        .iter()
                        .map(|tab| self.tab_snapshot(tab, window_id, app))
                        .filter(|tab| !is_read_only_tab_snapshot(tab))
                        .collect();
                    ProjectSnapshot {
                        root: session.project.root.clone(),
                        name: session.project.name.clone(),
                        tab_groups: tab_group_snapshots(&tabs, tab_groups),
                        tabs,
                        active_tab_index: *active_tab_index,
                    }
                }
                ProjectTabs::Pending {
                    tabs,
                    tab_groups,
                    active_tab_index,
                    ..
                } => ProjectSnapshot {
                    root: session.project.root.clone(),
                    name: session.project.name.clone(),
                    tabs: tabs.clone(),
                    tab_groups: tab_groups.clone(),
                    active_tab_index: *active_tab_index,
                },
            })
            .collect();
        WindowProjectsSnapshot {
            active: self.active_project.clone(),
            previous: self.previous_project.clone(),
            hidden,
        }
    }

    pub(super) fn restore_projects(
        &mut self,
        projects: WindowProjectsSnapshot,
        block_lists: Arc<HashMap<PaneUuid, Vec<SerializedBlockListItem>>>,
    ) {
        let WindowProjectsSnapshot {
            active,
            previous,
            hidden,
        } = projects;
        self.hidden_projects = hidden
            .into_iter()
            .filter(|project| project.root != active.root)
            .map(|project| ProjectSession {
                project: ProjectRef {
                    root: project.root,
                    name: project.name,
                },
                tabs: ProjectTabs::Pending {
                    tabs: project.tabs,
                    tab_groups: project.tab_groups,
                    active_tab_index: project.active_tab_index,
                    block_lists: block_lists.clone(),
                },
            })
            .collect();
        self.previous_project = previous;
        self.active_project = active;
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
            Some(ProjectTabs::Pending {
                tabs,
                tab_groups,
                active_tab_index,
                block_lists,
            }) if !tabs.is_empty() => {
                if FeatureFlag::GroupedTabs.is_enabled() {
                    self.tab_groups = tab_groups_from_snapshots(&tab_groups);
                }
                for saved_tab in &tabs {
                    self.restore_tab_from_snapshot(saved_tab, block_lists.clone(), ctx);
                }
                self.activate_tab_internal(active_tab_index, ctx);
            }
            Some(ProjectTabs::Live { .. }) | Some(ProjectTabs::Pending { .. }) | None => {
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

fn log_ignored_hidden_event(pane_group: &ViewHandle<PaneGroup>) {
    let pane_group_id = pane_group.id();
    log::debug!("Ignoring event from hidden pane group {pane_group_id:?}");
}

#[cfg(test)]
#[path = "projects_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "projects_events_tests.rs"]
mod events_tests;
