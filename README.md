# tb — horizontal tree file browser

Terminal clone of Conrad Barski's (@lisperati) file browser: the tree grows
left→right, every opened folder fans its children out as a column, many
branches stay open at once, joined by elbow connectors.

The selection line is fixed: root → cursor runs as one straight horizontal
line through mid-screen (continuing past the cursor through each folder's
last-visited child). Moving up/down scrolls that column about the line;
the selector never moves, the tree does.

Branch character: the line is a double "tube" (like the original's hollow
cables) with proper junctions where branches cross it; every other branch is
tinted by the recursive heat of the folder it grows from, so recent work
glows through the wiring; a light sweeps along the line into the cursor on
each move (tail length follows speed); closed folders carry a small `›` bud.

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
· J/K jump 10 · g/G first/last · . dotfiles (hidden by default) · - reroot up
· c collapse others · r reload
· ? help · q / esc quit. Mouse: click selects, click again opens, wheel moves/scrolls.

## Shell
`!` opens a command line in the status bar; enter runs it in the selected folder
(a file's own folder) through `$SHELL -ic`, so aliases and rc functions work.
`$f` is the selected path (`!vim $f`). `s` opens a full shell there. Either way
tb hands over the terminal and comes back exactly where it was when the program
exits (ctrl-c, ctrl-d, :q ...), with that folder reloaded. One-liners that finish
in under 3 s wait for a key so their output can be read. Up/down recall earlier
commands; ctrl-u/ctrl-w erase; esc cancels.
Ctrl-Z inside a `!` command ends that program rather than pausing it (its shell
exits underneath it); inside `s` it is ordinary job control.

`source ~/tree-browser/tb.bash` in `~/.bashrc` wraps `tb` so that quitting with
`q` leaves the shell in the selected folder; esc / ctrl-c leave it where it was.
Underneath: `tb --cwd-file PATH` writes that folder to PATH on `q`.

Preview: markdown via `glow`, everything else via `bat`, binaries via `file`.
j/k, space/PgDn, ctrl-d/u, g/G scroll; q/esc/h close.

Images (png jpg gif webp bmp tiff ico svg avif heic ...) and PDFs preview as
pictures: PDFs one page at a time (`pdftoppm`), j/k/space flip pages, g/G
first/last. Formats the `image` crate can't decode go through ImageMagick
`convert`. Pixels use whatever the terminal answers to the startup query:
kitty graphics (Ghostty, Kitty, herdr), sixel, iTerm2; otherwise unicode
half-blocks, which work in any truecolor terminal (Termius, tmux). `i` flips
between pixels and half-blocks. Inside herdr tb starts in half-blocks: herdr
claims kitty for every attached client whatever terminal it draws into, so
press `i` (or set `TB_GRAPHICS=kitty`) when that terminal really is
Ghostty/Kitty. Inside tmux the query is skipped and half-blocks are used.
