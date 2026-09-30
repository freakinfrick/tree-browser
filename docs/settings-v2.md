# Settings v2: ten more rows in the `,` menu

Spec for one build. Every new setting follows the existing pattern in
`src/settings.rs`: a field on `Settings`, an `Item` row, and arms in `show`,
`store`, `adjust`, `reset`, `set`. The config file keeps working unchanged;
new keys are optional and default to today's behavior unless noted.

## Layout

**Column width** (existing `max_name`, relabeled). Label "Name width" becomes
"Column width". Config key stays `max_name` so existing files still load.
Help: "Widest a column gets; longer names are cut with a …".

**Columns** `columns = "fit" | "equal"`, default `fit`.
- fit: today. A column is as wide as its longest name, up to Column width.
- equal: every column is exactly Column width wide, so the tree is a grid and
  columns don't shift when folders open. Change is in `layout.rs` where
  `colw` is computed.

**Name details** `details = "off" | "age" | "size" | "both"`, default `off`.
Dim suffix after each name, counted in the column width:
- age: `now 5m 3h 2d 4w 8mo 3y` (folders: recursive mtime, same as heat).
- size: the existing `human()` format (folders: recursive size).
- both: `2d · 12M`.

## Order

**Folders first** `folders_first = bool`, default `false`. Folders above files
for every sort key. Type sort already does this and is unaffected.

**Natural sort** `natural_sort = bool`, default `true`. Digit runs compare as
numbers: `file2` before `file10`. Applies wherever names are the tiebreak in
`Tree::cmp`.

## Look

**Heat range** `heat_range = "day" | "week" | "month" | "year" | "5y"`,
default `5y` (today's gradient). The age at which a file reaches the coldest
color. Implemented as one scale on age before `anim::heat` looks it up:
`age * (5y / range)`. Legend unchanged.

**Tree lines** `lines = "rounded" | "square" | "heavy" | "double" | "ascii"`,
default `rounded`. Swaps the junction table in `layout.rs:89` and the plain
`─` in `ui.rs:986`. ascii: `+ | -`.

## Behavior

**Mouse** `mouse = bool`, default `true`. Off releases the mouse so the
terminal can select text. Applies live (Enable/DisableMouseCapture), and
`shell.rs` must not turn it back on after `s`.

**Image previews** `graphics = "auto" | "pixels" | "blocks" | "off"`, default
`auto` (today's detection). `TB_GRAPHICS` still wins for the run it's set in.
The `i` key in a preview keeps toggling for that preview only.

**Text preview** `preview = "styled" | "bat" | "plain"`, default `styled`.
- styled: today (glow for markdown, bat otherwise).
- bat: bat for everything.
- plain: raw text, no external programs.

**Wrap lines** `wrap = bool`, default `true`. Off cuts long lines at the edge
(bat `--wrap=never`, plain truncates). glow always wraps.

**Remember place** `remember = bool`, default `false`. On quit, save the open
folders and the selection for the starting folder; on the next `tb` in that
folder, reopen them. Stored in `$XDG_STATE_HOME/tb/places` (else
`~/.local/state/tb/places`), newest 50 folders kept. Paths that no longer
exist are skipped.

## Menu order

Layout: Row spacing, Column gap, Column width, Columns, Name details
Order: Sort by, Reverse, Folders first, Natural sort, Dotfiles
Look: Accent, Heat colors, Heat range, Tree lines, Legend, Motion
Behavior: Live updates, Ripples, Git status, Dim ignored, Mouse, Image
previews, Text preview, Wrap lines, Remember place

25 rows; the panel already scrolls to the selection.

## Done when

- `cargo test` passes, with a parse/store round-trip test per new key and unit
  tests for natural sort, equal columns, heat scaling, age/size formatting.
- `cargo clippy` clean.
- `-h` text and the `?` help mention anything a user would look for
  (Mouse, Remember place).
- README settings section updated.
- Each setting checked by eye in a real `tb` run.
