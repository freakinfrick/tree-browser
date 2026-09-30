//! Arena-backed file tree with lazy directory loading.
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    /// Own mtime (lstat). Recursive mtime for dirs lives in the mtime cache.
    pub mtime: SystemTime,
    /// Own size in bytes (lstat); 0 for dirs, whose total lives in `rec`.
    pub size: u64,
    /// Recursive (newest beneath, walk complete, newest ignoring dotfiles,
    /// bytes beneath), copied from the mtime cache when results land, so
    /// per-frame code never hashes paths.
    pub rec: Option<(SystemTime, bool, SystemTime, u64)>,
    pub parent: Option<usize>,
    /// None = not read yet.
    pub children: Option<Vec<usize>>,
    pub expanded: bool,
    /// Child the cursor last sat on, restored when re-entering.
    pub last: Option<usize>,
}

/// What a folder's entries are ordered by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortKey {
    Name,
    /// Newest first, by heat (recursive for folders).
    Modified,
    /// Largest first, by recursive size for folders.
    Size,
    /// Folders first, then by extension.
    Type,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sort {
    pub key: SortKey,
    /// Flip the key's natural direction.
    pub rev: bool,
}

impl Default for Sort {
    fn default() -> Sort {
        Sort { key: SortKey::Name, rev: false }
    }
}

impl Sort {
    /// Parse `name`, `modified`, `size` or `type`, with a leading `-` to reverse.
    pub fn parse(s: &str) -> Option<Sort> {
        let (rev, word) = match s.strip_prefix('-') {
            Some(w) => (true, w),
            None => (false, s),
        };
        let key = match word {
            "name" => SortKey::Name,
            "modified" | "mtime" | "time" => SortKey::Modified,
            "size" => SortKey::Size,
            "type" | "ext" => SortKey::Type,
            _ => return None,
        };
        Some(Sort { key, rev })
    }

    /// Next key in the `o` cycle, in its natural direction.
    pub fn next(self) -> Sort {
        let key = match self.key {
            SortKey::Name => SortKey::Modified,
            SortKey::Modified => SortKey::Size,
            SortKey::Size => SortKey::Type,
            SortKey::Type => SortKey::Name,
        };
        Sort { key, rev: false }
    }

    /// Status bar text, e.g. "largest first".
    pub fn describe(self) -> &'static str {
        match (self.key, self.rev) {
            (SortKey::Name, false) => "name a–z",
            (SortKey::Name, true) => "name z–a",
            (SortKey::Modified, false) => "newest first",
            (SortKey::Modified, true) => "oldest first",
            (SortKey::Size, false) => "largest first",
            (SortKey::Size, true) => "smallest first",
            (SortKey::Type, false) => "type a–z",
            (SortKey::Type, true) => "type z–a",
        }
    }
}

/// What a `refresh` found.
#[derive(Default)]
pub struct Refreshed {
    /// Entries came or went, rather than just their mtimes, sizes or order changing.
    pub changed: bool,
    /// New entries and ones whose mtime moved.
    pub touched: Vec<usize>,
    /// Entries no longer there (still in the arena).
    pub gone: Vec<usize>,
}

pub struct Tree {
    pub nodes: Vec<Node>,
    pub root: usize,
    pub show_hidden: bool,
    pub sort: Sort,
    /// Folders above files under every sort key.
    pub folders_first: bool,
    /// Digit runs in names compare as numbers.
    pub natural: bool,
    /// Always visible even when hidden (the root..cursor path), so the
    /// cursor can never sit inside something invisible.
    pub reveal: HashSet<usize>,
}

fn display_name(path: &Path) -> String {
    match path.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => path.to_string_lossy().into_owned(),
    }
}

pub fn make(path: PathBuf, parent: Option<usize>) -> Node {
    let meta = fs::symlink_metadata(&path).ok();
    let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
    Node {
        name: display_name(&path),
        is_dir,
        size: meta.as_ref().filter(|_| !is_dir).map_or(0, |m| m.len()),
        mtime: meta.and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH),
        path,
        rec: None,
        parent,
        children: None,
        expanded: false,
        last: None,
    }
}

/// A directory's entries, unordered: the tree sorts them.
fn listing(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir).map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect()).unwrap_or_default()
}

/// Lowercased extension, "" for none (dotfiles like `.bashrc` have none).
fn ext(name: &str) -> String {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Compare with runs of digits as numbers: `file2` < `file10`. Equal
/// numbers with different zero padding fall back to the text.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        let (Some(ca), Some(cb)) = (a.chars().next(), b.chars().next()) else { return a.len().cmp(&b.len()) };
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let run = |s: &str| s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
            let (na, nb) = (&a[..run(a)], &b[..run(b)]);
            let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
            let o = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb)).then_with(|| na.len().cmp(&nb.len()));
            if o != Ordering::Equal {
                return o;
            }
            (a, b) = (&a[na.len()..], &b[nb.len()..]);
        } else {
            if ca != cb {
                return ca.cmp(&cb);
            }
            (a, b) = (&a[ca.len_utf8()..], &b[cb.len_utf8()..]);
        }
    }
}

impl Tree {
    pub fn new(path: &Path) -> Tree {
        Tree {
            nodes: vec![make(path.to_path_buf(), None)],
            root: 0,
            show_hidden: false,
            sort: Sort::default(),
            folders_first: false,
            natural: true,
            reveal: HashSet::new(),
        }
    }

    /// Bytes at or under the node: a file's own size, a folder's walked
    /// total (0 until its walk lands).
    pub fn size(&self, id: usize) -> u64 {
        let n = &self.nodes[id];
        match n.rec {
            Some(r) if n.is_dir => r.3,
            _ => n.size,
        }
    }

    /// Where `a` goes relative to its sibling `b` under the current sort.
    /// Name (case-insensitive, then exact) breaks ties, so the order is total.
    pub fn cmp(&self, a: usize, b: usize) -> Ordering {
        let (x, y) = (&self.nodes[a], &self.nodes[b]);
        let by_key = match self.sort.key {
            SortKey::Name => Ordering::Equal,
            SortKey::Modified => self.heat(b).cmp(&self.heat(a)),
            SortKey::Size => self.size(b).cmp(&self.size(a)),
            SortKey::Type => y.is_dir.cmp(&x.is_dir).then_with(|| ext(&x.name).cmp(&ext(&y.name))),
        };
        let (lx, ly) = (x.name.to_lowercase(), y.name.to_lowercase());
        let by_name = if self.natural { natural_cmp(&lx, &ly) } else { lx.cmp(&ly) };
        let o = by_key.then(by_name).then_with(|| x.name.cmp(&y.name));
        let o = if self.sort.rev { o.reverse() } else { o };
        if self.folders_first { y.is_dir.cmp(&x.is_dir).then(o) } else { o }
    }

    /// Put a loaded folder's entries in sort order. Returns true if it moved any.
    fn sort_kids(&mut self, id: usize) -> bool {
        let Some(mut kids) = self.nodes[id].children.take() else { return false };
        let before = kids.clone();
        kids.sort_by(|&a, &b| self.cmp(a, b));
        let moved = kids != before;
        self.nodes[id].children = Some(kids);
        moved
    }

    /// Re-sort every loaded folder still reachable from the root, after the
    /// sort changes or the heat or sizes it sorts by do. Returns true if
    /// anything moved.
    pub fn resort(&mut self) -> bool {
        let mut moved = false;
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            moved |= self.sort_kids(id);
            if let Some(kids) = &self.nodes[id].children {
                stack.extend(kids.iter().copied().filter(|&k| self.nodes[k].children.is_some()));
            }
        }
        moved
    }

    /// Read a directory's entries once, in sort order.
    pub fn load(&mut self, id: usize) {
        if self.nodes[id].children.is_some() || !self.nodes[id].is_dir {
            return;
        }
        let kids = listing(&self.nodes[id].path)
            .into_iter()
            .map(|p| {
                self.nodes.push(make(p, Some(id)));
                self.nodes.len() - 1
            })
            .collect();
        self.nodes[id].children = Some(kids);
        self.sort_kids(id);
    }

    /// Take entries already read elsewhere (the `e` walk) as a folder's listing.
    pub fn load_nodes(&mut self, id: usize, nodes: Vec<Node>) {
        if self.nodes[id].children.is_some() || !self.nodes[id].is_dir {
            return;
        }
        let kids = nodes
            .into_iter()
            .map(|mut n| {
                n.parent = Some(id);
                self.nodes.push(n);
                self.nodes.len() - 1
            })
            .collect();
        self.nodes[id].children = Some(kids);
        self.sort_kids(id);
    }

    /// Drop a directory's loaded children so the next load re-reads the disk.
    /// Old nodes stay in the arena, unreachable.
    pub fn reload(&mut self, id: usize) {
        self.nodes[id].children = None;
        self.nodes[id].last = None;
        self.load(id);
    }

    /// Re-read a loaded directory in place: entries still there keep their
    /// nodes (fold state, last cursor, animation), new ones are added, gone
    /// ones dropped (left in the arena, unreachable).
    pub fn refresh(&mut self, id: usize) -> Refreshed {
        let mut out = Refreshed::default();
        let Some(old) = self.nodes[id].children.clone() else { return out };
        let by_path: HashMap<PathBuf, usize> = old.iter().map(|&k| (self.nodes[k].path.clone(), k)).collect();
        let mut kids = Vec::new();
        let (mut kept, mut added) = (0, false);
        for p in listing(&self.nodes[id].path) {
            let fresh = make(p, Some(id));
            match by_path.get(&fresh.path) {
                // Same kind: refresh the mtime and size, keep everything else.
                Some(&k) if self.nodes[k].is_dir == fresh.is_dir => {
                    if self.nodes[k].mtime != fresh.mtime {
                        self.nodes[k].mtime = fresh.mtime;
                        out.touched.push(k);
                    }
                    self.nodes[k].size = fresh.size;
                    kids.push(k);
                    kept += 1;
                }
                _ => {
                    self.nodes.push(fresh);
                    kids.push(self.nodes.len() - 1);
                    out.touched.push(self.nodes.len() - 1);
                    added = true;
                }
            }
        }
        out.gone = old.iter().copied().filter(|k| !kids.contains(k)).collect();
        // Order alone doesn't count: the sort moves entries as mtimes and sizes change.
        out.changed = added || kept != old.len();
        if self.nodes[id].last.is_some_and(|l| !kids.contains(&l)) {
            self.nodes[id].last = None;
        }
        self.nodes[id].children = Some(kids);
        self.sort_kids(id);
        out
    }

    /// Still reachable from the root (not dropped by a reload or refresh).
    pub fn attached(&self, id: usize) -> bool {
        let path = self.path_to(id);
        path[0] == self.root
            && path.windows(2).all(|w| self.nodes[w[0]].children.as_ref().is_some_and(|k| k.contains(&w[1])))
    }

    /// Loaded, expanded, reachable directories: what's open on screen.
    pub fn open_dirs(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            let n = &self.nodes[id];
            if let (true, Some(kids)) = (n.expanded || id == self.root, &n.children) {
                out.push(id);
                stack.extend(kids.iter().copied().filter(|&k| self.nodes[k].is_dir));
            }
        }
        out
    }

    pub fn is_hidden(&self, id: usize) -> bool {
        self.nodes[id].name.starts_with('.')
    }

    /// Visible children: dot entries are skipped unless shown or revealed.
    pub fn kids(&self, id: usize) -> Vec<usize> {
        let all = self.nodes[id].children.as_deref().unwrap_or(&[]);
        all.iter().copied().filter(|&k| self.show_hidden || !self.is_hidden(k) || self.reveal.contains(&k)).collect()
    }

    /// Newest change at or under the node, honoring the hidden toggle.
    pub fn heat(&self, id: usize) -> SystemTime {
        let n = &self.nodes[id];
        match n.rec {
            Some((all, ..)) if self.show_hidden => all.max(n.mtime),
            // Visible view ignores the dir's own mtime (dotfile churn bumps
            // it) unless nothing visible exists beneath at all.
            Some((_, _, vis, _)) if vis > SystemTime::UNIX_EPOCH => vis,
            _ => n.mtime,
        }
    }

    /// Make the root's parent directory the new root, grafting the current tree in.
    pub fn reroot_up(&mut self) -> bool {
        let old = self.root;
        let Some(up) = self.nodes[old].path.parent().map(Path::to_path_buf) else {
            return false;
        };
        self.nodes.push(make(up, None));
        let new = self.nodes.len() - 1;
        self.load(new);
        let old_path = self.nodes[old].path.clone();
        let mut kids = self.nodes[new].children.take().unwrap_or_default();
        match kids.iter().position(|&k| self.nodes[k].path == old_path) {
            Some(i) => kids[i] = old,
            None => kids.push(old),
        }
        self.nodes[old].parent = Some(new);
        self.nodes[old].name = display_name(&old_path);
        let n = &mut self.nodes[new];
        n.children = Some(kids);
        n.expanded = true;
        n.last = Some(old);
        self.root = new;
        // The old root carries walked heat and size its fresh stand-in lacked.
        self.sort_kids(new);
        true
    }

    /// Root first, `id` last.
    pub fn path_to(&self, id: usize) -> Vec<usize> {
        let mut v = vec![id];
        let mut cur = id;
        while let Some(p) = self.nodes[cur].parent {
            v.push(p);
            cur = p;
        }
        v.reverse();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order_counts_numbers() {
        let mut v = ["file10", "file2", "file1", "file02", "a", "file", "b1c", "b1b"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["a", "b1b", "b1c", "file", "file1", "file2", "file02", "file10"]);
        assert_eq!(natural_cmp("x99999999999999999999999", "x100000000000000000000000"), Ordering::Less, "no overflow");
    }
}
