# Architecture of Middle-earth

*System design review, Third Age 3018. Prepared by nobody, approved by nobody.
Circulated to the Council; Elrond read it, sighed, and said "it'll have to do."*

## Overview

Middle-earth is a **distributed system** with one persistent resource (the
One Ring), one write-only sink (Mount Doom), and one over-provisioned
observability layer (the Eye of Sauron). The entire plot is an exercise in
moving a single object from the Shire to a volcano without the distributed
monolith noticing.

## Components

| component | type | responsibility | notes |
|---|---|---|---|
| the One Ring | state | the whole point | do not cache, do not mount, do not read |
| the Shire | data origin | created and inherited the ring | lowest latency, highest calories |
| the Fellowship | deployment | nine replicas carrying the payload | no failover plan; bus factor = 1 |
| the Council | scheduler | assigned the task | item 4 deferred to lunch |
| Moria | legacy monolith | dark, cramped, on fire | do not rebuild |
| Lothlórien | staging env | safe, beautiful, temporary | time moves differently |
| Isengard | rogue service | industrialised the forest | later flooded (see Fangorn) |
| the Eye | monitoring | watches all, sees all | does not observe the Shire subnet |
| Mount Doom | termination | `rm -rf` for rings | write-only; no rollback |
| the Eagles | rescue API | `POST /save` | undocumented, invoked too late, works anyway |

## Data flow

```
ring ──(inherit)──► Frodo ──(carry)──► Council ──(plan)──► Fellowship
      ──(flee)──► Moria ──(under)──► Lothlórien ──(river)──► Rauros
      ──(split)──► two pipelines:
                       A) Frodo + Sam ──(guide: gollum)──► Cirith Ungol ──► Doom
                       B) everyone else ──(war)──► the world ──► diversions
ring ──(drop)──► Doom ──► 0x0000 0000 0000
```

## Key design decisions

1. **Do not use the ring.** Considered; rejected unanimously minus one.
   Rationale: every previous owner is now a cautionary tale (`see: Gollum,
   Isildur, all of them`).
2. **Do not fly the ring to Mordor.** The Eagles are not a taxi; also it would
   end the story in chapter three.
3. **Hide the plan from the Eye.** The entire strategy depends on the
   observability layer assuming nobody would *walk* there. This is the one
   assumption that holds.

## Anti-patterns observed

- **Single point of failure:** one ring, one bearer, one route, one volcano.
- **Zero-trust violation:** palantíri ship with default credentials and no
  rate limit (`12_isengard/orthanc/palantir-access.log`, `14_gondor/.../palantir`).
- **Unpatched vulnerability:** the culvert under the Deeping Wall
  (`11_rohan/helms-deep/culvert-bug.md`).
- **No load test:** Minas Tirith accepted a siege of 100k+ without a
  capacity plan (`14_gondor/minas-tirith/siege-of-gondor.md`).

## Failure modes (known)

- **The gate is a riddle, not a secret.** Anyone who can read opens Moria
  (`07_moria/west-gate/lock.rs`). Fixed by being abandoned anyway.
- **The wall ships with a hole.** The culvert (`11_rohan/helms-deep/wall-repairs.rs`)
  is a single point of failure that is now a feature (`CHANGELOG.md`, v2.1.0).
- **The route only compiles overland.** `src/layout.rs` ends at Rauros; the
  Mordor legs are `LAST_LEGS`, unmapped by the Eye.

## Reboot procedure

If the Ring survives: run `make destroy-ring`. If Gandalf is down, wait three
days; he comes back with more privileges and a different theme.
