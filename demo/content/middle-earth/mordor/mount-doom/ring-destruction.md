# Mount Doom: ring destruction event

**Date:** 3019-03-25 · **Result:** the Ring is no more · **Status:** `0x0000`
**Filed by:** SAMWISE GAMGEE (the other witness is a volcano) · **Ticket:** `doom-0001`
**Cross-refs:** `mordor/barad-dur/structural-failure.log` · `eagles/flight-log.csv` ·
`mordor/mount-doom/eruption-forecast.csv` · `mordor/mount-doom/the-last-march.md`

## The final log

```text
03-25 10:00  frodo & sam reach the mountain; sam carries frodo the last part
03-25 10:30  the Cracks of Doom: the chamber where the ring was forged
03-25 10:31  frodo stands at the edge. he does not drop it.
03-25 10:31  frodo: "the ring is mine."
03-25 10:31  gollum: arrives, having never once stopped following
03-25 10:31  struggle at the edge. a bite. a finger. a precious.
03-25 10:32  gollum, holding the ring: dances. steps too far.
03-25 10:32  ring (with gollum attached): falls into the fire
03-25 10:33  mount doom: ERUPTION (final). barad-dur: offline. the eye: closed.
```

## Provenance

- Forged here: SA 1600, by Sauron, in the Cracks of Doom (`the-ring/forging.md`).
- The only place it could be unmade was the place it was made. This was never
  in question; the only question was who would carry it there, and the answer
  was "a gardener and, unwillingly, a guide."
- The ring was not destroyed by anyone. It was destroyed by *greed* — the one
  force in Middle-earth strong enough to carry it over the edge.

## The Crack of Doom

- "the ring is mine" — Frodo Baggins, the Crack of Doom, 3019-03-25. This is
  the moment the Ring betrays its bearer one last time: at the end of the
  road, it turns the bearer's own will into the will that will not let it go.
- The Ring was not dropped. It was *taken*. Gollum bit the finger off —
  Frodo's third finger, left hand, the one that once wore the Ring — and the
  Ring fell with the finger's new owner still attached.
- "unexpected failover" — the incident write-up's term for Gollum. The plan
  had no step for "the guide resolves the final conflict by dancing too close
  to the edge." The plan is not the plan; the plan is what happened.

## Eruption

- ERUPTION (final) at 10:33. See `eruption-forecast.csv`: the forecaster
  logged day 5 as "everything is falling down" and then stopped logging.
- Barad-dûr failed with the Ring; see `mordor/barad-dur/structural-failure.log`
  for the structural root cause (the root cause is gone).
- The Eye went dark; see `.eye-is-watching.log`, final entry: "uptime reset to
  zero, permanently."

## Rescue

- Two hobbits went in. Three things came back out (two hobbits, one less finger).
- The Eagles arrived afterward, which is their entire role in the story —
  "The Eagles are coming!" cried at the Black Gate, 3019-03-25. See
  `eagles/flight-log.csv` for the pickup (passengers: two, cargo: none, ring: N/A).

## Aftermath

- the tower of Barad-dûr: failed (`mordor/barad-dur/structural-failure.log`).
- the Eye: uptime counter reset to zero, permanently (`mordor/barad-dur/eye-uptime.prom`).
- the sky: clear for the first time in an age. Not logged, but reported.

## Notes

- "I can't carry it for you, but I can carry you." — Samwise Gamgee, on the
  slopes of Mount Doom, 3019-03-25. This is why the log above reaches the
  Cracks at all; see `the-last-march.md`.
- Gollum's fall is recorded here not as a death but as a "removed from the
  party" event with cause: gravity, assisted by joy.
