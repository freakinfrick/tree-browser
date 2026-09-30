//! tb — horizontal tree file browser. Color = recency (recursive for dirs).
#[cfg(not(unix))]
compile_error!("tb needs a Unix-like OS (Linux, macOS, BSD): it drives the tty with termios and signals");

mod anim;
mod audio;
mod layout;
mod media;
mod mtime;
mod shell;
mod tree;
mod ui;
mod watch;

use std::io::stdout;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton,
    MouseEventKind,
};
use ratatui::crossterm::execute;

use anim::{Damped, Scene};
use mtime::Mtime;
use shell::Run;
use tree::{Sort, SortKey, Tree};
use ui::Preview;

/// Frame budget while animating. Idle = no frames at all.
const FRAME: Duration = Duration::from_micros(16_667);

/// A live-change ripple waits this long per level before lighting the next folder up.
pub const RIPPLE_STEP: Duration = Duration::from_millis(90);
/// And is dropped this long after it reaches a node, faded out by then.
pub const RIPPLE_LIFE: Duration = Duration::from_millis(2200);

/// A live change lighting a node: when the ripple reaches it, and how
/// brightly (1 at the change, dimmer each level up).
#[derive(Clone, Copy, Debug)]
pub struct Ripple {
    pub start: Instant,
    pub strength: f32,
}

pub struct App {
    pub tree: Tree,
    pub cursor: usize,
    pub mt: Mtime,
    pub scene: Scene,
    pub cam: Option<(Damped, Damped)>,
    /// Route flash on navigation, decays 1 -> 0.
    pub flash: f32,
    /// Signal bead running along the line toward the cursor (world x).
    pub bead: Option<Damped>,
    /// Cursor (id, world x) last frame, to launch the bead from.
    pub last_cursor: Option<(usize, i32)>,
    pub help: bool,
    pub help_anim: f32,
    pub view: (u16, u16),
    /// Last frame's clickable labels: (screen x, y, width, node).
    pub hits: Vec<(i32, i32, i32, usize)>,
    /// Last frame's line columns: screen x range [lo, hi) and the line node.
    pub cols: Vec<(i32, i32, usize)>,
    /// Last known pointer cell, for the hover cue.
    pub mouse: Option<(u16, u16)>,
    /// Camera x target held still after the wheel takes over another column,
    /// so the column stays under the pointer. Keys and clicks release it.
    pub cam_hold: Option<f32>,
    /// Last frame's camera x target.
    pub cam_tx: f32,
    pub preview: Option<Preview>,
    /// Bumped whenever tree, cursor, or heat data change; layout is cached per epoch.
    pub epoch: u64,
    pub lay: Option<(u64, layout::Layout)>,
    /// Terminal graphics for image/PDF previews; None = captions only (TB_GRAPHICS=off).
    pub picker: Option<ratatui_image::picker::Picker>,
    /// What the terminal claimed, so `i` can switch back from half-blocks.
    pub detected: ratatui_image::picker::ProtocolType,
    /// A preview drew pixels since the last full repaint.
    pub graphics_shown: bool,
    /// Clear the terminal before the next frame.
    pub repaint: bool,
    /// `!` command line being typed; None = prompt closed.
    pub prompt: Option<String>,
    /// `/` query being typed; None = closed.
    pub search: Option<Search>,
    /// Last confirmed query and the folder whose column it searched, for n / N.
    last_search: Option<(String, Option<usize>)>,
    history: Vec<String>,
    /// Steps back into history (0 = the fresh line).
    hist_at: usize,
    /// Command queued for the main loop, which owns the terminal.
    pending: Option<Run>,
    /// Quit with q: the wrapper cd's the shell to work_dir().
    cd_on_quit: bool,
    /// Started with --cwd-file, so q really does cd (status bar says so).
    pub can_cd: bool,
    /// Polls the open folders for changes; None = off (TB_LIVE=off, and in tests).
    live: Option<watch::Watch>,
    /// Live changes climbing the tree, by node.
    pub ripples: std::collections::HashMap<usize, Ripple>,
}

pub struct Search {
    pub query: String,
    /// Cursor when `/` was pressed; Esc returns here.
    origin: usize,
}

impl App {
    fn new(start: PathBuf) -> App {
        // Root at the parent so the start dir's siblings show on the left, like the clip.
        let root = start.parent().map(PathBuf::from).unwrap_or_else(|| start.clone());
        let mut tree = Tree::new(&root);
        tree.load(tree.root);
        tree.nodes[tree.root].expanded = true;
        // Raw children: the start dir may itself be hidden (tb ~/.claude).
        let start_id = tree.nodes[tree.root]
            .children
            .iter()
            .flatten()
            .copied()
            .find(|&k| tree.nodes[k].path == start)
            .unwrap_or(tree.root);
        tree.nodes[tree.root].last = Some(start_id);
        tree.reveal = tree.path_to(start_id).into_iter().collect();
        let mut app = App {
            tree,
            cursor: start_id,
            mt: Mtime::spawn(),
            scene: Scene::default(),
            cam: None,
            flash: 0.0,
            bead: None,
            last_cursor: None,
            help: false,
            help_anim: 0.0,
            view: (80, 24),
            hits: Vec::new(),
            cols: Vec::new(),
            mouse: None,
            cam_hold: None,
            cam_tx: 0.0,
            preview: None,
            epoch: 0,
            lay: None,
            picker: None,
            detected: ratatui_image::picker::ProtocolType::Halfblocks,
            graphics_shown: false,
            repaint: false,
            prompt: None,
            search: None,
            last_search: None,
            history: Vec::new(),
            hist_at: 0,
            pending: None,
            live: None,
            ripples: std::collections::HashMap::new(),
            cd_on_quit: false,
            can_cd: false,
        };
        app.enter();
        app
    }

    /// Newest mtime at or under the node.
    pub fn heat_of(&self, id: usize) -> SystemTime {
        self.tree.heat(id)
    }

    /// Label text. Pure name: anything that changes a label's width later
    /// (like a scan-capped marker) would shift every column to its right.
    pub fn label(&self, id: usize) -> String {
        self.tree.nodes[id].name.clone()
    }

    /// Copy fresh mtime-cache results onto the nodes, and re-sort if the
    /// order depends on them.
    fn absorb(&mut self) {
        for n in self.tree.nodes.iter_mut().filter(|n| n.is_dir) {
            n.rec = self.mt.cache.get(&n.path).map(|c| (c.0, c.1, c.3, c.4));
        }
        if matches!(self.tree.sort.key, SortKey::Modified | SortKey::Size) {
            self.tree.resort();
        }
    }

    fn set_sort(&mut self, sort: Sort) {
        self.tree.sort = sort;
        self.tree.resort();
    }

    fn siblings(&self) -> Vec<usize> {
        match self.tree.nodes[self.cursor].parent {
            Some(p) => self.tree.kids(p).to_vec(),
            None => vec![self.cursor],
        }
    }

    fn set_cursor(&mut self, id: usize) {
        if id != self.cursor {
            self.flash = 1.0;
        }
        self.cursor = id;
        self.tree.reveal = self.tree.path_to(id).into_iter().collect();
        if let Some(p) = self.tree.nodes[id].parent {
            self.tree.nodes[p].last = Some(id);
        }
    }

    fn step(&mut self, delta: isize) {
        let sib = self.siblings();
        let i = sib.iter().position(|&s| s == self.cursor).unwrap_or(0) as isize;
        let j = (i + delta).clamp(0, sib.len() as isize - 1) as usize;
        self.set_cursor(sib[j]);
    }

    /// Expand the cursor dir and move into it; open files in the preview.
    fn enter(&mut self) {
        let id = self.cursor;
        if !self.tree.nodes[id].is_dir {
            let w = ui::popup(ratatui::layout::Rect::new(0, 0, self.view.0, self.view.1 + 1)).width;
            self.preview = Some(Preview::load(&self.tree.nodes[id].path, w));
            return;
        }
        self.tree.load(id);
        self.tree.nodes[id].expanded = true;
        let kids = self.tree.kids(id);
        let last = self.tree.nodes[id].last.filter(|l| kids.contains(l));
        if let Some(k) = last.or(kids.first().copied()) {
            self.set_cursor(k);
        }
    }

    fn leave(&mut self) {
        if self.tree.nodes[self.cursor].parent.is_none() {
            self.tree.reroot_up();
            self.tree.reveal = self.tree.path_to(self.cursor).into_iter().collect();
        }
        if let Some(p) = self.tree.nodes[self.cursor].parent {
            self.set_cursor(p);
        }
    }

    fn toggle(&mut self) {
        let id = self.cursor;
        if self.tree.nodes[id].is_dir {
            self.tree.load(id);
            self.tree.nodes[id].expanded ^= true;
        }
    }

    /// Fold every branch that isn't on the cursor path.
    fn collapse_others(&mut self) {
        let path = self.tree.path_to(self.cursor);
        for (i, n) in self.tree.nodes.iter_mut().enumerate() {
            n.expanded = path[..path.len() - 1].contains(&i);
        }
    }

    fn reload(&mut self) {
        let id = if self.tree.nodes[self.cursor].is_dir && self.tree.nodes[self.cursor].expanded {
            self.cursor
        } else {
            self.tree.nodes[self.cursor].parent.unwrap_or(self.cursor)
        };
        let path = self.tree.nodes[id].path.clone();
        self.mt.invalidate(&path);
        for a in self.tree.path_to(id) {
            let p = self.tree.nodes[a].path.clone();
            self.mt.cache.remove(&p);
        }
        let name = self.tree.nodes[self.cursor].path.clone();
        self.tree.reload(id);
        self.absorb();
        let again = self.tree.kids(id).iter().copied().find(|&k| self.tree.nodes[k].path == name);
        if id != self.cursor {
            self.set_cursor(again.or(self.tree.kids(id).first().copied()).unwrap_or(id));
        }
    }

    /// Point the watcher at whatever is open now, and apply what it saw.
    /// Returns true if the tree changed.
    fn live_poll(&mut self) -> bool {
        let Some(w) = &mut self.live else { return false };
        w.set(self.tree.open_dirs().into_iter().map(|d| self.tree.nodes[d].path.clone()).collect());
        let changes = w.poll();
        !changes.is_empty() && self.apply_changes(changes)
    }

    /// Fold live changes into the tree: re-list each changed folder in place,
    /// warm it and its ancestors, and move the cursor off anything deleted.
    fn apply_changes(&mut self, changes: Vec<watch::Change>) -> bool {
        let open: std::collections::HashMap<PathBuf, usize> =
            self.tree.open_dirs().into_iter().map(|d| (self.tree.nodes[d].path.clone(), d)).collect();
        let mut any = false;
        for c in changes {
            let Some(&id) = open.get(&c.dir) else { continue };
            any = true;
            let r = self.tree.refresh(id);
            // Light what changed, or the folder itself when what changed is gone.
            let shown = |k: &usize| self.tree.show_hidden || !self.tree.is_hidden(*k);
            let mut origins: Vec<usize> = r.touched.iter().copied().filter(shown).collect();
            if r.gone.iter().any(shown) {
                origins.push(id);
            }
            self.ripple(&origins);
            if r.changed || c.listing {
                // Entries came or went: re-walk this folder for an exact heat.
                self.mt.forget(&c.dir);
                self.mt.request(&c.dir);
            }
            // Warm the folder and everything above it. A dot-folder on the way
            // up stops the change from heating the dotfiles-hidden view above it.
            let mut vis = c.newest_vis;
            for a in self.tree.path_to(id).into_iter().rev() {
                let p = self.tree.nodes[a].path.clone();
                self.mt.bump(&p, c.newest, vis);
                if self.tree.is_hidden(a) {
                    vis = SystemTime::UNIX_EPOCH;
                }
            }
        }
        if !any {
            return false;
        }
        self.reattach();
        self.absorb();
        self.epoch += 1;
        true
    }

    /// Start ripples at `origins` that climb to the root, a level per RIPPLE_STEP.
    fn ripple(&mut self, origins: &[usize]) {
        let now = Instant::now();
        for &o in origins {
            for (k, id) in self.tree.path_to(o).into_iter().rev().enumerate() {
                let fresh = Ripple { start: now + RIPPLE_STEP * k as u32, strength: 0.8f32.powi(k as i32).max(0.35) };
                let r = self.ripples.entry(id).or_insert(fresh);
                if fresh.start < r.start {
                    // A closer change gets here first.
                    *r = Ripple { strength: r.strength.max(fresh.strength), ..fresh };
                } else if now.saturating_duration_since(r.start) > RIPPLE_STEP * 4 {
                    // Mostly faded: flash again.
                    *r = fresh;
                }
            }
        }
    }

    /// If the cursor's entry (or a folder above it) was deleted, land on the
    /// entry that took its place in the listing, else its folder.
    fn reattach(&mut self) {
        let path = self.tree.path_to(self.cursor);
        let Some(w) = path.windows(2).find(|w| !self.tree.nodes[w[0]].children.as_ref().is_some_and(|k| k.contains(&w[1])))
        else {
            return;
        };
        let (dir, gone) = (w[0], w[1]);
        let kids = self.tree.kids(dir);
        let to = kids.iter().copied().find(|&k| self.tree.cmp(k, gone).is_ge()).or(kids.last().copied());
        self.set_cursor(to.unwrap_or(dir));
        if let Some(s) = &mut self.search
            && !self.tree.attached(s.origin)
        {
            s.origin = self.cursor;
        }
    }

    /// Folder commands run in and q lands in: the cursor if it's a folder, else its parent.
    pub fn work_dir(&self) -> PathBuf {
        let n = &self.tree.nodes[self.cursor];
        match n.parent {
            Some(p) if !n.is_dir => self.tree.nodes[p].path.clone(),
            _ => n.path.clone(),
        }
    }

    /// Line editing for the `!` prompt. Enter queues the command.
    fn prompt_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        let Some(line) = &mut self.prompt else { return };
        let ctrl = mods.contains(KeyModifiers::CONTROL);
        match code {
            KeyCode::Esc => self.prompt = None,
            KeyCode::Char('c') if ctrl => self.prompt = None,
            KeyCode::Backspace if line.is_empty() => self.prompt = None,
            KeyCode::Backspace => {
                line.pop();
            }
            KeyCode::Char('u') if ctrl => line.clear(),
            KeyCode::Char('w') if ctrl => {
                let t = line.trim_end().len();
                let cut = line[..t].rfind(' ').map_or(0, |i| i + 1);
                line.truncate(cut);
            }
            KeyCode::Up | KeyCode::Down => {
                let at = if code == KeyCode::Up { self.hist_at + 1 } else { self.hist_at.saturating_sub(1) };
                self.hist_at = at.min(self.history.len());
                *line = match self.hist_at {
                    0 => String::new(),
                    k => self.history[self.history.len() - k].clone(),
                };
            }
            KeyCode::Enter => {
                let cmd = line.trim().to_string();
                self.prompt = None;
                if !cmd.is_empty() {
                    self.history.retain(|h| *h != cmd);
                    self.history.push(cmd.clone());
                    self.pending = Some(Run::Cmd(cmd));
                }
            }
            KeyCode::Char(c) if !ctrl => line.push(c),
            _ => {}
        }
    }

    /// Entries of `col` whose name contains `q` (smart case).
    fn matches(&self, q: &str, col: &[usize]) -> Vec<usize> {
        col.iter().copied().filter(|&k| hit(&self.tree.nodes[k].name, q).is_some()).collect()
    }

    /// Where typing `q` lands: the first name starting with it, else the first containing it.
    pub fn find(&self, q: &str) -> Option<usize> {
        let m = self.matches(q, &self.siblings());
        m.iter().copied().find(|&k| hit(&self.tree.nodes[k].name, q).is_some_and(|r| r.start == 0)).or(m.first().copied())
    }

    /// Cursor's place among the matches of `q` (1-based, 0 when off them) and their count.
    pub fn match_pos(&self, q: &str) -> (usize, usize) {
        let m = self.matches(q, &self.siblings());
        (m.iter().position(|&k| k == self.cursor).map_or(0, |i| i + 1), m.len())
    }

    /// n / N: next or previous match in the searched column, wrapping. Enter moved the
    /// cursor into the match, so its entry in that column is the cursor's ancestor there.
    fn find_next(&mut self, dir: isize) {
        let Some((q, col)) = self.last_search.clone() else { return };
        let path = self.tree.path_to(self.cursor);
        let under = |c| path.iter().position(|&a| a == c).and_then(|i| path.get(i + 1).copied());
        let (list, cur) = match col.and_then(|c| under(c).map(|e| (self.tree.kids(c).to_vec(), e))) {
            Some(found) => found,
            None => (self.siblings(), self.cursor),
        };
        self.cycle(&q, dir, &list, cur);
    }

    /// Next or previous match of `q` in `list` after `cur`, wrapping.
    fn cycle(&mut self, q: &str, dir: isize, list: &[usize], cur: usize) {
        let at = |id| list.iter().position(|&s| s == id).unwrap_or(0) as isize;
        let (cur, n) = (at(cur), list.len() as isize);
        let next = self.matches(q, list).into_iter().min_by_key(|&k| {
            let d = (at(k) - cur) * dir;
            if d > 0 { d } else { d + n }
        });
        if let Some(k) = next {
            self.set_cursor(k);
        }
    }

    /// Line editing for the `/` prompt; the cursor follows the query as it's typed.
    fn search_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        let Some(s) = &mut self.search else { return };
        let ctrl = mods.contains(KeyModifiers::CONTROL);
        let origin = s.origin;
        // Step through the matches without leaving the prompt.
        let step = match code {
            KeyCode::Tab | KeyCode::Down => 1,
            KeyCode::Char('n') if ctrl => 1,
            KeyCode::BackTab | KeyCode::Up if !s.query.is_empty() => -1,
            KeyCode::Char('p') if ctrl => -1,
            _ => 0,
        };
        if step != 0 {
            let q = s.query.clone();
            self.cycle(&q, step, &self.siblings(), self.cursor);
            return;
        }
        match code {
            // Open what the query landed on, as Enter would.
            KeyCode::Enter => {
                let q = std::mem::take(&mut s.query);
                self.search = None;
                // The cursor may have been cycled off find()'s pick, so open the cursor.
                if self.matches(&q, &self.siblings()).contains(&self.cursor) {
                    self.last_search = Some((q, self.tree.nodes[self.cursor].parent));
                    self.enter();
                }
                return;
            }
            KeyCode::Esc => self.search = None,
            KeyCode::Char('c') if ctrl => self.search = None,
            KeyCode::Backspace if s.query.is_empty() => self.search = None,
            KeyCode::Backspace => {
                s.query.pop();
            }
            // Up on an empty line recalls the last find.
            KeyCode::Up => s.query = self.last_search.as_ref().map(|l| l.0.clone()).unwrap_or_default(),
            KeyCode::Char('u') if ctrl => s.query.clear(),
            KeyCode::Char('w') if ctrl => {
                let t = s.query.trim_end_matches(|c: char| !c.is_alphanumeric());
                let keep = t.trim_end_matches(char::is_alphanumeric).len();
                s.query.truncate(keep);
            }
            KeyCode::Char(c) if !ctrl => s.query.push(c),
            _ => return,
        }
        let to = self.search.as_ref().filter(|s| !s.query.is_empty()).and_then(|s| self.find(&s.query));
        self.set_cursor(to.unwrap_or(origin));
    }

    /// Open preview that is not already closing.
    fn open_preview(&mut self) -> Option<&mut Preview> {
        self.preview.as_mut().filter(|p| !p.closing)
    }

    /// Returns false to quit.
    fn key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        self.cam_hold = None;
        if self.prompt.is_some() {
            self.prompt_key(code, mods);
            return true;
        }
        if self.search.is_some() {
            self.search_key(code, mods);
            return true;
        }
        if code == KeyCode::Char('i')
            && let (Some(p), Some(m)) = (&mut self.picker, self.preview.as_mut().and_then(|pv| pv.media.as_mut()))
        {
            media::toggle(p, self.detected);
            m.reencode();
            self.repaint = true;
            return true;
        }
        if let Some(pv) = self.open_preview() {
            let page = pv.page.max(1) as i32;
            let ctrl = mods.contains(KeyModifiers::CONTROL);
            if let Some(a) = &mut pv.audio {
                let shift = mods.contains(KeyModifiers::SHIFT);
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => pv.closing = true,
                    KeyCode::Char('c') if ctrl => pv.closing = true,
                    KeyCode::Char(' ' | 'p') | KeyCode::Enter => a.toggle(),
                    KeyCode::Left | KeyCode::Right if shift => a.seek_by(if code == KeyCode::Left { -30.0 } else { 30.0 }),
                    KeyCode::Left | KeyCode::Char('h') => a.seek_by(-5.0),
                    KeyCode::Right | KeyCode::Char('l') => a.seek_by(5.0),
                    KeyCode::Char('H') | KeyCode::PageUp => a.seek_by(-30.0),
                    KeyCode::Char('L') | KeyCode::PageDown => a.seek_by(30.0),
                    KeyCode::Char('g') | KeyCode::Home => a.seek(Duration::ZERO),
                    KeyCode::Char(d @ '0'..='9') => a.seek_frac(d.to_digit(10).unwrap() as f32 / 10.0),
                    KeyCode::Up | KeyCode::Char('+' | '=') => a.volume_by(0.1),
                    KeyCode::Down | KeyCode::Char('-' | '_') => a.volume_by(-0.1),
                    KeyCode::Char('m') => a.toggle_mute(),
                    _ => {}
                }
                return true;
            }
            if let Some(m) = &mut pv.media {
                // Pages instead of lines.
                match code {
                    KeyCode::Char('q') | KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => pv.closing = true,
                    KeyCode::Char('j' | 'l' | 'n' | ' ') | KeyCode::Down | KeyCode::Right | KeyCode::PageDown => {
                        m.flip(1)
                    }
                    KeyCode::Char('k' | 'p') | KeyCode::Up | KeyCode::PageUp => m.flip(-1),
                    KeyCode::Char('g') | KeyCode::Home => m.goto(0),
                    KeyCode::Char('G') | KeyCode::End => m.goto(isize::MAX / 2),
                    _ => {}
                }
                return true;
            }
            match code {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => pv.closing = true,
                KeyCode::Char('j') | KeyCode::Down => pv.scroll += 1,
                KeyCode::Char('k') | KeyCode::Up => pv.scroll -= 1,
                KeyCode::Char('d') if ctrl => pv.scroll += page / 2,
                KeyCode::Char('u') if ctrl => pv.scroll -= page / 2,
                KeyCode::PageDown | KeyCode::Char(' ') => pv.scroll += page,
                KeyCode::PageUp => pv.scroll -= page,
                KeyCode::Char('g') | KeyCode::Home => pv.scroll = 0,
                KeyCode::Char('G') | KeyCode::End => pv.scroll = i32::MAX / 2,
                _ => {}
            }
            return true;
        }
        if self.help && code != KeyCode::Char('?') {
            // Any key dismisses help (q included, so it never quits by surprise).
            self.help = false;
            return true;
        }
        match code {
            KeyCode::Char('q') => {
                self.cd_on_quit = true;
                return false;
            }
            KeyCode::Esc => return false,
            KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => return false,
            KeyCode::Char('j') | KeyCode::Down => self.step(1),
            KeyCode::Char('k') | KeyCode::Up => self.step(-1),
            KeyCode::Char('J') | KeyCode::PageDown => self.step(10),
            KeyCode::Char('K') | KeyCode::PageUp => self.step(-10),
            KeyCode::Char('g') | KeyCode::Home => self.step(isize::MIN / 2),
            KeyCode::Char('G') | KeyCode::End => self.step(isize::MAX / 2),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => self.enter(),
            KeyCode::Char('h') | KeyCode::Left => self.leave(),
            KeyCode::Char(' ') | KeyCode::Tab => self.toggle(),
            KeyCode::Char('-') | KeyCode::Backspace => {
                self.tree.reroot_up();
                self.tree.reveal = self.tree.path_to(self.cursor).into_iter().collect();
                self.flash = 1.0;
            }
            KeyCode::Char('.') => {
                self.tree.show_hidden ^= true;
                // Heat differs with dotfiles in or out.
                if self.tree.sort.key == SortKey::Modified {
                    self.tree.resort();
                }
            }
            KeyCode::Char('o') => self.set_sort(self.tree.sort.next()),
            KeyCode::Char('O') => self.set_sort(Sort { rev: !self.tree.sort.rev, ..self.tree.sort }),
            KeyCode::Char('c') => self.collapse_others(),
            KeyCode::Char('r') => self.reload(),
            KeyCode::Char('?') => self.help ^= true,
            KeyCode::Char('!') => {
                self.prompt = Some(String::new());
                self.hist_at = 0;
            }
            KeyCode::Char('s') => self.pending = Some(Run::Shell),
            KeyCode::Char('/') => self.search = Some(Search { query: String::new(), origin: self.cursor }),
            KeyCode::Char('n') => self.find_next(1),
            KeyCode::Char('N') => self.find_next(-1),
            _ => {}
        }
        true
    }

    fn mouse(&mut self, kind: MouseEventKind, col: u16, row: u16) {
        // Clicks would silently change the folder the typed command runs in.
        if self.prompt.is_some() || self.search.is_some() {
            return;
        }
        if let Some(pv) = self.open_preview() {
            if let Some(a) = &mut pv.audio {
                let b = a.bar;
                let on_bar = b.width > 0 && row >= b.y && row < b.bottom() && col + 1 >= b.x && col <= b.right();
                let at = || (col.saturating_sub(b.x) as f32 / b.width.saturating_sub(1).max(1) as f32).clamp(0.0, 1.0);
                match kind {
                    MouseEventKind::ScrollDown => a.seek_by(5.0),
                    MouseEventKind::ScrollUp => a.seek_by(-5.0),
                    MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) if on_bar => {
                        a.seek_frac(at())
                    }
                    MouseEventKind::Down(MouseButton::Left) => pv.closing = true,
                    _ => {}
                }
                return;
            }
            match kind {
                MouseEventKind::ScrollDown if pv.media.is_some() => pv.media.as_mut().unwrap().flip(1),
                MouseEventKind::ScrollUp if pv.media.is_some() => pv.media.as_mut().unwrap().flip(-1),
                MouseEventKind::ScrollDown => pv.scroll += 3,
                MouseEventKind::ScrollUp => pv.scroll -= 3,
                MouseEventKind::Down(MouseButton::Left) => pv.closing = true,
                _ => {}
            }
            return;
        }
        match kind {
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                self.mouse = Some((col, row));
                match self.column_at(col, row) {
                    // Another column on the line: this tick only takes it over
                    // (its node is already on the line); later ticks scroll it.
                    Some(id) if id != self.cursor => {
                        self.cam_hold.get_or_insert(self.cam_tx);
                        self.set_cursor(id);
                    }
                    _ => self.step(if kind == MouseEventKind::ScrollDown { 1 } else { -1 }),
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.cam_hold = None;
                let (c, r) = (col as i32, row as i32);
                // Label cells plus the marker cell to their left.
                let hit = self.hits.iter().find(|h| r == h.1 && c >= h.0 - 1 && c < h.0 + h.2).map(|h| h.3);
                match hit {
                    Some(id) if id == self.cursor => self.enter(),
                    Some(id) => self.set_cursor(id),
                    None => {}
                }
            }
            _ => {}
        }
    }

    /// Line node whose column band holds the cell, from last frame's bands.
    pub fn column_at(&self, col: u16, row: u16) -> Option<usize> {
        let c = col as i32;
        (row < self.view.1).then(|| self.cols.iter().find(|b| c >= b.0 && c < b.1).map(|b| b.2)).flatten()
    }

    /// Column the wheel would take over: pointer over a line column other than the cursor's.
    pub fn hover(&self) -> Option<usize> {
        if self.prompt.is_some() || self.search.is_some() || self.help || self.preview.is_some() {
            return None;
        }
        let (c, r) = self.mouse?;
        self.column_at(c, r).filter(|&id| id != self.cursor)
    }

    /// Something on screen moves on its own (a playing sound, a waveform still being read).
    fn ticking(&self) -> bool {
        self.preview.as_ref().and_then(|p| p.audio.as_ref()).is_some_and(|a| {
            a.state() == audio::State::Playing || !a.wave_done
        })
    }

    /// Track the pointer; true when the hover cue changed and needs a frame.
    fn pointer(&mut self, col: u16, row: u16) -> bool {
        let before = self.hover();
        self.mouse = Some((col, row));
        self.hover() != before
    }
}

/// Byte range of `q` in `name`. Smart case: an uppercase letter in `q` makes it exact.
pub fn hit(name: &str, q: &str) -> Option<std::ops::Range<usize>> {
    if q.is_empty() {
        return None;
    }
    let exact = q.chars().any(char::is_uppercase);
    let fold = |c: char| if exact { c } else { c.to_lowercase().next().unwrap_or(c) };
    let want: Vec<char> = q.chars().map(fold).collect();
    name.char_indices().find_map(|(i, _)| {
        let mut it = name[i..].char_indices();
        for &w in &want {
            match it.next() {
                Some((_, c)) if fold(c) == w => {}
                _ => return None,
            }
        }
        let end = it.next().map_or(name.len(), |(j, _)| i + j);
        Some(i..end)
    })
}

fn restore() {
    audio::unhush();
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
}

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut arg, mut cwd_file) = (None, None);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--cwd-file" => cwd_file = args.next(),
            _ if arg.is_none() => arg = Some(a),
            _ => {}
        }
    }
    let arg = arg.unwrap_or_else(|| ".".into());
    if arg == "-h" || arg == "--help" {
        println!(
            "usage: tb [--cwd-file PATH] [DIR]\n\nhjkl/arrows move · enter/l open · space fold · . dotfiles · - reroot up · c collapse others · r reload · ? help\n\
             ! run a command in the selected folder ($f = selection) · s shell there (exit returns) · q quit · esc quit\n\
             --cwd-file: on q, write the selected folder there (tb.bash turns that into cd)\n\
             image/pdf preview: j/k page · i pixels <-> half-blocks · TB_GRAPHICS=halfblocks|kitty|sixel|iterm2|off\n\
             audio preview: plays at once · space pause · left/right 5 s · shift 30 s · 0-9 jump · up/down volume · m mute\n\
             o cycles the sort (name · modified · size · type) · O reverses it · TB_SORT=size (or -size, modified, type) sets the start\n\
             open folders update live (about once a second) · TB_LIVE=off turns that off\n\
             color = last modified (dirs: newest anything inside): red = minutes, orange = hours, tan = days, grey = weeks, blue = years"
        );
        return Ok(());
    }
    let start = std::fs::canonicalize(&arg)?;
    let mut app = App::new(start);
    if let Ok(s) = std::env::var("TB_SORT") {
        match Sort::parse(&s) {
            Some(sort) => app.set_sort(sort),
            None => {
                eprintln!("tb: TB_SORT={s:?}: expected name, modified, size or type, with - in front to reverse");
                std::process::exit(2);
            }
        }
    }
    app.can_cd = cwd_file.is_some();
    if std::env::var("TB_LIVE").map_or(true, |v| v != "off") {
        app.live = Some(watch::Watch::spawn(watch::EVERY));
    }
    let mut term = ratatui::init();
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        audio::unhush();
        let _ = execute!(stdout(), DisableMouseCapture);
        hook(info);
    }));
    execute!(stdout(), EnableMouseCapture)?;
    // Query after entering the alternate screen, before reading events.
    if let Some((p, detected)) = media::picker() {
        app.picker = Some(p);
        app.detected = detected;
    }

    let mut dirty = true;
    let mut animating = false;
    let mut last = Instant::now();
    let res = loop {
        // Idle + input: draw at once (latency). Mid-animation: hold to the frame
        // budget so bursts of input coalesce instead of pushing past 60 fps.
        if (dirty || animating) && (!animating || last.elapsed() >= FRAME) {
            let now = Instant::now();
            // Resuming from idle: one nominal frame, not the idle gap.
            let dt = if animating { (now - last).as_secs_f32().min(0.05) } else { FRAME.as_secs_f32() };
            last = now;
            if std::mem::take(&mut app.repaint) {
                let _ = term.clear();
            }
            let mut moving = false;
            if let Err(e) = term.draw(|f| moving = ui::frame(f, &mut app, dt)) {
                break Err(e);
            }
            animating = moving;
            dirty = false;
        }
        // Sleep until the next frame is due, or indefinitely-ish when idle
        // (wake periodically to pick up background mtime results).
        let timeout = match (animating, app.ticking()) {
            (true, _) => FRAME.saturating_sub(last.elapsed()),
            (false, true) => Duration::from_millis(50),
            (false, false) => Duration::from_millis(100),
        };
        match event::poll(timeout) {
            Ok(true) => {}
            Ok(false) => {
                if app.mt.poll() {
                    app.absorb();
                    app.epoch += 1;
                    dirty = true;
                }
                dirty |= app.live_poll();
                dirty |= app.ticking();
                continue;
            }
            Err(e) => break Err(e),
        }
        // Drain everything queued so key repeat never lags behind frames.
        let mut quit = false;
        loop {
            match event::read() {
                Ok(Event::Key(k)) if k.kind != KeyEventKind::Release => {
                    if !app.key(k.code, k.modifiers) {
                        quit = true;
                        break;
                    }
                    // Keys typed after Enter belong to the command, not the tree.
                    if app.pending.is_some() {
                        break;
                    }
                }
                // Plain motion only moves the hover cue: no frame unless it changed.
                Ok(Event::Mouse(m)) if m.kind == MouseEventKind::Moved => {
                    if app.pointer(m.column, m.row) {
                        dirty = true;
                    }
                    if !matches!(event::poll(Duration::ZERO), Ok(true)) {
                        break;
                    }
                    continue;
                }
                Ok(Event::Mouse(m)) => app.mouse(m.kind, m.column, m.row),
                Ok(_) => {}
                Err(_) => break,
            }
            dirty = true;
            if !matches!(event::poll(Duration::ZERO), Ok(true)) {
                break;
            }
        }
        if quit {
            break Ok(());
        }
        if let Some(run) = app.pending.take() {
            let sel = app.tree.nodes[app.cursor].path.clone();
            if let Err(e) = shell::run(&mut term, &app.work_dir(), &sel, &run) {
                break Err(e);
            }
            // The command may have created or removed files.
            app.reload();
            dirty = true;
        }
        app.epoch += 1;
        if app.mt.poll() {
            app.absorb();
        }
    };
    restore();
    if let (true, Some(f)) = (app.cd_on_quit, cwd_file) {
        std::fs::write(f, app.work_dir().as_os_str().as_bytes())?;
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn app_in(name: &str, files: &[&str]) -> App {
        let root = std::env::temp_dir().join(format!("tb-search-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("d")).unwrap();
        for f in files {
            fs::write(root.join("d").join(f), "").unwrap();
        }
        App::new(root.join("d"))
    }

    fn at(app: &App) -> &str {
        &app.tree.nodes[app.cursor].name
    }

    fn typed(app: &mut App, keys: &str) {
        for c in keys.chars() {
            app.key(KeyCode::Char(c), KeyModifiers::NONE);
        }
    }

    #[test]
    fn search_follows_typing_prefers_prefix_and_cycles() {
        let mut app = app_in("cycle", &["alpha", "beta", "Bravo.txt", "zebra-b"]);
        assert_eq!(at(&app), "alpha");
        typed(&mut app, "/b");
        assert_eq!(at(&app), "beta");
        typed(&mut app, "r");
        assert_eq!(at(&app), "Bravo.txt");
        typed(&mut app, "zz");
        assert_eq!(at(&app), "alpha", "no match falls back to where / started");
        app.key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.search.is_none());
        assert_eq!(at(&app), "alpha");

        typed(&mut app, "/b");
        app.key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(at(&app), "beta");
        assert!(app.preview.take().is_some(), "enter opens the match");
        typed(&mut app, "n");
        assert_eq!(at(&app), "Bravo.txt");
        typed(&mut app, "n");
        assert_eq!(at(&app), "zebra-b");
        typed(&mut app, "n");
        assert_eq!(at(&app), "beta", "wraps");
        typed(&mut app, "N");
        assert_eq!(at(&app), "zebra-b");
    }

    #[test]
    fn hit_is_smart_case_and_char_aligned() {
        assert_eq!(hit("Bravo.txt", "rav"), Some(1..4));
        assert_eq!(hit("Bravo.txt", "b"), Some(0..1));
        assert_eq!(hit("Bravo.txt", "B"), Some(0..1));
        assert_eq!(hit("bravo", "B"), None, "uppercase in the query means exact case");
        assert_eq!(hit("naïve.md", "ÏV"), None);
        assert_eq!(hit("naïve.md", "ïv"), Some(2..5));
        assert_eq!(hit("NAÏVE", "ïv"), Some(2..5));
        assert_eq!(hit("x", ""), None);
    }

    #[test]
    fn search_cycles_while_typing_and_recalls() {
        let mut app = app_in("tab", &["alpha", "beta", "Bravo.txt", "zebra-b"]);
        typed(&mut app, "/b");
        assert_eq!(app.match_pos("b"), (1, 3));
        app.key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(at(&app), "Bravo.txt");
        assert_eq!(app.match_pos("b"), (2, 3));
        app.key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(at(&app), "zebra-b");
        app.key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(at(&app), "Bravo.txt");
        app.key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(at(&app), "Bravo.txt", "enter opens the cycled match, not find()'s first pick");
        assert!(app.preview.take().is_some());

        typed(&mut app, "/B");
        assert_eq!(at(&app), "Bravo.txt", "smart case skips beta");
        app.key(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(app.search.as_ref().unwrap().query, "");
        app.key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.search.as_ref().unwrap().query, "b", "up on an empty line recalls the last find");
        app.key(KeyCode::Esc, KeyModifiers::NONE);
    }

    #[test]
    fn search_enter_opens_a_folder() {
        let mut app = app_in("open", &["alpha"]);
        let d = app.tree.nodes[app.cursor].path.parent().unwrap().join("sub");
        fs::create_dir_all(d.join("inner")).unwrap();
        app.reload();
        typed(&mut app, "/su");
        app.key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(at(&app), "inner");
    }

    #[test]
    fn n_cycles_the_searched_column_not_the_opened_folder() {
        let mut app = app_in("scope", &["bar", "zed"]);
        let d = app.tree.nodes[app.cursor].path.parent().unwrap().to_path_buf();
        fs::create_dir_all(d.join("ba-dir")).unwrap();
        fs::write(d.join("ba-dir/ba-one"), "").unwrap();
        fs::write(d.join("ba-dir/ba-two"), "").unwrap();
        app.reload();
        typed(&mut app, "/ba");
        app.key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(at(&app), "ba-one");
        typed(&mut app, "n");
        assert_eq!(at(&app), "bar", "back in the searched column, past ba-dir");
        typed(&mut app, "N");
        assert_eq!(at(&app), "ba-dir");
    }

    fn change(app: &App, dir: usize, listing: bool) -> watch::Change {
        let now = SystemTime::now();
        watch::Change { dir: app.tree.nodes[dir].path.clone(), listing, newest: now, newest_vis: now }
    }

    #[test]
    fn live_change_adds_entries_and_keeps_nodes() {
        let mut app = app_in("live-add", &["alpha", "beta"]);
        let d = app.tree.nodes[app.cursor].parent.unwrap();
        let (alpha, beta) = (app.cursor, app.tree.kids(d)[1]);
        fs::write(app.tree.nodes[d].path.join("apple"), "").unwrap();
        assert!(app.apply_changes(vec![change(&app, d, true)]));
        let names: Vec<_> = app.tree.kids(d).iter().map(|&k| app.tree.nodes[k].name.clone()).collect();
        assert_eq!(names, ["alpha", "apple", "beta"]);
        assert_eq!(app.cursor, alpha, "cursor stays put");
        assert!(app.tree.kids(d).contains(&beta), "surviving entries keep their nodes");
    }

    #[test]
    fn live_change_moves_the_cursor_off_a_deleted_entry() {
        let mut app = app_in("live-rm", &["alpha", "beta", "gamma"]);
        let d = app.tree.nodes[app.cursor].parent.unwrap();
        app.step(1);
        assert_eq!(at(&app), "beta");
        fs::remove_file(app.tree.nodes[app.cursor].path.clone()).unwrap();
        app.apply_changes(vec![change(&app, d, true)]);
        assert_eq!(at(&app), "gamma", "lands on what took its place");
        fs::remove_file(app.tree.nodes[app.cursor].path.clone()).unwrap();
        app.apply_changes(vec![change(&app, d, true)]);
        assert_eq!(at(&app), "alpha", "or the last entry when it was at the end");
    }

    fn names(app: &App, dir: usize) -> Vec<String> {
        app.tree.kids(dir).iter().map(|&k| app.tree.nodes[k].name.clone()).collect()
    }

    /// d/ holds big.log (300 B, a year old), mid.txt (20 B, now), a/ (1000 B inside, a week old)
    /// and Zed (no extension, 5 B, a day old). The cursor starts on a/.
    fn sort_fixture(name: &str) -> (App, usize) {
        let root = std::env::temp_dir().join(format!("tb-sort-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("d/a")).unwrap();
        let ago = |days: u64| SystemTime::now() - Duration::from_secs(86400 * days);
        for (f, n, t) in [("big.log", 300, ago(365)), ("mid.txt", 20, SystemTime::now()), ("Zed", 5, ago(1))] {
            fs::write(root.join("d").join(f), vec![0; n]).unwrap();
            fs::File::open(root.join("d").join(f)).unwrap().set_modified(t).unwrap();
        }
        fs::write(root.join("d/a/inner"), [0; 1000]).unwrap();
        fs::File::open(root.join("d/a/inner")).unwrap().set_modified(ago(7)).unwrap();
        let mut app = App::new(root.join("d"));
        let d = app.tree.nodes[app.cursor].parent.unwrap();
        app.mt.request(&root.join("d/a"));
        let t0 = std::time::Instant::now();
        while app.tree.nodes[app.cursor].rec.is_none() && t0.elapsed() < Duration::from_secs(5) {
            app.mt.poll();
            app.absorb();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(at(&app), "a");
        (app, d)
    }

    #[test]
    fn o_cycles_sorts_and_the_cursor_rides_along() {
        let (mut app, d) = sort_fixture("cycle");
        assert_eq!(names(&app, d), ["a", "big.log", "mid.txt", "Zed"]);
        typed(&mut app, "o");
        assert_eq!(names(&app, d), ["mid.txt", "Zed", "a", "big.log"], "newest first, folders by what's inside");
        typed(&mut app, "o");
        assert_eq!(names(&app, d), ["a", "big.log", "mid.txt", "Zed"], "largest first, folders by total size");
        typed(&mut app, "O");
        assert_eq!(names(&app, d), ["Zed", "mid.txt", "big.log", "a"], "O flips it");
        typed(&mut app, "o");
        assert_eq!(names(&app, d), ["a", "Zed", "big.log", "mid.txt"], "folders, then no extension, then by extension");
        assert!(!app.tree.sort.rev, "a new key starts in its natural direction");
        typed(&mut app, "o");
        assert_eq!(app.tree.sort, Sort::default());
        assert_eq!(at(&app), "a", "the cursor stays on its entry through every re-sort");
        typed(&mut app, "j");
        assert_eq!(at(&app), "big.log", "and moves in the new order");
    }

    #[test]
    fn live_changes_keep_the_sort() {
        let (mut app, d) = sort_fixture("live");
        app.set_sort(Sort::parse("modified").unwrap());
        fs::write(app.tree.nodes[d].path.join("new.md"), "").unwrap();
        app.apply_changes(vec![change(&app, d, true)]);
        assert_eq!(names(&app, d)[0], "new.md", "a fresh file lands at the top");
        app.step(isize::MAX / 2);
        assert_eq!(at(&app), "big.log");
        fs::remove_file(app.tree.nodes[app.cursor].path.clone()).unwrap();
        app.apply_changes(vec![change(&app, d, true)]);
        assert_eq!(at(&app), "a", "a deleted last entry hands the cursor to the one before it");
    }

    #[test]
    fn sort_parses_names_and_a_reversing_dash() {
        assert_eq!(Sort::parse("size"), Some(Sort { key: SortKey::Size, rev: false }));
        assert_eq!(Sort::parse("-modified"), Some(Sort { key: SortKey::Modified, rev: true }));
        assert_eq!(Sort::parse("type"), Some(Sort { key: SortKey::Type, rev: false }));
        assert_eq!(Sort::parse("bogus"), None);
    }

    #[test]
    fn live_change_ripples_up_from_what_changed() {
        let mut app = app_in("ripple", &["alpha", "beta", ".hidden"]);
        let d = app.tree.nodes[app.cursor].parent.unwrap();
        let (root, alpha, beta) = (app.tree.root, app.cursor, app.tree.kids(d)[1]);
        let bump = |app: &App, name: &str| {
            let p = app.tree.nodes[d].path.join(name);
            fs::File::open(p).unwrap().set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
        };
        bump(&app, ".hidden");
        app.apply_changes(vec![change(&app, d, false)]);
        assert!(app.ripples.is_empty(), "a hidden file changing lights nothing while dotfiles are hidden");

        bump(&app, "beta");
        app.apply_changes(vec![change(&app, d, false)]);
        assert!(!app.ripples.contains_key(&alpha), "untouched siblings stay dark");
        let (b, dr, rr) = (app.ripples[&beta], app.ripples[&d], app.ripples[&root]);
        assert!(b.start < dr.start && dr.start < rr.start, "climbs a level at a time");
        assert!(b.strength > dr.strength && dr.strength > rr.strength, "dimmer as it climbs");

        fs::remove_file(app.tree.nodes[alpha].path.clone()).unwrap();
        app.ripples.clear();
        app.apply_changes(vec![change(&app, d, true)]);
        assert_eq!(app.ripples[&d].strength, 1.0, "a deletion lights the folder it left");
    }

    #[test]
    fn live_change_warms_the_folder_and_its_ancestors() {
        let mut app = app_in("live-heat", &["alpha"]);
        let d = app.tree.nodes[app.cursor].parent.unwrap();
        let old = SystemTime::now() - Duration::from_secs(86400 * 400);
        let root = app.tree.root;
        for id in [d, root] {
            let p = app.tree.nodes[id].path.clone();
            app.mt.cache.insert(p, (old, true, true, old, 0));
        }
        app.absorb();
        assert!(app.heat_of(root) <= old);
        app.apply_changes(vec![change(&app, d, false)]);
        let recent = SystemTime::now() - Duration::from_secs(60);
        assert!(app.heat_of(d) > recent && app.heat_of(root) > recent);
    }

}
