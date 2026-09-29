# Postmortem: the breaking of the Fellowship

**Filed by:** Samwise Gamgee (dictated), Amon Hen, 3019-02-26 · **Ticket:** FSHIP-3019-DISSOLVE
**Location:** Amon Hen, west bank of the Anduin · **Date:** 3019-02-26
**Severity:** SEV-1 (team permanently split) · **Status:** closed (irreversibly)

**Cross-refs:** `anduin/amon-hen/boromir-last-stand.md`, `anduin/boats.csv`, `anduin/last-known-locations.json`.

## Timeline

- **morning** — Frodo asks for an hour to think. Boromir follows him.
- **afternoon** — Boromir proposes a change of plan ("give me the ring").
  Proposal rejected at the source.
- **afternoon** — Frodo puts on the ring. Vanishes. Locates a seat with a
  view. Decides the team is not safe.
- **evening** — a band of uruk-hai arrives. The whole company splits to defend
  Merry and Pippin.
- **dusk** — Boromir holds the shore. The horn is sounded twice. The third
  blast is heard from too far away to reach.

## What went well

- Frodo and Sam crossed the river. Quietly, unobserved, on the *right* side.
- Merry and Pippin were not killed (see `isengard` and `fangorn` for the sequel).

## What went badly

- the Fellowship v1.0 is dissolved. No rollback, no reunion (until the end).
- Boromir fell. `last-known-locations.json` has the final state.

## The split decision

- Frodo resolved to go alone. "I will go to Mordor alone." — Frodo, Amon Hen, 3019-02-26 (per Sam's account).
- Sam refused to leave him. "I'm coming too, or neither of us isn't going." — Samwise, Amon Hen, 3019-02-26 (verbatim per the bearer, though the bearer disputes the grammar).
- Disposition of transport: one boat taken across the Anduin and cut loose — see `anduin/boats.csv` for the boat and its later status ("sunk later").
- The remainder of the company did not discover the departure until the boat was already gone; the pursuit of the captives began on foot, one boat short.

## Root cause

A nine-way load-balancing scheme with a single shared resource and no
consensus algorithm. Eventually the resource chose its own route.

## Lessons

- One ring, one bearer, one decision. Committees are for councils, not rivers.
- If a companion keeps saying "for Gondor," check his hands for the payload.
