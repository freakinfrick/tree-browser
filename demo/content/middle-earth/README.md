# Middle-earth

**status:** `PRODUCTION` · **incident:** the War of the Ring · **severity:** `SEV-0`

A complete operational record of the Third Age, told the way the machines
saw it: metrics, configs, tickets, logs, postmortems, and the occasional
ledger of pipe-weed. There is no prose novel here. The story is **assembled**
from the artifacts — every file in this tree is a real document, and read in
order they *are* the book.

## The system, at a glance

```
                 ┌────────────────┐
                 │    THE EYE     │  observability: 24/7, lidless, never sleeps
                 └───────┬────────┘
                         │ watches everything
      ┌──────────────────┼───────────────────────┐
      ▼                  ▼                       ▼
 the-shire           rivendell                mordor
 (ring created,      (council + maps,         (the ring must be
  found, inherited)   the plan is drawn)       dropped in the fire)
```

The **One Ring** is the system's single point of failure. It originated in
`02_the-shire`, was carried through `04_bree` → `05_rivendell` → `07_moria`
→ `08_lothlorien` → `09_anduin`, and must reach the one service that can
retire it: **Mount Doom** (`15_mordor/mount-doom/`). See `SECURITY.md` for
the vulnerability report and `ARCHITECTURE.md` for the design nobody approved.

## Locations (chronological)

Places are filed as `NN_name` so the tree reads in story order — from the
Hobbit's map (2941) to the last ship West (3021). Every location directory
opens with a `README.md`: its service record, geography, history, and the
artifacts filed inside it.

| # | directory | place | first on the record |
|---|---|---|---|
| 01 | `01_lonely-mountain` | Erebor, the Lonely Mountain | 2941 — Smaug retired |
| 02 | `02_the-shire` | the Shire | 2941 — the Ring comes home |
| 03 | `03_old-forest` | the Old Forest & the Barrow-downs | 3018-09-30 |
| 04 | `04_bree` | Bree & Weathertop | 3018-09-29 |
| 05 | `05_rivendell` | Imladris, the Last Homely House | 3018-10 |
| 06 | `06_misty-mountains` | the Hithaeglir (Caradhras) | 3019-01-12 |
| 07 | `07_moria` | Khazad-dûm | 3019-01-15 |
| 08 | `08_lothlorien` | Lórien | 3019-01-17 |
| 09 | `09_anduin` | the Great River (Rauros) | 3019-02-26 |
| 10 | `10_fangorn` | Fangorn Forest | 3019-02-26 |
| 11 | `11_rohan` | Rohan (Edoras, Helm's Deep) | 3019-03-02 |
| 12 | `12_isengard` | Isengard (Orthanc) | 3019-03-03 |
| 13 | `13_dead-marshes` | the Dead Marshes | 3019-03 (Frodo & Sam) |
| 14 | `14_gondor` | Gondor (Ithilien, Minas Tirith) | 3019-03-07 |
| 15 | `15_mordor` | Mordor (Mount Doom) | 3019-03-13 |
| 16 | `16_grey-havens` | Mithlond, the Grey Havens | 3021-09-29 |

Entities that are not places keep unnumbered homes: `the-ring` (the object),
`gollum` (the guide), `eagles` (air rescue), `palantiri` (the seeing-stones),
plus `runbooks/` (incident response) and `src/` (the only code that compiles).

## Regions (services)

| region | what it runs | health |
|---|---|---|
| `the-ring` | the object itself: provenance, bearers, the rhyme | 🔥 destroyed |
| `01_lonely-mountain` | a dragon's retirement audit | 🟡 dormant |
| `02_the-shire` | agriculture, inns, a postal service, second breakfast | 🟢 recovering |
| `03_old-forest` | Tom Bombadil, Old Man Willow, the Barrow-downs | 🟢 unbothered |
| `04_bree` | the Prancing Pony: register, bar tab, lost & found | 🟢 |
| `05_rivendell` | the Council, the library, maps, moon-letters | 🟢 |
| `06_misty-mountains` | weather (hostile), one pass that says *no* | 🟡 |
| `07_moria` | a mine, a book of records, drums | 🔴 do not enter |
| `08_lothlorien` | the Mirror, lembas, cloaks, a mallorn census | 🟢 |
| `09_anduin` | river logistics, the breaking of the Fellowship | 🟡 |
| `10_fangorn` | entmoot (in session), tree migration | 🟢 slow |
| `11_rohan` | horses, a wall with a culvert bug | 🟢 |
| `12_isengard` | uruk-hai production, one stolen palantír | ⚫ decommissioned |
| `13_dead-marshes` | the pools you must not look into | ⚠️ follow the guide |
| `14_gondor` | beacons, stewards, a city under siege | 🟡 |
| `15_mordor` | the Eye, HR, nine Nazgûl, one volcano | 🔴 the target |
| `gollum` | one guide, conditionally trustworthy | ⚠️ |
| `eagles` | air rescue (grounded, then reactivated) | 🟢 |
| `16_grey-havens` | departures, one-way | 🟢 |
| `palantiri` | the seven seeing-stones: unauthenticated scrying | 🟠 monitor |

## Start here

- `CAST.md` — every character, filed where the machines filed them.
- `SECURITY.md` — the vulnerability report. The Ring is the bug.
- `CHANGELOG.md` — the story as release notes (v1 Fellowship → v3 Return).
- `TIMELINE.md` — the raw event stream, Third Age 2941–3021.
- `ARCHITECTURE.md` — the system design: components, data flow, anti-patterns.
- `runbooks/` — incident response (Ring detected, Nazgûl, siege).
- `src/` — the only code that actually compiles (a map, a gate, a wall).

## How the record is filed

Every document opens with a small header — who filed it, when (Third Age), and
the ticket it answers — and every quote names its speaker, its place, and its
date:

> "You shall not pass!" — Gandalf, the Bridge of Khazad-dûm, 3019-01-15.

Names are filed in `CAST.md`. If a document cites someone you don't recognise,
check there: a name in a document is a filing, not a cameo.

## The one thing to remember

> One does not simply walk into Mordor. Its black gates are guarded by more
> than orcs — but there is a ticket for that in `15_mordor/black-gate/`.

```rust
fn main() { println!("one ring"); }
```

Not all those who wander are lost. Some of them are just in `git log`.
