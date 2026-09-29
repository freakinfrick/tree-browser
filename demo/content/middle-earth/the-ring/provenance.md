# The One Ring: provenance

**Object:** the Ruling Ring, forged by Sauron in Mount Doom, Second Age c. 1600
**Status:** DESTROYED (3019-03-25, in the same forge that made it)
**Owners:** nine, give or take a cave.

## Chain of custody

See `bearers.csv` for the full ledger. The short version:

1. **Sauron** — SA 1600, made it; used it; lost it (a finger, an Age).
2. **Isildur** — SA 3441, cut it from Sauron; refused to destroy it; died
   with it on (TA 2).
3. **the Anduin** — TA 2–2463, held it for two and a half thousand years
   (the Gladden Fields).
4. **Déagol** — TA 2463, found it; was killed for it within the hour.
5. **Gollum (Sméagol)** — TA 2463–2941, took it; vanished into the dark for
   478 years.
6. **Bilbo Baggins** — TA 2941–3018, won it in a riddle game; held it for 60
   years.
7. **Frodo Baggins** — TA 3018–3019, inherited it; carried it to Mordor.
8. **Samwise Gamgee** — TA 3019-03-14, carried it (briefly) while Frodo was
   taken; returned it.
9. **Gollum, again** — TA 3019-03-25, took it at the edge of the fire.
10. **the Fire** — TA 3019-03-25, the only owner that kept it.

## Properties

- makes its bearer invisible (the bearer, not the ring).
- extends life; thins it; stretches it — as Bilbo put it, "like butter
  scraped over too much bread" — Bilbo Baggins, Bag End, TA 3001.
- has a will of its own. Slips off fingers. Betrays bearers. Wants only to
  return to Sauron.

## Handling note

Do **not** cache it. The Ring is not a value you store; it is a process that
co-opts the host. The advisory is explicit: "Do not mount it. Do not cache
it. Do not *think* about using it." — `SECURITY.md` (CVE-3018-0001). A
bearer is the only transport it has never been able to outlive quietly.

## Why it was never simply destroyed

Everyone who held it and could have unmade it, kept it instead. The one
person who volunteered to carry it did so precisely because he did not want
it — which was the only qualification that worked.

> See `SECURITY.md` (CVE-3018-0001) for the advisory.
