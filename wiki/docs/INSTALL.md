# Skald — Install

Skald ships as one binary. This page owns the installation options; the
shortcuts and the on-disk state format live in [MORE.md](MORE.md).

## Requirements

- macOS 13 or newer, or Ubuntu 24.04.
- Linux needs an X11 or Wayland session and ALSA (PipeWire and PulseAudio both
  provide an ALSA device).
- FLAC, WAV and MP3 files. Cover art is read from embedded tags, then
  `cover.jpg`, `cover.jpeg`, `cover.png`, `folder.jpg`, `folder.jpeg` or
  `folder.png` in the track's folder.

Nothing else is needed at runtime: the binary embeds its only font and has no
runtime data files.

## Install script

```console
$ curl -LsSf https://raw.githubusercontent.com/manti-by/skald/master/install.sh | sh
```

The script downloads the newest release for your machine, verifies its SHA-256
and installs into `~/.local/bin`. Use `--dir <path>` or `SKALD_INSTALL_DIR` to
change that, and `--version v0.1.1` to pin a release. Run it with `--help` for
the whole environment list.

## From source

The build needs a stable Rust toolchain. On Ubuntu 24.04 install the native
dependencies first:

```console
$ sudo apt-get install -y build-essential libasound2-dev libfontconfig1-dev \
    libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev \
    libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

Then, on either platform:

```console
$ git clone https://github.com/manti-by/skald.git
$ cd skald
$ make release
$ ./target/release/skald
```

`make install` runs `cargo install --path .` and puts `skald` in
`$CARGO_HOME/bin`.

## First run

Skald creates `~/.config/skald/settings.json` and
`~/.config/skald/state.json` on first launch, then opens an empty playlist.
Use **File → Add Folder…** (`Cmd+O` / `Ctrl+O`) to add a folder, or
**File → Add Files…** (`Cmd+Shift+O` / `Ctrl+Shift+O`) to add individual
tracks. Folders are scanned recursively.

## Uninstall

Remove the binary and, if you want to drop the saved playlist and settings,
`~/.config/skald`:

```console
$ rm -rf ~/.config/skald ~/.local/bin/skald
```
