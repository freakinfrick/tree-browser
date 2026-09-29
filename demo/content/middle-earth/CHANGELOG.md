# Changelog

All notable changes to Middle-earth are documented here. This file *is* the
plot, expressed as release notes.

## [Unreleased] — the archive, annotated
### Docs
- every record now carries who filed it and who said what (`CAST.md`).
- the map now ends at the fire: `src/layout.rs` gains the Mordor legs.

## [3.1.0] — 3019, late (Scouring of the Shire)
### Changed
- Removed ruffians from the Shire (`the-shire/scouring-of-the-shire/`).
- Removed the new mill; restored the old one; remembered the party tree.
- Bag End repainted; spoons restored to 12/12.
### Deprecated
- Lobelia Sackville-Baggins (returned the spoons, left anyway).

## [3.0.0] — 3019-03-25 (Return of the King)
### Breaking
- **The One Ring destroyed.** `mount-doom` now writes, never reads.
- Barad-dûr removed from the network (`mordor/barad-dur/structural-failure.log`).
- the Eye: decommissioned, uptime reset to zero, permanently.
### Added
- a king (`gondor/minas-tirith/stewards-succession.md` — the chair is no longer empty).
- the Eagles reactivated (`eagles/flight-log.csv`).
### Removed
- Sauron. Nazgûl count 9 → 0 (`mordor/barad-dur/nazgul-roster.csv`).

## [2.1.0] — 3019-03-15 (the Battle)
### Added
- siege defense at Minas Tirith (`gondor/minas-tirith/siege-of-gondor.md`).
- the charge at the Pelennor (`gondor/pelennor-fields/`).
- an unexpected army (`rohan/paths-of-the-dead/`).
### Fixed
- the Witch-king: removed by a woman and a hobbit, per prophecy.
- the culvert bug is now a feature (`rohan/helms-deep/wall-repairs.rs`).

## [2.0.0] — 3019-02/03 (The Two Towers)
### Breaking
- Fellowship v1.0 split at Rauros (`anduin/amon-hen/fellowship-dissolution.md`).
- Boromir removed from the party (`anduin/amon-hen/boromir-last-stand.md`).
- Gandalf: rebooted as `gandalf.the.white` after a crash in Moria
  (`fangorn/gandalf-reboot.log`).
### Added
- two hobbits captured, one misdelivered to Fangorn.
- Helm's Deep held (`rohan/helms-deep/siege-postmortem.md`).
- Isengard flooded by a forest (`isengard/orthanc/uruk-hai-production.csv`, week 6).
### Security
- `CVE-3019-0003` culvert disclosed (post-exploitation).

## [1.2.0] — 3019-01 (Moria)
### Added
- a gate that opens for "friend" (`moria/west-gate/lock.rs`).
- drums in the deep (`moria/mazarbul-chamber/drums-in-the-deep.wav`).
### Breaking
- **Gandalf is down.** Cause: a Balrog. Full report in
  `moria/balrog-incident-report.md`.
### Known issues
- the Bridge of Khazad-dûm is now two bridges (`moria/durins-bridge-inspection.md`).

## [1.1.0] — 3018-10 (Council of Elrond)
### Added
- a council (`rivendell/council-minutes/`).
- a volunteer (`rivendell/council-minutes/who-carries-the-ring.md`).
- three more volunteers who were not asked.

## [1.0.0] — 3018-09 (The Fellowship)
### Added
- nine walkers, one pony, no budget (`anduin/boats.csv`).
- a party at Bag End that ended in a flash and a bang
  (`the-shire/bag-end/unwanted-visitors.log`).

## [0.9.0] — 2941 (The Hobbit)
### Added
- a burglar (unqualified).
- a dragon, later retired (`lonely-mountain/treasure-audit.csv`).
- a ring, found in a cave under the mountain. *(See CVE-3018-0001.)*
