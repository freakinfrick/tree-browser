//! Recursive-max mtime for directories, computed on a background thread.
//!
//! A dir's heat = newest mtime of anything beneath it. One walk caches the
//! result for every descendant dir it passes through, so expanding a child
//! later is a cache hit. Walks are capped; capped results are marked
//! incomplete. Symlinks are never followed.
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::SystemTime;

const WALK_CAP: usize = 50_000;

/// (dir, newest mtime beneath, walk finished without hitting the cap)
type Entry = (PathBuf, SystemTime, bool);

pub struct Mtime {
    tx: Sender<PathBuf>,
    rx: Receiver<Vec<Entry>>,
    pub cache: HashMap<PathBuf, (SystemTime, bool)>,
    pending: HashSet<PathBuf>,
}

impl Mtime {
    pub fn spawn() -> Mtime {
        let (req_tx, req_rx) = mpsc::channel::<PathBuf>();
        let (res_tx, res_rx) = mpsc::channel();
        thread::spawn(move || worker(req_rx, res_tx));
        Mtime { tx: req_tx, rx: res_rx, cache: HashMap::new(), pending: HashSet::new() }
    }

    pub fn request(&mut self, dir: &Path) {
        if !self.cache.contains_key(dir) && self.pending.insert(dir.to_path_buf()) {
            let _ = self.tx.send(dir.to_path_buf());
        }
    }

    /// Forget cached results at and below `dir` (after a reload).
    pub fn invalidate(&mut self, dir: &Path) {
        self.cache.retain(|p, _| !p.starts_with(dir));
        self.pending.retain(|p| !p.starts_with(dir));
    }

    /// Drain finished walks. Returns true if anything changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(batch) = self.rx.try_recv() {
            for (p, t, done) in batch {
                self.pending.remove(&p);
                // A complete result from a deeper walk beats a capped one from above.
                let keep_old = self.cache.get(&p).is_some_and(|&(_, d)| d && !done);
                if !keep_old {
                    self.cache.insert(p, (t, done));
                }
            }
            changed = true;
        }
        changed
    }
}

fn worker(rx: Receiver<PathBuf>, tx: Sender<Vec<Entry>>) {
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    let mut done: HashSet<PathBuf> = HashSet::new();
    loop {
        // Newest requests first: they are what the cursor is near.
        while let Ok(p) = rx.try_recv() {
            queue.push_front(p);
        }
        let dir = match queue.pop_front() {
            Some(d) => d,
            None => match rx.recv() {
                Ok(d) => d,
                Err(_) => return,
            },
        };
        let mut out = Vec::new();
        if done.contains(&dir) {
            continue;
        }
        let mut budget = WALK_CAP;
        walk(&dir, &mut budget, &mut out);
        for (p, _, complete) in &out {
            if *complete {
                done.insert(p.clone());
            }
        }
        if tx.send(out).is_err() {
            return;
        }
    }
}

pub fn walk(dir: &Path, budget: &mut usize, out: &mut Vec<Entry>) -> (SystemTime, bool) {
    let mut newest = fs::symlink_metadata(dir)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut complete = true;
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            if *budget == 0 {
                complete = false;
                break;
            }
            *budget -= 1;
            // DirEntry::metadata is lstat on Linux: symlinks are not followed.
            if let Ok(t) = e.metadata().and_then(|m| m.modified()) {
                newest = newest.max(t);
            }
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                let (t, c) = walk(&e.path(), budget, out);
                newest = newest.max(t);
                complete &= c;
            }
        }
    }
    out.push((dir.to_path_buf(), newest, complete));
    (newest, complete)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn hot_file_deep_heats_every_ancestor() {
        let root = std::env::temp_dir().join(format!("tb-mtime-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a/b/c")).unwrap();
        fs::create_dir_all(root.join("cold")).unwrap();
        let old = SystemTime::now() - Duration::from_secs(86400 * 400);
        let hot = root.join("a/b/c/hot.txt");
        fs::write(&hot, "x").unwrap();
        for d in ["", "a", "a/b", "a/b/c", "cold"] {
            fs::File::open(root.join(d)).unwrap().set_modified(old).unwrap();
        }
        let mut out = Vec::new();
        let mut budget = WALK_CAP;
        walk(&root, &mut budget, &mut out);
        let get = |p: &str| out.iter().find(|e| e.0 == root.join(p)).unwrap().1;
        let recent = SystemTime::now() - Duration::from_secs(60);
        for d in ["", "a", "a/b", "a/b/c"] {
            assert!(get(d) > recent, "{d:?} should be hot");
        }
        assert!(get("cold") < recent, "cold stays cold");
    }

    #[test]
    fn cap_marks_incomplete() {
        let root = std::env::temp_dir().join(format!("tb-cap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        for i in 0..5 {
            fs::write(root.join(i.to_string()), "").unwrap();
        }
        let mut out = Vec::new();
        let mut budget = 3;
        assert!(!walk(&root, &mut budget, &mut out).1);
    }
}
