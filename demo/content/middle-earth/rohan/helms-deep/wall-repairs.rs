//! Deeping Wall: repair queue after the siege.

#[derive(Debug, PartialEq)]
enum Damage {
    Blasted { width_m: u32 },
    Scaled,
    Fine,
}

fn priority(section: &str, d: &Damage) -> u32 {
    match (section, d) {
        ("culvert", Damage::Blasted { .. }) => 100, // see culvert-bug.md
        (_, Damage::Blasted { width_m }) => 50 + width_m,
        (_, Damage::Scaled) => 10,
        (_, Damage::Fine) => 0,
    }
}

fn main() {
    let queue = [("culvert", Damage::Blasted { width_m: 12 }), ("gate", Damage::Scaled), ("tower", Damage::Fine)];
    for (s, d) in &queue {
        println!("{s:>8}: {d:?} -> {}", priority(s, d));
    }
}
