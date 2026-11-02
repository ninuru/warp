use warp_core::semantic_selection::SemanticSelection;

use super::*;
use crate::terminal::event_listener::ChannelEventListener;
use crate::terminal::model::ObfuscateSecrets;
use crate::terminal::model::ansi::Handler as _;
use crate::terminal::{SizeInfo, SizeUpdate, SizeUpdateReason};

const ROWS: usize = 4;
const COLUMNS: usize = 12;

fn new_screen(lines: &[&str]) -> AltScreen {
    let mut screen = AltScreen::new(
        SizeInfo::new_without_font_metrics(ROWS, COLUMNS),
        0,
        ChannelEventListener::new_for_test(),
        ObfuscateSecrets::No,
    );
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            screen.carriage_return();
            screen.linefeed();
        }
        line.chars().for_each(|c| screen.input(c));
    }
    screen
}

fn screen_with_text() -> AltScreen {
    new_screen(&["alpha one", "beta two", "gamma three"])
}

fn move_cursor(screen: &mut AltScreen, motion: CopyModeMotion) -> Point {
    let target = screen.copy_mode_target(motion);
    screen
        .copy_mode_move_to(target)
        .expect("copy mode should stay active")
        .cursor
}

fn place_cursor(screen: &mut AltScreen, cursor: Point) {
    screen
        .copy_mode_move_to(Some(cursor))
        .expect("copy mode should stay active");
    assert_eq!(screen.copy_mode_cursor(), Some(cursor));
}

fn yank(screen: &AltScreen) -> String {
    screen
        .selection_to_string(&SemanticSelection::mock(false, ""))
        .expect("selection should exist")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn resize(screen: &mut AltScreen, rows: usize, columns: usize) {
    let last_size = SizeInfo::new_without_font_metrics(ROWS, COLUMNS);
    let new_size = SizeInfo::new_without_font_metrics(rows, columns);
    screen.resize(&SizeUpdate {
        update_reason: SizeUpdateReason::Refresh,
        last_size,
        new_size,
        new_gap_height: None,
        natural_rows: rows,
        natural_cols: columns,
    });
}

#[test]
fn copy_mode_alt_screen_enters_at_the_app_cursor() {
    let mut screen = screen_with_text();
    let state = screen.enter_copy_mode().expect("copy mode should start");
    assert_eq!(state.cursor, Point::new(2, 11));
    assert_eq!(state.anchor, None);
    assert!(screen.selection().is_none());

    screen.exit_copy_mode();
    assert!(screen.copy_mode().is_none());
}

#[test]
fn copy_mode_alt_screen_motions_cover_the_visible_screen() {
    let mut screen = screen_with_text();
    screen.enter_copy_mode();

    let steps = [
        (CopyModeMotion::LineStart, Point::new(2, 0)),
        (CopyModeMotion::Up, Point::new(1, 0)),
        (CopyModeMotion::WordForward, Point::new(1, 5)),
        (CopyModeMotion::WordEnd, Point::new(1, 7)),
        (CopyModeMotion::WordBackward, Point::new(1, 5)),
        (CopyModeMotion::FirstNonBlank, Point::new(1, 0)),
        (CopyModeMotion::LineEnd, Point::new(1, 7)),
        (CopyModeMotion::Right, Point::new(1, 8)),
        (CopyModeMotion::Left, Point::new(1, 7)),
        (CopyModeMotion::Down, Point::new(2, 7)),
        (CopyModeMotion::BigWordBackward, Point::new(2, 6)),
        (CopyModeMotion::BigWordBackward, Point::new(2, 0)),
        (CopyModeMotion::BigWordEnd, Point::new(2, 4)),
        (CopyModeMotion::BigWordForward, Point::new(2, 6)),
        (CopyModeMotion::BufferTop, Point::new(0, 0)),
        (CopyModeMotion::BufferBottom, Point::new(3, 0)),
        (CopyModeMotion::ViewportTop, Point::new(0, 0)),
        (CopyModeMotion::ViewportMiddle, Point::new(1, 0)),
        (CopyModeMotion::ViewportBottom, Point::new(3, 0)),
        (CopyModeMotion::HalfPageUp, Point::new(1, 0)),
        (CopyModeMotion::HalfPageDown, Point::new(3, 0)),
        (CopyModeMotion::Down, Point::new(3, 0)),
    ];
    for (motion, expected) in steps {
        assert_eq!(move_cursor(&mut screen, motion), expected, "{motion:?}");
    }

    place_cursor(&mut screen, Point::new(0, 6));
    assert_eq!(
        move_cursor(&mut screen, CopyModeMotion::WordForward),
        Point::new(1, 0)
    );
    place_cursor(&mut screen, Point::new(0, 11));
    assert_eq!(
        move_cursor(&mut screen, CopyModeMotion::Right),
        Point::new(0, 11)
    );
    let state = screen
        .copy_mode_move_to(Some(Point::new(9, 40)))
        .expect("copy mode should stay active");
    assert_eq!(state.cursor, Point::new(ROWS - 1, COLUMNS - 1));
}

#[test]
fn copy_mode_alt_screen_visual_yanks_words_and_lines() {
    let mut screen = screen_with_text();
    screen.enter_copy_mode();
    place_cursor(&mut screen, Point::new(1, 0));

    screen.copy_mode_toggle_visual(SelectionType::Simple);
    move_cursor(&mut screen, CopyModeMotion::WordEnd);
    assert_eq!(yank(&screen), "beta");

    move_cursor(&mut screen, CopyModeMotion::Down);
    assert_eq!(yank(&screen), "beta two\ngamm");

    let state = screen
        .copy_mode_swap_anchor()
        .expect("copy mode should stay active");
    assert_eq!(state.cursor, Point::new(1, 0));
    assert_eq!(state.anchor, Some(Point::new(2, 3)));
    move_cursor(&mut screen, CopyModeMotion::Up);
    assert_eq!(yank(&screen), "alpha one\nbeta two\ngamm");

    let state = screen
        .copy_mode_toggle_visual(SelectionType::Lines)
        .expect("copy mode should stay active");
    assert_eq!(state.selection_type, SelectionType::Lines);
    assert_eq!(yank(&screen), "alpha one\nbeta two\ngamma three");

    let state = screen
        .copy_mode_toggle_visual(SelectionType::Lines)
        .expect("copy mode should stay active");
    assert_eq!(state.anchor, None);
    assert!(screen.selection().is_none());
    assert!(screen.copy_mode().is_some());
}

#[test]
fn copy_mode_alt_screen_cursor_rotates_with_scrolls() {
    let mut screen = screen_with_text();
    screen.enter_copy_mode();
    place_cursor(&mut screen, Point::new(1, 0));
    screen.copy_mode_toggle_visual(SelectionType::Simple);
    move_cursor(&mut screen, CopyModeMotion::WordEnd);

    screen.goto(VisibleRow(ROWS - 1), 0);
    screen.linefeed();
    let state = screen.copy_mode().expect("copy mode should stay active");
    assert_eq!(state.cursor, Point::new(0, 3));
    assert_eq!(state.anchor, Some(Point::new(0, 0)));
    assert_eq!(yank(&screen), "beta");

    screen.goto(VisibleRow(0), 0);
    screen.reverse_index();
    let state = screen.copy_mode().expect("copy mode should stay active");
    assert_eq!(state.cursor, Point::new(1, 3));
    assert_eq!(state.anchor, Some(Point::new(1, 0)));
    assert_eq!(yank(&screen), "beta");
}

#[test]
fn copy_mode_alt_screen_exits_on_resize_and_mouse_selection() {
    let mut screen = screen_with_text();
    screen.enter_copy_mode();
    resize(&mut screen, ROWS, COLUMNS);
    assert!(screen.copy_mode().is_some());

    resize(&mut screen, ROWS + 1, COLUMNS);
    assert!(screen.copy_mode().is_none());

    screen.enter_copy_mode();
    screen.start_selection(Point::new(0, 0), SelectionType::Simple, Side::Left);
    assert!(screen.copy_mode().is_none());
}
