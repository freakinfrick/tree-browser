# Demo v2: a bigger Middle-earth, with a Fangorn that runs its own project

Spec from the 2026-09-30 interview. Work starts on an explicit go.

## Contract

- **Whimsy stays in `demo/`.** The main README, `src/` and CI stay plain. `demo/README.md`
  stays a factual how-to. Every joke lives under `demo/content/middle-earth/` (the crate
  already excludes `/demo`).
- **Whole map, more evenly.** New files in every numbered region, plus regions the journey
  skipped. Those go under an unnumbered `elsewhere/` (`dale`, `umbar`, `harad`, `dol-amroth`,
  `grey-mountains`), so the 01–16 journey order and its numbering stay as they are.
- **The Ents get special respect.** `10_fangorn/` becomes the deepest, richest branch and the
  one place the rebrand is "full meta": the forest hosts treebeard's own (fictional) project,
  and its voice is slow, formal and never hasty. The Ents are never the joke; hastiness is.
- **Every file type.** Text/markdown/csv/logs, code and config, images, audio, PDFs, and one
  deliberately deep branch.
- **Re-record.** New captions say "treebeard", one new Fangorn beat, then a new MP4 and README GIF.

## Fangorn (10_fangorn)

Keep the existing files. Add:

- `treebeard/`: the forest's own record of the tool, in Entish-committee voice.
  - `README.md`, `CHANGELOG.md` (entries an Age apart), `ROADMAP.md` ("before the next Age")
  - `rfcs/0001-rename-from-tree-browser.md`: the rename, debated at length; `rfcs/0000-template.md`
  - `entmoot/name-vote.csv` (one row per Ent, one vote each, taken over days)
  - `issues/`: filed by hobbits and others (Pippin: "too slow"; Merry: "please add second-breakfast
    mode"; a Huorn: "moved without a ticket"), each with a slow Ent reply
  - `treebeard.toml` (`hasty = false`), `Makefile`, `src/entish.rs` (a lint that rejects names
    too short to mean anything), `.github/workflows/entmoot-ci.yml` (a job that takes three days)
  - `entmoot-proceedings.pdf.src`: multi-page, one page per day of debate
- `entwives/`: `missing-persons.csv`, `sightings.log`, `search-index.json`, `last-letter.md`
- `old-growth/`: the deep branch, age rings as folders
  (`the-eldest/rings/first-age/.../third-age/`), a short note at each level, so `e` (explode)
  and horizontal scrolling get a real workout
- `entish-hum.wav`: synthesised in `make_fixture.py` like the Moria drums (a low, slow drone, ~6 s)
- One or two public-domain/CC images of very old trees (via `images.json`, still gitignored)

**Heat story:** the rename RFC and `name-vote.csv` are minutes old, so Fangorn glows red while
the rest of the forest is ancient blue. Everything else in `10_fangorn` keeps an old age.

## Rest of the map

Two to five new files per numbered region, in the existing incident-report / ledger / minutes /
log voice, with light nods to the rename where natural (at most a line or two outside Fangorn).
One or two new images across the map. `elsewhere/` gets three or four files per region.

## Recording

- Captions: "treebeard — a horizontal tree file browser" to open, "treebeard · Rust + ratatui" to close.
- New beat after the live-change beat: go to `10_fangorn`, show it glowing, open
  `treebeard/rfcs/0001-rename-from-tree-browser.md`. Target: the full take grows by at most ~8 s,
  the GIF stays ~36–40 s (add one reel cut, trim another if needed).
- New files must not shift the existing moves; where they do, re-choreograph so every
  `expect()` still passes. Existing git story (M / + / ?) unchanged.

## Execution: /conductor swarm, then one chain step

Content is independent per branch, so writers run in parallel; tooling and recording touch
shared scripts, so they run after, in one pane.

1. **Parallel writers** (each owns only its folders, touches no script; ends with a file list
   and any `AGES` / `images.json` entries it wants, written to `demo/.v2/<worker>.json`):
   - `fangorn`: all of `10_fangorn/` (largest; the meta project + old-growth + entwives)
   - `north`: regions 01–05
   - `middle`: regions 06–09 and 11–12
   - `south`: regions 13–16 plus the unnumbered folders (`eagles`, `gollum`, `palantiri`,
     `runbooks`, `the-ring`)
   - `elsewhere`: the new `elsewhere/` regions
2. **Integrator** (one pane, after all writers finish): merges the `.v2/*.json` into
   `make_fixture.py` (`AGES`, the hum synth) and `images.json`, runs `fetch_images.py` and
   `make_fixture.py`, reviews tone across branches, then re-choreographs `record.py` / `reel.py`,
   records, renders and regenerates `docs/demo.mp4` and `docs/demo.gif`.

## Done when

- `make_fixture.py` and `record.py` run clean; every `expect()` passes.
- `docs/demo.mp4` and `docs/demo.gif` regenerated; GIF size in line with today's.
- `cargo test` and clippy still pass (nothing in `src/` should change).
