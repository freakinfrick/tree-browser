# Changelog

All notable changes to Middle-earth are documented here. This file *is* the
plot, expressed as release notes.

## [Unreleased] — the archive, annotated
### Changed
- locations re-filed chronologically as `01_lonely-mountain` … `16_grey-havens`
  so the tree reads in story order; each location now opens with a `README.md`
  service record (geography + history). Entities that are not places
  (`the-ring`, `gollum`, `eagles`, `palantiri`) stay unnumbered.
### Docs
- every record now carries who filed it and who said what (`CAST.md`).
- the map now ends at the fire: `src/layout.rs` gains the Mordor legs.

## [3.1.0] — 3019, late (Scouring of the Shire)
### Changed
- Removed ruffians from the Shire (`02_the-shire/scouring-of-the-shire/`).
- Removed the new mill; restored the old one; remembered the party tree.
- Bag End repainted; spoons restored to 12/12.
### Deprecated
- Lobelia Sackville-Baggins (returned the spoons, left anyway).

## [3.0.0] — 3019-03-25 (Return of the King)
### Breaking
- **The One Ring destroyed.** `mount-doom` now writes, never reads.
- Barad-dûr removed from the network (`15_mordor/barad-dur/structural-failure.log`).
- the Eye: decommissioned, uptime reset to zero, permanently.
### Added
- a king (`14_gondor/minas-tirith/stewards-succession.md` — the chair is no longer empty).
- the Eagles reactivated (`eagles/flight-log.csv`).
### Removed
- Sauron. Nazgûl count 9 → 0 (`15_mordor/barad-dur/nazgul-roster.csv`).

## [2.1.0] — 3019-03-15 (the Battle)
### Added
- siege defense at Minas Tirith (`14_gondor/minas-tirith/siege-of-gondor.md`).
- the charge at the Pelennor (`14_gondor/pelennor-fields/`).
- an unexpected army (`11_rohan/paths-of-the-dead/`).
### Fixed
- the Witch-king: removed by a woman and a hobbit, per prophecy.
- the culvert bug is now a feature (`11_rohan/helms-deep/wall-repairs.rs`).

## [2.0.0] — 3019-02/03 (The Two Towers)
### Breaking
- Fellowship v1.0 split at Rauros (`09_anduin/amon-hen/fellowship-dissolution.md`).
- Boromir removed from the party (`09_anduin/amon-hen/boromir-last-stand.md`).
- Gandalf: rebooted as `gandalf.the.white` after a crash in Moria
  (`10_fangorn/gandalf-reboot.log`).
### Added
- two hobbits captured, one misdelivered to Fangorn.
- Helm's Deep held (`11_rohan/helms-deep/siege-postmortem.md`).
- Isengard flooded by a forest (`12_isengard/orthanc/uruk-hai-production.csv`, week 6).
### Security
- `CVE-3019-0003` culvert disclosed (post-exploitation).

## [1.2.0] — 3019-01 (Moria)
### Added
- a gate that opens for "friend" (`07_moria/west-gate/lock.rs`).
- drums in the deep (`07_moria/mazarbul-chamber/drums-in-the-deep.wav`).
### Breaking
- **Gandalf is down.** Cause: a Balrog. Full report in
  `07_moria/balrog-incident-report.md`.
### Known issues
- the Bridge of Khazad-dûm is now two bridges (`07_moria/durins-bridge-inspection.md`).

## [1.1.0] — 3018-10 (Council of Elrond)
### Added
- a council (`05_rivendell/council-minutes/`).
- a volunteer (`05_rivendell/council-minutes/who-carries-the-ring.md`).
- three more volunteers who were not asked.

## [1.0.0] — 3018-09 (The Fellowship)
### Added
- nine walkers, one pony, no budget (`09_anduin/boats.csv`).
- a party at Bag End that ended in a flash and a bang
  (`02_the-shire/bag-end/unwanted-visitors.log`).

## [0.9.0] — 2941 (The Hobbit)
### Added
- a burglar (unqualified).
- a dragon, later retired (`01_lonely-mountain/treasure-audit.csv`).
- a ring, found in a cave under the mountain. *(See CVE-3018-0001.)*
