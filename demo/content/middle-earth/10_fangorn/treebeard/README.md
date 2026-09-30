# treebeard — project record

> **Record:** `fangorn-proj-0001` · **Maintainer:** Fangorn (called Treebeard
> by those in a hurry) · **Status:** stable, and not to be hurried ·
> **Formerly:** `tree-browser` (see `rfcs/0001-rename-from-tree-browser.md`) ·
> **Cross-refs:** `CHANGELOG.md`, `ROADMAP.md`, `entmoot/name-vote.csv`,
> `issues/`, `../old-growth/`

**What it is:** a way of looking at a forest sideways. Each tree is shown with
its branches laid out to the right, the old wood cool and blue, the new
growth warm. You walk along a branch; you do not climb.

**Command:** `tb`. The Entmoot notes, for the record, that this is two
letters long and therefore not a name. It is permitted as a *call*, the way
a hobbit may shout "Treebeard!" across a glade without meaning any
disrespect (`rfcs/0001-rename-from-tree-browser.md`, section 6).

## Why it exists

The forest is very large and very old, and it is not possible to keep all
of it in one's head, even a head as large as an Ent's. Treebeard wished to
see which parts of the wood had been stirring lately and which had been
asleep since the Elder Days. The colours do this: a glade that has not
changed since the First Age stays blue; a glade where a hobbit has just
sat down turns red at once.

## Using it

    tb                   # the forest where you stand
    tb ../old-growth     # somewhere older
    e                    # explode a branch: show all of it, all the way down

Press `e` on `../old-growth/` if you have an afternoon. It goes down a long
way. That is the point of it.

## Design principles

1. **Do not be hasty.** Nothing is deleted without being asked twice.
2. **Names should mean something.** See `src/entish.rs`.
3. **Old things are not stale.** Blue is a colour of honour here.
4. **The forest is the source of truth.** The tool only looks.

## Governance

Decisions are taken by Entmoot (`entmoot/`). An Entmoot is convened when
something important must be decided, and ends when it has been. The
shortest on record took three days; the rename took rather longer.

## Status of the Entwives integration

Blocked. Upstream has not been reachable since the Second Age. See
`../entwives/` and `issues/0005-have-you-looked-in-the-shire.md`.
