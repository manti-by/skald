# Skald — Reference

Shortcuts, the state format and the supported formats. Installation is in
[INSTALL.md](INSTALL.md); this page owns everything else.

## Shortcuts

| Action | macOS | Linux |
| --- | --- | --- |
| Add a folder (recursive) | `Cmd+O` | `Ctrl+O` |
| Add files | `Cmd+Shift+O` | `Ctrl+Shift+O` |
| Equalizer | `Cmd+E` | `Ctrl+E` |
| About | `Cmd+I` | `Ctrl+I` |
| Play / pause | `Space` | `Space` |
| Play the selected row | `Enter` or double-click | `Enter` or double-click |
| Previous / next row | `Up` / `Down` | `Up` / `Down` |
| Select a range | `Shift`-click | `Shift`-click |
| Add or remove one row from the selection | `Cmd`-click | `Ctrl`-click |
| Remove the selection from the playlist | `Delete` / `Backspace` | `Delete` / `Backspace` |
| Seek | click or drag the progress bar | click or drag the progress bar |
| Volume | click or drag the volume bar | click or drag the volume bar |
| Close the equalizer or About | `Esc` or click outside | `Esc` or click outside |
| Quit | `Cmd+Q` | `Ctrl+Q` |

Removing a row never deletes the file. Missing files are dropped from the
playlist when Skald starts, so a moved library does not leave dead rows.

## Supported formats

| Container | Codecs | Notes |
| --- | --- | --- |
| FLAC | FLAC | 16/24-bit, any sample rate |
| WAV | PCM (integer and float) | Any channel count |
| MP3 | MPEG Layer III | CBR and VBR, ID3v1/ID3v2 tags |

Cover art is read from the audio file's embedded picture tag. If there is none,
Skald looks for `cover.jpg`, `cover.jpeg`, `cover.png`, `folder.jpg`,
`folder.jpeg` or `folder.png` beside the track. JPEG and PNG are supported.

The row shows the file's average bitrate and the now-playing line shows the
decoded format, sample rate and channel layout, for example
`FLAC 2117kbps 44100Hz Stereo`.

## State

Both files are JSON and are written atomically: a temp file in the same
directory is renamed over the old one, so an interrupted write cannot truncate
the state. They are created with defaults on first run.

### `~/.config/skald/settings.json`

```json
{
  "window": {
    "x": 120.0,
    "y": 90.0,
    "width": 1320.0,
    "height": 768.0
  },
  "volume": 0.8,
  "eq": [0.0, 0.0, 0.0, 0.0, 0.0]
}
```

`eq` is five gains in dB, in the order 63 Hz, 250 Hz, 1 kHz, 4 kHz, 16 kHz,
each clamped to ±6 dB. On Linux the config directory honours
`$XDG_CONFIG_HOME`; macOS always uses `~/.config/skald` per the ticket.

### `~/.config/skald/state.json`

```json
{
  "tracks": [
    "/Users/me/Music/Album/01 - Track.flac",
    "/Users/me/Music/Album/02 - Track.mp3"
  ],
  "last_track": "/Users/me/Music/Album/01 - Track.flac",
  "last_position_ms": 48250
}
```

Only file paths are stored; the tags are read again at startup. Paths that no
longer exist are skipped. The last track and position are restored on launch,
paused, so the player never starts making noise by itself.
