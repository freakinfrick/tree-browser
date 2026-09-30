//! Git status for the repos the open folders live in, on a background thread.
//!
//! The worker is told which folders are open, finds the repo each belongs to
//! (the nearest ancestor holding `.git`), and runs `git status` on those repos
//! whenever the set changes, when poked (a live change, a reload), and every
//! few seconds for what the file watcher can't see (commits, `git add`).
//! `--no-optional-locks` keeps it from taking the index lock out from under
//! the user's own git commands.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

pub const EVERY: Duration = Duration::from_secs(3);

/// One path's state, in rising order of how much it wants attention: a folder
/// shows the highest state of anything inside it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum St {
    Ignored,
    Untracked,
    /// Changes staged, none on top in the worktree.
    Staged,
    Modified,
    Conflict,
}

impl St {
    /// The one-cell marker drawn after a file's name.
    pub fn glyph(self) -> &'static str {
        match self {
            St::Ignored => "",
            St::Untracked => "?",
            St::Staged => "+",
            St::Modified => "M",
            St::Conflict => "!",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            St::Ignored => "ignored",
            St::Untracked => "untracked",
            St::Staged => "staged",
            St::Modified => "modified",
            St::Conflict => "conflict",
        }
    }
}

#[derive(Default, Debug)]
pub struct Repo {
    /// Branch, with ahead/behind arrows: `main ↑1`.
    pub branch: String,
    /// Files and folders with a state, absolute. Folders carry the highest
    /// state beneath them (ignored doesn't climb).
    pub paths: HashMap<PathBuf, St>,
    /// Folders reported whole: everything inside shares the state.
    pub whole: HashMap<PathBuf, St>,
}

impl Repo {
    /// State of a path inside this repo, None if clean.
    pub fn get(&self, top: &Path, path: &Path) -> Option<St> {
        if let Some(&s) = self.paths.get(path) {
            return Some(s);
        }
        path.ancestors().take_while(|a| a.starts_with(top) && *a != top).find_map(|a| self.whole.get(a).copied())
    }
}

enum Msg {
    Dirs(Vec<PathBuf>),
    Poke,
}

pub struct Git {
    tx: Sender<Msg>,
    rx: Receiver<(PathBuf, Option<Repo>)>,
    sent: Vec<PathBuf>,
    /// Repo top -> its last status.
    pub repos: HashMap<PathBuf, Repo>,
}

impl Git {
    pub fn spawn(every: Duration) -> Git {
        let (tx, req) = mpsc::channel();
        let (res, rx) = mpsc::channel();
        thread::spawn(move || worker(req, res, every));
        Git { tx, rx, sent: Vec::new(), repos: HashMap::new() }
    }

    /// The open folders; only sent on when it differs from last time.
    pub fn set(&mut self, dirs: Vec<PathBuf>) {
        if dirs != self.sent {
            let _ = self.tx.send(Msg::Dirs(dirs.clone()));
            self.sent = dirs;
        }
    }

    /// Something changed on disk: re-run status now.
    pub fn poke(&self) {
        let _ = self.tx.send(Msg::Poke);
    }

    /// Take finished results. Returns true if anything changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((top, repo)) = self.rx.try_recv() {
            match repo {
                Some(r) => self.repos.insert(top, r),
                None => self.repos.remove(&top),
            };
            changed = true;
        }
        changed
    }

    /// The repo holding `path` and its top.
    pub fn repo_of(&self, path: &Path) -> Option<(&Path, &Repo)> {
        path.ancestors().find_map(|a| self.repos.get_key_value(a)).map(|(t, r)| (t.as_path(), r))
    }

    /// State of `path`, None if clean or not in a repo.
    pub fn state(&self, path: &Path) -> Option<St> {
        self.repo_of(path).and_then(|(top, r)| r.get(top, path))
    }
}

/// Nearest ancestor (or `dir` itself) holding a `.git` dir or file.
pub fn top_of(dir: &Path) -> Option<PathBuf> {
    dir.ancestors().find(|a| a.join(".git").symlink_metadata().is_ok()).map(Path::to_path_buf)
}

fn worker(rx: Receiver<Msg>, tx: Sender<(PathBuf, Option<Repo>)>, every: Duration) {
    let mut tops: HashSet<PathBuf> = HashSet::new();
    // Last status sent per repo, so an unchanged repo isn't sent again.
    let mut last: HashMap<PathBuf, Vec<u8>> = HashMap::new();
    loop {
        let mut fresh = Vec::new();
        let all = match rx.recv_timeout(every) {
            Ok(m) => {
                let mut poke = false;
                for m in std::iter::once(m).chain(rx.try_iter()) {
                    match m {
                        Msg::Dirs(dirs) => {
                            let now: HashSet<PathBuf> = dirs.iter().filter_map(|d| top_of(d)).collect();
                            fresh = now.difference(&tops).cloned().collect();
                            for gone in tops.difference(&now) {
                                last.remove(gone);
                            }
                            tops = now;
                        }
                        Msg::Poke => poke = true,
                    }
                }
                poke
            }
            Err(RecvTimeoutError::Timeout) => true,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let run: Vec<PathBuf> = if all { tops.iter().cloned().collect() } else { fresh };
        for top in run {
            let out = status(&top);
            if last.get(&top) == out.as_ref() {
                continue;
            }
            let repo = out.as_deref().map(|o| parse(&top, o));
            match out {
                Some(o) => last.insert(top.clone(), o),
                None => last.remove(&top),
            };
            if tx.send((top, repo)).is_err() {
                return;
            }
        }
    }
}

fn status(top: &Path) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .args(["--no-optional-locks", "status", "--porcelain=v1", "-z", "--branch", "--ignored", "--untracked-files=normal"])
        .current_dir(top)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// `## main...origin/main [ahead 1, behind 2]` -> `main ↑1 ↓2`.
fn branch(line: &str) -> String {
    let line = line.trim_start_matches("## ");
    let (name, rest) = match line.split_once(" [") {
        Some((n, r)) => (n, r.trim_end_matches(']')),
        None => (line, ""),
    };
    let name = name.strip_prefix("No commits yet on ").unwrap_or(name);
    let name = name.split("...").next().unwrap_or(name);
    let name = if name == "HEAD (no branch)" { "detached" } else { name };
    let mut s = name.to_string();
    for part in rest.split(", ") {
        if let Some(n) = part.strip_prefix("ahead ") {
            s += &format!(" ↑{n}");
        } else if let Some(n) = part.strip_prefix("behind ") {
            s += &format!(" ↓{n}");
        }
    }
    s
}

fn classify(xy: &[u8]) -> St {
    match xy {
        b"??" => St::Untracked,
        b"!!" => St::Ignored,
        [b'U', _] | [_, b'U'] | b"AA" | b"DD" => St::Conflict,
        [_, y] if *y != b' ' => St::Modified,
        _ => St::Staged,
    }
}

/// Parse `git status --porcelain=v1 -z --branch` output for the repo at `top`.
pub fn parse(top: &Path, out: &[u8]) -> Repo {
    let mut repo = Repo::default();
    let mut fields = out.split(|&b| b == 0).filter(|f| !f.is_empty());
    while let Some(f) = fields.next() {
        let f = String::from_utf8_lossy(f);
        if f.starts_with("## ") {
            repo.branch = branch(&f);
            continue;
        }
        if f.len() < 4 {
            continue;
        }
        let st = classify(&f.as_bytes()[..2]);
        // Renames and copies carry the old path as the next field.
        if matches!(f.as_bytes()[0], b'R' | b'C') {
            fields.next();
        }
        let rel = &f[3..];
        let path = top.join(rel.trim_end_matches('/'));
        if rel.ends_with('/') {
            repo.whole.insert(path.clone(), st);
        }
        if st != St::Ignored {
            for a in path.ancestors().skip(1).take_while(|a| a.starts_with(top) && *a != top) {
                let e = repo.paths.entry(a.to_path_buf()).or_insert(st);
                *e = (*e).max(st);
            }
        }
        let e = repo.paths.entry(path).or_insert(st);
        *e = (*e).max(st);
    }
    repo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_states_folders_and_branch() {
        let top = Path::new("/r");
        let out = b"## main...origin/main [ahead 2, behind 1]\0 M src/a.rs\0M  src/b.rs\0?? notes/\0!! target/\0\
                    R  src/new.rs\0src/old.rs\0UU src/deep/c.rs\0";
        let r = parse(top, out);
        assert_eq!(r.branch, "main ↑2 ↓1");
        let get = |p: &str| r.get(top, &top.join(p));
        assert_eq!(get("src/a.rs"), Some(St::Modified));
        assert_eq!(get("src/b.rs"), Some(St::Staged));
        assert_eq!(get("src/new.rs"), Some(St::Staged));
        assert_eq!(get("src/old.rs"), None, "a rename's old path isn't an entry");
        assert_eq!(get("src/deep"), Some(St::Conflict));
        assert_eq!(get("src"), Some(St::Conflict), "folders take the loudest state inside");
        assert_eq!(get("notes/todo.md"), Some(St::Untracked), "inside an untracked folder");
        assert_eq!(get("target/debug/tb"), Some(St::Ignored));
        assert_eq!(get("README.md"), None);
    }

    #[test]
    fn branch_names() {
        assert_eq!(branch("## main"), "main");
        assert_eq!(branch("## No commits yet on trunk"), "trunk");
        assert_eq!(branch("## HEAD (no branch)"), "detached");
        assert_eq!(branch("## dev...origin/dev [behind 3]"), "dev ↓3");
    }

    #[test]
    fn worker_reports_a_real_repo() {
        let root = std::env::temp_dir().join(format!("tb-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        let git = |args: &[&str]| {
            Command::new("git").args(args).current_dir(&root).stdout(Stdio::null()).stderr(Stdio::null()).status().unwrap()
        };
        if !git(&["init", "-q", "-b", "main"]).success() {
            return; // no git here
        }
        std::fs::write(root.join("src/a.rs"), "one").unwrap();
        std::fs::write(root.join(".gitignore"), "out/\n").unwrap();
        git(&["add", "."]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "init"]);
        std::fs::write(root.join("src/a.rs"), "two").unwrap();
        std::fs::write(root.join("new.txt"), "").unwrap();
        std::fs::create_dir_all(root.join("out")).unwrap();
        std::fs::write(root.join("out/bin"), "").unwrap();

        let mut g = Git::spawn(Duration::from_secs(60));
        g.set(vec![root.join("src")]);
        let t0 = std::time::Instant::now();
        while !g.poll() && t0.elapsed() < Duration::from_secs(5) {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(g.repo_of(&root.join("src/a.rs")).map(|r| r.1.branch.as_str()), Some("main"));
        assert_eq!(g.state(&root.join("src/a.rs")), Some(St::Modified));
        assert_eq!(g.state(&root.join("src")), Some(St::Modified));
        assert_eq!(g.state(&root.join("new.txt")), Some(St::Untracked));
        assert_eq!(g.state(&root.join("out/bin")), Some(St::Ignored));
        assert_eq!(g.state(&root.join(".gitignore")), None);
    }
}
