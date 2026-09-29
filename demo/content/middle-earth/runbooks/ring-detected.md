# Runbook: Ring detected

**Alert:** `RingDetected` (see `mordor/barad-dur/alerts.yml`)
**Severity:** the highest there is.
**Owner:** the Council of Elrond (on-call: Gandalf) · **Escalation:**
Council → Elrond (`rivendell/`) → the Ring-bearer's companions. There is no
higher tier; if the Council is unreachable, consult `the-ring/provenance.md`
and do not improvise.

## Triage

1. Do not pick it up.
2. If you have already picked it up, put it down. You will not want to. Put
   it down anyway.
3. Do not put it on. If you have put it on, take it off; the Eye now knows
   exactly where you are, and the only thing between you and the Nine is
   distance.

## Response

- Route the object to the nearest Council-approved carrier (a hobbit, ideally
  one who does not want it).
- Destination: Mount Doom (`mordor/mount-doom/`). Method: walking. No eagles.

## Escalation

If the bearer reports hearing a voice, seeing an Eye, or calling the ring
"precious", escalate immediately. The ring is not a load to be balanced; it
is a bearer to be managed.

## Postmortem

See `mordor/mount-doom/ring-destruction.md`. There is no other resolution.

**Last run:** TA 3019-03-25, the Cracks of Doom. Outcome: alert cleared, but
only after a second seizure attempt by a prior owner (`the-ring/bearers.csv`,
row "Gollum, TA 3019-03-25"). Lesson: the Ring re-attaches to whoever last
held it; plan for a hand-off, not a handover.
