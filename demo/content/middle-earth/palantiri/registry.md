# The Palantíri: device registry

The seeing-stones of Númenor, given to the Faithful. Seven were brought to
Middle-earth. They form a peer-to-peer scrying network with no
authentication, no encryption, and no logout (see `SECURITY.md`,
CVE-3019-0002).

| stone | location | status | last known user / event |
|---|---|---|---|
| the master stone | Avallónë, in the West | unreachable from Middle-earth | — (no return channel) |
| 1 | Elostirion (the Tower Hills, by the Grey Havens) | intact; shows only the sea | unmanned; peered at the sea, 3021-09-29 |
| 2 | Amon Sûl (Weathertop) | lost with the kingdom of Arthedain | last user unknown; offline since the fall |
| 3 | Annúminas (Arnor) | lost with King Arvedui; sunk with him | Arvedui, 1975 (drowned with the stone) |
| 4 | Orthanc (Isengard) | intact; last used by a hobbit | Peregrin Took, 3019-03-06 02:30 (`isengard/orthanc/palantir-access.log`) |
| 5 | Minas Anor (Minas Tirith) | intact; drove one Steward to despair | Denethor II, 3019-03-13/15 (despair; see `gondor/minas-tirith/stewards-succession.md`) |
| 6 | Minas Ithil (Minas Morgul) | captured; in the hands of the enemy | the Witch-king, continuously (hostile operator) |
| 7 | Osgiliath (the great stone) | lost in the river during the Kin-strife | unknown; lost c. 1437 (the Kin-strife) |

## Last known events

- **Saruman's fall:** Orthanc's stone was the instrument of Saruman's
  corruption; his sessions with the Barad-dûr stone are logged in
  `isengard/orthanc/palantir-access.log` ("user no longer sure who is
  controlling whom", 3019-01-09). The stone was afterwards thrown down from
  Orthanc and picked up by a hobbit, whose first session ended in screaming
  (3019-03-06 02:31).
- **Denethor's despair:** the Minas Tirith stone showed the Steward nothing
  he could use, and he chose to believe it. See
  `gondor/minas-tirith/stewards-succession.md` for the succession note.
- **Pippin's look:** Peregrin Took, 3019-03-06 02:30, Orthanc. First contact
  was answered with "who are you?"; the session terminated by
  `user_screamed`. No further sessions by this user on record.
- **No auth, no logout:** all of the above were unauthenticated. Refer to
  `SECURITY.md`, CVE-3019-0002, for the standing finding.

## Operating notes

- The stones cannot be made to lie; they show what is, or what the will of
  the other side wills. Denethor learned this the hard way.
- A strong will can see far; a weak will sees only what Sauron allows.
- The correct posture for a seeing-stone is face-down in a drawer, with a
  wet cloth over it.
