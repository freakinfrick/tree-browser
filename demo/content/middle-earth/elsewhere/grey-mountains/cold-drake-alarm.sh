#!/usr/bin/env bash
# cold-drake-alarm.sh — door alarm for the northern halls
# filed: the gate-wardens, Ered Mithrin · cross-refs: drake-sightings.log
# Cold-drakes have no fire, so the heat sensors never tripped. This checks
# the other signs. Last run: TA 2589, a minute late.
set -euo pipefail

signs=(
  "frost on the inside of the door"
  "ravens silent"
  "gold missing from the outer hall"
  "long scratches, fresh"
)

count=0
for s in "${signs[@]}"; do
  if [[ -n "${OBSERVED:-}" && "$OBSERVED" == *"$s"* ]]; then
    echo "SIGN: $s"
    count=$((count + 1))
  fi
done

if (( count >= 2 )); then
  echo "ALARM: cold-drake at the door. wake the King. do NOT open the door."
  exit 1
elif (( count == 1 )); then
  echo "WATCH: one sign. double the guard. tell the King in the morning."
else
  echo "quiet. (it was also quiet the night before 2589-10-21.)"
fi
