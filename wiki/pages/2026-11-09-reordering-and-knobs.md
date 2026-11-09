# MNT-242: Skald follow-ups — reordering and progress knobs

Date: 2026-11-09
Ticket: MNT-242 (follow-up session, branch `feature/mnt-242-reorder-and-knobs`, PR to `master`)

## Context

Two small asks arrived after the original cut: the progress and volume bar
handles should be circles rather than 14px stubs, sized 8–12px from the bar
height; and rows in the playlist should be movable, even though reordering
sits in the build plan's out-of-scope list ("playlists management") and was
never granted a scope ticket of its own. Both changes keep to the MNT-242
conventions: geometry constants live next to the view that uses them, and
everything the user can touch is documented once, in `wiki/docs/MORE.md`.

## What changed

- `src/ui/now_playing.rs` — `progress_thumb(fraction, bar_height, color)`
  renders a circular handle centred on the end of the fill, on a zero-width
  anchor at `left(relative(fraction))` offset by half the knob so its centre
  always tracks the fill edge. `thumb_diameter` grows with the bar height and
  clamps to 8–12px (6px bars → 10px knobs today). The volume bar's old
  `fraction.max(0.06)` minimum stub is gone; the knob is always visible on
  its own. Drag and click behaviour is untouched.
- `src/actions.rs` — `MoveSelectedUp` / `MoveSelectedDown`, bound to
  `Alt+Up` / `Alt+Down` (Option on macOS), plus a strengthened shortcut test
  that asserts the bindings and their modifiers.
- `src/app.rs` — `move_selected(delta)` which permutes `tracks`, keeps the
  selection on the same rows, and remaps `current` so a playing track keeps
  playing. The pure step function `move_selected_order` carries its own
  unit tests, per the convention that testable helpers are tested in
  isolation.
- `src/main.rs` — "Move Selected Up" / "Move Selected Down" entries in the
  Playback menu, next to the shortcut so they never drift apart.
- `wiki/docs/MORE.md` — the new shortcut and the note that the new order
  lands in `state.json`, so a moved playlist survives a restart.

`make check` (fmt, clippy `-D warnings`, 49 unit tests) is green.

## Decisions

- **Keyboard move first, no row drag.** The plan kept row drag-and-drop out
  of scope and GPUI gives no list dragging primitives. `Alt+Up` / `Alt+Down`
  moves the whole selection one row per press — blocks keep their internal
  order, non-adjacent rows each take one step, and rows at the list edge
  stay put. Row dragging remains a future ticket.
- **The playing track follows its row.** Moving rows only touches the model;
  the player holds the path it was given, so a track keeps playing when its
  index changes. `current` is remapped through the same permutation, so
  auto-advance stays in step with what the user sees.
- **No-op epochs are cheap.** If the step function leaves the order
  untouched (the selection is already at the edge) the method returns before
  cloning or touching `state.json`, so a key held down at the top of the list
  does no work.
- **Knob = height + 4px, clamped.** A helper against a bar height keeps the
  sizing rule in one place; if a bar ever grows past 8px the knob still never
  exceeds 12px.

## Follow-ups

- Mouse/row drag-and-drop onto the list still makes sense for large
  rearrangements; it needs a manual drop model since GPUI has no list drag
  helper.
- The knob half-overhangs the bar ends at 0% and 100%; the track-list panel
  clips at its own edges, so rechecking the window geometry is optional.
- Menu items display the `Alt+Up` / `Alt+Down` shortcut glyph as macOS renders
  it; worth a glance on Ubuntu where `alt` maps to a different keycap.
