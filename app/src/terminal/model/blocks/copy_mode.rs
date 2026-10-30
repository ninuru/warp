use vim::vim::{Direction, WordBound, WordType};
use vim::vim_word_iterator_from_offset;
use warp_terminal::model::grid::cell::DEFAULT_CHAR;
use warpui::text::SelectionType;
use warpui::units::IntoLines as _;

use super::{BlockList, BlockListPoint};
use crate::terminal::GridType;
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
    pub fn copy_mode_start(&self) -> Option<CopyModeState> {
        let mut index = self.last_non_hidden_block_by_index()?;
        loop {
            let output_grid = self.block_at(index)?.output_grid();
            if !output_grid.is_empty() {
                let point = Point::new(output_grid.len_displayed() - 1, 0);
                return Some(CopyModeState {
                    cursor: WithinBlock::new(point, index, GridType::Output),
                    anchor: None,
                });
            }
            index = self.prev_non_hidden_block_from_index(index)?;
        }
    }

    pub fn copy_mode_move(
        &mut self,
        state: CopyModeState,
        motion: CopyModeMotion,
        inverted_blocklist: bool,
    ) -> CopyModeState {
        let cursor = state.cursor;
        let point = *cursor.get();
        let last_column = self.size.columns.saturating_sub(1);
        let new_cursor = match motion {
            CopyModeMotion::Left => Some(Self::with_point(
                cursor,
                Point::new(point.row, point.col.saturating_sub(1)),
            )),
            CopyModeMotion::Right => Some(Self::with_point(
                cursor,
                Point::new(point.row, (point.col + 1).min(last_column)),
            )),
            CopyModeMotion::Up if point.row > 0 => Some(Self::with_point(
                cursor,
                Point::new(point.row - 1, point.col),
            )),
            CopyModeMotion::Up => self.seek_up_to_next_grid(
                cursor.block_index,
                cursor.grid,
                point.col,
                inverted_blocklist,
            ),
            CopyModeMotion::Down if point.row + 1 < self.copy_mode_grid_rows(&cursor) => Some(
                Self::with_point(cursor, Point::new(point.row + 1, point.col)),
            ),
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
        let new_state = CopyModeState {
            cursor: new_cursor.unwrap_or(cursor),
            anchor: state.anchor,
        };
        self.copy_mode_sync_selection(&new_state);
        new_state
    }

    pub fn copy_mode_toggle_visual(&mut self, state: CopyModeState) -> CopyModeState {
        let new_state = CopyModeState {
            cursor: state.cursor,
            anchor: match state.anchor {
                Some(_) => None,
                None => Some(state.cursor),
            },
        };
        self.copy_mode_sync_selection(&new_state);
        new_state
    }

    pub fn copy_mode_sync_selection(&mut self, state: &CopyModeState) {
        let cursor = self.copy_mode_point(&state.cursor);
        let anchor = state
            .anchor
            .map_or(cursor, |anchor| self.copy_mode_point(&anchor));
        let (anchor_side, cursor_side) =
            if (cursor.row, cursor.column) < (anchor.row, anchor.column) {
                (Side::Right, Side::Left)
            } else {
                (Side::Left, Side::Right)
            };
        self.start_selection(anchor, SelectionType::Simple, anchor_side);
        self.update_selection(cursor, cursor_side);
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
        let col = offset.as_usize().min(self.size.columns.saturating_sub(1));
        Some(Self::with_point(*cursor, Point::new(point.row, col)))
    }

    pub fn copy_mode_row_text(&self, point: &WithinBlock<Point>) -> Option<String> {
        let grid = self.block_at(point.block_index)?.grid_of_type(point.grid)?;
        let row = grid.grid_handler().row(point.get().row)?;
        Some(
            (0..self.size.columns)
                .filter_map(|col| row.get(col).map(|cell| cell.c))
                .map(|c| if c == DEFAULT_CHAR { ' ' } else { c })
                .collect(),
        )
    }

    fn copy_mode_point(&self, point: &WithinBlock<Point>) -> BlockListPoint {
        let mut block_list_point = BlockListPoint::from_within_block_point(point, self);
        block_list_point.row += 0.5.into_lines();
        block_list_point
    }

    fn copy_mode_grid_rows(&self, point: &WithinBlock<Point>) -> usize {
        self.block_at(point.block_index)
            .and_then(|block| block.grid_of_type(point.grid))
            .map_or(0, |grid| grid.len_displayed())
    }

    fn with_point(location: WithinBlock<Point>, point: Point) -> WithinBlock<Point> {
        WithinBlock::new(point, location.block_index, location.grid)
    }
}

#[cfg(test)]
#[path = "copy_mode_tests.rs"]
mod tests;
