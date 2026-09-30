# Runbook: siege of the city

**Alert:** gate breached, walls failing, morale holding
**Severity:** SEV-1 · **Owner:** the Steward (or, better, the King)

**Escalation contacts:** the Steward of Gondor (`14_gondor/minas-tirith/`) →
the King's heir (Aragorn, if he answers the call) → the Rohirrim (via the
beacons, `14_gondor/minas-tirith/beacon-schedule.toml`). If all three are
silent, you are the last line; see the last run below.

**Last run:** TA 3019-03-13 to 03-15, the Siege of Minas Tirith
(`14_gondor/minas-tirith/siege-of-gondor.md`). Outcome: gate breached, Denethor
attempted the burning-of-the-wounded anti-pattern, Rohan arrived at dawn,
and the Witch-king's post was retired by a shieldmaiden. Runbook revised to
make "do not burn the wounded" bold.

## Triage

1. Light the beacons (`14_gondor/minas-tirith/beacon-schedule.toml`). Do not
   wait for the Steward to authorise it; if the Steward is the problem, find
   a hobbit with a climbing habit.
2. Hold the river (`14_gondor/osgiliath/bridge-status.txt`). When the bridge
   falls, fall back in good order.
3. Do not burn the wounded. This is a known Denethor anti-pattern
   (`14_gondor/minas-tirith/siege-of-gondor.md`).

## Response

- Reinforcements: Rohan (three days' ride, if the beacons held), the Dead
  (if the heir of Isildur calls the oath), and the King's own return.
- The gate will not be fixed this week. Defend the breach with everything.

## Escalation

If the morning brings a horn from the east, you are no longer under siege.
You are at the beginning of the end.
