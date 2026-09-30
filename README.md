<div align="center">

# tb

**A horizontal tree file browser for the terminal.**
The tree grows left → right, every folder you open fans out as a new column,
and color tells you where work happened recently.

[![CI](https://github.com/freakinfrick/tree-browser/actions/workflows/ci.yml/badge.svg)](https://github.com/freakinfrick/tree-browser/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange?logo=rust)](https://www.rust-lang.org)
![Linux | macOS](https://img.shields.io/badge/platform-linux%20%7C%20macOS-lightgrey)

<a href="docs/demo.mp4"><img src="docs/demo.gif" alt="tb highlight reel: heat colors, explode, audio waveform, GIF preview, a live change re-heating its folders, mouse wheel, git diff, bat, a shell, find, sort and settings" width="860"></a>

<sub>36-second highlights · <a href="docs/demo.mp4">full demo video (77 s, 1080p60)</a></sub>

</div>

## Highlights

- **Many branches open at once.** Open folders stay open side by side, joined by elbow connectors, like
  [Conrad Barski's](http://www.lisperati.com/) file browser that inspired it.
- **Heat colors from recursive mtime.** A folder is colored by the newest change *anywhere* inside it,
  so you can spot where the action is from the top of the tree.
- **Live.** Open folders update as files come, go and change (about once a second), and the heat
  climbs the tree as you work. Each change flashes where it happened and ripples up the connectors
  through every folder above it, so a build or an agent working in the tree shows up at a glance.
  Set `TB_LIVE=off` to turn it off.
- **A fixed selection line.** Root → cursor is always one straight line through mid-screen.
  Moving up and down scrolls the column through the line; the tree moves, the selector doesn't.
- **Physics-driven motion.** Every node rides a critically damped spring: folders unfurl and fold back,
  siblings glide aside, and connectors re-route every frame. 60 fps while moving, zero frames when idle.
- **Rich previews.** Markdown through `glow`, code through `bat`, and images and PDF pages as real pictures
  over kitty, sixel or iTerm2 graphics, with a half-block fallback for any truecolor terminal.
- **Sound.** Audio files play the moment you open them, over a waveform with a scrubber you can
  click or drag, and simple keys for pause, seek and volume.
- **Git aware.** Changed files carry a marker, closed folders show what's changed inside, ignored
  build output fades back, the status bar names the branch, and `d` in a preview shows the diff.
- **Yours to tune.** `,` opens 28 settings: spacing, column width, age and size after names, sort
  order, accent and heat colors, heat range, line style and branch length, motion, previews, the mouse, reopening
  where you left off, and every feature switch, saved to a small config file.
- **Shell without leaving.** `!` runs a command in the selected folder, `s` opens a shell there, and
  `q` can leave your shell `cd`'d to wherever you ended up.

## Install

```sh
cargo install --git https://github.com/freakinfrick/tree-browser
```

or from a clone:

```sh
git clone https://github.com/freakinfrick/tree-browser
cd tree-browser
cargo install --path .     # puts `tb` in ~/.cargo/bin
```

**Requirements:** Rust 1.88+, a Unix-like OS (Linux, macOS, BSD; Windows isn't supported) and a
truecolor terminal. The build needs no system packages: on Linux and the BSDs, sound plays through the
`libasound.so.2` already on the system (loaded when a preview opens; without it previews stay
quiet), and `cargo install --no-default-features ...` leaves sound out entirely. Previews use these tools from `PATH` when they're installed:

| Tool | Used for |
|---|---|
| [`bat`](https://github.com/sharkdp/bat) | syntax-highlighted text |
| [`glow`](https://github.com/charmbracelet/glow) | rendered markdown |
| `file` | describing binaries |
| `pdfinfo`, `pdftoppm` ([poppler](https://poppler.freedesktop.org/)) | PDF pages |
| `convert` ([ImageMagick](https://imagemagick.org/)) | image formats the `image` crate can't decode |

## Usage

```sh
tb [DIR]                  # default: current dir; starts rooted at DIR's parent
tb --cwd-file PATH [DIR]  # on q, write the selected folder to PATH
```

### Keys

| Key | Action |
|---|---|
| `j` `k` | move down / up through the whole open tree: into an open folder, then on to its next sibling |
| `↓` `↑` | move straight down / up the column |
| `J` `K` / `PgDn` `PgUp` | jump 10 |
| `g` `G` / `Home` `End` | first / last |
| `l` `→` `Enter` | open: expand a folder, preview a file |
| `h` `←` | back to the parent |
| `Space` `Tab` | fold / unfold |
| `/` | find in the current column as you type; `Enter` opens the match, `Esc` goes back |
| `Tab` `↓` / `⇧Tab` `↑` | while finding: next / previous match (`↑` on an empty line recalls the last find) |
| `n` `N` | next / previous match of the last find |
| `-` `Backspace` | re-root one level up |
| `c` | collapse everything off the cursor path |
| `e` | explode: open every folder inside the selected one (a file's own folder); `Esc` stops it |
| `.` | show / hide dotfiles (hidden by default) |
| `o` | cycle the sort: name → newest → largest → type |
| `O` | reverse the sort |
| `r` | reload, re-walking heat deep inside closed folders (open ones update live) |
| `!` | run a shell command in the selected folder |
| `s` | open a shell in the selected folder |
| `,` | settings |
| `?` | help overlay |
| `q` | quit (and `cd` there, with the shell integration below) |
| `Esc` `Ctrl-C` | quit and stay where you were |

**Mouse:** click selects, click again opens. The wheel scrolls the column under the pointer: over any other
column, or another open folder's list, the first tick takes it over (a faint pill marks it on hover)
and the next ones scroll it. A quick flick glides on after you stop; any key or click halts it, and
slow notches stay one step each (**Wheel speed** and **Momentum** in the settings). Turn **Mouse** off
in the settings to select text with it instead.

**In a preview:** `j` `k`, `Space` `PgDn`, `Ctrl-D` `Ctrl-U`, `g` `G` scroll text; for images and PDFs
`j` `k` `Space` flip pages and `g` `G` jump to the first / last. `i` switches between pixels and
half-blocks. `d` shows a changed file's git diff (and back). `q` `Esc` `h` close.

**In an audio preview:**

| Key | Action |
|---|---|
| `Space` `p` `Enter` | pause / play (at the end: play again from the start) |
| `←` `→` / `h` `l` | back / forward 5 s |
| `⇧←` `⇧→` / `H` `L` / `PgUp` `PgDn` | back / forward 30 s |
| `0` … `9` | jump to 0% … 90% |
| `g` `Home` | back to the start |
| `↑` `↓` / `+` `-` | volume up / down |
| `m` | mute |
| `q` `Esc` | stop and close |

Click or drag on the waveform or scrubber to seek there; the wheel seeks 5 s.

### Explode

`e` is the opposite of `c`: it opens every folder inside the selected one, all the way down, and
they unfurl together. The folders are read on a background thread. While that runs, a spinner turns
where the folder's `›` bud sits and the status bar counts folders; `Esc` stops it. The walk goes a
level at a time and stops after 400 folders or 6,000 entries, so a huge tree opens its top levels
rather than flooding the screen, and the status bar says when it stopped early. Hidden folders
(unless dotfiles are shown) and folders git ignores, like `node_modules/` and `target/`, are listed
but left closed. Symlinks are never followed. `c` folds everything back up.

## Color = recency

Files are colored by when they were last modified. Folders are colored by the newest modification
*anywhere inside*, walked recursively on a background thread and capped at 50k entries (a trailing
`~` means the cap was hit, so the color may be too cold).

The gradient is continuous in log-time:

**red** (minutes) → **orange** (hours) → **tan** (days) → **grey** (weeks) → **slate** (a year) → **blue** (5 y+)

Colors cross-fade when heat data lands instead of popping. White marks the cursor path and dim
marks branches off it. The line itself is a double "tube" with proper junctions; every other branch
is tinted by the heat of the folder it grows from, a light sweeps along the line into the cursor on
each move, and closed folders carry a small `›` bud.

When an open folder changes on disk, the entry that changed flashes with an ember behind its name.
The flash then climbs its elbow to the folder, and on up a level every 90 ms, dimming as it goes, before
everything settles back to its heat color. A deleted entry flashes the folder it left.

## Sorting

Folders list their entries by name to start with (case-insensitive, folders and files mixed).
`o` steps through the other orders, and `O` flips the current one:

| Order | First | Folders count |
|---|---|---|
| name | a → z | by their own name |
| modified | newest | by the newest change anywhere inside, like their color |
| size | largest | by the total size of everything inside |
| type | folders, then files by extension | as a group ahead of files |

Ties fall back to the name. The sort applies to every open folder at once, and the status bar shows
it (`⇅ largest first`) whenever it isn't plain name order. Folder sizes come from the same background
walk as the heat, so a folder's size can change after it opens, and entries slide into place as
walks finish. The status bar shows the total too, with a trailing `+` when the walk hit its cap. By
modified, live updates lift freshly changed files to the top as you work.

To start in a different order, set `TB_SORT` to `name`, `modified`, `size` or `type`, with a leading
`-` to reverse it (`TB_SORT=-size` puts the smallest first).

## Git

Inside a git repo, each file with changes gets a one-letter marker after its name:

| Marker | Meaning |
|---|---|
| `M` (yellow) | modified in the worktree |
| `+` (green) | staged, nothing more on top |
| `?` (cyan) | untracked |
| `!` (pink) | merge conflict |

A closed folder's `›` bud takes the color of the most urgent change anywhere inside it, so you can
follow a change down from the top. Ignored files and folders (`target/`, `node_modules/`) are dimmed.
The status bar shows the branch with ahead/behind counts (`⎇ main ↑1`) and the selected entry's
state. When a changed file is open in the preview, `d` switches between the file and its diff against
`HEAD`, staged and unstaged changes together.

Status comes from `git status` on a background thread, for the repos that the open folders are in. It
re-runs when files change, on `r`, and every 3 s to catch commits and `git add`. It runs with
`--no-optional-locks`, so it never takes the index lock from your own git commands. Set `TB_GIT=off`
to turn it off.

## Settings

`,` opens the settings on the right, over the tree, so each change shows as you make it. `j` `k` move,
`h` `l` (or `←` `→`, `Enter`, `Space`) change the value, `r` puts it back to the default, and `Esc` or `,`
closes the panel.

| Setting | Values | Default |
|---|---|---|
| Row spacing | blank rows between entries, 0–3 | 0 |
| Column gap | space before the next column, 3–12 | 3 |
| Column width | widest a column gets; longer names are cut with `…`, 12–60 | 28 |
| Columns | fit (as wide as the longest name), equal (every column the column width) | fit |
| Name details | off, age, size, both, dimmed after each name | off |
| Sort by, Reverse | same as `o` and `O` | name, off |
| Folders first | folders above files under every sort | off |
| Natural sort | `file2` before `file10` | on |
| Dotfiles | same as `.` | hidden |
| Accent | indigo, teal, violet, amber, mono (lines, selector, highlights) | indigo |
| Heat colors | ember (red → blue), aurora (yellow → purple, avoids red-green), mono | ember |
| Heat range | age that gets the coldest color: day, week, month, year, 5y | 5y |
| Tree lines | rounded, square, heavy, double, ascii | rounded |
| Branch offset | line between each join and its name, 0–4 (`├name` → `├──name`) | 0 |
| Legend | the color key in the status bar | on |
| Motion | slow, normal, fast, instant (no animation) | normal |
| Live updates, Ripples, Git status, Dim ignored | on / off | on |
| Explode ignored | let `e` open git-ignored folders too (a folder you explode directly always opens) | off |
| Step through | what `j` `k` `J` `K` walk: folder (its own list), column (every open list in the column), tree (the whole open tree in reading order); `g` `G` always stay in the folder; arrows and `PgUp` `PgDn` move straight up and down the column, and the wheel scrolls the column under the pointer | tree |
| Mouse | off gives the mouse back to the terminal for selecting text | on |
| Wheel speed | entries per wheel notch, 1, 2, 3, 5; text previews scroll three lines for each | 1 |
| Momentum | how far a quick flick glides on after the last notch: off, short, medium, long | short |
| Image previews | auto, pixels, blocks (half-blocks), off (captions only) | auto |
| Text preview | styled (glow for markdown, bat for the rest), bat, plain | styled |
| Wrap lines | off cuts long lines at the preview's edge | on |
| Remember place | reopen the folders and selection you left, per starting folder | off |

Remember place keeps the last 50 starting folders in `~/.local/state/tb/places` (under
`$XDG_STATE_HOME` if that's set).

Every change is saved immediately to `~/.config/tb/config.toml` (under `$XDG_CONFIG_HOME` if that's
set, or wherever `TB_CONFIG` points). It's plain `key = value` TOML that you can also edit by hand. tb
rewrites only the line for the setting you changed, so your comments stay. If a line can't be read,
tb uses the default for that setting and lists the problem at the bottom of the settings panel.

```toml
row_spacing = 1
accent = "teal"
palette = "aurora"
sort = "modified"
```

`TB_SORT`, `TB_LIVE=off`, `TB_GIT=off` and `TB_GRAPHICS` override the file for that run.

Text size isn't a setting: a terminal program can't change its font. Use your terminal's zoom
(usually `Ctrl`/`Cmd` and `+` / `-`) and tb re-lays itself out to fit.

## Shell integration

`!` opens a command line in the status bar. `Enter` runs it in the selected folder (a file's own
folder) through `$SHELL -ic`, so aliases and rc functions work, and `$f` is the selected path
(`!vim $f`). `s` opens a full interactive shell there instead.

Either way tb hands over the terminal and comes back exactly where it was when the program exits,
with that folder reloaded. One-liners that finish in under 3 s wait for a key so you can read their
output. In the prompt, `↑` `↓` recall earlier commands, `Ctrl-U` `Ctrl-W` erase, and `Esc` cancels.
`Ctrl-Z` inside a `!` command ends that program rather than pausing it (its shell exits underneath
it); inside `s` it's ordinary job control.

To make `q` leave your shell in the folder you ended on, source the wrapper from `~/.bashrc`:

```sh
source /path/to/tree-browser/tb.bash
```

`Esc` and `Ctrl-C` still leave the shell where it was. The wrapper is a thin layer over
`tb --cwd-file`, so porting it to another shell is a few lines.

## Audio previews

mp3, flac, wav, ogg/vorbis, m4a/aac open as a player instead of text. Sound starts right away on
the default output device and stops when the popup closes. The waveform draws in as a worker reads
the file, and the part already played lights up. Decoding is built in (via
[rodio](https://github.com/RustAudio/rodio)/symphonia), so no external player is needed. Over SSH
the sound plays on the machine tb runs on, not the one you're typing at.

If there's no output device the popup still shows the waveform and length and says why it's
silent.

## Image and PDF previews

Images (png, jpg, gif, webp, bmp, tiff, ico, svg, avif, heic, …) and PDFs preview as pictures, PDFs one
page at a time. tb uses whatever graphics protocol the terminal answers to at startup:

| Terminal | Protocol |
|---|---|
| Ghostty, Kitty | kitty graphics |
| terminals with sixel support | sixel |
| iTerm2 | iTerm2 inline images |
| anything else with truecolor (tmux, Termius, …) | Unicode half-blocks |

Inside tmux the query is skipped and half-blocks are used. Inside herdr, tb starts in half-blocks
because herdr claims kitty support for every attached client whatever terminal it draws into; press
`i`, or set `TB_GRAPHICS=kitty`, when that terminal really is Ghostty or Kitty.

**Image previews** in the settings picks one for good: auto (the above), pixels (whatever the terminal
claimed), blocks, or off. To override the detection for one run, set `TB_GRAPHICS` to `kitty`, `sixel`, `iterm2`, `halfblocks` or `off`
(captions only).

## Demo

The video above was recorded from the real binary, not mocked up. `demo/` has the whole pipeline:
a Middle-earth fixture tree with staggered mtimes, a tmux-driven recorder that captures the
terminal about 250 times a second, and a renderer that turns the captures into 1080p frames for
ffmpeg. See [`demo/README.md`](demo/README.md) to rebuild it.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.

The demo fixture under `demo/` is a parody of Tolkien's Middle-earth for showing tb off. It isn't
part of the program and isn't covered by the above; the images it downloads carry their own
Wikimedia Commons licenses (see [`demo/README.md`](demo/README.md)).
