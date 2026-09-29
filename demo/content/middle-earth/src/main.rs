use std::collections::HashMap;

mod anim;
mod layout;

/// Where is everyone? — the Fellowship location ledger.
///
/// Updated as the Company breaks at Rauros (3019-02-26). See
/// `anduin/last-known-locations.json` for the machine-readable copy and
/// `CAST.md` for who these people are.
fn main() {
    let mut fellowship = HashMap::new();
    for (who, at) in [
        ("Frodo", "Mordor"),
        ("Sam", "Mordor"),
        ("Aragorn", "Gondor"),
        ("Legolas", "Gondor"),
        ("Gimli", "Gondor"),
        ("Merry", "Rohan"),
        ("Pippin", "Minas Tirith"),
        ("Gandalf", "everywhere, just in time"),
        ("Boromir", "Anduin (boat)"), // posthumous; see anduin/amon-hen/
    ] {
        fellowship.insert(who, at);
    }
    let mut v: Vec<_> = fellowship.into_iter().collect();
    v.sort();
    for (who, at) in v {
        println!("{who:>8} -> {at}");
    }
    println!();
    println!("the route (overland):");
    for leg in layout::ROUTE {
        println!("  {:<12} -> {:<12} {:>3} leagues", leg.from, leg.to, leg.leagues);
    }
    println!(
        "  total {} leagues ({} with Mordor)",
        layout::total(),
        layout::total_including_mordor()
    );
    println!("pace at 11:00: {:.2} leagues/hour", anim::pace(11.0));
}
