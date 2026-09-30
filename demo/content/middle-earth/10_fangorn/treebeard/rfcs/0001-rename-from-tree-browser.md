# RFC 0001: Rename `tree-browser` to `treebeard`

- **Proposed by:** Fangorn, eldest of the Ents (Treebeard, to hobbits)
- **Opened:** 3019-03-01, the morning after two hobbits came into the forest
- **Entmoot:** the Second Derndingle Entmoot, convened for this alone,
  F.A. 121-03-01 to 03-12 (twelve days; the first Entmoot took three)
- **Status:** **accepted** — just now, after rather more than an Age
- **Tally:** `../entmoot/name-vote.csv`
- **Record:** `../entmoot-proceedings.pdf` (one page per day)
- **Supersedes:** nothing. Nothing here has been superseded in a hurry.

## 1. Summary

The project that shows the forest sideways has been called `tree-browser`.
That is not a name. It is a description, and a short one, of the kind that
Men put on a door. This RFC proposes that the project be called `treebeard`,
and that the command, `tb`, be left exactly where it is.

## 2. Motivation

The name `tree-browser` was given by a young hobbit (Peregrin Took) on
3019-03-01, while he was being carried. He asked, "Is there a way to see
the whole forest at once?", and then, before an answer could be begun,
"a tree browser, like?" The name was entered in the ledger the same
morning. It is the hastiest act ever recorded in Fangorn, and it was not
even done by an Ent.

Three objections have stood against it since:

1. **It says what the thing does, not what it is.** A hill is not called
   "a thing for standing on". (The Entish word for a hill begins
   *a-lalla-lalla-rumba-kamanda-lindor-burume* and goes on to say a good deal
   more about it; see section 5.)
2. **"Browse" is what goats do to young trees.** Several Ents raised this
   on the first day and it was not answered satisfactorily on the second.
3. **It has a hyphen in it.** The Entmoot has no position on hyphens.
   It was noted.

## 3. Detailed design

- The project is called **treebeard**.
- The command stays **`tb`**. Muscle memory is a kind of root, and roots
  are not pulled up lightly.
- The old home, `freakinfrick/tree-browser`, is to point at the new one,
  the way a path through the wood still leads you to the glade after the
  glade has been renamed.
- The folder the project grows in keeps its old name. Moving it would
  disturb the soil for no gain.

### Precedent

Fangorn has renamed a place before. On 3019-08-22 the Ring of Isengard,
flooded and planted, was given a new name: the **Treegarth of Orthanc**.
Nobody objected. The previous owner was not in a position to.

## 4. Haste assessment

1. *Could this wait?* It has waited, from 3019-03-01 into the Fourth Age.
2. *If it waited, what would be lost?* Nothing, except that the name would
   go on being wrong, and wrong things have a way of taking root.
3. *Has anyone said "hoom" yet?* Yes. Many times. See the proceedings.

## 5. Alternatives considered

| name | proposed by | outcome |
|---|---|---|
| keep `tree-browser` | Pippin, on reflection | not hasty enough to keep |
| the full Entish name of the forest | Fangorn | withdrawn; three days to type |
| *Taurelilomea-tumbalemorna...* | Fangorn | withdrawn; still typing |
| `ent` | Bregalad | rejected by `../src/entish.rs` (too short) |
| `hoom` | the back of the glade | a remark, not a proposal |
| `huorn` | a Huorn | nobody saw who filed it |
| `treebeard` | Fangorn | **accepted** |

## 6. The objection from the lint

`../src/entish.rs` rejects any name shorter than it should be. `tb` is two
letters. The Entmoot ruled that `tb` is not a *name*; it is what you call
out across a glade when you want someone to turn round. Treebeard answers
to "Treebeard" from hobbits on the same terms, and has for some time.
An exemption is recorded in `../treebeard.toml` under `[lint.calls]`.

## 7. Is `treebeard` a name?

This was the longest part of the debate (days 4 to 11 of the proceedings).
"Treebeard" is itself only a short name, a hobbit's name, and it does not
tell the story of the thing it belongs to. Fangorn's own view, entered on
day 9: "It is a good enough name for something that must be said quickly.
The tool is used by people in a hurry. Let them have a name that is also
in a hurry, and let the forest keep its real one."

## 8. What the trees think

Asked. The birches were in favour. The oaks have not yet answered.
The Huorns moved slightly nearer, which the clerk has recorded as assent.

## 9. Unresolved questions

- Whether the Entwives, when found, will be consulted retroactively.
  (Fangorn: "Yes.")
- Whether this RFC was itself too hasty. Deferred to the next Age.

---
*Accepted by the Entmoot. Filed in the forest by the clerk (a birch).*
*"We have decided." — Fangorn, chair.*
