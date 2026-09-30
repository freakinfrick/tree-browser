//! `e`: open every folder under one, read on a background thread.
//!
//! The walk is breadth-first, so when it hits the cap the shallow levels
//! are the ones that open. Hidden folders (unless shown) and folders git
//! ignores are listed but not descended into: nobody wants node_modules
//! unfurled across the screen. Symlinks are never followed.
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::tree::{make, Node};

/// At most this many folders open...
pub const MAX_DIRS: usize = 400;
/// ...or this many entries appear, whichever comes first.
pub const MAX_ENTRIES: usize = 6000;

pub enum Msg {
    /// Folders read so far.
    Progress(usize),
    /// Every folder read, parents before children, and whether a cap stopped it.
    Done(Vec<(PathBuf, Vec<Node>)>, bool),
}

/// Start reading everything under `root`. Dropping the receiver cancels it.
pub fn spawn(root: PathBuf, show_hidden: bool, skip: HashSet<PathBuf>) -> Receiver<Msg> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (out, capped) = walk(root, show_hidden, &skip, &mut |n| tx.send(Msg::Progress(n)).is_ok());
        let _ = tx.send(Msg::Done(out, capped));
    });
    rx
}

/// Breadth-first read. `progress` returning false stops early (cancelled).
pub fn walk(
    root: PathBuf,
    show_hidden: bool,
    skip: &HashSet<PathBuf>,
    progress: &mut dyn FnMut(usize) -> bool,
) -> (Vec<(PathBuf, Vec<Node>)>, bool) {
    let mut out = Vec::new();
    let mut queue = VecDeque::from([root]);
    let mut entries = 0;
    while let Some(dir) = queue.pop_front() {
        if out.len() >= MAX_DIRS || entries >= MAX_ENTRIES {
            return (out, true);
        }
        let kids: Vec<Node> = fs::read_dir(&dir)
            .map(|rd| rd.flatten().map(|e| make(e.path(), None)).collect())
            .unwrap_or_default();
        entries += kids.len();
        for k in &kids {
            if k.is_dir && (show_hidden || !k.name.starts_with('.')) && !skip.contains(&k.path) {
                queue.push_back(k.path.clone());
            }
        }
        out.push((dir, kids));
        if out.len() % 8 == 0 && !progress(out.len()) {
            return (out, true);
        }
    }
    (out, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_breadth_first_and_skips_hidden_and_ignored() {
        let root = std::env::temp_dir().join(format!("tb-explode-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for d in ["a/b/c", ".git/objects", "node_modules/x", "d"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        fs::write(root.join("a/b/c/deep.txt"), "").unwrap();
        let skip = HashSet::from([root.join("node_modules")]);
        let (out, capped) = walk(root.clone(), false, &skip, &mut |_| true);
        let dirs: Vec<_> = out.iter().map(|(p, _)| p.strip_prefix(&root).unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(dirs[0], "");
        assert!(dirs.contains(&"a/b/c".to_string()));
        assert!(!dirs.iter().any(|d| d.starts_with(".git") || d.starts_with("node_modules")), "{dirs:?}");
        let depth = |d: &str| d.matches('/').count() + !d.is_empty() as usize;
        assert!(dirs.windows(2).all(|w| depth(&w[0]) <= depth(&w[1])), "shallow first: {dirs:?}");
        assert!(!capped);
        let names: Vec<_> = out[0].1.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&".git") && names.contains(&"node_modules"), "still listed, just not opened");
    }

    #[test]
    fn stops_at_the_cap_and_when_cancelled() {
        let root = std::env::temp_dir().join(format!("tb-explode-cap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for i in 0..20 {
            fs::create_dir_all(root.join(format!("d{i}/inner"))).unwrap();
        }
        let (out, capped) = walk(root.clone(), false, &HashSet::new(), &mut |n| n < 8);
        assert!(capped && out.len() == 8, "cancelled after the first progress report");
        let (out, capped) = walk(root, false, &HashSet::new(), &mut |_| true);
        assert!(!capped && out.len() == 41);
    }
}
