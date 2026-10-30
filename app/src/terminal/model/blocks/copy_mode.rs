use vim::vim::{Direction, WordBound, WordType};
use vim::vim_word_iterator_from_offset;
use warp_terminal::model::grid::cell::DEFAULT_CHAR;
use warpui::text::SelectionType;
use warpui::units::IntoLines as _;

use super::{BlockList, BlockListPoint};
use crate::terminal::GridType;
use crate::terminal::model::grid::Dimensions;
use crate::terminal::model::index::{Point, Side};
use crate::terminal::model::terminal_model::WithinBlock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyModeMotion {
    Left,
    Right,
    Up,
    Down,
    WordForward,
    WordBackward,
    WordEnd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CopyModeState {
    pub cursor: WithinBlock<Point>,
    pub anchor: Option<WithinBlock<Point>>,
}

impl BlockList {
    pub fn copy_mode(&self) -> Option<CopyModeState> {
        self.copy_mode
    }

    pub fn copy_mode_cursor(&self) -> Option<WithinBlock<Point>> {
        self.copy_mode
            .and_then(|state| self.clamp_copy_mode_point(state.cursor))
    }

    pub fn enter_copy_mode(&mut self, inverted_blocklist: bool) -> Option<CopyModeState> {
        let cursor = self.copy_mode_start_cursor()?;
        self.clear_selection();
        self.set_copy_mode(
            CopyModeState {
                cursor,
                anchor: None,
            },
            inverted_blocklist,
        );
        self.copy_mode
    }

    pub fn exit_copy_mode(&mut self) {
        if self.copy_mode.is_some() {
            self.clear_selection();
        }
    }

    pub fn copy_mode_move(
        &mut self,
        motion: CopyModeMotion,
        inverted_blocklist: bool,
    ) -> Option<CopyModeState> {
        let state = self.clamped_copy_mode()?;
        let cursor = state.cursor;
        let point = *cursor.get();
        let new_cursor = match motion {
            CopyModeMotion::Left => Some(with_point(
                cursor,
                Point::new(point.row, point.col.saturating_sub(1)),
            )),
            CopyModeMotion::Right => Some(with_point(cursor, Point::new(point.row, point.col + 1))),
            CopyModeMotion::Up if point.row > 0 => {
                Some(with_point(cursor, Point::new(point.row - 1, point.col)))
            }
            CopyModeMotion::Up => self.seek_up_to_next_grid(
                cursor.block_index,
                cursor.grid,
                point.col,
                inverted_blocklist,
            ),
            CopyModeMotion::Down if point.row + 1 < self.copy_mode_grid_rows(&cursor) => {
                Some(with_point(cursor, Point::new(point.row + 1, point.col)))
            }
            CopyModeMotion::Down => self.seek_down_to_next_grid(
                cursor.block_index,
                cursor.grid,
                point.col,
                inverted_blocklist,
            ),
            CopyModeMotion::WordForward => {
                self.copy_mode_word_motion(&cursor, Direction::Forward, WordBound::Start)
            }
            CopyModeMotion::WordBackward => {
                self.copy_mode_word_motion(&cursor, Direction::Backward, WordBound::Start)
            }
            CopyModeMotion::WordEnd => {
                self.copy_mode_word_motion(&cursor, Direction::Forward, WordBound::End)
            }
        };
        let cursor = new_cursor
            .and_then(|new_cursor| self.clamp_copy_mode_point(new_cursor))
            .unwrap_or(cursor);
        self.set_copy_mode(
            CopyModeState {
                cursor,
                anchor: state.anchor,
            },
            inverted_blocklist,
        );
        self.copy_mode
    }

    pub fn copy_mode_toggle_visual(&mut self, inverted_blocklist: bool) -> Option<CopyModeState> {
        let state = self.clamped_copy_mode()?;
        let anchor = match state.anchor {
            Some(_) => None,
            None => Some(state.cursor),
        };
        self.set_copy_mode(
            CopyModeState {
                cursor: state.cursor,
                anchor,
            },
            inverted_blocklist,
        );
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

    fn clamped_copy_mode(&mut self) -> Option<CopyModeState> {
        let state = self.copy_mode?;
        let Some(cursor) = self.clamp_copy_mode_point(state.cursor) else {
            self.exit_copy_mode();
            return None;
        };
        let anchor = state
            .anchor
            .and_then(|anchor| self.clamp_copy_mode_point(anchor));
        Some(CopyModeState { cursor, anchor })
    }

    fn set_copy_mode(&mut self, state: CopyModeState, inverted_blocklist: bool) {
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
                self.start_selection(anchor_point, SelectionType::Simple, anchor_side);
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

    fn copy_mode_word_motion(
        &self,
        cursor: &WithinBlock<Point>,
        direction: Direction,
        bound: WordBound,
    ) -> Option<WithinBlock<Point>> {
        let point = *cursor.get();
        let text = self.copy_mode_row_text(cursor)?;
        let offset = vim_word_iterator_from_offset(
            point.col,
            text.as_str(),
            direction,
            bound,
            WordType::Default,
        )
        .ok()?
        .next()?;
        Some(with_point(
            *cursor,
            Point::new(point.row, offset.as_usize()),
        ))
    }

    fn copy_mode_row_text(&self, point: &WithinBlock<Point>) -> Option<String> {
        let grid = self.block_at(point.block_index)?.grid_of_type(point.grid)?;
        let grid_handler = grid.grid_handler();
        let row = grid_handler.row(point.get().row)?;
        Some(
            (0..grid_handler.columns())
                .filter_map(|col| row.get(col).map(|cell| cell.c))
                .map(|c| if c == DEFAULT_CHAR { ' ' } else { c })
                .collect(),
        )
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
