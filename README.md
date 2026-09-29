# tb — horizontal tree file browser

Terminal clone of Conrad Barski's (@lisperati) file browser: the tree grows
left→right, every opened folder fans its children out as a column, many
branches stay open at once, joined by elbow connectors. The camera glides
to follow the cursor.

    tb [DIR]        # default: current dir; starts rooted at DIR's parent

## Color = recency
Files: last modified. Folders: newest modification *anywhere inside* (walked
recursively on a background thread, capped at 50k entries; a trailing `~`
means the cap was hit and the color may be too cold).

red <1h · orange <1d · tan <1w · grey <1mo · slate <1y · blue older.
White = cursor path; dim = branches off the cursor path.

## Keys
hjkl / arrows move · l/enter open (dir: expand, file: preview) · space fold
· J/K jump 10 · g/G first/last · - reroot up · c collapse others · r reload · q quit

Preview: markdown via `glow`, everything else via `bat`, binaries via `file`.
j/k, space/PgDn, ctrl-d/u, g/G scroll; q/esc/h close.
