# BUG: culvert under the Deeping Wall is a single point of failure

**Severity:** critical · **Status:** exploited in production

## Steps to reproduce
1. Have a wall that is otherwise unbreachable.
2. Leave a drain under it for the stream.
3. Let a wizard learn about it.
4. Enemy places *fire of Orthanc* in the culvert.

## Expected
Wall holds.

## Actual
Wall has a hole the size of an army.

## Fix
Brick up the culvert. Also: stop letting Gríma read the architecture docs.
