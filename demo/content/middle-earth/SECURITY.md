# Security advisories: Middle-earth

## CVE-3018-0001 — The One Ring is a remote-access trojan

| field | value |
|---|---|
| severity | **CRITICAL** (10.0) |
| affected | all of Middle-earth, and everything it touches |
| vector | proximity, greed, possession |
| fixed in | v3.0.0 (cast into Mount Doom) |

### Description

A single artifact grants its bearer invisibility, long life, and the
undivided attention of Sauron. It is also *itself* a listening device: any
bearer is gradually owned by the artifact's original author.

### Reproduction

1. `Gollum:` `my preciousss` (already owned).
2. `Bilbo:` "what have I got in my pocket?" (partial ownership).
3. `Boromir:` "give it to me" (failed; see `anduin/amon-hen/`).

### Mitigation

- Do **not** mount it. Do not cache it. Do not *think* about using it.
- If you must move it, use a load-balanced bearer who does not want it.
- If you see a lidless eye, you have already been seen. The correct response
  is to keep walking and pretend you are a hobbit.

---

## CVE-3019-0002 — Palantír: unauthenticated scrying

| field | value |
|---|---|
| severity | HIGH (8.0) |
| affected | `orthanc-stone`, `barad-dur-stone`, `minas-tirith-stone` |
| vector | peer-to-peer link with no authentication |
| fixed in | never (the stones were not recalled) |

### Description

Any user who touches a seeing-stone is connected directly to the network.
There is no authentication, no session token, and no logout. Consequences
documented in `isengard/orthanc/palantir-access.log` (Saruman was `user_screamed`
and the remote was still not sure who was controlling whom).

### Mitigation

Turn the stone face-down. Keep a wet cloth nearby.

---

## CVE-3019-0003 — Culvert under the Deeping Wall

| field | value |
|---|---|
| severity | CRITICAL (9.5) |
| affected | `rohan/helms-deep` |
| vector | drainage ditch + explosive ("fire of Orthanc") |
| fixed in | `rohan/helms-deep/wall-repairs.rs` |

### Description

A load-bearing wall was shipped with a hole in it for a stream. The enemy
read the docs (Gríma leaked them) and put a bomb in the hole. See
`rohan/helms-deep/culvert-bug.md` for the postmortem.

---

## Other known issues (not yet assigned CVEs)

- **The Watcher in the Water** (`moria/west-gate/watcher-incident.md`): an
  unauthenticated tentacle with no rate limit.
- **The Morgul-blade** (`bree/weathertop/incident-report.md`): a wound that is
  also a countdown. Mitigation: Elrond, fast.
- **The Black Gate API** (`mordor/black-gate/api.yaml`): accepts one ring and
  nothing else; see `mordor/mount-doom/one-does-not-simply.txt`.

## Responsible disclosure

Please report vulnerabilities to the Council of Elrond, *not* to Mordor.
Mordor will fix nothing and will add you to `complaints-to-sauron.txt`.
