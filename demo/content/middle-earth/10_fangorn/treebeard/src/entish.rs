//! entish: a lint that rejects names too short to mean anything.
//!
//! "Real names tell you the story of the things they belong to." A name of
//! two or three letters tells no story at all; it is a noise you make to
//! get someone's attention. Those are allowed as *calls* (see
//! `treebeard.toml`, `[lint.calls]`) but never as names.

use std::{env, fs, path::Path, process::ExitCode};

/// Anything shorter than this is a call, not a name. RFC 0001, section 6.
const MIN_NAME_LEN: usize = 9;

/// Short words the Entmoot has agreed may be shouted across a glade.
const CALLS: &[&str] = &["tb", "hoom", "hom", "Treebeard"];

/// Words that are unkind to young trees.
const REJECT: &[&str] = &["browser", "browse"];

#[derive(Debug, PartialEq)]
enum Verdict {
    Name,
    Call,
    TooShort(usize),
    Unkind(&'static str),
}

fn judge(stem: &str) -> Verdict {
    if CALLS.contains(&stem) {
        return Verdict::Call;
    }
    if let Some(w) = REJECT.iter().find(|w| stem.to_lowercase().contains(**w)) {
        return Verdict::Unkind(w);
    }
    let len = stem.chars().filter(|c| c.is_alphabetic()).count();
    if len < MIN_NAME_LEN { Verdict::TooShort(len) } else { Verdict::Name }
}

fn walk(dir: &Path, faults: &mut usize) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        match judge(stem) {
            Verdict::Name | Verdict::Call => {}
            Verdict::TooShort(n) => {
                *faults += 1;
                eprintln!("hoom: {} is {n} letters; that is not yet a name", path.display());
            }
            Verdict::Unkind(w) => {
                *faults += 1;
                eprintln!("hrum: {} says '{w}'; that is what goats do", path.display());
            }
        }
        if path.is_dir() {
            walk(&path, faults); // slowly; there is no hurry
        }
    }
}

fn main() -> ExitCode {
    let root = env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut faults = 0;
    walk(Path::new(&root), &mut faults);
    if faults == 0 {
        println!("every name here means something.");
        ExitCode::SUCCESS
    } else {
        println!("{faults} names are too hasty. take your time with them.");
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_are_hasty() {
        assert_eq!(judge("ent"), Verdict::TooShort(3));
    }

    #[test]
    fn the_command_is_a_call() {
        assert_eq!(judge("tb"), Verdict::Call);
    }

    #[test]
    fn the_old_name_is_unkind() {
        assert_eq!(judge("tree-browser"), Verdict::Unkind("browser"));
    }

    #[test]
    fn a_real_name_passes() {
        assert_eq!(judge("Taurelilomea-tumbalemorna"), Verdict::Name);
    }
}
