//! Arena-backed file tree with lazy directory loading.
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    /// Own mtime (lstat). Recursive mtime for dirs lives in the mtime cache.
    pub mtime: SystemTime,
    pub parent: Option<usize>,
    /// None = not read yet.
    pub children: Option<Vec<usize>>,
    pub expanded: bool,
    /// Child the cursor last sat on, restored when re-entering.
    pub last: Option<usize>,
}

pub struct Tree {
    pub nodes: Vec<Node>,
    pub root: usize,
}

fn display_name(path: &Path) -> String {
    match path.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => path.to_string_lossy().into_owned(),
    }
}

fn make(path: PathBuf, parent: Option<usize>) -> Node {
    let meta = fs::symlink_metadata(&path).ok();
    Node {
        name: display_name(&path),
        is_dir: meta.as_ref().is_some_and(|m| m.is_dir()),
        mtime: meta.and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH),
        path,
        parent,
        children: None,
        expanded: false,
        last: None,
    }
}

impl Tree {
    pub fn new(path: &Path) -> Tree {
        Tree { nodes: vec![make(path.to_path_buf(), None)], root: 0 }
    }

    /// Read a directory's entries once. Sorted case-insensitively, dirs and files mixed.
    pub fn load(&mut self, id: usize) {
        if self.nodes[id].children.is_some() || !self.nodes[id].is_dir {
            return;
        }
        let mut paths: Vec<PathBuf> = fs::read_dir(&self.nodes[id].path)
            .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        paths.sort_by_key(|p| display_name(p).to_lowercase());
        let kids = paths
            .into_iter()
            .map(|p| {
                self.nodes.push(make(p, Some(id)));
                self.nodes.len() - 1
            })
            .collect();
        self.nodes[id].children = Some(kids);
    }

    /// Drop a directory's loaded children so the next load re-reads the disk.
    /// Old nodes stay in the arena, unreachable.
    pub fn reload(&mut self, id: usize) {
        self.nodes[id].children = None;
        self.nodes[id].last = None;
        self.load(id);
    }

    pub fn kids(&self, id: usize) -> &[usize] {
        self.nodes[id].children.as_deref().unwrap_or(&[])
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
