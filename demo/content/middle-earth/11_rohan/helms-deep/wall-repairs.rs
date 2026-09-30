//! Deeping Wall: repair queue after the siege of Helm's Deep (3019-03-03/04).
//!
//! # The culvert bug
//! The wall shipped with a drainage culvert; Gríma Wormtongue leaked the
//! blueprint to Saruman, who placed the "fire of Orthanc" in it. See
//! `11_rohan/helms-deep/culvert-bug.md` and `SECURITY.md` (CVE-3019-0003).
//! Post-exploitation it is now a feature (`CHANGELOG.md`, v2.1.0).

#[derive(Debug, PartialEq)]
enum Damage {
    Blasted { width_m: u32 },
    Scaled,   // ladders, since removed
    Fine,     // "we could build it higher" — nobody
    Flooded,  // the ent-flood did this one a favour
}

/// Repair priority: the culvert goes first, because it is where the wall
/// stopped being a wall.
fn priority(section: &str, d: &Damage) -> u32 {
    match (section, d) {
        ("culvert", Damage::Blasted { .. }) => 100, // see culvert-bug.md
        (_, Damage::Blasted { width_m }) => 50 + width_m,
        (_, Damage::Scaled) => 10,
        (_, Damage::Flooded) => 5,
        (_, Damage::Fine) => 0,
    }
}

fn main() {
    let queue = [
        ("culvert", Damage::Blasted { width_m: 12 }),
        ("gate", Damage::Scaled),
        ("tower", Damage::Fine),
        ("postern", Damage::Flooded),
    ];
    for (s, d) in &queue {
        println!("{s:>8}: {d:?} -> {}", priority(s, d));
    }
}
