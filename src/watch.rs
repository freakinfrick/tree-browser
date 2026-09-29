//! Live updates: a background thread re-lists the open folders about once a
//! second and reports the ones that changed.
//!
//! Polling rather than inotify / FSEvents: no dependency, the same behavior on
//! every Unix and on network mounts, and the watched set is small (only the
//! folders that are open on screen). A change deep inside a closed folder isn't
//! seen until that folder is opened or `r` re-walks it.
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

pub const EVERY: Duration = Duration::from_millis(1000);

/// An open folder whose entries changed since the last look.
#[derive(Debug)]
pub struct Change {
    pub dir: PathBuf,
    /// Entries were added or removed (not just modified).
    pub listing: bool,
    /// Newest mtime among the changed entries (and the folder itself when the listing changed).
    pub newest: SystemTime,
    /// Same, over non-dot files only, for the dotfiles-hidden view.
    pub newest_vis: SystemTime,
}

pub struct Watch {
    tx: Sender<Vec<PathBuf>>,
    rx: Receiver<Vec<Change>>,
    sent: Vec<PathBuf>,
}

impl Watch {
    pub fn spawn(every: Duration) -> Watch {
        let (set_tx, set_rx) = mpsc::channel();
        let (ch_tx, ch_rx) = mpsc::channel();
        thread::spawn(move || worker(set_rx, ch_tx, every));
        Watch { tx: set_tx, rx: ch_rx, sent: Vec::new() }
    }

    /// The folders to watch; only sent on when it differs from last time.
    pub fn set(&mut self, dirs: Vec<PathBuf>) {
        if dirs != self.sent {
            let _ = self.tx.send(dirs.clone());
            self.sent = dirs;
        }
    }

    pub fn poll(&mut self) -> Vec<Change> {
        self.rx.try_iter().flatten().collect()
    }
}

/// name -> (mtime, size, is_dir)
type Snap = HashMap<OsString, (SystemTime, u64, bool)>;

fn snap(dir: &PathBuf) -> Snap {
    let Ok(rd) = fs::read_dir(dir) else { return Snap::new() };
    rd.flatten()
        .map(|e| {
            // DirEntry::metadata is lstat on Linux: symlinks are not followed.
            let m = e.metadata().ok();
            let t = m.as_ref().and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH);
            (e.file_name(), (t, m.as_ref().map_or(0, |m| m.len()), m.is_some_and(|m| m.is_dir())))
        })
        .collect()
}

/// What changed between two looks at `dir`, if anything.
fn diff(dir: &PathBuf, old: &Snap, new: &Snap) -> Option<Change> {
    let listing = old.len() != new.len() || new.keys().any(|k| !old.contains_key(k));
    let mut newest = SystemTime::UNIX_EPOCH;
    let mut newest_vis = SystemTime::UNIX_EPOCH;
    let mut any = listing;
    for (name, v) in new {
        if old.get(name) == Some(v) {
            continue;
        }
        any = true;
        newest = newest.max(v.0);
        if !v.2 && name.as_encoded_bytes().first() != Some(&b'.') {
            newest_vis = newest_vis.max(v.0);
        }
    }
    if listing && let Ok(t) = fs::symlink_metadata(dir).and_then(|m| m.modified()) {
        newest = newest.max(t);
    }
    any.then(|| Change { dir: dir.clone(), listing, newest, newest_vis })
}

fn worker(rx: Receiver<Vec<PathBuf>>, tx: Sender<Vec<Change>>, every: Duration) {
    let mut snaps: HashMap<PathBuf, Snap> = HashMap::new();
    let mut next = Instant::now() + every;
    loop {
        match rx.recv_timeout(next.saturating_duration_since(Instant::now())) {
            Ok(mut set) => {
                while let Ok(s) = rx.try_recv() {
                    set = s;
                }
                snaps.retain(|d, _| set.contains(d));
                // First look at a newly opened folder is the baseline, not a change.
                for d in set {
                    snaps.entry(d).or_insert_with_key(snap);
                }
                if Instant::now() < next {
                    continue;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        next = Instant::now() + every;
        let mut out = Vec::new();
        for (dir, old) in snaps.iter_mut() {
            let new = snap(dir);
            if let Some(c) = diff(dir, old, &new) {
                out.push(c);
            }
            *old = new;
        }
        if !out.is_empty() && tx.send(out).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(w: &mut Watch) -> Vec<Change> {
        let t0 = Instant::now();
        loop {
            let c = w.poll();
            if !c.is_empty() || t0.elapsed() > Duration::from_secs(5) {
                return c;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn reports_added_modified_and_removed_entries() {
        let root = std::env::temp_dir().join(format!("tb-watch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a"), "").unwrap();
        let mut w = Watch::spawn(Duration::from_millis(20));
        w.set(vec![root.clone()]);
        thread::sleep(Duration::from_millis(60));
        assert!(w.poll().is_empty(), "the first look is a baseline");

        fs::write(root.join("b"), "").unwrap();
        let c = wait(&mut w);
        assert_eq!(c.len(), 1);
        assert!(c[0].listing && c[0].dir == root);
        assert!(c[0].newest_vis > SystemTime::UNIX_EPOCH);

        fs::write(root.join("a"), "grown").unwrap();
        let c = wait(&mut w);
        assert!(!c[0].listing, "a modified file isn't a listing change");

        fs::write(root.join(".hidden"), "").unwrap();
        let c = wait(&mut w);
        assert_eq!(c[0].newest_vis, SystemTime::UNIX_EPOCH, "dotfiles don't heat the visible view");

        fs::remove_file(root.join("b")).unwrap();
        assert!(wait(&mut w)[0].listing);

        w.set(Vec::new());
        thread::sleep(Duration::from_millis(30));
        fs::write(root.join("c"), "").unwrap();
        thread::sleep(Duration::from_millis(80));
        assert!(w.poll().is_empty(), "unwatched folders are dropped");
    }
}
