/// The west gate of Moria: opens for anyone who can read the inscription.
pub struct Gate {
    open: bool,
}

impl Gate {
    pub fn new() -> Self {
        Gate { open: false }
    }

    /// "Speak, friend, and enter": the password is the word *friend*.
    pub fn speak(&mut self, word: &str) -> bool {
        self.open |= word.eq_ignore_ascii_case("mellon");
        self.open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn riddle() {
        let mut g = Gate::new();
        assert!(!g.speak("open"));
        assert!(!g.speak("Durin"));
        assert!(g.speak("Mellon"));
    }
}
