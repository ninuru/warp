use warpui::ViewContext;

use super::Workspace;

impl Workspace {
    pub(crate) fn activate_last_active_tab(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(&pane_group_id) = self.tab_mru_order.get(1) {
            self.activate_tab_by_pane_group_id(pane_group_id, ctx);
        }
    }
}

#[cfg(test)]
#[path = "last_active_tab_tests.rs"]
mod tests;
