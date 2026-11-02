use std::cmp::Ordering;
use std::iter;

use vim::vim::{Direction, WordType};
use warp_terminal::model::grid::cell::DEFAULT_CHAR;
use warpui::text::SelectionType;
use warpui::text::words::is_default_word_boundary;
use warpui::units::{IntoLines as _, Lines};

use super::{BlockList, BlockListPoint};
use crate::terminal::GridType;
use crate::terminal::find::{BlockGridMatch, BlockListMatch};
use crate::terminal::model::cell::Flags;
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
    BigWordForward,
    BigWordBackward,
    BigWordEnd,
    LineStart,
    FirstNonBlank,
    LineEnd,
    BufferTop,
    BufferBottom,
    HalfPageUp,
    HalfPageDown,
    ViewportTop,
    ViewportMiddle,
    ViewportBottom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CopyModeState {
    pub cursor: WithinBlock<Point>,
    pub anchor: Option<WithinBlock<Point>>,
    pub selection_type: SelectionType,
}

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

#[derive(Default)]
struct CopyModeLine {
    chars: Vec<(usize, char)>,
    joins_next: bool,
}

#[derive(Clone, Copy)]
enum CopyModeCell {
    Char(WithinBlock<Point>, char),
    RowBreak,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CharClass {
    Blank,
    Punctuation,
    Word,
}

impl CopyModeCell {
    fn class(&self, word_type: WordType) -> CharClass {
        match (self, word_type) {
            (CopyModeCell::RowBreak, _) => CharClass::Blank,
            (CopyModeCell::Char(_, c), _) if c.is_whitespace() => CharClass::Blank,
            (CopyModeCell::Char(..), WordType::BigWord) => CharClass::Word,
            (CopyModeCell::Char(_, c), WordType::Default) if is_default_word_boundary(*c) => {
                CharClass::Punctuation
            }
            (CopyModeCell::Char(..), WordType::Default) => CharClass::Word,
        }
    }

    fn point(&self) -> Option<WithinBlock<Point>> {
        match self {
            CopyModeCell::Char(point, _) => Some(*point),
            CopyModeCell::RowBreak => None,
        }
    }
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
                selection_type: SelectionType::Simple,
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

    pub fn copy_mode_target<F: Fn(Lines) -> Lines>(
        &self,
        motion: CopyModeMotion,
        inverted_blocklist: bool,
        viewport: &CopyModeViewport<F>,
    ) -> Option<WithinBlock<Point>> {
        let cursor = self.copy_mode_cursor()?;
        let point = *cursor.get();
        match motion {
            CopyModeMotion::Left => Some(with_point(
                cursor,
                Point::new(point.row, point.col.saturating_sub(1)),
            )),
            CopyModeMotion::Right => Some(with_point(cursor, Point::new(point.row, point.col + 1))),
            CopyModeMotion::Up => {
                self.copy_mode_adjacent_row(&cursor, Direction::Backward, inverted_blocklist)
            }
            CopyModeMotion::Down => {
                self.copy_mode_adjacent_row(&cursor, Direction::Forward, inverted_blocklist)
            }
            CopyModeMotion::WordForward => {
                self.copy_mode_word_start(&cursor, WordType::Default, inverted_blocklist)
            }
            CopyModeMotion::BigWordForward => {
                self.copy_mode_word_start(&cursor, WordType::BigWord, inverted_blocklist)
            }
            CopyModeMotion::WordBackward => self.copy_mode_word_end(
                &cursor,
                Direction::Backward,
                WordType::Default,
                inverted_blocklist,
            ),
            CopyModeMotion::BigWordBackward => self.copy_mode_word_end(
                &cursor,
                Direction::Backward,
                WordType::BigWord,
                inverted_blocklist,
            ),
            CopyModeMotion::WordEnd => self.copy_mode_word_end(
                &cursor,
                Direction::Forward,
                WordType::Default,
                inverted_blocklist,
            ),
            CopyModeMotion::BigWordEnd => self.copy_mode_word_end(
                &cursor,
                Direction::Forward,
                WordType::BigWord,
                inverted_blocklist,
            ),
            CopyModeMotion::LineStart => {
                let start = self.copy_mode_logical_line_edge(&cursor, Direction::Backward);
                Some(with_point(start, Point::new(start.get().row, 0)))
            }
            CopyModeMotion::FirstNonBlank => self.copy_mode_first_non_blank(
                &self.copy_mode_logical_line_edge(&cursor, Direction::Backward),
            ),
            CopyModeMotion::LineEnd => self.copy_mode_last_non_blank(
                &self.copy_mode_logical_line_edge(&cursor, Direction::Forward),
            ),
            CopyModeMotion::BufferTop => {
                self.copy_mode_buffer_edge(&cursor, Direction::Backward, inverted_blocklist)
            }
            CopyModeMotion::BufferBottom => {
                self.copy_mode_buffer_edge(&cursor, Direction::Forward, inverted_blocklist)
            }
            CopyModeMotion::HalfPageUp => self.copy_mode_step_rows(
                &cursor,
                Direction::Backward,
                viewport.half_page(),
                inverted_blocklist,
            ),
            CopyModeMotion::HalfPageDown => self.copy_mode_step_rows(
                &cursor,
                Direction::Forward,
                viewport.half_page(),
                inverted_blocklist,
            ),
            CopyModeMotion::ViewportTop => self
                .copy_mode_visible_rows(&cursor, inverted_blocklist, viewport)
                .first()
                .and_then(|row| self.copy_mode_first_non_blank(row)),
            CopyModeMotion::ViewportMiddle => {
                let rows = self.copy_mode_visible_rows(&cursor, inverted_blocklist, viewport);
                rows.get(rows.len().saturating_sub(1) / 2)
                    .and_then(|row| self.copy_mode_first_non_blank(row))
            }
            CopyModeMotion::ViewportBottom => self
                .copy_mode_visible_rows(&cursor, inverted_blocklist, viewport)
                .last()
                .and_then(|row| self.copy_mode_first_non_blank(row)),
        }
    }

    pub fn copy_mode_move_to(
        &mut self,
        target: Option<WithinBlock<Point>>,
        inverted_blocklist: bool,
    ) -> Option<CopyModeState> {
        let state = self.clamped_copy_mode()?;
        let cursor = target
            .and_then(|target| self.clamp_copy_mode_point(target))
            .unwrap_or(state.cursor);
        self.set_copy_mode(CopyModeState { cursor, ..state }, inverted_blocklist);
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
    ) -> Option<CopyModeState> {
        let state = self.clamped_copy_mode()?;
        let anchor = match state.anchor {
            Some(_) if state.selection_type == selection_type => None,
            Some(anchor) => Some(anchor),
            None => Some(state.cursor),
        };
        self.set_copy_mode(
            CopyModeState {
                cursor: state.cursor,
                anchor,
                selection_type,
            },
            inverted_blocklist,
        );
        self.copy_mode
    }

    pub fn copy_mode_swap_anchor(&mut self, inverted_blocklist: bool) -> Option<CopyModeState> {
        let state = self.clamped_copy_mode()?;
        let Some(anchor) = state.anchor else {
            return self.copy_mode;
        };
        self.set_copy_mode(
            CopyModeState {
                cursor: anchor,
                anchor: Some(state.cursor),
                selection_type: state.selection_type,
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
        Some(CopyModeState {
            cursor,
            anchor,
            selection_type: state.selection_type,
        })
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

    fn copy_mode_rows(
        &self,
        start: WithinBlock<Point>,
        direction: Direction,
        inverted_blocklist: bool,
    ) -> impl Iterator<Item = WithinBlock<Point>> + '_ {
        iter::successors(Some(start), move |row| {
            self.copy_mode_adjacent_row(row, direction, inverted_blocklist)
        })
    }

    fn copy_mode_step_rows(
        &self,
        cursor: &WithinBlock<Point>,
        direction: Direction,
        count: usize,
        inverted_blocklist: bool,
    ) -> Option<WithinBlock<Point>> {
        self.copy_mode_rows(*cursor, direction, inverted_blocklist)
            .take(count + 1)
            .last()
    }

    fn copy_mode_buffer_edge(
        &self,
        cursor: &WithinBlock<Point>,
        direction: Direction,
        inverted_blocklist: bool,
    ) -> Option<WithinBlock<Point>> {
        let edge_grid = iter::successors(Some(*cursor), |grid| match direction {
            Direction::Forward => {
                self.seek_down_to_next_grid(grid.block_index, grid.grid, 0, inverted_blocklist)
            }
            Direction::Backward => {
                self.seek_up_to_next_grid(grid.block_index, grid.grid, 0, inverted_blocklist)
            }
        })
        .last()?;
        let row = match direction {
            Direction::Forward => self.copy_mode_grid_rows(&edge_grid).checked_sub(1)?,
            Direction::Backward => 0,
        };
        self.copy_mode_first_non_blank(&with_point(edge_grid, Point::new(row, 0)))
    }

    fn copy_mode_visible_rows<F: Fn(Lines) -> Lines>(
        &self,
        cursor: &WithinBlock<Point>,
        inverted_blocklist: bool,
        viewport: &CopyModeViewport<F>,
    ) -> Vec<WithinBlock<Point>> {
        let position = |row: &WithinBlock<Point>| {
            let block_list_row = BlockListPoint::from_within_block_point(row, self).row;
            let visual_row = (viewport.visual_row)(block_list_row);
            if visual_row < viewport.top {
                Ordering::Less
            } else if visual_row > viewport.bottom {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        };
        let visible_row = match position(cursor) {
            Ordering::Less => self
                .copy_mode_rows(*cursor, Direction::Forward, inverted_blocklist)
                .find(|row| position(row) != Ordering::Less),
            Ordering::Equal => Some(*cursor),
            Ordering::Greater => self
                .copy_mode_rows(*cursor, Direction::Backward, inverted_blocklist)
                .find(|row| position(row) != Ordering::Greater),
        };
        let Some(top) = visible_row
            .filter(|row| position(row) == Ordering::Equal)
            .and_then(|row| {
                self.copy_mode_rows(row, Direction::Backward, inverted_blocklist)
                    .take_while(|row| position(row) == Ordering::Equal)
                    .last()
            })
        else {
            return Vec::new();
        };
        self.copy_mode_rows(top, Direction::Forward, inverted_blocklist)
            .take_while(|row| position(row) == Ordering::Equal)
            .collect()
    }

    fn copy_mode_logical_line_edge(
        &self,
        cursor: &WithinBlock<Point>,
        direction: Direction,
    ) -> WithinBlock<Point> {
        iter::successors(Some(*cursor), |row| {
            let point = *row.get();
            match direction {
                Direction::Forward => self
                    .copy_mode_row_wraps(row)
                    .then(|| with_point(*row, Point::new(point.row + 1, point.col))),
                Direction::Backward => point
                    .row
                    .checked_sub(1)
                    .map(|prev_row| with_point(*row, Point::new(prev_row, point.col)))
                    .filter(|prev| self.copy_mode_row_wraps(prev)),
            }
        })
        .last()
        .unwrap_or(*cursor)
    }

    fn copy_mode_first_non_blank(&self, row: &WithinBlock<Point>) -> Option<WithinBlock<Point>> {
        let col = self
            .copy_mode_line(row)?
            .chars
            .into_iter()
            .find(|(_, c)| !c.is_whitespace())
            .map_or(0, |(col, _)| col);
        Some(with_point(*row, Point::new(row.get().row, col)))
    }

    fn copy_mode_last_non_blank(&self, row: &WithinBlock<Point>) -> Option<WithinBlock<Point>> {
        let col = self
            .copy_mode_line(row)?
            .chars
            .into_iter()
            .rfind(|(_, c)| !c.is_whitespace())
            .map_or(0, |(col, _)| col);
        Some(with_point(*row, Point::new(row.get().row, col)))
    }

    fn copy_mode_word_start(
        &self,
        cursor: &WithinBlock<Point>,
        word_type: WordType,
        inverted_blocklist: bool,
    ) -> Option<WithinBlock<Point>> {
        let mut cells = self.copy_mode_cells(cursor, Direction::Forward, inverted_blocklist);
        let start_class = cells.next()?.class(word_type);
        cells
            .scan(start_class, |prev_class, cell| {
                let class = cell.class(word_type);
                let is_word_start = class != CharClass::Blank && class != *prev_class;
                *prev_class = class;
                Some((cell, is_word_start))
            })
            .find_map(|(cell, is_word_start)| is_word_start.then(|| cell.point()).flatten())
    }

    fn copy_mode_word_end(
        &self,
        cursor: &WithinBlock<Point>,
        direction: Direction,
        word_type: WordType,
        inverted_blocklist: bool,
    ) -> Option<WithinBlock<Point>> {
        let mut cells = self
            .copy_mode_cells(cursor, direction, inverted_blocklist)
            .skip(1)
            .skip_while(|cell| cell.class(word_type) == CharClass::Blank);
        let first = cells.next()?;
        let class = first.class(word_type);
        cells
            .take_while(|cell| cell.class(word_type) == class)
            .last()
            .unwrap_or(first)
            .point()
    }

    fn copy_mode_cells(
        &self,
        cursor: &WithinBlock<Point>,
        direction: Direction,
        inverted_blocklist: bool,
    ) -> impl Iterator<Item = CopyModeCell> + '_ {
        let cursor_col = cursor.get().col;
        self.copy_mode_rows(*cursor, direction, inverted_blocklist)
            .enumerate()
            .flat_map(move |(index, row)| {
                let CopyModeLine { chars, joins_next } =
                    self.copy_mode_line(&row).unwrap_or_default();
                let start_col = chars
                    .iter()
                    .map(|(col, _)| *col)
                    .take_while(|col| *col <= cursor_col)
                    .last()
                    .unwrap_or(0);
                let cells = chars
                    .into_iter()
                    .filter(|(col, _)| match direction {
                        Direction::Forward => index > 0 || *col >= start_col,
                        Direction::Backward => index > 0 || *col <= cursor_col,
                    })
                    .map(|(col, c)| {
                        CopyModeCell::Char(with_point(row, Point::new(row.get().row, col)), c)
                    });
                match direction {
                    Direction::Forward => cells
                        .chain((!joins_next).then_some(CopyModeCell::RowBreak))
                        .collect::<Vec<_>>(),
                    Direction::Backward => (index > 0 && !joins_next)
                        .then_some(CopyModeCell::RowBreak)
                        .into_iter()
                        .chain(cells.rev())
                        .collect(),
                }
            })
    }

    fn copy_mode_line(&self, row: &WithinBlock<Point>) -> Option<CopyModeLine> {
        let grid = self.block_at(row.block_index)?.grid_of_type(row.grid)?;
        let grid_handler = grid.grid_handler();
        let original_row =
            grid_handler.maybe_translate_row_from_displayed_to_original(row.get().row);
        let cells = grid_handler.row(original_row)?;
        let chars = (0..grid_handler.columns())
            .filter_map(|col| cells.get(col).map(|cell| (col, cell)))
            .filter(|(_, cell)| {
                !cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            })
            .map(|(col, cell)| (col, if cell.c == DEFAULT_CHAR { ' ' } else { cell.c }))
            .collect();
        Some(CopyModeLine {
            chars,
            joins_next: self.copy_mode_row_wraps(row),
        })
    }

    fn copy_mode_row_wraps(&self, row: &WithinBlock<Point>) -> bool {
        let displayed_row = row.get().row;
        self.block_at(row.block_index)
            .and_then(|block| block.grid_of_type(row.grid))
            .is_some_and(|grid| {
                let grid_handler = grid.grid_handler();
                displayed_row + 1 < grid.len_displayed()
                    && grid_handler.row_wraps(
                        grid_handler.maybe_translate_row_from_displayed_to_original(displayed_row),
                    )
            })
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
