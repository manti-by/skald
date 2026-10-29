# media/ — icon source assets

| File | Purpose |
|---|---|
| `skald.svg` | **The** Skald icon: a single angular stroke, black ink on a transparent 171×215 canvas. Source for every icon asset the app ships. |

Nothing is hand-edited under `assets/` — those files are generated.

## Origin & license

- Artwork authored for this project (one `path`, `stroke-width="20"`, no fill).
  Project license: MIT (see the repository root).

## How this file is used

Every icon asset is **generated** from this one SVG by `make icons`, which runs
`scripts/make-icons.sh`; never edit the PNGs or the `.icns` by hand:

| Generated file | Made by |
|---|---|
| `assets/icon.icns` (macOS app bundle icon) | `scripts/make-icons.sh`: `svg-glyph.py` → rasterize → `png-template.py` → iconset → `.icns` |
| `icons.iconset/` (scratch, git-ignored) | the same script, passed to `iconutil` |

The `.app` bundle that carries the icon is built by `make app`
(`scripts/bundle.sh`), and the macOS release archive ships that bundle next to
the bare `skald` binary.

Two normalization steps sit between the art and the rasterizer:

- `scripts/svg-glyph.py` squares the canvas and adds a 9% macOS margin, so the
  glyph is never drawn edge-to-edge. It also forces any white ink to black,
  which keeps the pipeline independent of the colour the art was authored in.
- `scripts/png-template.py` converts the rasterized ink to alpha. `qlmanage`
  draws on opaque white; without this the icon is a white square in the Dock.

If the app icon ever changes, replace `media/skald.svg` here and re-run
`make icons` — do not edit derived files under `assets/`.
