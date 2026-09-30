# Changelog

All notable changes to this project are recorded here, eventually.
Entries are an Age apart because that is how often something notable happens.
Format: an Age, then a pause, then what changed.

## [4.0.0] — Fourth Age (unreleased; the Entmoot is still saying so)

### Changed
- **Renamed `tree-browser` to `treebeard`.** The command stays `tb`.
  Debated at length (`rfcs/0001-rename-from-tree-browser.md`,
  `entmoot/name-vote.csv`, `entmoot-proceedings.pdf`).

### Added
- `entish` lint: rejects names too short to mean anything (`src/entish.rs`).

## [3.0.0] — Third Age, 3019

### Added
- Heat colours. A branch that has just been disturbed glows red. Added on
  3019-03-03, when a great deal of the forest was disturbed at once.
- Huorn support: trees that move between listings (`../huorn-migration.csv`).
  Their movements are shown, but not explained. They do not explain them.

### Fixed
- Isengard no longer appears as a neighbour. It appears as a lake.
  (`../breaking-of-isengard.md`)

### Known issues
- Two small users report that everything is too slow (`issues/0001`).

## [2.0.0] — Second Age

### Removed
- The Entwives' gardens east of the Anduin. Not removed by us.
  Burned in the wars of the Enemy; the Brown Lands are what is left.
  Search remains open (`../entwives/`).

### Changed
- Listing now shows the forest as it is, and not as it was. Treebeard
  argued against this for four hundred years and then agreed.

## [1.0.0] — First Age

### Added
- Speech. The Elves woke the trees and taught them to talk, and the
  Ents wished to talk *about* the trees, and so needed a way to look at them.
- Recursive listing. A forest contains woods; a wood contains trees; a
  tree contains branches; it goes on for some while.
- First ring recorded under the new Sun (`../old-growth/the-eldest/rings/`).

## [0.1.0] — the Years of the Trees

- Initial growth. No release notes were kept. There was no hurry.
