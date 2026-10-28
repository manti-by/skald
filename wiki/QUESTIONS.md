# Questions

Open questions about Skald, and the answers once they are settled.

## Resolved

### Which way should the light theme go?

The wireframes only vary the cover art. The `light` frame still uses the dark
panel and white text, while the ticket's build plan calls for a light palette
derived from a bright cover. Implementation follows the build plan: a bright
cover swaps the overlay to white, the panel to white at 50%, and text to near
black. The dark palette matches the wireframes exactly.

### Transport buttons?

The plan's confirmed decisions say no transport buttons, even though the
wireframe has a play triangle in the now-playing row. Playback is driven by
double-click/Enter, Space, and the progress bar. No transport row is drawn.

### Visible rows?

10 data rows plus a header. The wireframe shows 11; the ticket description wins.
The panel keeps the wireframe's 444px height, so the extra 37px is breathing
room below the last row.
