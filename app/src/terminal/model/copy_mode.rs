use std::iter;

use vim::vim::{Direction, WordType};
use warp_terminal::model::grid::cell::DEFAULT_CHAR;
use warpui::text::SelectionType;
use warpui::text::words::is_default_word_boundary;

use crate::terminal::model::cell::Flags;
use crate::terminal::model::grid::Dimensions;
use crate::terminal::model::grid::grid_handler::GridHandler;
use crate::terminal::model::index::Point;

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
pub struct CopyModeState<P> {
    pub cursor: P,
    pub anchor: Option<P>,
    pub selection_type: SelectionType,
}

impl<P: Copy> CopyModeState<P> {
    pub(super) fn new(cursor: P) -> Self {
        Self {
            cursor,
            anchor: None,
            selection_type: SelectionType::Simple,
        }
    }

    pub(super) fn with_cursor(self, cursor: P) -> Self {
        Self { cursor, ..self }
    }

    pub(super) fn toggled_visual(self, selection_type: SelectionType) -> Self {
        let anchor = match self.anchor {
            Some(_) if self.selection_type == selection_type => None,
            Some(anchor) => Some(anchor),
            None => Some(self.cursor),
        };
        Self {
            cursor: self.cursor,
            anchor,
            selection_type,
        }
    }

    pub(super) fn swapped(self) -> Self {
        match self.anchor {
            Some(anchor) => Self {
                cursor: anchor,
                anchor: Some(self.cursor),
                selection_type: self.selection_type,
            },
            None => self,
        }
    }
}

pub(super) struct CopyModeGridRow<'a> {
    pub grid: &'a GridHandler,
    pub row: usize,
    pub rows: usize,
}

impl CopyModeGridRow<'_> {
    fn wraps(&self) -> bool {
        self.row + 1 < self.rows
            && self.grid.row_wraps(
                self.grid
                    .maybe_translate_row_from_displayed_to_original(self.row),
            )
    }

    fn line(&self) -> Option<CopyModeLine> {
        let original_row = self
            .grid
            .maybe_translate_row_from_displayed_to_original(self.row);
        let cells = self.grid.row(original_row)?;
        let chars = (0..self.grid.columns())
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
            joins_next: self.wraps(),
        })
    }
}

#[derive(Default)]
struct CopyModeLine {
    chars: Vec<(usize, char)>,
    joins_next: bool,
}

#[derive(Clone, Copy)]
pub(super) enum CopyModeCell<P> {
    Char(P, char),
    RowBreak,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CharClass {
    Blank,
    Punctuation,
    Word,
}

impl<P: Copy> CopyModeCell<P> {
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

    fn point(&self) -> Option<P> {
        match self {
            CopyModeCell::Char(point, _) => Some(*point),
            CopyModeCell::RowBreak => None,
        }
    }
}

pub(super) trait CopyModeGrid {
    type Point: Copy;

    fn position(point: &Self::Point) -> Point;

    fn with_position(point: &Self::Point, position: Point) -> Self::Point;

    fn grid_row(&self, row: &Self::Point) -> Option<CopyModeGridRow<'_>>;

    fn adjacent_row(&self, row: &Self::Point, direction: Direction) -> Option<Self::Point>;

    fn buffer_edge_row(&self, cursor: &Self::Point, direction: Direction) -> Option<Self::Point>;

    fn viewport_rows(&self, cursor: &Self::Point) -> Vec<Self::Point>;

    fn half_page(&self) -> usize;

    fn copy_mode_target(
        &self,
        cursor: &Self::Point,
        motion: CopyModeMotion,
    ) -> Option<Self::Point> {
        let point = Self::position(cursor);
        match motion {
            CopyModeMotion::Left => Some(Self::with_position(
                cursor,
                Point::new(point.row, point.col.saturating_sub(1)),
            )),
            CopyModeMotion::Right => Some(Self::with_position(
                cursor,
                Point::new(point.row, point.col + 1),
            )),
            CopyModeMotion::Up => self.adjacent_row(cursor, Direction::Backward),
            CopyModeMotion::Down => self.adjacent_row(cursor, Direction::Forward),
            CopyModeMotion::WordForward => self.word_start(cursor, WordType::Default),
            CopyModeMotion::BigWordForward => self.word_start(cursor, WordType::BigWord),
            CopyModeMotion::WordBackward => {
                self.word_end(cursor, Direction::Backward, WordType::Default)
            }
            CopyModeMotion::BigWordBackward => {
                self.word_end(cursor, Direction::Backward, WordType::BigWord)
            }
            CopyModeMotion::WordEnd => self.word_end(cursor, Direction::Forward, WordType::Default),
            CopyModeMotion::BigWordEnd => {
                self.word_end(cursor, Direction::Forward, WordType::BigWord)
            }
            CopyModeMotion::LineStart => {
                let start = self.logical_line_edge(cursor, Direction::Backward);
                let row = Self::position(&start).row;
                Some(Self::with_position(&start, Point::new(row, 0)))
            }
            CopyModeMotion::FirstNonBlank => {
                self.first_non_blank(&self.logical_line_edge(cursor, Direction::Backward))
            }
            CopyModeMotion::LineEnd => {
                self.last_non_blank(&self.logical_line_edge(cursor, Direction::Forward))
            }
            CopyModeMotion::BufferTop => self
                .buffer_edge_row(cursor, Direction::Backward)
                .and_then(|row| self.first_non_blank(&row)),
            CopyModeMotion::BufferBottom => self
                .buffer_edge_row(cursor, Direction::Forward)
                .and_then(|row| self.first_non_blank(&row)),
            CopyModeMotion::HalfPageUp => {
                self.step_rows(cursor, Direction::Backward, self.half_page())
            }
            CopyModeMotion::HalfPageDown => {
                self.step_rows(cursor, Direction::Forward, self.half_page())
            }
            CopyModeMotion::ViewportTop => self
                .viewport_rows(cursor)
                .first()
                .and_then(|row| self.first_non_blank(row)),
            CopyModeMotion::ViewportMiddle => {
                let rows = self.viewport_rows(cursor);
                rows.get(rows.len().saturating_sub(1) / 2)
                    .and_then(|row| self.first_non_blank(row))
            }
            CopyModeMotion::ViewportBottom => self
                .viewport_rows(cursor)
                .last()
                .and_then(|row| self.first_non_blank(row)),
        }
    }

    fn rows_from(
        &self,
        start: Self::Point,
        direction: Direction,
    ) -> impl Iterator<Item = Self::Point> + '_ {
        iter::successors(Some(start), move |row| self.adjacent_row(row, direction))
    }

    fn step_rows(
        &self,
        cursor: &Self::Point,
        direction: Direction,
        count: usize,
    ) -> Option<Self::Point> {
        self.rows_from(*cursor, direction).take(count + 1).last()
    }

    fn wraps_into_next(&self, row: &Self::Point) -> bool {
        self.grid_row(row).is_some_and(|grid_row| grid_row.wraps())
    }

    fn logical_line_edge(&self, cursor: &Self::Point, direction: Direction) -> Self::Point {
        iter::successors(Some(*cursor), |row| {
            let point = Self::position(row);
            match direction {
                Direction::Forward => self
                    .wraps_into_next(row)
                    .then(|| Self::with_position(row, Point::new(point.row + 1, point.col))),
                Direction::Backward => point
                    .row
                    .checked_sub(1)
                    .map(|prev_row| Self::with_position(row, Point::new(prev_row, point.col)))
                    .filter(|prev| self.wraps_into_next(prev)),
            }
        })
        .last()
        .unwrap_or(*cursor)
    }

    fn first_non_blank(&self, row: &Self::Point) -> Option<Self::Point> {
        let col = self
            .grid_row(row)?
            .line()?
            .chars
            .into_iter()
            .find(|(_, c)| !c.is_whitespace())
            .map_or(0, |(col, _)| col);
        Some(Self::with_position(
            row,
            Point::new(Self::position(row).row, col),
        ))
    }

    fn last_non_blank(&self, row: &Self::Point) -> Option<Self::Point> {
        let col = self
            .grid_row(row)?
            .line()?
            .chars
            .into_iter()
            .rfind(|(_, c)| !c.is_whitespace())
            .map_or(0, |(col, _)| col);
        Some(Self::with_position(
            row,
            Point::new(Self::position(row).row, col),
        ))
    }

    fn word_start(&self, cursor: &Self::Point, word_type: WordType) -> Option<Self::Point> {
        let mut cells = self.cells_from(cursor, Direction::Forward);
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

    fn word_end(
        &self,
        cursor: &Self::Point,
        direction: Direction,
        word_type: WordType,
    ) -> Option<Self::Point> {
        let mut cells = self
            .cells_from(cursor, direction)
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

    fn cells_from(
        &self,
        cursor: &Self::Point,
        direction: Direction,
    ) -> impl Iterator<Item = CopyModeCell<Self::Point>> + '_ {
        let cursor_col = Self::position(cursor).col;
        self.rows_from(*cursor, direction)
            .enumerate()
            .flat_map(move |(index, row)| {
                let CopyModeLine { chars, joins_next } = self
                    .grid_row(&row)
                    .and_then(|grid_row| grid_row.line())
                    .unwrap_or_default();
                let row_index = Self::position(&row).row;
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
                        CopyModeCell::Char(Self::with_position(&row, Point::new(row_index, col)), c)
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
}

impl CopyModeGrid for GridHandler {
    type Point = Point;

    fn position(point: &Point) -> Point {
        *point
    }

    fn with_position(_point: &Point, position: Point) -> Point {
        position
    }

    fn grid_row(&self, row: &Point) -> Option<CopyModeGridRow<'_>> {
        let rows = Dimensions::visible_rows(self);
        (row.row < rows).then_some(CopyModeGridRow {
            grid: self,
            row: row.row,
            rows,
        })
    }

    fn adjacent_row(&self, row: &Point, direction: Direction) -> Option<Point> {
        let adjacent_row = match direction {
            Direction::Forward => {
                Some(row.row + 1).filter(|next| *next < Dimensions::visible_rows(self))
            }
            Direction::Backward => row.row.checked_sub(1),
        };
        adjacent_row.map(|adjacent_row| Point::new(adjacent_row, row.col))
    }

    fn buffer_edge_row(&self, _cursor: &Point, direction: Direction) -> Option<Point> {
        let row = match direction {
            Direction::Forward => Dimensions::visible_rows(self).checked_sub(1)?,
            Direction::Backward => 0,
        };
        Some(Point::new(row, 0))
    }

    fn viewport_rows(&self, _cursor: &Point) -> Vec<Point> {
        (0..Dimensions::visible_rows(self))
            .map(|row| Point::new(row, 0))
            .collect()
    }

    fn half_page(&self) -> usize {
        (Dimensions::visible_rows(self) / 2).max(1)
    }
}
