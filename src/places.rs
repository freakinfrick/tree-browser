//! Remember place: the folders left open and the selection, per starting
//! folder, in `$XDG_STATE_HOME/tb/places` (else `~/.local/state/tb/places`).
//!
//! Plain lines, newest start first:
//!
//! ```text
//! = /where/tb/started
//! + /a/folder/left/open
//! @ /the/selection
//! ```
use std::fs;
use std::path::{Path, PathBuf};

/// Starting folders kept; older ones drop off the end.
const KEEP: usize = 50;

#[derive(Default, PartialEq, Debug)]
pub struct Place {
    pub open: Vec<PathBuf>,
    pub at: Option<PathBuf>,
}

pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("tb").join("places"))
}

fn parse(text: &str) -> Vec<(PathBuf, Place)> {
    let mut out: Vec<(PathBuf, Place)> = Vec::new();
    for line in text.lines() {
        let Some((tag, p)) = line.split_once(' ') else { continue };
        let p = PathBuf::from(p);
        match (tag, out.last_mut()) {
            ("=", _) => out.push((p, Place::default())),
            ("+", Some(last)) => last.1.open.push(p),
            ("@", Some(last)) => last.1.at = Some(p),
            _ => {}
        }
    }
    out
}

fn show(all: &[(PathBuf, Place)]) -> String {
    let mut s = String::new();
    let line = |s: &mut String, tag: &str, p: &Path| {
        // A newline in a path would split the record; such paths aren't kept.
        if let Some(p) = p.to_str().filter(|p| !p.contains('\n')) {
            s.push_str(&format!("{tag} {p}\n"));
        }
    };
    for (start, place) in all {
        line(&mut s, "=", start);
        for p in &place.open {
            line(&mut s, "+", p);
        }
        if let Some(p) = &place.at {
            line(&mut s, "@", p);
        }
    }
    s
}

pub fn load(file: &Path, start: &Path) -> Option<Place> {
    let text = fs::read_to_string(file).ok()?;
    parse(&text).into_iter().find(|(s, _)| s == start).map(|(_, p)| p)
}

/// Store `place` for `start`, first in the file.
pub fn save(file: &Path, start: &Path, place: Place) -> std::io::Result<()> {
    let mut all = fs::read_to_string(file).map(|t| parse(&t)).unwrap_or_default();
    all.retain(|(s, _)| s != start);
    all.insert(0, (start.to_path_buf(), place));
    all.truncate(KEEP);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(file, show(&all))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_newest_first_and_replaces_a_start() {
        let dir = std::env::temp_dir().join(format!("tb-places-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let f = dir.join("tb/places");
        let one = |n: &str| Place { open: vec![PathBuf::from(format!("/{n}/x"))], at: Some(PathBuf::from(format!("/{n}/x/y"))) };
        save(&f, Path::new("/a"), one("a")).unwrap();
        save(&f, Path::new("/b"), one("b")).unwrap();
        save(&f, Path::new("/a"), one("a2")).unwrap();
        let text = fs::read_to_string(&f).unwrap();
        assert!(text.starts_with("= /a\n+ /a2/x\n@ /a2/x/y\n= /b\n"), "{text}");
        assert_eq!(load(&f, Path::new("/b")), Some(one("b")));
        assert_eq!(load(&f, Path::new("/c")), None);
        let _ = fs::remove_dir_all(&dir);
    }
}
