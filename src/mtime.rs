//! Recursive-max mtime for directories, computed on a background thread.
//!
//! A dir's heat = newest mtime of anything beneath it. One walk caches the
//! result for every descendant dir it passes through, so expanding a child
//! later is a cache hit. Walks are capped; capped results are marked
//! incomplete. An incomplete result picked up in passing (from an ancestor's
//! walk) is re-walked when requested directly; a directly-requested capped
//! result is final. Symlinks are never followed.
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::SystemTime;

const WALK_CAP: usize = 50_000;

/// (dir, newest mtime beneath, walk finished without hitting the cap)
type Entry = (PathBuf, SystemTime, bool);

/// (newest mtime beneath, complete, came from a walk rooted at this dir)
pub type Cached = (SystemTime, bool, bool);

pub struct Mtime {
    tx: Sender<PathBuf>,
    rx: Receiver<Vec<(PathBuf, Cached)>>,
    pub cache: HashMap<PathBuf, Cached>,
    pending: HashSet<PathBuf>,
}

impl Mtime {
    pub fn spawn() -> Mtime {
        Self::with_cap(WALK_CAP)
    }

    pub fn with_cap(cap: usize) -> Mtime {
        let (req_tx, req_rx) = mpsc::channel::<PathBuf>();
        let (res_tx, res_rx) = mpsc::channel();
        thread::spawn(move || worker(req_rx, res_tx, cap));
        Mtime { tx: req_tx, rx: res_rx, cache: HashMap::new(), pending: HashSet::new() }
    }

    pub fn request(&mut self, dir: &Path) {
        let stale = self.cache.get(dir).is_none_or(|c| !c.1 && !c.2);
        if stale && self.pending.insert(dir.to_path_buf()) {
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
            // Better-quality results win: complete > direct-capped > passing-capped.
            let rank = |c: &Cached| 2 * c.1 as u8 + c.2 as u8;
            for (p, c) in batch {
                self.pending.remove(&p);
                if self.cache.get(&p).is_none_or(|old| rank(old) <= rank(&c)) {
                    self.cache.insert(p, c);
                }
            }
            changed = true;
        }
        changed
    }
}

// Dedup lives on the main side (cache + pending); a repeat request here
// means the main side wants a fresh walk (reload, or a passing-capped dir).
fn worker(rx: Receiver<PathBuf>, tx: Sender<Vec<(PathBuf, Cached)>>, cap: usize) {
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
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
        let mut budget = cap;
        walk(&dir, &mut budget, &mut out);
        let out = out.into_iter().map(|(p, t, c)| {
            let direct = p == dir;
            (p, (t, c, direct))
        });
        if tx.send(out.collect()).is_err() {
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

    fn settle(m: &mut Mtime) {
        let t0 = std::time::Instant::now();
        while !m.pending.is_empty() && t0.elapsed() < Duration::from_secs(5) {
            m.poll();
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn passing_capped_dir_rewalks_and_reload_rewalks() {
        let root = std::env::temp_dir().join(format!("tb-rewalk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("x/big")).unwrap();
        for i in 0..3 {
            fs::write(root.join(format!("x/big/{i}")), "").unwrap();
        }
        // Budget 4: x, big, two files -> cap hit inside big.
        let mut m = Mtime::with_cap(4);
        m.request(&root);
        settle(&mut m);
        let big = root.join("x/big");
        assert_eq!(m.cache[&big].1, false, "capped in passing");
        assert_eq!(m.cache[&big].2, false);
        assert_eq!(m.cache[&root], (m.cache[&root].0, false, true), "direct-capped");
        m.request(&root);
        assert!(m.pending.is_empty(), "direct-capped is final, no re-request loop");
        m.request(&big);
        settle(&mut m);
        assert!(m.cache[&big].1, "direct walk of big fits in budget");
        m.invalidate(&big);
        m.request(&big);
        settle(&mut m);
        assert!(m.cache.get(&big).is_some_and(|c| c.1), "re-walk after invalidate arrives");
    }
}
