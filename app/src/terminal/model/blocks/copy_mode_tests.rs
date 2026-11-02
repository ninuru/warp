use warp_core::semantic_selection::SemanticSelection;
use warpui::App;
use warpui::units::{IntoLines as _, Lines};

use super::*;
use crate::terminal::block_filter::BlockFilterQuery;
use crate::terminal::event_listener::ChannelEventListener;
use crate::terminal::model::blocks::insert_block;
use crate::terminal::model::blocks::tests::new_bootstrapped_block_list;
use crate::terminal::model::terminal_model::BlockIndex;
use crate::terminal::{SizeInfo, SizeUpdate, SizeUpdateReason};

fn new_block_list() -> BlockList {
    new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test())
}

type TestViewport = CopyModeViewport<fn(Lines) -> Lines>;

fn identity_row(row: Lines) -> Lines {
    row
}

fn viewport(top: Lines, bottom: Lines) -> TestViewport {
    CopyModeViewport {
        top,
        bottom,
        visual_row: identity_row,
    }
}

fn full_viewport() -> TestViewport {
    viewport(0.0.into_lines(), 10_000.0.into_lines())
}

fn move_cursor_in_viewport(
    blocks: &mut BlockList,
    motion: CopyModeMotion,
    inverted: bool,
    viewport: &TestViewport,
) -> WithinBlock<Point> {
    let target = blocks.copy_mode_target(motion, inverted, viewport);
    blocks
        .copy_mode_move_to(target, inverted)
        .expect("copy mode should stay active")
        .cursor
}

fn move_cursor(
    blocks: &mut BlockList,
    motion: CopyModeMotion,
    inverted: bool,
) -> WithinBlock<Point> {
    move_cursor_in_viewport(blocks, motion, inverted, &full_viewport())
}

fn output_point(block_index: BlockIndex, row: usize, col: usize) -> WithinBlock<Point> {
    WithinBlock::new(Point::new(row, col), block_index, GridType::Output)
}

fn place_cursor(blocks: &mut BlockList, cursor: WithinBlock<Point>) {
    blocks
        .copy_mode_move_to(Some(cursor), false)
        .expect("copy mode should stay active");
    assert_eq!(blocks.copy_mode_cursor(), Some(cursor));
}

fn assert_motion(
    blocks: &mut BlockList,
    from: WithinBlock<Point>,
    motion: CopyModeMotion,
    expected: WithinBlock<Point>,
) {
    place_cursor(blocks, from);
    assert_eq!(
        move_cursor(blocks, motion, false),
        expected,
        "{motion:?} from {from:?}"
    );
}

fn resize_columns(blocks: &mut BlockList, columns: usize, update_old_blocks: bool) {
    let last_size = *blocks.size();
    let new_size = SizeInfo::new_without_font_metrics(last_size.rows(), columns);
    let size_update = SizeUpdate {
        update_reason: SizeUpdateReason::Refresh,
        last_size,
        new_size,
        new_gap_height: None,
        natural_rows: new_size.rows(),
        natural_cols: columns,
    };
    blocks.resize(&size_update, update_old_blocks);
}

fn insert_two_blocks(blocks: &mut BlockList) -> (BlockIndex, BlockIndex) {
    let first = insert_block(blocks, "echo a", "aa\nbb");
    let second = insert_block(blocks, "echo c", "cc\ndd");
    (first, second)
}

#[test]
fn copy_mode_moves_selects_and_yanks_block_output() {
    App::test((), |app| async move {
        app.read(|ctx| {
            let mut blocks = new_block_list();
            let block_index = insert_block(&mut blocks, "echo", "ab cd\nef gh");
            let semantic_selection = SemanticSelection::mock(false, "");

            let state = blocks
                .enter_copy_mode(false)
                .expect("copy mode should start");
            assert_eq!(
                state.cursor,
                WithinBlock::new(Point::new(1, 0), block_index, GridType::Output)
            );
            assert_eq!(state.anchor, None);
            assert!(blocks.selection().is_none());
            assert_eq!(blocks.copy_mode_cursor(), Some(state.cursor));

            move_cursor(&mut blocks, CopyModeMotion::Up, false);
            let cursor = move_cursor(&mut blocks, CopyModeMotion::WordForward, false);
            assert_eq!(*cursor.get(), Point::new(0, 3));
            assert!(blocks.selection().is_none());

            let state = blocks
                .copy_mode_toggle_visual(false)
                .expect("copy mode should stay active");
            assert_eq!(state.anchor, Some(state.cursor));
            assert_eq!(
                blocks.selection_to_string(&semantic_selection, false, ctx),
                Some("c".to_string())
            );

            let cursor = move_cursor(&mut blocks, CopyModeMotion::WordEnd, false);
            assert_eq!(*cursor.get(), Point::new(0, 4));
            assert_eq!(
                blocks.selection_to_string(&semantic_selection, false, ctx),
                Some("cd".to_string())
            );

            move_cursor(&mut blocks, CopyModeMotion::Down, false);
            let cursor = move_cursor(&mut blocks, CopyModeMotion::Left, false);
            assert_eq!(*cursor.get(), Point::new(1, 3));
            assert_eq!(
                blocks.selection_to_string(&semantic_selection, false, ctx),
                Some("cd\nef g".to_string())
            );

            let cursor = move_cursor(&mut blocks, CopyModeMotion::WordBackward, false);
            assert_eq!(*cursor.get(), Point::new(1, 0));

            let state = blocks
                .copy_mode_toggle_visual(false)
                .expect("copy mode should stay active");
            assert_eq!(state.anchor, None);
            assert!(blocks.selection().is_none());

            move_cursor(&mut blocks, CopyModeMotion::Up, false);
            let cursor = move_cursor(&mut blocks, CopyModeMotion::Up, false);
            assert_eq!(cursor.grid, GridType::PromptAndCommand);
            assert_eq!(cursor.block_index, block_index);

            blocks.exit_copy_mode();
            assert_eq!(blocks.copy_mode(), None);
            assert!(blocks.selection().is_none());
        })
    })
}

#[test]
fn copy_mode_reverse_selection_includes_anchor_and_cursor() {
    App::test((), |app| async move {
        app.read(|ctx| {
            let mut blocks = new_block_list();
            insert_block(&mut blocks, "echo", "ab cd\nef gh");
            let semantic_selection = SemanticSelection::mock(false, "");

            blocks.enter_copy_mode(false);
            move_cursor(&mut blocks, CopyModeMotion::Right, false);
            blocks.copy_mode_toggle_visual(false);
            move_cursor(&mut blocks, CopyModeMotion::Up, false);
            let cursor = move_cursor(&mut blocks, CopyModeMotion::Right, false);
            assert_eq!(*cursor.get(), Point::new(0, 2));
            assert_eq!(
                blocks.selection_to_string(&semantic_selection, false, ctx),
                Some(" cd\nef".to_string())
            );
        })
    })
}

#[test]
fn copy_mode_crosses_blocks_in_normal_block_list() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let (first, second) = insert_two_blocks(&mut blocks);

        let state = blocks
            .enter_copy_mode(false)
            .expect("copy mode should start");
        assert_eq!(
            state.cursor,
            WithinBlock::new(Point::new(1, 0), second, GridType::Output)
        );

        move_cursor(&mut blocks, CopyModeMotion::Up, false);
        let cursor = move_cursor(&mut blocks, CopyModeMotion::Up, false);
        assert_eq!(
            (cursor.block_index, cursor.grid),
            (second, GridType::PromptAndCommand)
        );

        let cursor = move_cursor(&mut blocks, CopyModeMotion::Up, false);
        assert_eq!(
            cursor,
            WithinBlock::new(Point::new(1, 0), first, GridType::Output)
        );

        let cursor = move_cursor(&mut blocks, CopyModeMotion::Down, false);
        assert_eq!(
            (cursor.block_index, cursor.grid),
            (second, GridType::PromptAndCommand)
        );
    })
}

#[test]
fn copy_mode_crosses_blocks_in_inverted_block_list() {
    App::test((), |app| async move {
        app.read(|ctx| {
            let mut blocks = new_block_list();
            let (first, second) = insert_two_blocks(&mut blocks);
            let semantic_selection = SemanticSelection::mock(false, "");

            let state = blocks
                .enter_copy_mode(true)
                .expect("copy mode should start");
            assert_eq!(
                state.cursor,
                WithinBlock::new(Point::new(1, 0), second, GridType::Output)
            );

            move_cursor(&mut blocks, CopyModeMotion::Up, true);
            let header = move_cursor(&mut blocks, CopyModeMotion::Up, true);
            assert_eq!(
                (header.block_index, header.grid),
                (second, GridType::PromptAndCommand)
            );
            assert_eq!(move_cursor(&mut blocks, CopyModeMotion::Up, true), header);

            move_cursor(&mut blocks, CopyModeMotion::Down, true);
            let cursor = move_cursor(&mut blocks, CopyModeMotion::Down, true);
            assert_eq!(
                cursor,
                WithinBlock::new(Point::new(1, 0), second, GridType::Output)
            );

            blocks.copy_mode_toggle_visual(true);
            let cursor = move_cursor(&mut blocks, CopyModeMotion::Down, true);
            assert_eq!(
                cursor,
                WithinBlock::new(Point::new(0, 0), first, GridType::PromptAndCommand)
            );
            let text = blocks
                .selection_to_string(&semantic_selection, true, ctx)
                .expect("selection should have text");
            assert!(
                text.starts_with("dd\n"),
                "unexpected selection text: {text:?}"
            );

            move_cursor(&mut blocks, CopyModeMotion::Up, true);
            let cursor = move_cursor(&mut blocks, CopyModeMotion::Right, true);
            assert_eq!(
                cursor,
                WithinBlock::new(Point::new(1, 1), second, GridType::Output)
            );
            assert_eq!(
                blocks.selection_to_string(&semantic_selection, true, ctx),
                Some("dd".to_string())
            );

            move_cursor(&mut blocks, CopyModeMotion::Up, true);
            move_cursor(&mut blocks, CopyModeMotion::Left, true);
            assert_eq!(
                blocks.selection_to_string(&semantic_selection, true, ctx),
                Some("cc\nd".to_string())
            );
        })
    })
}

#[test]
fn copy_mode_clamps_cursor_to_narrower_old_grid() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let block_index = insert_block(&mut blocks, "echo", "ab cd\nef gh");
        let old_columns = blocks.size().columns();
        resize_columns(&mut blocks, old_columns + 13, false);
        assert!(blocks.size().columns() > old_columns);

        let state = blocks
            .enter_copy_mode(false)
            .expect("copy mode should start");
        assert_eq!(state.cursor.block_index, block_index);

        let mut cursor = state.cursor;
        for _ in 0..blocks.size().columns() {
            cursor = move_cursor(&mut blocks, CopyModeMotion::Right, false);
        }
        assert_eq!(*cursor.get(), Point::new(1, old_columns - 1));

        let cursor = move_cursor(&mut blocks, CopyModeMotion::WordEnd, false);
        assert!(cursor.get().col < old_columns);
    })
}

#[test]
fn copy_mode_exits_when_selection_is_replaced_or_cleared() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        insert_block(&mut blocks, "echo", "ab cd\nef gh");

        blocks.enter_copy_mode(false);
        let wider_columns = blocks.size().columns() + 2;
        resize_columns(&mut blocks, wider_columns, true);
        assert_eq!(blocks.copy_mode(), None);
        assert_eq!(
            blocks.copy_mode_target(CopyModeMotion::Left, false, &full_viewport()),
            None
        );
        assert_eq!(blocks.copy_mode_move_to(None, false), None);

        blocks.enter_copy_mode(false);
        let point = BlockListPoint::new(0.5.into_lines(), 0);
        blocks.start_selection(point, SelectionType::Simple, Side::Left);
        assert_eq!(blocks.copy_mode(), None);
        assert!(blocks.selection().is_some());

        blocks.enter_copy_mode(false);
        assert!(blocks.selection().is_none());
        blocks.clear_selection();
        assert_eq!(blocks.copy_mode(), None);

        blocks.enter_copy_mode(false);
        blocks.set_next_gap_height_in_lines(1.0.into_lines());
        blocks.clear_visible_screen();
        assert_eq!(blocks.copy_mode(), None);
    })
}

#[test]
fn copy_mode_word_motions_cross_rows_and_grids() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let block = insert_block(&mut blocks, "echo", "ab cd\nef gh");
        blocks.enter_copy_mode(false);

        assert_motion(
            &mut blocks,
            output_point(block, 0, 3),
            CopyModeMotion::WordForward,
            output_point(block, 1, 0),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 4),
            CopyModeMotion::WordEnd,
            output_point(block, 1, 1),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 1, 0),
            CopyModeMotion::WordBackward,
            output_point(block, 0, 3),
        );
    })
}

#[test]
fn copy_mode_word_motions_cross_blocks() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let (first, second) = insert_two_blocks(&mut blocks);
        blocks.enter_copy_mode(false);

        place_cursor(&mut blocks, output_point(first, 1, 0));
        let cursor = move_cursor(&mut blocks, CopyModeMotion::WordForward, false);
        assert_eq!(
            (cursor.block_index, cursor.grid),
            (second, GridType::PromptAndCommand)
        );
        let cursor = move_cursor(&mut blocks, CopyModeMotion::WordBackward, false);
        assert_eq!(cursor, output_point(first, 1, 0));

        let cursor = move_cursor(&mut blocks, CopyModeMotion::WordEnd, false);
        assert_eq!(cursor, output_point(first, 1, 1));
        let cursor = move_cursor(&mut blocks, CopyModeMotion::WordEnd, false);
        assert_eq!(
            (cursor.block_index, cursor.grid),
            (second, GridType::PromptAndCommand)
        );

        let state = blocks
            .enter_copy_mode(true)
            .expect("copy mode should start");
        assert_eq!(state.cursor, output_point(second, 1, 0));
        let cursor = move_cursor(&mut blocks, CopyModeMotion::WordForward, true);
        assert_eq!(
            (cursor.block_index, cursor.grid),
            (first, GridType::PromptAndCommand)
        );
    })
}

#[test]
fn copy_mode_word_motions_follow_wrapped_lines() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let block = insert_block(&mut blocks, "echo", "  ab cdefgh\nxy");
        blocks.enter_copy_mode(false);

        assert_motion(
            &mut blocks,
            output_point(block, 0, 2),
            CopyModeMotion::WordForward,
            output_point(block, 0, 5),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 5),
            CopyModeMotion::WordForward,
            output_point(block, 2, 0),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 3),
            CopyModeMotion::WordEnd,
            output_point(block, 1, 3),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 2, 0),
            CopyModeMotion::WordBackward,
            output_point(block, 0, 5),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 1, 2),
            CopyModeMotion::WordBackward,
            output_point(block, 0, 5),
        );
    })
}

#[test]
fn copy_mode_big_word_motions_skip_punctuation() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let block = insert_block(&mut blocks, "echo", "a.b cd\nx-y");
        blocks.enter_copy_mode(false);

        assert_motion(
            &mut blocks,
            output_point(block, 0, 0),
            CopyModeMotion::WordForward,
            output_point(block, 0, 1),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 0),
            CopyModeMotion::BigWordForward,
            output_point(block, 0, 4),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 4),
            CopyModeMotion::BigWordForward,
            output_point(block, 1, 0),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 0),
            CopyModeMotion::WordEnd,
            output_point(block, 0, 1),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 0),
            CopyModeMotion::BigWordEnd,
            output_point(block, 0, 2),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 5),
            CopyModeMotion::BigWordEnd,
            output_point(block, 1, 2),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 1, 2),
            CopyModeMotion::WordBackward,
            output_point(block, 1, 1),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 1, 2),
            CopyModeMotion::BigWordBackward,
            output_point(block, 1, 0),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 1, 0),
            CopyModeMotion::BigWordBackward,
            output_point(block, 0, 4),
        );
    })
}

#[test]
fn copy_mode_motions_treat_wide_chars_as_one_cell() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let block = insert_block(&mut blocks, "echo", "日本 a\nab 日本日\nabc 日本日");
        blocks.enter_copy_mode(false);

        assert_motion(
            &mut blocks,
            output_point(block, 0, 0),
            CopyModeMotion::WordForward,
            output_point(block, 0, 5),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 0),
            CopyModeMotion::WordEnd,
            output_point(block, 0, 2),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 5),
            CopyModeMotion::WordBackward,
            output_point(block, 0, 0),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 0),
            CopyModeMotion::LineEnd,
            output_point(block, 0, 5),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 1, 1),
            CopyModeMotion::WordEnd,
            output_point(block, 2, 0),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 2, 0),
            CopyModeMotion::WordBackward,
            output_point(block, 1, 3),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 3, 4),
            CopyModeMotion::WordEnd,
            output_point(block, 4, 2),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 3, 0),
            CopyModeMotion::LineEnd,
            output_point(block, 4, 2),
        );
    })
}

#[test]
fn copy_mode_line_motions_follow_wrapped_lines() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let block = insert_block(&mut blocks, "echo", "  ab cdefgh\nxy");
        blocks.enter_copy_mode(false);

        assert_motion(
            &mut blocks,
            output_point(block, 1, 2),
            CopyModeMotion::LineStart,
            output_point(block, 0, 0),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 1, 2),
            CopyModeMotion::FirstNonBlank,
            output_point(block, 0, 2),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 0, 0),
            CopyModeMotion::LineEnd,
            output_point(block, 1, 3),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 2, 0),
            CopyModeMotion::LineEnd,
            output_point(block, 2, 1),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 2, 1),
            CopyModeMotion::LineStart,
            output_point(block, 2, 0),
        );
    })
}

#[test]
fn copy_mode_buffer_motions_jump_to_first_and_last_rows() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let (first, second) = insert_two_blocks(&mut blocks);
        blocks.enter_copy_mode(false);

        let cursor = move_cursor(&mut blocks, CopyModeMotion::BufferTop, false);
        assert_eq!(
            (cursor.block_index, cursor.grid, cursor.get().row),
            (first, GridType::PromptAndCommand, 0)
        );

        let cursor = move_cursor(&mut blocks, CopyModeMotion::BufferBottom, false);
        assert_eq!(cursor, output_point(second, 1, 0));

        let state = blocks
            .enter_copy_mode(true)
            .expect("copy mode should start");
        assert_eq!(state.cursor, output_point(second, 1, 0));
        let cursor = move_cursor(&mut blocks, CopyModeMotion::BufferBottom, true);
        assert_eq!(cursor, output_point(first, 1, 0));
        let cursor = move_cursor(&mut blocks, CopyModeMotion::BufferTop, true);
        assert_eq!(
            (cursor.block_index, cursor.grid, cursor.get().row),
            (second, GridType::PromptAndCommand, 0)
        );
    })
}

fn insert_numbered_block(blocks: &mut BlockList) -> BlockIndex {
    let output = (0..10).map(|row| format!(" {row}")).collect::<Vec<_>>();
    insert_block(blocks, "echo", &output.join("\n"))
}

fn output_row_top(blocks: &BlockList, block: BlockIndex, row: usize) -> Lines {
    BlockListPoint::from_within_block_point(&output_point(block, row, 0), blocks).row
}

#[test]
fn copy_mode_half_page_motions_move_by_half_the_viewport() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let block = insert_numbered_block(&mut blocks);
        blocks.enter_copy_mode(false);
        let half_page = viewport(0.0.into_lines(), 7.0.into_lines());

        place_cursor(&mut blocks, output_point(block, 9, 1));
        let cursor =
            move_cursor_in_viewport(&mut blocks, CopyModeMotion::HalfPageUp, false, &half_page);
        assert_eq!(cursor, output_point(block, 5, 1));
        let cursor =
            move_cursor_in_viewport(&mut blocks, CopyModeMotion::HalfPageUp, false, &half_page);
        assert_eq!(cursor, output_point(block, 1, 1));

        let cursor =
            move_cursor_in_viewport(&mut blocks, CopyModeMotion::HalfPageDown, false, &half_page);
        assert_eq!(cursor, output_point(block, 5, 1));
        let cursor =
            move_cursor_in_viewport(&mut blocks, CopyModeMotion::HalfPageDown, false, &half_page);
        assert_eq!(cursor, output_point(block, 9, 1));
        let cursor =
            move_cursor_in_viewport(&mut blocks, CopyModeMotion::HalfPageDown, false, &half_page);
        assert_eq!(cursor, output_point(block, 9, 1));
    })
}

#[test]
fn copy_mode_viewport_motions_land_on_visible_rows() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let block = insert_numbered_block(&mut blocks);
        blocks.enter_copy_mode(false);
        let visible = viewport(
            output_row_top(&blocks, block, 3),
            output_row_top(&blocks, block, 7),
        );

        for start in [9, 5, 0] {
            for (motion, row) in [
                (CopyModeMotion::ViewportTop, 3),
                (CopyModeMotion::ViewportMiddle, 5),
                (CopyModeMotion::ViewportBottom, 7),
            ] {
                place_cursor(&mut blocks, output_point(block, start, 0));
                let cursor = move_cursor_in_viewport(&mut blocks, motion, false, &visible);
                assert_eq!(
                    cursor,
                    output_point(block, row, 1),
                    "{motion:?} from row {start}"
                );
            }
        }
    })
}

#[test]
fn copy_mode_motions_use_displayed_rows_of_filtered_block() {
    App::test((), |_app| async move {
        let mut blocks = new_block_list();
        let block = insert_block(&mut blocks, "echo", "foo a\nb\nfoo cd");
        blocks.filter_block_output(
            block,
            BlockFilterQuery {
                query: "foo".to_string(),
                num_context_lines: 0,
                regex_enabled: false,
                case_sensitivity_enabled: false,
                invert_filter_enabled: false,
                is_active: true,
            },
        );
        let state = blocks
            .enter_copy_mode(false)
            .expect("copy mode should start");
        assert_eq!(state.cursor, output_point(block, 1, 0));

        assert_motion(
            &mut blocks,
            output_point(block, 0, 4),
            CopyModeMotion::WordForward,
            output_point(block, 1, 0),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 1, 0),
            CopyModeMotion::WordEnd,
            output_point(block, 1, 2),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 1, 0),
            CopyModeMotion::WordBackward,
            output_point(block, 0, 4),
        );
        assert_motion(
            &mut blocks,
            output_point(block, 1, 0),
            CopyModeMotion::LineEnd,
            output_point(block, 1, 5),
        );
    })
}
