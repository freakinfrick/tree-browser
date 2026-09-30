//! The west gate of Moria (the Doors of Durin).
//!
//! # Provenance
//! - Drawn by Narvi the dwarf; the inscription written by Celebrimbor of
//!   Eregion, in the Second Age. Full context: `07_moria/west-gate/password-hint.txt`.
//! - Inscription (Sindarin): *"Ennyn Durin Aran Moria. Pedo mellon a minno."*
//!   — "The Doors of Durin, Lord of Moria. Speak, friend, and enter."
//! - The password is not a secret; it is a riddle. The word for *friend* is
//!   written on the door, and everyone overthought it for a while.
//!
//! # Incident history
//! - 3019-01-15: the Fellowship reaches the door. Gandalf reads it aloud and
//!   stalls; Frodo asks what the Elvish for "friend" is; the door opens.
//!   Immediately afterwards the Watcher attacks (`07_moria/west-gate/watcher-incident.md`).

/// The gate opens for anyone who can read the inscription.
pub struct Gate {
    open: bool,
}

impl Gate {
    pub fn new() -> Self {
        Gate { open: false }
    }

    /// "Speak, friend, and enter": the password is the word *mellon*
    /// (Sindarin for "friend"). Case-insensitive, because a door does not
    /// hold shift.
    pub fn speak(&mut self, word: &str) -> bool {
        self.open |= word.eq_ignore_ascii_case("mellon");
        self.open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The attempts recorded in `07_moria/west-gate/password-hint.txt`.
    #[test]
    fn riddle() {
        let mut g = Gate::new();
        assert!(!g.speak("open"));        // Gandalf: literal, wrong
        assert!(!g.speak("open sesame")); // Merry: wrong story
        assert!(!g.speak("Durin"));       // Gimli: the owner, also wrong
        assert!(!g.speak("friend"));      // English does not count
        assert!(g.speak("Mellon"));       // Gandalf, second attempt
    }

    /// Once open, the gate stays open — but so does the Watcher's interest.
    #[test]
    fn stays_open() {
        let mut g = Gate::new();
        g.speak("mellon");
        assert!(g.speak("anything")); // already open; the door no longer cares
    }
}
