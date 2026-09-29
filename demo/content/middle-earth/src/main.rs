use std::collections::HashMap;

mod anim;
mod layout;

/// Where is everyone?
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
        ("Boromir", "Anduin (boat)"),
    ] {
        fellowship.insert(who, at);
    }
    let mut v: Vec<_> = fellowship.into_iter().collect();
    v.sort();
    for (who, at) in v {
        println!("{who:>8} -> {at}");
    }
}
