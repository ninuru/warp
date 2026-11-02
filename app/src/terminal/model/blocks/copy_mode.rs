use std::cmp::Ordering;
use std::iter;

use vim::vim::Direction;
use warpui::text::SelectionType;
use warpui::units::{IntoLines as _, Lines};

use super::{BlockList, BlockListPoint};
use crate::terminal::GridType;
use crate::terminal::find::{BlockGridMatch, BlockListMatch};
use crate::terminal::model::copy_mode::{CopyModeGrid, CopyModeGridRow};
pub use crate::terminal::model::copy_mode::{CopyModeMotion, CopyModeState};
use crate::terminal::model::grid::Dimensions;
use crate::terminal::model::index::{Point, Side};
use crate::terminal::model::terminal_model::WithinBlock;

pub struct CopyModeViewport<F> {
    pub top: Lines,
    pub bottom: Lines,
    pub visual_row: F,
}

impl<F: Fn(Lines) -> Lines> CopyModeViewport<F> {
    fn half_page(&self) -> usize {
        (((self.bottom - self.top).as_f64() + 1.) / 2.)
            .floor()
            .max(1.) as usize
    }
}

struct BlockListCopyGrid<'a, F> {
    block_list: &'a BlockList,
    inverted_blocklist: bool,
    viewport: &'a CopyModeViewport<F>,
}

impl<F: Fn(Lines) -> Lines> CopyModeGrid for BlockListCopyGrid<'_, F> {
    type Point = WithinBlock<Point>;

    fn position(point: &WithinBlock<Point>) -> Point {
        *point.get()
    }

    fn with_position(point: &WithinBlock<Point>, position: Point) -> WithinBlock<Point> {
        with_point(*point, position)
    }

    fn grid_row(&self, row: &WithinBlock<Point>) -> Option<CopyModeGridRow<'_>> {
        let grid = self
            .block_list
            .block_at(row.block_index)?
            .grid_of_type(row.grid)?;
        Some(CopyModeGridRow {
            grid: grid.grid_handler(),
            row: row.get().row,
            rows: grid.len_displayed(),
        })
    }

    fn adjacent_row(
        &self,
        row: &WithinBlock<Point>,
        direction: Direction,
    ) -> Option<WithinBlock<Point>> {
        self.block_list
            .copy_mode_adjacent_row(row, direction, self.inverted_blocklist)
    }

    fn buffer_edge_row(
        &self,
        cursor: &WithinBlock<Point>,
        direction: Direction,
    ) -> Option<WithinBlock<Point>> {
        let edge_grid = iter::successors(Some(*cursor), |grid| match direction {
            Direction::Forward => self.block_list.seek_down_to_next_grid(
                grid.block_index,
                grid.grid,
                0,
                self.inverted_blocklist,
            ),
            Direction::Backward => self.block_list.seek_up_to_next_grid(
                grid.block_index,
                grid.grid,
                0,
                self.inverted_blocklist,
            ),
        })
        .last()?;
        let row = match direction {
            Direction::Forward => self
                .block_list
                .copy_mode_grid_rows(&edge_grid)
                .checked_sub(1)?,
            Direction::Backward => 0,
        };
        Some(with_point(edge_grid, Point::new(row, 0)))
    }

    fn viewport_rows(&self, cursor: &WithinBlock<Point>) -> Vec<WithinBlock<Point>> {
        let position = |row: &WithinBlock<Point>| {
            let block_list_row = BlockListPoint::from_within_block_point(row, self.block_list).row;
            let visual_row = (self.viewport.visual_row)(block_list_row);
            if visual_row < self.viewport.top {
                Ordering::Less
            } else if visual_row > self.viewport.bottom {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        };
        let visible_row = match position(cursor) {
            Ordering::Less => self
                .rows_from(*cursor, Direction::Forward)
                .find(|row| position(row) != Ordering::Less),
            Ordering::Equal => Some(*cursor),
            Ordering::Greater => self
                .rows_from(*cursor, Direction::Backward)
                .find(|row| position(row) != Ordering::Greater),
        };
        let Some(top) = visible_row
            .filter(|row| position(row) == Ordering::Equal)
            .and_then(|row| {
                self.rows_from(row, Direction::Backward)
                    .take_while(|row| position(row) == Ordering::Equal)
                    .last()
            })
        else {
            return Vec::new();
        };
        self.rows_from(top, Direction::Forward)
            .take_while(|row| position(row) == Ordering::Equal)
            .collect()
    }

    fn half_page(&self) -> usize {
        self.viewport.half_page()
    }
}

impl BlockList {
    pub fn copy_mode(&self) -> Option<CopyModeState<WithinBlock<Point>>> {
        self.copy_mode
    }

    pub fn copy_mode_cursor(&self) -> Option<WithinBlock<Point>> {
        self.copy_mode
            .and_then(|state| self.clamp_copy_mode_point(state.cursor))
    }

    pub fn enter_copy_mode(
        &mut self,
        inverted_blocklist: bool,
    ) -> Option<CopyModeState<WithinBlock<Point>>> {
        let cursor = self.copy_mode_start_cursor()?;
        self.clear_selection();
        self.set_copy_mode(CopyModeState::new(cursor), inverted_blocklist);
        self.copy_mode
    }

    pub fn exit_copy_mode(&mut self) {
        if self.copy_mode.is_some() {
            self.clear_selection();
        }
    }

    pub fn copy_mode_target<F: Fn(Lines) -> Lines>(
        &self,
        motion: CopyModeMotion,
        inverted_blocklist: bool,
        viewport: &CopyModeViewport<F>,
    ) -> Option<WithinBlock<Point>> {
        let cursor = self.copy_mode_cursor()?;
        BlockListCopyGrid {
            block_list: self,
            inverted_blocklist,
            viewport,
        }
        .copy_mode_target(&cursor, motion)
    }

    pub fn copy_mode_move_to(
        &mut self,
        target: Option<WithinBlock<Point>>,
        inverted_blocklist: bool,
    ) -> Option<CopyModeState<WithinBlock<Point>>> {
        let state = self.clamped_copy_mode()?;
        let cursor = target
            .and_then(|target| self.clamp_copy_mode_point(target))
            .unwrap_or(state.cursor);
        self.set_copy_mode(state.with_cursor(cursor), inverted_blocklist);
        self.copy_mode
    }

    pub fn copy_mode_match_point(
        &self,
        original_match: &BlockListMatch,
    ) -> Option<WithinBlock<Point>> {
        match original_match {
            BlockListMatch::CommandBlock(BlockGridMatch {
                grid_type,
                range,
                block_index,
                ..
            }) => {
                let grid = self.block_at(*block_index)?.grid_of_type(*grid_type)?;
                let point = grid
                    .grid_handler()
                    .maybe_translate_point_from_original_to_displayed(*range.start());
                self.clamp_copy_mode_point(WithinBlock::new(point, *block_index, *grid_type))
            }
            BlockListMatch::RichContent { .. } => None,
        }
    }

    pub fn copy_mode_toggle_visual(
        &mut self,
        selection_type: SelectionType,
        inverted_blocklist: bool,
    ) -> Option<CopyModeState<WithinBlock<Point>>> {
        let state = self.clamped_copy_mode()?;
        self.set_copy_mode(state.toggled_visual(selection_type), inverted_blocklist);
        self.copy_mode
    }

    pub fn copy_mode_swap_anchor(
        &mut self,
        inverted_blocklist: bool,
    ) -> Option<CopyModeState<WithinBlock<Point>>> {
        let state = self.clamped_copy_mode()?;
        self.set_copy_mode(state.swapped(), inverted_blocklist);
        self.copy_mode
    }

    fn copy_mode_start_cursor(&self) -> Option<WithinBlock<Point>> {
        let mut index = self.last_non_hidden_block_by_index()?;
        loop {
            let output_grid = self.block_at(index)?.output_grid();
            if !output_grid.is_empty() {
                let point = Point::new(output_grid.len_displayed().saturating_sub(1), 0);
                return self.clamp_copy_mode_point(WithinBlock::new(
                    point,
                    index,
                    GridType::Output,
                ));
            }
            index = self.prev_non_hidden_block_from_index(index)?;
        }
    }

    fn clamped_copy_mode(&mut self) -> Option<CopyModeState<WithinBlock<Point>>> {
        let state = self.copy_mode?;
        let Some(cursor) = self.clamp_copy_mode_point(state.cursor) else {
            self.exit_copy_mode();
            return None;
        };
        let anchor = state
            .anchor
            .and_then(|anchor| self.clamp_copy_mode_point(anchor));
        Some(CopyModeState {
            cursor,
            anchor,
            selection_type: state.selection_type,
        })
    }

    fn set_copy_mode(
        &mut self,
        state: CopyModeState<WithinBlock<Point>>,
        inverted_blocklist: bool,
    ) {
        match state.anchor {
            None => {
                if self.selection.is_some() {
                    self.clear_selection();
                }
            }
            Some(anchor) => {
                let cursor_point = copy_mode_point(&state.cursor, self);
                let anchor_point = copy_mode_point(&anchor, self);
                let (anchor_side, cursor_side) =
                    if is_visually_before(&state.cursor, &anchor, inverted_blocklist) {
                        (Side::Right, Side::Left)
                    } else {
                        (Side::Left, Side::Right)
                    };
                self.start_selection(anchor_point, state.selection_type, anchor_side);
                self.update_selection(cursor_point, cursor_side);
            }
        }
        self.copy_mode = Some(state);
    }

    fn clamp_copy_mode_point(&self, point: WithinBlock<Point>) -> Option<WithinBlock<Point>> {
        let grid = self.block_at(point.block_index)?.grid_of_type(point.grid)?;
        let last_row = grid.len_displayed().checked_sub(1)?;
        let last_column = grid.grid_handler().columns().checked_sub(1)?;
        let inner = point.get();
        Some(with_point(
            point,
            Point::new(inner.row.min(last_row), inner.col.min(last_column)),
        ))
    }

    fn copy_mode_adjacent_row(
        &self,
        row: &WithinBlock<Point>,
        direction: Direction,
        inverted_blocklist: bool,
    ) -> Option<WithinBlock<Point>> {
        let point = *row.get();
        let adjacent = match direction {
            Direction::Forward if point.row + 1 < self.copy_mode_grid_rows(row) => {
                Some(with_point(*row, Point::new(point.row + 1, point.col)))
            }
            Direction::Forward => self.seek_down_to_next_grid(
                row.block_index,
                row.grid,
                point.col,
                inverted_blocklist,
            ),
            Direction::Backward if point.row > 0 => {
                Some(with_point(*row, Point::new(point.row - 1, point.col)))
            }
            Direction::Backward => {
                self.seek_up_to_next_grid(row.block_index, row.grid, point.col, inverted_blocklist)
            }
        };
        adjacent.and_then(|adjacent| self.clamp_copy_mode_point(adjacent))
    }

    fn copy_mode_grid_rows(&self, point: &WithinBlock<Point>) -> usize {
        self.block_at(point.block_index)
            .and_then(|block| block.grid_of_type(point.grid))
            .map_or(0, |grid| grid.len_displayed())
    }
}

fn is_visually_before(
    point: &WithinBlock<Point>,
    other: &WithinBlock<Point>,
    inverted_blocklist: bool,
) -> bool {
    if point.block_index == other.block_index {
        point < other
    } else {
        (point.block_index < other.block_index) ^ inverted_blocklist
    }
}

fn copy_mode_point(point: &WithinBlock<Point>, block_list: &BlockList) -> BlockListPoint {
    let mut block_list_point = BlockListPoint::from_within_block_point(point, block_list);
    block_list_point.row += 0.5.into_lines();
    block_list_point
}

fn with_point(location: WithinBlock<Point>, point: Point) -> WithinBlock<Point> {
    WithinBlock::new(point, location.block_index, location.grid)
}

#[cfg(test)]
#[path = "copy_mode_tests.rs"]
mod tests;
