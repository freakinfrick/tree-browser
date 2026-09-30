# The Siege of Minas Tirith: capacity report

**Date:** 3019-03-13 to 03-15 · **Attacker load:** ~100,000+ concurrent ·
**Defender capacity:** ~5,000, then fewer · **Plan:** hold until a miracle.
**Filed by:** the watch-captain of the Citadel, from the sixth level ·
**Record:** SIEGE-03 · **severity:** SEV-1, ongoing.

## Inbound traffic

| vector | volume | mitigations |
|---|---|---|
| the host of Mordor | ~100k | gate, wall, and will |
| catapult fire | sustained | the wall is now partly rubble |
| fire from siege towers | high | the defenders' own fire |
| the Nazgûl | 9 | fear (no countermeasure) |
| Grond, the battering ram | 1 | the gate did not survive it |

## Timeline

- **3019-03-13** — Osgiliath falls; the garrison is driven back across the
  river. Faramir is carried in, wounded (see `../osgiliath/bridge-status.txt`).
  The siege closes around the Pelennor by nightfall.
- **3019-03-13 (night)** — the Rammas Echor is breached; the survivors of
  the out-wall come in with the enemy at their heels.
- **3019-03-14** — the Pelennor is lost; the city is invested on all sides.
  Grond is brought up under cover; the Witch-king waits for the gate.
- **3019-03-15 (dawn)** — the gate breaks under Grond. The Witch-king enters.
  Rohan's horns answer from the north; the charge of the Rohirrim breaks the
  siege (see `../pelennor-fields/battle-report.md`).

## Notable events

- **the gate breaks** under Grond. The Witch-king enters. The gate will not
  be fixed this week.
- **Denethor** (the Steward) declares the cause lost and attempts to burn his
  own son, Faramir, and himself. Pippin and Gandalf intervene; two of three
  saved. Denethor dies on the pyre in the House of the Stewards (3019-03-15).
  The palantír of the White Tower is thereafter unusable — see
  `palantiri/` — and is not to be touched by bare hands.
- **the Rooster crows.** Somewhere far off, a horn is heard. It is not an
  alarm; it is an arrival.

## Resolution

Rohan arrives at dawn, and Aragorn from the river. The siege is broken on
the third day. The city stands. The gate still needs a door.

> The beacons (`beacon-schedule.toml`) were lit on 03-09. Rohan answered.
> It took them three days to ride. That is the latency of hope.

## Addendum (filed 3019-03-16)

Grond was abandoned in the ruin of the gate and did not enter the city. The
ram was forged in Mordor for this one door; it is retained as evidence, see
`../pelennor-fields/battle-report.md`. The Witch-king fell on the field that
same morning — "I am no man." — Éowyn of Rohan, before the walls
(3019-03-15), recorded in `../pelennor-fields/eowyn-vs-witchking.md`.

## See also

- `../osgiliath/bridge-status.txt` — the crossing that bought the city a day.
- `../pelennor-fields/battle-report.md` — field report for 3019-03-15.
- `beacon-schedule.toml` — the signal that started the clock.
- `runbooks/siege.md` — standing orders for a city under the shadow.