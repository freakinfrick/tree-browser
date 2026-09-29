# tb — horizontal tree file browser

Terminal clone of Conrad Barski's (@lisperati) file browser: the tree grows
left→right, every opened folder fans its children out as a column, many
branches stay open at once, joined by elbow connectors.

Motion is physics-driven: every node rides a critically damped spring
(SmoothDamp: no overshoot, velocity carries through retargets). Expanding
unfurls children out of the parent, collapsing sucks them back in and fades
them out, siblings glide aside, and connectors are re-routed every frame from
the animated positions. Camera, highlight bar, preview popup (grows out of
the cursor row, dims the backdrop) and scrolling all ease the same way.
60 fps while anything moves, zero frames when idle.

    tb [DIR]        # default: current dir; starts rooted at DIR's parent

## Color = recency
Files: last modified. Folders: newest modification *anywhere inside* (walked
recursively on a background thread, capped at 50k entries; a trailing `~`
means the cap was hit and the color may be too cold).

Continuous log-time gradient: red (minutes) → orange (hours) → tan (days)
→ grey (weeks) → slate (a year) → blue (5y+). Colors cross-fade when heat
data lands instead of popping.
White = cursor path; dim = branches off the cursor path.

## Keys
hjkl / arrows move · l/enter open (dir: expand, file: preview) · space fold
· J/K jump 10 · g/G first/last · - reroot up · c collapse others · r reload
· ? help · q quit. Mouse: click selects, click again opens, wheel moves/scrolls.

Preview: markdown via `glow`, everything else via `bat`, binaries via `file`.
j/k, space/PgDn, ctrl-d/u, g/G scroll; q/esc/h close.
