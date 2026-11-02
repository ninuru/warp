use std::ops::Range;

use warpui::text::SelectionType;

use super::AltScreen;
use crate::terminal::model::copy_mode::{CopyModeGrid, CopyModeMotion, CopyModeState};
use crate::terminal::model::grid::Dimensions;
use crate::terminal::model::index::{Point, Side, VisibleRow};
use crate::terminal::model::selection::ScrollDelta;

impl AltScreen {
    pub fn copy_mode(&self) -> Option<CopyModeState<Point>> {
        self.copy_mode
    }

    pub fn copy_mode_cursor(&self) -> Option<Point> {
        self.copy_mode
            .and_then(|state| self.clamp_copy_mode_point(state.cursor))
    }

    pub fn enter_copy_mode(&mut self) -> Option<CopyModeState<Point>> {
        let cursor = self.clamp_copy_mode_point(self.grid_handler.cursor_point())?;
        self.clear_selection();
        self.set_copy_mode(CopyModeState::new(cursor));
        self.copy_mode
    }

    pub fn exit_copy_mode(&mut self) {
        if self.copy_mode.is_some() {
            self.clear_selection();
        }
    }

    pub fn copy_mode_target(&self, motion: CopyModeMotion) -> Option<Point> {
        let cursor = self.copy_mode_cursor()?;
        self.grid_handler.copy_mode_target(&cursor, motion)
    }

    pub fn copy_mode_move_to(&mut self, target: Option<Point>) -> Option<CopyModeState<Point>> {
        let state = self.clamped_copy_mode()?;
        let cursor = target
            .and_then(|target| self.clamp_copy_mode_point(target))
            .unwrap_or(state.cursor);
        self.set_copy_mode(state.with_cursor(cursor));
        self.copy_mode
    }

    pub fn copy_mode_toggle_visual(
        &mut self,
        selection_type: SelectionType,
    ) -> Option<CopyModeState<Point>> {
        let state = self.clamped_copy_mode()?;
        self.set_copy_mode(state.toggled_visual(selection_type));
        self.copy_mode
    }

    pub fn copy_mode_swap_anchor(&mut self) -> Option<CopyModeState<Point>> {
        let state = self.clamped_copy_mode()?;
        self.set_copy_mode(state.swapped());
        self.copy_mode
    }

    pub(super) fn rotate_copy_mode(&mut self, delta: ScrollDelta) {
        let lines = match delta {
            ScrollDelta::Up { lines } | ScrollDelta::Down { lines } => lines,
        };
        let Some(state) = self.copy_mode.filter(|_| lines > 0) else {
            return;
        };
        let region = self.grid_handler.scroll_region().clone();
        let rotate = |point| rotate_point(point, &region, delta);
        self.set_copy_mode(CopyModeState {
            cursor: rotate(state.cursor),
            anchor: state.anchor.map(rotate),
            selection_type: state.selection_type,
        });
    }

    fn clamped_copy_mode(&mut self) -> Option<CopyModeState<Point>> {
        let state = self.copy_mode?;
        let Some(cursor) = self.clamp_copy_mode_point(state.cursor) else {
            self.exit_copy_mode();
            return None;
        };
        Some(CopyModeState {
            cursor,
            anchor: state
                .anchor
                .and_then(|anchor| self.clamp_copy_mode_point(anchor)),
            selection_type: state.selection_type,
        })
    }

    fn set_copy_mode(&mut self, state: CopyModeState<Point>) {
        match state.anchor {
            None => {
                if self.selection.is_some() {
                    self.clear_selection();
                }
            }
            Some(anchor) => {
                let (anchor_side, cursor_side) = if state.cursor < anchor {
                    (Side::Right, Side::Left)
                } else {
                    (Side::Left, Side::Right)
                };
                self.start_selection(anchor, state.selection_type, anchor_side);
                self.update_selection(state.cursor, cursor_side);
            }
        }
        self.copy_mode = Some(state);
    }

    fn clamp_copy_mode_point(&self, point: Point) -> Option<Point> {
        let last_row = self.grid_handler.visible_rows().checked_sub(1)?;
        let last_column = self.grid_handler.columns().checked_sub(1)?;
        Some(Point::new(
            point.row.min(last_row),
            point.col.min(last_column),
        ))
    }
}

fn rotate_point(point: Point, region: &Range<VisibleRow>, delta: ScrollDelta) -> Point {
    let (top, bottom) = (region.start.0, region.end.0);
    if point.row < top || point.row >= bottom {
        return point;
    }
    let row = match delta {
        ScrollDelta::Up { lines } => point.row.saturating_sub(lines).max(top),
        ScrollDelta::Down { lines } => (point.row + lines).min(bottom.saturating_sub(1)),
    };
    Point::new(row, point.col)
}

#[cfg(test)]
#[path = "copy_mode_tests.rs"]
mod tests;
