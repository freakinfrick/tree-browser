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
`the-shire`, was carried through `bree` → `rivendell` → `moria` → `lothlorien`
→ `anduin`, and must reach the one service that can retire it: **Mount Doom**.
See `SECURITY.md` for the vulnerability report and `ARCHITECTURE.md` for the
design nobody approved.

## Regions (services)

| region | what it runs | health |
|---|---|---|
| `the-ring` | the object itself: provenance, bearers, the rhyme | 🔥 destroyed |
| `the-shire` | agriculture, inns, a postal service, second breakfast | 🟢 recovering |
| `bree` | the Prancing Pony: register, bar tab, lost & found | 🟢 |
| `old-forest` | Tom Bombadil, Old Man Willow, the Barrow-downs | 🟢 unbothered |
| `rivendell` | the Council, the library, maps, moon-letters | 🟢 |
| `misty-mountains` | weather (hostile), one pass that says *no* | 🟡 |
| `moria` | a mine, a book of records, drums | 🔴 do not enter |
| `dead-marshes` | the pools you must not look into | ⚠️ follow the guide |
| `lothlorien` | the Mirror, lembas, cloaks, a mallorn census | 🟢 |
| `fangorn` | entmoot (in session), tree migration | 🟢 slow |
| `isengard` | uruk-hai production, one stolen palantír | ⚫ decommissioned |
| `rohan` | horses, a wall with a culvert bug | 🟢 |
| `gondor` | beacons, stewards, a city under siege | 🟡 |
| `mordor` | the Eye, HR, nine Nazgûl, one volcano | 🔴 the target |
| `anduin` | river logistics, the breaking of the Fellowship | 🟡 |
| `gollum` | one guide, conditionally trustworthy | ⚠️ |
| `eagles` | air rescue (grounded, then reactivated) | 🟢 |
| `grey-havens` | departures, one-way | 🟢 |
| `lonely-mountain` | a dragon's retirement audit | 🟡 dormant |
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
> than orcs — but there is a ticket for that in `mordor/black-gate/`.

```rust
fn main() { println!("one ring"); }
```

Not all those who wander are lost. Some of them are just in `git log`.
