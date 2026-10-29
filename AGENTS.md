# AGENTS.md

Guidance for agents and contributors working on `skald` itself.

## What this project is

`skald` is a single-binary desktop music player for macOS and Ubuntu 24.04,
built with Rust and [GPUI](https://gpui.rs). It plays FLAC, WAV and MP3, shows a
blurred album-art background, one flat track list, a live spectrogram and a
five-band equalizer. The look follows the four wireframes attached to MNT-242:
1320×768 design frame, 350px sidebar, 761×444 track panel, 37px rows.

State lives in `~/.config/skald/` as JSON: `settings.json` for window, volume
and EQ, `state.json` for the playlist and the last played track. Both are
created on first run.

## Commands

| Command | Purpose |
| --- | --- |
| `make build` | Build the binary into `target/debug/skald`. |
| `make run` | Launch the player. |
| `make test` | Run the unit tests. No test opens a window or touches the audio device. |
| `make check` | `fmt --check`, clippy with `-D warnings`, and tests; the gate before committing. |
| `make release` | Runs the `make check` gate, then builds `target/release/skald`. |
| `make install` | `cargo install --path .`, into `CARGO_HOME/bin`. What `wiki/docs/INSTALL.md` tells users. |
| `make clean` | `cargo clean`. |
| `cargo fmt` | Format. `cargo fmt --check` must be clean. |
| `cargo clippy --all-targets -- -D warnings` | Lint. Warnings are failures. |

## Layout

| Path | Role |
| --- | --- |
| `src/main.rs` | Process entry: logging, fonts, menu wiring, window creation. |
| `src/app.rs` | `SkaldApp`, the root view. Owns the playlist, player handle, overlays and persisted state. |
| `src/actions.rs` | GPUI actions and their key bindings. |
| `src/ui/` | One module per region of the design frame: `shell`, `track_list`, `sidebar`, `now_playing`, `eq_overlay`, `about_overlay`, `add_overlay`, `titlebar`, `theme`. |
| `src/library/` | Playlist model, recursive directory scan, tag and cover extraction. |
| `src/player/` | `engine` (cpal output and commands), `decoder` (symphonia), `dsp` (biquad EQ and FFT taps), `spectrum` (waterfall buffer). |
| `src/settings/` | Paths, JSON schemas, first-run creation and atomic writes. |
| `assets/fonts/` | Bundled Inter (SIL Open Font License). GPUI needs a font it can load on both platforms. |
| `wiki/docs/` | User-facing docs. `INSTALL.md` owns install; `MORE.md` owns shortcuts and the state format. |

## Conventions

- The design frame is the source of truth for geometry. All spacing constants
  live in `src/ui/theme.rs` or next to the view that uses them; do not scatter
  magic numbers.
- The audio callback must never allocate, lock or block. It reads decoded
  frames from a ring buffer and writes output samples. UI notifications go
  through atomics or channels drained by the GPUI timer.
- Decoding happens on the player thread, not the UI thread. Scanning a folder
  happens on a background executor task and reports back in batches.
- Playlist removal only edits the playlist. Never touch the file on disk.
- Every module that can be tested in isolation carries a `#[cfg(test)] mod
  tests` block. Keep tests free of audio devices, network and real sleeps.
- Files in `~/.config/skald` are written through a temp file plus rename, so an
  interrupted write cannot truncate the state.
- Anything the user sees is documented in exactly one place. `README.md` is the
  short front door; `wiki/docs/MORE.md` owns shortcuts and the state format;
  `wiki/docs/INSTALL.md` owns install. Do not restate the same list twice.

## Working on this repository

Releases are cut by CI from a version bump, not by hand: bump `version` in
`Cargo.toml`, merge to `master`, and `.github/workflows/release.yml` builds the
four platform binaries and publishes `v<version>`. The publish job is gated
behind `make check`, so a red tree never releases. `.github/workflows/ci.yml`
runs that gate on macOS and Ubuntu 24.04 for every push and pull request.

Both workflows keep their shell in `.github/scripts/`, one script per step,
rather than inline `run:` blocks. The scripts are `bash`, take their inputs from
the environment rather than arguments, and assume they run from the repository
root.
