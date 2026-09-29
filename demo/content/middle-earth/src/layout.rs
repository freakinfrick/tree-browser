/// Places the fellowship on the map. Distances in leagues.
pub struct Leg {
    pub from: &'static str,
    pub to: &'static str,
    pub leagues: u32,
}

pub const ROUTE: &[Leg] = &[
    Leg { from: "Hobbiton", to: "Bree", leagues: 45 },
    Leg { from: "Bree", to: "Rivendell", leagues: 120 },
    Leg { from: "Rivendell", to: "Moria", leagues: 110 },
    Leg { from: "Moria", to: "Lórien", leagues: 30 },
    Leg { from: "Lórien", to: "Rauros", leagues: 130 },
];

pub fn total() -> u32 {
    ROUTE.iter().map(|l| l.leagues).sum()
}
