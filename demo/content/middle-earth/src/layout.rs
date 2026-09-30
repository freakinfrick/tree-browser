//! Places the fellowship on the map. Distances in leagues, as argued about
//! at the Council of Elrond and corrected by Aragorn at least twice.

/// One leg of the journey.
pub struct Leg {
    pub from: &'static str,
    pub to: &'static str,
    pub leagues: u32,
}

/// The overland route. The Eye assumes nobody would *walk* this; that
/// assumption is the entire plan (see `ARCHITECTURE.md`).
pub const ROUTE: &[Leg] = &[
    Leg { from: "Hobbiton", to: "Bree", leagues: 45 },
    Leg { from: "Bree", to: "Rivendell", leagues: 120 },
    Leg { from: "Rivendell", to: "Moria", leagues: 110 }, // the pass said no
    Leg { from: "Moria", to: "Lórien", leagues: 30 },
    Leg { from: "Lórien", to: "Rauros", leagues: 130 },   // the fellowship ends here
];

/// The route continues past Rauros, but only for two of the original nine.
/// See `09_anduin/last-known-locations.json`.
pub const LAST_LEGS: &[Leg] = &[
    Leg { from: "Rauros", to: "Cirith Ungol", leagues: 90 }, // guided, conditionally
    Leg { from: "Cirith Ungol", to: "Mount Doom", leagues: 12 },
];

pub fn total() -> u32 {
    ROUTE.iter().map(|l| l.leagues).sum()
}

pub fn total_including_mordor() -> u32 {
    total() + LAST_LEGS.iter().map(|l| l.leagues).sum::<u32>()
}
