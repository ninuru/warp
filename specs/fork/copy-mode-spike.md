# M3 Copy mode (C-a [) spike result (#14)

Throwaway spike code on branch `spike/copy-mode` (from `fork/m2-fixes` ee8b9f8dd), commit
98e1bdc87, 415 added lines including the test. Line references below point into that commit.

## Decision

**Extend the block-list and alt-screen selection; no dedicated copy-mode view.** Confirmed by the
spike:
- A copy-mode cursor is a `WithinBlock<Point>` (block list) or a `Point` (alt screen). Every piece
  of existing code speaks these types: grid seek helpers, `BlockListPoint::from_within_block_point`,
  `selection_to_string`, find matches. A dedicated view would have to re-render scrollback that
  `BlockListElement` already paints, including selection highlight and scroll.
- Visual selection is just `start_selection(anchor)` + `update_selection(cursor)` after each motion;
  yank is `selection_to_string` + `ctx.clipboard().write`. No new model storage beyond the state.
- Keys never reach the PTY or the input: a fixed binding under `id!("CopyMode")` wins for bound
  keys, and two early returns swallow everything else.
- State stays on `TerminalView` (`copy_mode: Option<CopyModeState>`); motions live on `BlockList`
  so they are unit-testable without a view.

## What the spike built (task 1, working code)

- Model: `app/src/terminal/model/blocks/copy_mode.rs`
  - `CopyModeState { cursor, anchor }`, `CopyModeMotion` (h/j/k/l/w/b/e) (:13-27).
  - `copy_mode_start` (:30): last output row of the newest non-hidden block with output.
  - `copy_mode_move` (:45): h/l clamp in the row, j/k step rows and cross grids via the existing
    `seek_up_to_next_grid`/`seek_down_to_next_grid` (blocks.rs:2584/2641, inversion-aware).
  - `copy_mode_toggle_visual` (:100), `copy_mode_sync_selection` (:112): without an anchor the
    cursor is drawn as a one-cell selection; with an anchor the selection spans anchor..cursor with
    sides swapped when the cursor is before the anchor.
- View: `app/src/terminal/view.rs`
  - `enter_copy_mode`/`copy_mode_move`/`copy_mode_toggle_visual`/`copy_mode_yank`/`exit_copy_mode`/
    `scroll_to_copy_mode_cursor` (:23949-24033). Yank only with an anchor, then exit.
  - Keymap context `CopyMode` (:29795). Key guards in `keydown_on_terminal` (:9849) and
    `typed_characters_on_terminal` (:9896). Focus hold: `self.copy_mode.is_some()` is part of
    `should_focus_terminal` in `redetermine_global_focus_with_policy` (:22156).
- Actions: `TerminalAction::{EnterCopyMode, CopyModeMove, CopyModeToggleVisual, CopyModeYank,
  ExitCopyMode}` (view/action.rs:189-193).
- Bindings (view/init.rs:301-351): editable `terminal:enter_copy_mode` (no default key), fixed
  h/j/k/l/w/b/e/v/y/q/escape under `Terminal & CopyMode`. Prefix `[` now maps to
  `terminal:enter_copy_mode` (prefix_keys/config.rs:34).
- Proof: `app/src/terminal/model/blocks/copy_mode_tests.rs`, see "Test" below.

Not verified in the spike: live painting and scrolling in the GUI (no manual run, build budget).
The one-cell-selection cursor reuses the selection renderer, so it should paint; a real block
cursor needs a paint call (follow-up 1).

## Answers to tasks 2-4

**2. Alt screen.** Mapping is trivial: alt-screen selection points are plain grid `Point`s in the
visible grid (`AltScreen::start_selection`/`update_selection`, alt_screen.rs:209/224). The view
already uses them directly for mouse selection (`begin_alt_selection`/`update_alt_selection`, view.rs:18991/19013).
Behaviour under app redraws: selection is rotated on scroll-region scrolls (`rotate_selection`,
alt_screen.rs:353, called from linefeed/scroll_up/scroll_down/insert/delete lines :473-559) and
cleared on resize (:124-128) and on alt-screen entry (terminal_model.rs:2255). Plain repaints
(Neovim redraw, `less` page) do not touch the selection, so it stays on the same cells while the
text under it changes; yank copies what is there at yank time. The copy cursor must be rotated the
same way (store it next to the selection in `AltScreen`, or re-read it from the selection head).
No scrollback in the alt screen: copy mode there is limited to the visible screen, which matches
tmux copy-mode over a full-screen app. Enter currently returns early when the alt screen is active.

**3. w/b/e on grid rows.** Works as-is. `copy_mode_row_text` (copy_mode.rs:148) builds a `String`
from `GridHandler::row(row)` cells; `vim_word_iterator_from_offset` takes `&str` because
`TextBuffer` is implemented for `str` (warpui_core/src/text/mod.rs:268). Column == char offset as
long as each cell yields one char (wide-char spacers are cells too, so the mapping holds). Gotcha
found by the test: never-written cells hold `DEFAULT_CHAR = '\0'` (warp_terminal grid/cell.rs:16),
which the vim iterator treats as a word char, so `e` ran to the row end; the row text must map
`'\0'` to `' '` (copy_mode.rs:154). The test covers w, e and b. Limits: motions stop at the row end (no wrap to the next row or grid yet), and
`row()` indexes storage rows, not displayed rows, so a block filter would misplace them.

**4. `/` search.** Yes. `TerminalFindModel::focused_block_list_match` (find/model.rs:299) returns
`BlockListMatch::CommandBlock(BlockGridMatch { grid_type, range, block_index })`
(find/model/block_list.rs:225); `WithinBlock::new(*range.start(), block_index, grid_type)` is
exactly the copy cursor. `scroll_to_match` (view.rs:24074) already turns it into a scroll. Alt
screen find returns `Vec<RangeInclusive<Point>>` (alt_screen.rs:344), also usable. Caveats: the
async find path stores absolute/original coordinates (`AbsoluteMatch`, async_find.rs:130) that must
go through `to_original_range` (:197); matches in filtered rows are flagged `is_filtered`; rich
content (AI block) matches have no grid point and must be skipped. Plan: `/` and `?` open the
find bar, Enter closes it and moves the cursor to the focused match, `n`/`N` call
`goto_next_find_match` (view.rs:20777) and re-read the focused match.

## Risks found

- **Grid widths across blocks.** `BlockList::resize` only resizes old blocks when
  `update_old_blocks` is true (blocks.rs:2311-2333); the spike clamps with `self.size.columns`, which
  can exceed an old block's width. Use the grid's own `columns()` per cursor grid. Resize also
  clears the selection (:2314) while `copy_mode` survives: exit copy mode on resize.
- **Refocusing the input on new output.** `redetermine_global_focus` runs on block completion and
  would hand focus back to the input; the spike keeps the terminal focused while copy mode is on.
  Still open: clicks, `clear_selected_text` from other paths (mouse down, block selection, find)
  leave a stale `copy_mode`. Every path that clears the text selection must also exit copy mode.
- **Inverted block list.** `seek_*_to_next_grid` take `inverted_blocklist`, so j/k across blocks
  follow the visual order. `copy_mode_start` picks the newest block by index, correct for both
  orders. Not yet tested inverted; add a test with `inverted = true`.
- **Binding precedence.** The matcher tries all editable bindings before any fixed binding
  (warpui_core/src/keymap.rs:441-463). A user editable binding on a plain letter or escape in a
  Terminal context would beat the fixed `CopyMode` bindings. Either register copy-mode keys as
  editable bindings or add `!id!("CopyMode")` to colliding editable bindings.
- **Active long-running block.** Output keeps streaming into the active block; the cursor stays on
  its `WithinBlock` point, but truncation or `clear` can invalidate it. Clamp on every motion.
- **Copy-on-select.** The spike does not call `maybe_copy_selection_to_clipboard`, so moving the
  cursor does not overwrite the clipboard. Keep it that way.

## Follow-up tickets

### 1. (#24) Copy mode core: state, cursor painting, exit paths (block list)
- Move `CopyModeState` and motions from the spike into production code with unit tests.
- Paint a block cursor at `cursor` instead of a one-cell selection (reuse `render_selection` with
  `SelectionCursorRenderLocation::End`, block_list_element.rs:169/2073).
- Exit on resize, mouse down, block selection, focus loss to another pane, block list clear.
- Clamp the cursor per grid width and after truncation.
- Done when: enter, h/j/k/l, v, y, q/Escape work in the GUI, keys never reach PTY or input, and
  state tests pass for normal and inverted block lists.
- Estimate: 250 lines production, 150 test, 1.5 days.

### 2. (#25) Motions: words across rows, line ends, jumps
- w/b/e/W/B/E crossing row and grid boundaries (continue on the next row's text).
- 0, ^, $, g, G, C-u, C-d, H/M/L relative to the viewport.
- Word motions respect displayed rows when a block filter is active.
- Done when: each motion has a test on a mock block list, including wide chars and wrapped lines.
- Estimate: 200 lines production, 150 test, 1 day.

### 3. (#26) Line-wise visual `V` and rect selection
- `V` selects whole rows (`SelectionType::Lines` or expanded start/end columns).
- Optional `C-v` maps to `SelectionType::Rect`.
- `o` swaps anchor and cursor.
- Done when: yank of `V` over two blocks returns the full lines, covered by tests.
- Estimate: 80 lines production, 60 test, 0.5 day.

### 4. (#27) Search in copy mode (`/`, `?`, `n`, `N`)
- `/` and `?` open the find bar with direction; Enter returns focus to copy mode.
- Cursor jumps to `focused_block_list_match` (sync and async paths, original coordinates).
- `n`/`N` step through matches and move the cursor; skip rich content matches.
- Done when: a test drives a find run on a block list and asserts cursor points for n/N.
- Estimate: 120 lines production, 80 test, 1 day.

### 5. (#28) Copy mode on the alt screen
- `CopyPoint::AltScreen(Point)` state; enter works when the alt screen is active.
- Cursor rotation with the alt-screen selection on scroll; exit on resize and alt-screen exit.
- Same motions over the visible screen via `AltScreen::grid_handler()`.
- Done when: copy mode over `less` and Neovim yanks the expected text in a manual check and in an
  alt-screen model test.
- Estimate: 150 lines production, 80 test, 1 day.

### 6. (#29) Key configuration and discoverability
- Copy-mode keys as editable bindings (avoids the editable-before-fixed precedence problem).
- Command palette entry "Enter copy mode"; mode indicator in the pane (e.g. "COPY" badge).
- Prefix `[` default stays `terminal:enter_copy_mode`.
- Done when: keys are rebindable in Settings, the palette entry works, and the badge shows.
- Estimate: 100 lines production, 0.5 day.

## Test

`copy_mode_moves_selects_and_yanks_block_output` inserts a block with command `echo` and output
`ab cd\nef gh` (7-column test grid), enters copy mode (cursor at output row 1, col 0), then k, w
(selection `c`), v, e (`cd`), j, h (`cd\nef g`), b (col 0), k, k (lands in the PromptAndCommand
grid). The yank text is asserted through `selection_to_string`, which `copy_mode_yank` writes to
the clipboard.

```
cargo nextest run -p warp --features gui,prefix_keys,project_sessions --build-jobs 1 -j 2 \
  -E 'test(/copy_mode_moves/)'
    PASS [   0.021s] (1/1) warp terminal::model::blocks::copy_mode::tests::copy_mode_moves_selects_and_yanks_block_output
     Summary [   0.028s] 1 test run: 1 passed, 7613 skipped
```
The first run (filter `copy_mode`) failed at `e` (col 6 instead of 4) because of the `'\0'` cells;
fixed, then rerun as above. `cargo fmt -p warp` ran after. The view part (actions, bindings, focus,
key guards) is covered only by `cargo check`, not by a GUI run.
