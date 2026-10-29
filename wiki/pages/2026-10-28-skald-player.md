# MNT-242: Skald player

Date: 2026-10-28
Ticket: MNT-242

## Context

The ticket asked for a Foobar2000-style desktop player in Rust and GPUI: album
art over a live spectrum in a 350px sidebar, one flat track list, a blurred
album-art backdrop, FLAC/WAV/MP3 playback, a five-band equalizer, and JSON state
under `~/.config/skald`. The four wireframes define a 1320×768 frame; the
accepted build-plan comment on the ticket resolved the details the drawings
leave open.

![Skald playing the dark theme](../images/skald-dark.png)

## What changed

A new single-crate binary, scaffolded to match `manti-by/sk`:

- `src/main.rs` — window, bundled Roboto fonts, the native menu bar, restored
  window bounds and the close handler that flushes state.
- `src/app.rs` — `SkaldApp`: playlist, selection and scroll, playback, cover and
  background loading, spectrum history, EQ drag state, import, persistence and
  the 50 ms UI tick.
- `src/player/` — symphonia decoder, cpal output stream, a lock-free stereo ring
  between the decode thread and the audio callback, biquad EQ, resampling to the
  device rate, and an FFT spectrum tap on the filtered signal.
- `src/library/` — recursive scan, tag and duration extraction, embedded/folder
  cover loading, average-luminance calculation and the blurred backdrop bitmap.
- `src/settings/` — `settings.json` and `state.json` schemas, first-run defaults,
  clamping and atomic temp-file-plus-rename writes.
- `src/ui/` — the shell, sidebar, track list, now-playing row, EQ and About
  overlays, the add-card empty state, the custom titlebar, and `theme.rs` as the
  single source of all design-frame geometry and both palettes.
- `Makefile`, `.github/workflows/{ci,release}.yml`, `.github/scripts/*.sh`,
  `install.sh`, `README.md`, `AGENTS.md`, `wiki/docs/` — the repository and
  release plumbing.

`make check` (fmt, clippy with `-D warnings`, 45 unit tests) is green. The app
was run against embedded-cover FLACs, tagged MP3s and a folder cover; playback,
pause, the waterfall, theme switching and restart recovery were checked by hand
on macOS, while seeking is covered by the decoder tests.

## Decisions

- **Ten data rows, not eleven.** The ticket description says ten and wins over
  the wireframes; the 444px panel keeps its drawn height, so the last 37px is
  empty until an eleventh row would have been drawn.
- **No transport buttons.** Double-click/Enter plays, Space pauses, and the end
  of a track advances; this matches the wireframes, which only show the
  play/pause glyph in the now-playing row.
- **Theme is derived, never chosen.** The average of the cover in linearized
  sRGB decides between the two palettes. The threshold is 0.42, so a cream
  cover reads light and a saturated mid-tone still reads dark.
- **EQ runs in the audio path, not on the UI thread.** Five peaking biquads are
  applied to decoded stereo frames before the ring buffer, and the spectrum taps
  the same filtered samples, so what you see is what you hear.
- **One output stream at the device default rate.** Decoded samples are
  resampled linearly when they differ; this avoids reopening the device for
  every track and keeps the callback allocation-free.
- **Manual list scrolling.** A fixed ten-row viewport plus a custom scrollbar
  matches the wireframe exactly; GPUI's native overflow scrolling was rejected
  because it cannot reproduce the 37px row viewport and 6px track.
- **Spectrum columns are paced, not dumped.** The decoder emits a packet's
  columns in a burst, which made the waterfall jump several pixels at once.
  The UI queues them and releases one column per hop (about 23 ms at 44.1 kHz)
  on a 16 ms tick, so the scroll is continuous and the audio clock sets the
  pace. A one-pixel box blur softens the raster without hiding transients.
- **Playlist wheel direction matches GPUI.** Positive wheel deltas mean "up"
  in GPUI (as in its own list), so the track list subtracts them instead of
  adding.
- **Separate Add Folder and Add Files commands.** The plan's single ambiguous
  "Add…" became `File → Add Folder…` (`Cmd/Ctrl+O`) and `File → Add Files…`
  (`Cmd/Ctrl+Shift+O`), because the scan path handles directories and files
  differently.
- **Fixed 1320×768 minimum.** The design frame is the smallest useful window;
  larger windows stretch the main column while the sidebar and modal sizes stay
  fixed.
- **Custom window chrome.** The OS titlebar is hidden with
  `appears_transparent` and the native traffic lights are moved offscreen;
  `titlebar.rs` draws close/minimize/maximize and owns the top 32px drag strip.
  macOS hands the original mouse-down event to `performWindowDragWithEvent`;
  Linux uses `start_window_move` and client-side decorations.
- **Bundled static Roboto instances.** GPUI needs fonts it can load on both
  platforms without relying on system font discovery. The three faces are
  static instances of the Roboto variable font, named so GPUI groups them as
  one family and resolves Regular, Medium and SemiBold by weight.

## Follow-ups

- CI builds on Ubuntu 24.04 but the app has only been exercised by hand on
  macOS; the first Linux run should check the ALSA default device, X11 vs
  Wayland and the native menu.
- GPUI 0.2.2 has no backdrop blur, so the background is a small bitmap scaled up
  rather than a live gaussian filter. It reads as the wireframe at normal window
  sizes but is not a true blur.
- The EQ panel is centered in the window; the SVG places it 36px right of centre
  at 1320px. Centering was kept so the modal stays put when the window grows.
- Remove-from-playlist is wired to Delete/Backspace and the Playback menu; a
  context menu was out of scope for this ticket.
