use warp_core::semantic_selection::SemanticSelection;
use warpui::App;

use super::*;
use crate::terminal::event_listener::ChannelEventListener;
use crate::terminal::model::blocks::insert_block;
use crate::terminal::model::blocks::tests::new_bootstrapped_block_list;

#[test]
fn copy_mode_moves_selects_and_yanks_block_output() {
    App::test((), |app| async move {
        app.read(|ctx| {
            let mut blocks =
                new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());
            let block_index = insert_block(&mut blocks, "echo", "ab cd\nef gh");
            let semantic_selection = SemanticSelection::mock(false, "");

            let state = blocks.copy_mode_start().expect("copy mode should start");
            assert_eq!(
                state.cursor,
                WithinBlock::new(Point::new(1, 0), block_index, GridType::Output)
            );

            let state = blocks.copy_mode_move(state, CopyModeMotion::Up, false);
            let state = blocks.copy_mode_move(state, CopyModeMotion::WordForward, false);
            assert_eq!(*state.cursor.get(), Point::new(0, 3));
            assert_eq!(
                blocks.selection_to_string(&semantic_selection, false, ctx),
                Some("c".to_string())
            );

            let state = blocks.copy_mode_toggle_visual(state);
            let state = blocks.copy_mode_move(state, CopyModeMotion::WordEnd, false);
            assert_eq!(*state.cursor.get(), Point::new(0, 4));
            assert_eq!(
                blocks.selection_to_string(&semantic_selection, false, ctx),
                Some("cd".to_string())
            );

            let state = blocks.copy_mode_move(state, CopyModeMotion::Down, false);
            let state = blocks.copy_mode_move(state, CopyModeMotion::Left, false);
            assert_eq!(*state.cursor.get(), Point::new(1, 3));
            assert_eq!(
                blocks.selection_to_string(&semantic_selection, false, ctx),
                Some("cd\nef g".to_string())
            );

            let state = blocks.copy_mode_move(state, CopyModeMotion::WordBackward, false);
            assert_eq!(*state.cursor.get(), Point::new(1, 0));

            let state = blocks.copy_mode_move(state, CopyModeMotion::Up, false);
            let state = blocks.copy_mode_move(state, CopyModeMotion::Up, false);
            assert_eq!(state.cursor.grid, GridType::PromptAndCommand);
            assert_eq!(state.cursor.block_index, block_index);
        })
    })
}
