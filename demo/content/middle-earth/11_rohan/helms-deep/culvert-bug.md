# BUG: culvert under the Deeping Wall is a single point of failure

**Severity:** critical · **Status:** exploited in production ·
**CVE:** CVE-3019-0003 (see `SECURITY.md`)

**Filed by:** Hama, engineer of the Hornburg · **Date:** 3019-03-04 (dawn, from the keep) ·
**Ticket:** HORN-0003 · **Impact:** wall breached; retreat forced; see `siege-postmortem.md`.

## Steps to reproduce
1. Have a wall that is otherwise unbreachable.
2. Leave a drain under it for the stream.
3. Let a wizard learn about it.
4. Enemy places *fire of Orthanc* in the culvert.

## Expected
Wall holds.

## Actual
Wall has a hole the size of an army.

## Root cause

The culvert was an open secret on paper: a drain wide enough for a man to
crawl, documented in the Hornburg's own plans. Those plans were shown to
Gríma Wormtongue in his capacity as the King's counsellor — see
`11_rohan/edoras/grima-wormtongue.md`, and note that Gríma answered to Saruman.
The information leak was therefore not a breach of the wall but of the
council chamber. The enemy arrived with the drain memorised.

The weapon, "fire of Orthanc", was a device of Saruman's making, delivered
by torch-bearers to the exact brickwork the plans described. It was not a
battering ram; it did not need to be.

## Fix

Brick up the culvert. Also: stop letting Gríma read the architecture docs.
Permanent repair tracked in `11_rohan/helms-deep/wall-repairs.rs` — the culvert
is sealed, and the stream now runs over stone, not under the wall.

## Addendum (3019-03-04, later)

The hole held the stream, and the stream held the enemy's charge for a time.
Count the culvert among the few things at the Hornburg that did any work
before it failed.

## See also

- `SECURITY.md` — CVE-3019-0003, the information-leak class this exploit belongs to.
- `11_rohan/helms-deep/siege-postmortem.md` — what the breach cost.
- `11_rohan/helms-deep/wall-repairs.rs` — the seal, in code.
