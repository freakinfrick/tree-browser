//! tb — horizontal tree file browser. Color = recency (recursive for dirs).
#[cfg(not(unix))]
compile_error!("tb needs a Unix-like OS (Linux, macOS, BSD): it drives the tty with termios and signals");

#[cfg(all(feature = "audio", unix, not(target_os = "macos")))]
mod alsa;
mod anim;
mod audio;
mod explode;
mod git;
mod layout;
mod media;
mod mtime;
mod places;
mod settings;
mod shell;
mod tree;
mod ui;
mod watch;

use std::io::stdout;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton,
    MouseEventKind,
};
use ratatui::crossterm::execute;

use anim::{Damped, Glide, Scene};
use mtime::Mtime;
use settings::{Settings, StepThrough, ITEMS};
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
    /// Signal bead running along the line toward the cursor (world x).
    pub bead: Option<Damped>,
    /// Cursor (id, world x) last frame, to launch the bead from.
    pub last_cursor: Option<(usize, i32)>,
    pub help: bool,
    pub help_anim: f32,
    pub view: (u16, u16),
    /// Last frame's clickable labels: (screen x, y, width, node).
    pub hits: Vec<(i32, i32, i32, usize)>,
    /// Last frame's columns: screen x range [lo, hi) and the line node (None
    /// past the line's end).
    pub cols: Vec<(i32, i32, Option<usize>)>,
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
    /// The picker as the terminal query left it, and the protocol `auto`
    /// picks; None = TB_GRAPHICS=off.
    graphics: Option<(ratatui_image::picker::Picker, ratatui_image::picker::ProtocolType)>,
    /// TB_GRAPHICS is set: the image previews setting waits for a run without it.
    graphics_forced: bool,
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
    /// Git status of the repos the open folders are in; None = off (TB_GIT=off, and in tests).
    pub git: Option<git::Git>,
    /// Live updates and git may run (off in tests, which drive changes by hand).
    services: bool,
    /// What `,` edits and the config file stores.
    pub settings: Settings,
    /// Settings menu: the selected row; None = closed.
    pub menu: Option<usize>,
    pub menu_anim: f32,
    /// Where settings are saved; None = no home to save in.
    pub config: Option<PathBuf>,
    /// Config file lines that couldn't be used, shown in the menu.
    pub config_errs: Vec<String>,
    /// Why the last save failed, shown in the menu.
    pub save_err: Option<String>,
    /// `e` reading everything under a folder; None = idle.
    pub exploding: Option<Exploding>,
    /// A short message for the status bar, and when it was posted.
    pub note: Option<(String, Instant)>,
    /// Wheel momentum, carried on each frame into the preview or the column.
    glide: Glide,
}

pub struct Exploding {
    /// The folder being exploded.
    pub target: usize,
    rx: std::sync::mpsc::Receiver<explode::Msg>,
    /// Folders read so far.
    pub folders: usize,
    pub born: Instant,
}

/// How long a status bar note stays up.
pub const NOTE: Duration = Duration::from_secs(3);

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
            graphics: None,
            graphics_forced: false,
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
            git: None,
            services: false,
            settings: Settings::default(),
            menu: None,
            menu_anim: 0.0,
            config: None,
            config_errs: Vec::new(),
            save_err: None,
            exploding: None,
            note: None,
            glide: Glide::default(),
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
        self.settings.sort = sort;
        self.apply_settings();
    }

    /// Room in the layout, from the settings.
    pub fn spacing(&self) -> layout::Spacing {
        let s = &self.settings;
        layout::Spacing {
            rows: s.row_spacing as i32,
            gap: s.column_gap as i32,
            maxw: s.max_name as usize,
            columns: s.columns,
            details: s.details,
            branch: s.branch_offset as i32,
        }
    }

    /// Push the settings into the tree, the colors and the background workers.
    fn apply_settings(&mut self) {
        let s = self.settings;
        anim::set_palette(s.palette);
        anim::set_heat_range(s.heat_range.secs());
        layout::set_line_style(s.lines);
        ui::set_accent(s.accent);
        let heat_changed = self.tree.show_hidden != s.show_hidden;
        self.tree.show_hidden = s.show_hidden;
        let order = (s.sort, s.folders_first, s.natural_sort);
        if (self.tree.sort, self.tree.folders_first, self.tree.natural) != order
            || (heat_changed && s.sort.key == SortKey::Modified)
        {
            (self.tree.sort, self.tree.folders_first, self.tree.natural) = order;
            self.tree.resort();
        }
        if !s.live {
            self.live = None;
            self.ripples.clear();
        } else if self.services && self.live.is_none() {
            self.live = Some(watch::Watch::spawn(watch::EVERY));
        }
        if !s.git {
            self.git = None;
        } else if self.services && self.git.is_none() {
            self.git = Some(git::Git::spawn(git::EVERY));
        }
        if !s.ripples {
            self.ripples.clear();
        }
        self.epoch += 1;
    }

    /// Pick the preview graphics the setting asks for, from what the
    /// terminal answered at startup. TB_GRAPHICS overrides it for the run.
    fn apply_graphics(&mut self) {
        use ratatui_image::picker::ProtocolType;
        use settings::Graphics;
        let Some((base, auto)) = &self.graphics else { return };
        let proto = match self.settings.graphics {
            _ if self.graphics_forced => *auto,
            Graphics::Off => {
                self.picker = None;
                return;
            }
            Graphics::Auto => *auto,
            Graphics::Pixels => self.detected,
            Graphics::Blocks => ProtocolType::Halfblocks,
        };
        let mut p = base.clone();
        p.set_protocol_type(proto);
        self.picker = Some(p);
    }

    /// One setting changed in the menu: apply it and save it.
    fn setting_changed(&mut self, key: &str) {
        self.apply_settings();
        if key == "graphics" {
            self.apply_graphics();
        }
        let Some(p) = &self.config else { return };
        // A new sort key starts unreversed, so the file's reverse must follow.
        let keys: &[&str] = if key == "sort" { &["sort", "sort_reverse"] } else { &[key] };
        self.save_err = keys.iter().find_map(|k| self.settings.save(p, k).err()).map(|e| format!("can't save: {e}"));
    }

    fn menu_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        let Some(i) = self.menu else { return };
        let n = ITEMS.len();
        let key = ITEMS[i].key;
        match code {
            KeyCode::Esc | KeyCode::Char(',' | 'q') => self.menu = None,
            KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => self.menu = None,
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => self.menu = Some((i + 1) % n),
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => self.menu = Some((i + n - 1) % n),
            KeyCode::Char('g') | KeyCode::Home => self.menu = Some(0),
            KeyCode::Char('G') | KeyCode::End => self.menu = Some(n - 1),
            KeyCode::Char('l' | ' ') | KeyCode::Right | KeyCode::Enter => {
                self.settings.adjust(key, 1);
                self.setting_changed(key);
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.settings.adjust(key, -1);
                self.setting_changed(key);
            }
            KeyCode::Char('r') => {
                self.settings.reset(key);
                self.setting_changed(key);
            }
            _ => {}
        }
    }

    fn siblings(&self) -> Vec<usize> {
        match self.tree.nodes[self.cursor].parent {
            Some(p) => self.tree.kids(p).to_vec(),
            None => vec![self.cursor],
        }
    }

    fn set_cursor(&mut self, id: usize) {
        self.cursor = id;
        self.tree.reveal = self.tree.path_to(id).into_iter().collect();
        if let Some(p) = self.tree.nodes[id].parent {
            self.tree.nodes[p].last = Some(id);
        }
        // The glide moves the cursor mid-frame, outside the loop's bump.
        self.epoch += 1;
    }

    /// Everything in the cursor's column, top to bottom: the lists of every
    /// open folder one column left, in the order the layout stacks them.
    fn column(&self) -> Vec<usize> {
        let depth = self.tree.path_to(self.cursor).len();
        let mut out = Vec::new();
        let mut stack = vec![(self.tree.root, 1)];
        while let Some((id, d)) = stack.pop() {
            if d == depth {
                out.push(id);
            } else if self.tree.nodes[id].expanded {
                stack.extend(self.tree.kids(id).into_iter().rev().map(|k| (k, d + 1)));
            }
        }
        out
    }

    /// Every visible node in reading order: a folder, then what's open
    /// inside it, then its next sibling.
    fn outline(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = vec![self.tree.root];
        while let Some(id) = stack.pop() {
            out.push(id);
            if self.tree.nodes[id].expanded {
                stack.extend(self.tree.kids(id).into_iter().rev());
            }
        }
        out
    }

    /// j / k / J / K, through whatever Step through says.
    fn step(&mut self, delta: isize) {
        let list = match self.settings.step {
            StepThrough::Folder => self.siblings(),
            StepThrough::Column => self.column(),
            StepThrough::Tree => self.outline(),
        };
        self.step_in(&list, delta);
    }

    /// Arrows and the wheel move straight up and down the column, whatever
    /// Step through says. The wheel must: a step into a child column would
    /// leave the pointer over the parent's, and the next tick would take the
    /// parent back.
    fn column_step(&mut self, delta: isize) {
        let list = if self.settings.step == StepThrough::Folder { self.siblings() } else { self.column() };
        self.step_in(&list, delta);
    }

    fn step_in(&mut self, list: &[usize], delta: isize) {
        let i = list.iter().position(|&s| s == self.cursor).unwrap_or(0) as isize;
        let j = (i + delta).clamp(0, list.len() as isize - 1) as usize;
        self.set_cursor(list[j]);
    }

    /// g / G: first or last in the folder, never past it.
    fn step_end(&mut self, last: bool) {
        let sib = self.siblings();
        let end = if last { sib.last() } else { sib.first() };
        if let Some(&id) = end {
            self.set_cursor(id);
        }
    }

    /// Expand the cursor dir and move into it; open files in the preview.
    fn enter(&mut self) {
        let id = self.cursor;
        if !self.tree.nodes[id].is_dir {
            self.open_file(id);
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

    /// The node at `p`, reading folders on the way; None if it's outside
    /// the tree or gone.
    fn node_at(&mut self, p: &Path) -> Option<usize> {
        let rel = p.strip_prefix(&self.tree.nodes[self.tree.root].path).ok()?;
        let mut id = self.tree.root;
        for part in rel.components() {
            self.tree.load(id);
            let kids = self.tree.nodes[id].children.as_ref()?;
            id = kids.iter().copied().find(|&k| self.tree.nodes[k].name.as_str() == part.as_os_str())?;
        }
        Some(id)
    }

    /// Open folders and the selection, for Remember place.
    fn place(&self) -> places::Place {
        let mut open = Vec::new();
        let mut stack = vec![self.tree.root];
        while let Some(id) = stack.pop() {
            let n = &self.tree.nodes[id];
            if n.expanded && n.is_dir {
                if id != self.tree.root {
                    open.push(n.path.clone());
                }
                stack.extend(self.tree.kids(id).iter().rev());
            }
        }
        places::Place { open, at: Some(self.tree.nodes[self.cursor].path.clone()) }
    }

    /// Reopen a remembered place; whatever no longer exists is skipped.
    fn restore_place(&mut self, place: places::Place) {
        for p in &place.open {
            if let Some(id) = self.node_at(p).filter(|&id| self.tree.nodes[id].is_dir) {
                self.tree.load(id);
                self.tree.nodes[id].expanded = true;
            }
        }
        if let Some(id) = place.at.as_deref().and_then(|p| self.node_at(p)) {
            for a in self.tree.path_to(id).into_iter().filter(|&a| a != id) {
                self.tree.nodes[a].expanded = true;
            }
            self.set_cursor(id);
        }
        self.epoch += 1;
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
        if let Some(g) = &self.git {
            g.poke();
        }
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

    /// `e`: open every folder under the selected one (a file's own folder),
    /// reading them on a worker while a spinner turns.
    fn explode(&mut self) {
        if self.exploding.is_some() {
            return;
        }
        let n = &self.tree.nodes[self.cursor];
        let target = if n.is_dir { self.cursor } else { n.parent.unwrap_or(self.cursor) };
        let skip = match &self.git {
            Some(g) if !self.settings.explode_ignored => g.ignored(),
            _ => Default::default(),
        };
        let rx = explode::spawn(self.tree.nodes[target].path.clone(), self.tree.show_hidden, skip);
        self.exploding = Some(Exploding { target, rx, folders: 0, born: Instant::now() });
    }

    /// Take the explode worker's news. Returns true if anything changed.
    fn explode_poll(&mut self) -> bool {
        let Some(x) = &mut self.exploding else { return false };
        let mut done = None;
        let mut any = false;
        loop {
            match x.rx.try_recv() {
                Ok(explode::Msg::Progress(n)) => {
                    x.folders = n;
                    any = true;
                }
                Ok(explode::Msg::Done(dirs, capped)) => {
                    done = Some((dirs, capped));
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return any,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
        let target = x.target;
        self.exploding = None;
        if let Some((dirs, capped)) = done {
            self.burst(target, dirs, capped);
        }
        true
    }

    /// Hang the walked folders on the tree and open them all at once, so
    /// they unfurl together.
    fn burst(&mut self, target: usize, dirs: Vec<(PathBuf, Vec<tree::Node>)>, capped: bool) {
        if !self.tree.attached(target) {
            return;
        }
        let mut id_of = std::collections::HashMap::from([(self.tree.nodes[target].path.clone(), target)]);
        let mut opened = 0;
        // Parents come before their children, so each folder's id is known by the time it's reached.
        for (dir, nodes) in dirs {
            let Some(&id) = id_of.get(&dir) else { continue };
            self.tree.load_nodes(id, nodes);
            self.tree.nodes[id].expanded = true;
            opened += 1;
            for &k in self.tree.nodes[id].children.as_deref().unwrap_or(&[]) {
                if self.tree.nodes[k].is_dir {
                    id_of.insert(self.tree.nodes[k].path.clone(), k);
                }
            }
        }
        let what = if opened == 1 { "1 folder".to_string() } else { format!("{opened} folders") };
        let msg = if capped { format!("opened the first {what} (stopped there)") } else { format!("opened {what}") };
        self.note = Some((msg, Instant::now()));
        self.absorb();
        self.epoch += 1;
    }

    /// Point git at the open folders and take its results. Returns true if they changed.
    fn git_poll(&mut self) -> bool {
        let Some(g) = &mut self.git else { return false };
        g.set(self.tree.open_dirs().into_iter().map(|d| self.tree.nodes[d].path.clone()).collect());
        g.poll()
    }

    /// Git state of a node, None if clean or not in a repo.
    pub fn git_state(&self, id: usize) -> Option<git::St> {
        self.git.as_ref()?.state(&self.tree.nodes[id].path)
    }

    /// Open the file preview, with a diff one `d` away when git has changes for it.
    fn open_file(&mut self, id: usize) {
        let w = ui::popup(ratatui::layout::Rect::new(0, 0, self.view.0, self.view.1 + 1)).width;
        let path = self.tree.nodes[id].path.clone();
        let mut pv = Preview::load(&path, w, self.settings.preview, self.settings.wrap);
        if pv.media.is_none()
            && pv.audio.is_none()
            && self.git_state(id).is_some_and(|s| s >= git::St::Staged)
            && let Some((top, _)) = self.git.as_ref().and_then(|g| g.repo_of(&path))
        {
            pv.diff = Some(top.to_path_buf());
        }
        self.preview = Some(pv);
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
            let ripples = self.settings.ripples;
            // Light what changed, or the folder itself when what changed is gone.
            let shown = |k: &usize| self.tree.show_hidden || !self.tree.is_hidden(*k);
            let mut origins: Vec<usize> = r.touched.iter().copied().filter(shown).collect();
            if r.gone.iter().any(shown) {
                origins.push(id);
            }
            if ripples {
                self.ripple(&origins);
            }
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
        if let Some(g) = &self.git {
            g.poke();
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
        self.glide.stop();
        if self.prompt.is_some() {
            self.prompt_key(code, mods);
            return true;
        }
        if self.search.is_some() {
            self.search_key(code, mods);
            return true;
        }
        if self.menu.is_some() {
            self.menu_key(code, mods);
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
                KeyCode::Char('d') => pv.toggle_diff(),
                KeyCode::Char('u') if ctrl => pv.scroll -= page / 2,
                KeyCode::PageDown | KeyCode::Char(' ') => pv.scroll += page,
                KeyCode::PageUp => pv.scroll -= page,
                KeyCode::Char('g') | KeyCode::Home => pv.scroll = 0,
                KeyCode::Char('G') | KeyCode::End => pv.scroll = i32::MAX / 2,
                _ => {}
            }
            return true;
        }
        // Esc stops an explode instead of quitting.
        if code == KeyCode::Esc && self.exploding.take().is_some() {
            self.note = Some(("explode cancelled".into(), Instant::now()));
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
            KeyCode::Char('j') => self.step(1),
            KeyCode::Char('k') => self.step(-1),
            KeyCode::Char('J') => self.step(10),
            KeyCode::Char('K') => self.step(-10),
            KeyCode::Down => self.column_step(1),
            KeyCode::Up => self.column_step(-1),
            KeyCode::PageDown => self.column_step(10),
            KeyCode::PageUp => self.column_step(-10),
            KeyCode::Char('g') | KeyCode::Home => self.step_end(false),
            KeyCode::Char('G') | KeyCode::End => self.step_end(true),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => self.enter(),
            KeyCode::Char('h') | KeyCode::Left => self.leave(),
            KeyCode::Char(' ') | KeyCode::Tab => self.toggle(),
            KeyCode::Char('-') | KeyCode::Backspace => {
                self.tree.reroot_up();
                self.tree.reveal = self.tree.path_to(self.cursor).into_iter().collect();
            }
            KeyCode::Char('.') => {
                self.settings.show_hidden ^= true;
                self.apply_settings();
            }
            KeyCode::Char('o') => self.set_sort(self.settings.sort.next()),
            KeyCode::Char('O') => self.set_sort(Sort { rev: !self.settings.sort.rev, ..self.settings.sort }),
            KeyCode::Char(',') => {
                self.menu = Some(0);
                self.help = false;
            }
            KeyCode::Char('c') => self.collapse_others(),
            KeyCode::Char('e') => self.explode(),
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
        if self.prompt.is_some() || self.search.is_some() || self.menu.is_some() {
            return;
        }
        if !matches!(kind, MouseEventKind::ScrollDown | MouseEventKind::ScrollUp) {
            self.glide.stop();
        }
        let (rows, tau) = (self.settings.wheel_speed as i32, self.settings.momentum.tau());
        let down = if kind == MouseEventKind::ScrollDown { 1 } else { -1 };
        // Field, not open_preview(): the glide is borrowed alongside.
        if let Some(pv) = self.preview.as_mut().filter(|p| !p.closing) {
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
                MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                    pv.scroll += self.glide.notch(down * 3 * rows, tau, Instant::now())
                }
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
                        self.glide.stop();
                        self.cam_hold.get_or_insert(self.cam_tx);
                        self.set_cursor(id);
                    }
                    _ => {
                        let n = self.glide.notch(down * rows, tau, Instant::now());
                        self.column_step(n as isize);
                    }
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

    /// Node the wheel takes over at the cell: the column's line node, or past
    /// the line's end the entry nearest the pointer. From last frame's bands.
    pub fn column_at(&self, col: u16, row: u16) -> Option<usize> {
        let c = col as i32;
        let b = self.cols.iter().find(|b| c >= b.0 && c < b.1).filter(|_| row < self.view.1)?;
        let mut here = self.hits.iter().filter(|h| h.0 >= b.0 && h.0 < b.1);
        match b.2 {
            // Right over another open folder's list in the line's column: that list.
            Some(line) => {
                let parent = |id: usize| self.tree.nodes[id].parent;
                let under = here.find(|h| h.1 == row as i32).map(|h| h.3);
                Some(under.filter(|&u| parent(u) != parent(line)).unwrap_or(line))
            }
            // Past the line's end there's no line node: the entry nearest the pointer.
            None => here.min_by_key(|h| (h.1 - row as i32).abs()).map(|h| h.3),
        }
    }

    /// Column the wheel would take over: pointer over a line column other than the cursor's.
    pub fn hover(&self) -> Option<usize> {
        if self.prompt.is_some() || self.search.is_some() || self.help || self.preview.is_some() {
            return None;
        }
        let (c, r) = self.mouse?;
        self.column_at(c, r).filter(|&id| id != self.cursor)
    }

    /// Carry the wheel's momentum `dt` seconds on. True while it still glides.
    fn glide(&mut self, dt: f32) -> bool {
        let n = self.glide.tick(self.settings.momentum.tau(), dt);
        if n != 0 {
            match self.preview.as_mut() {
                Some(pv) if !pv.closing && pv.audio.is_none() && pv.media.is_none() => pv.scroll += n,
                Some(_) => self.glide.stop(),
                None => {
                    let was = self.cursor;
                    self.column_step(n as isize);
                    if self.cursor == was {
                        self.glide.stop();
                    }
                }
            }
        }
        self.glide.moving()
    }

    /// Something on screen moves on its own (a playing sound, a waveform still being read).
    fn ticking(&self) -> bool {
        self.exploding.is_some()
            || self.note.as_ref().is_some_and(|n| n.1.elapsed() < NOTE)
            || self.preview.as_ref().and_then(|p| p.audio.as_ref()).is_some_and(|a| {
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
             , opens the settings (saved to ~/.config/tb/config.toml, or $TB_CONFIG): layout, sort, colors, lines, mouse, previews, remember place\n\
             e explodes the selected folder: opens every folder inside (hidden and git-ignored ones stay closed) · esc stops it\n\
             o cycles the sort (name · modified · size · type) · O reverses it · TB_SORT=size (or -size, modified, type) sets the start\n\
             open folders update live (about once a second) · TB_LIVE=off turns that off\n\
             git: M modified · + staged · ? untracked · ! conflict · ignored names dim · d in a preview shows the diff · TB_GIT=off\n\
             color = last modified (dirs: newest anything inside): red = minutes, orange = hours, tan = days, grey = weeks, blue = years"
        );
        return Ok(());
    }
    let start = std::fs::canonicalize(&arg)?;
    let mut app = App::new(start.clone());
    app.config = settings::path();
    (app.settings, app.config_errs) = Settings::load(app.config.as_deref());
    // Environment variables win for this run.
    if let Ok(s) = std::env::var("TB_SORT") {
        match Sort::parse(&s) {
            Some(sort) => app.settings.sort = sort,
            None => {
                eprintln!("tb: TB_SORT={s:?}: expected name, modified, size or type, with - in front to reverse");
                std::process::exit(2);
            }
        }
    }
    if std::env::var("TB_LIVE").is_ok_and(|v| v == "off") {
        app.settings.live = false;
    }
    if std::env::var("TB_GIT").is_ok_and(|v| v == "off") {
        app.settings.git = false;
    }
    app.can_cd = cwd_file.is_some();
    app.services = true;
    app.apply_settings();
    let saved = places::path().filter(|_| app.settings.remember).and_then(|f| places::load(&f, &start));
    if let Some(place) = saved {
        app.restore_place(place);
    }
    let mut term = ratatui::init();
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        audio::unhush();
        let _ = execute!(stdout(), DisableMouseCapture);
        hook(info);
    }));
    let mut mouse = app.settings.mouse;
    if mouse {
        execute!(stdout(), EnableMouseCapture)?;
    }
    // Query after entering the alternate screen, before reading events.
    if let Some((p, detected)) = media::picker() {
        app.graphics = Some((p.clone(), p.protocol_type()));
        app.detected = detected;
        app.graphics_forced = std::env::var("TB_GRAPHICS").is_ok_and(|v| !v.is_empty());
        app.apply_graphics();
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
            // Real time, not Motion's: momentum is its own setting.
            let mut moving = app.glide(dt);
            let dt = dt * app.settings.speed.factor();
            if let Err(e) = term.draw(|f| moving |= ui::frame(f, &mut app, dt)) {
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
                dirty |= app.git_poll();
                dirty |= app.explode_poll();
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
        if app.settings.mouse != mouse {
            mouse = app.settings.mouse;
            let _ = if mouse { execute!(stdout(), EnableMouseCapture) } else { execute!(stdout(), DisableMouseCapture) };
            app.mouse = None;
        }
        if let Some(run) = app.pending.take() {
            let sel = app.tree.nodes[app.cursor].path.clone();
            if let Err(e) = shell::run(&mut term, &app.work_dir(), &sel, &run, mouse) {
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
        app.git_poll();
        app.explode_poll();
    };
    restore();
    // Checked at quit, so turning it on in the menu counts for this run.
    if let (true, Some(f)) = (app.settings.remember, places::path()) {
        let _ = places::save(&f, &start, app.place());
    }
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
    fn folders_first_holds_under_every_sort() {
        let (mut app, d) = sort_fixture("folders");
        app.set_sort(Sort { key: SortKey::Modified, rev: false });
        assert_eq!(names(&app, d), ["mid.txt", "Zed", "a", "big.log"]);
        app.settings.folders_first = true;
        app.apply_settings();
        assert_eq!(names(&app, d), ["a", "mid.txt", "Zed", "big.log"]);
        app.set_sort(Sort { key: SortKey::Modified, rev: true });
        assert_eq!(names(&app, d), ["a", "big.log", "Zed", "mid.txt"], "reversing keeps folders on top");
    }

    #[test]
    fn remembered_place_reopens_folders_and_selection() {
        let mut app = app_in("place", &["one"]);
        let d = app.tree.nodes[app.cursor].path.parent().unwrap().to_path_buf();
        fs::create_dir_all(d.join("deep/er")).unwrap();
        fs::write(d.join("deep/er/leaf"), "").unwrap();
        app.reload();
        let deep = app.node_at(&d.join("deep")).unwrap();
        app.set_cursor(deep);
        app.enter();
        app.enter();
        assert_eq!(at(&app), "leaf");
        let place = app.place();
        assert_eq!(place.open, [d.clone(), d.join("deep"), d.join("deep/er")]);

        let mut fresh = App::new(d.clone());
        let gone = places::Place { open: vec![d.join("nope")], ..places::Place::default() };
        fresh.restore_place(gone);
        assert_eq!(at(&fresh), "deep", "a vanished folder is skipped");
        fresh.restore_place(place);
        assert_eq!(at(&fresh), "leaf");
        assert!(fresh.tree.nodes[fresh.tree.nodes[fresh.cursor].parent.unwrap()].expanded);
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
        app.step_end(true);
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
    fn settings_menu_changes_applies_and_saves() {
        let mut app = app_in("menu", &["alpha", "beta"]);
        let cfg = std::env::temp_dir().join(format!("tb-menu-{}/config.toml", std::process::id()));
        let _ = fs::remove_file(&cfg);
        app.config = Some(cfg.clone());
        typed(&mut app, ",");
        assert_eq!(app.menu, Some(0));
        typed(&mut app, "ll");
        assert_eq!(app.spacing().rows, 2, "row spacing is the first row");
        typed(&mut app, "h");
        assert_eq!(app.settings.row_spacing, 1);
        let at = |k: &str| ITEMS.iter().position(|i| i.key == k).unwrap();
        app.menu = Some(at("sort"));
        typed(&mut app, "l");
        assert_eq!(app.tree.sort.key, SortKey::Modified, "the tree follows at once");
        app.menu = Some(at("show_hidden"));
        typed(&mut app, " ");
        assert!(app.tree.show_hidden);
        typed(&mut app, "r");
        assert!(!app.tree.show_hidden, "r resets to the default");
        typed(&mut app, "j");
        assert_eq!(app.menu, Some(at("show_hidden") + 1));
        app.key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.menu.is_none());
        assert!(!app.key(KeyCode::Esc, KeyModifiers::NONE), "esc after closing quits as usual");

        let (saved, errs) = Settings::load(Some(&cfg));
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!((saved.row_spacing, saved.sort.key, saved.show_hidden), (1, SortKey::Modified, false));
        let text = fs::read_to_string(&cfg).unwrap();
        assert!(text.contains("sort_reverse = false"), "a new sort key saves its direction too: {text}");
    }

    #[test]
    fn j_and_k_walk_the_folder_the_column_or_the_tree() {
        let mut app = app_in("cross", &["x"]);
        let d = app.tree.nodes[app.cursor].path.parent().unwrap().to_path_buf();
        for f in ["a/1", "a/2/deep", "b/3", "c/4"] {
            fs::create_dir_all(d.join(f)).unwrap();
        }
        app.reload();
        for dir in ["a", "a/2", "c"] {
            let id = app.node_at(&d.join(dir)).unwrap();
            app.tree.load(id);
            app.tree.nodes[id].expanded = true;
        }
        let go = |app: &mut App, path: &str| {
            let id = app.node_at(&d.join(path)).unwrap();
            app.set_cursor(id);
        };
        let j = |app: &mut App| app.key(KeyCode::Char('j'), KeyModifiers::NONE);
        let k = |app: &mut App| app.key(KeyCode::Char('k'), KeyModifiers::NONE);

        // Tree (the default): into open folders, back out to the next sibling.
        go(&mut app, "a");
        let mut seen = Vec::new();
        for _ in 0..7 {
            j(&mut app);
            seen.push(at(&app).to_string());
        }
        assert_eq!(seen, ["1", "2", "deep", "b", "c", "4", "x"]);
        k(&mut app);
        k(&mut app);
        assert_eq!(at(&app), "c", "k walks it backwards");
        go(&mut app, "a/1");
        app.key(KeyCode::Char('G'), KeyModifiers::NONE);
        assert_eq!(at(&app), "2", "G stays in the folder");
        go(&mut app, "a");
        app.mouse(MouseEventKind::ScrollDown, 0, 0);
        assert_eq!(at(&app), "b", "the wheel scrolls the column, it doesn't dive into a");
        app.mouse(MouseEventKind::ScrollDown, 0, 0);
        assert_eq!(at(&app), "c");
        go(&mut app, "a");
        app.key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(at(&app), "b", "arrows stay in the column too");
        app.key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(at(&app), "a");
        go(&mut app, "a/2");

        // Column: every open list one column over, closed b passed by.
        app.settings.step = StepThrough::Column;
        j(&mut app);
        assert_eq!(at(&app), "4");
        j(&mut app);
        assert_eq!(at(&app), "4", "the column's end");
        app.key(KeyCode::Char('K'), KeyModifiers::NONE);
        assert_eq!(at(&app), "1");

        // Folder: its own list only.
        app.settings.step = StepThrough::Folder;
        go(&mut app, "c/4");
        k(&mut app);
        assert_eq!(at(&app), "4");
    }

    #[test]
    fn the_wheel_takes_the_list_under_the_pointer() {
        let mut app = app_in("wheel", &["x"]);
        let d = app.tree.nodes[app.cursor].path.parent().unwrap().to_path_buf();
        for f in ["a/1", "a/2", "b/3"] {
            fs::create_dir_all(d.join(f)).unwrap();
        }
        app.reload();
        let id = |app: &mut App, p: &str| app.node_at(&d.join(p)).unwrap();
        let (one, two, three) = (id(&mut app, "a/1"), id(&mut app, "a/2"), id(&mut app, "b/3"));
        app.view = (80, 24);
        // Column 10..20 on the line (its line node 3, b's), column 20..30 past its end.
        app.cols = vec![(10, 20, Some(three)), (20, 30, None)];
        app.hits = vec![(12, 4, 1, one), (12, 5, 1, two), (12, 6, 1, three), (22, 2, 1, one), (22, 9, 1, two)];
        assert_eq!(app.column_at(14, 4), Some(one), "right over a's list: a's entry");
        assert_eq!(app.column_at(14, 6), Some(three), "over the line's own list: the line node");
        assert_eq!(app.column_at(14, 12), Some(three), "on no entry: the line node");
        assert_eq!(app.column_at(24, 7), Some(two), "past the line's end: nearest the pointer");
        assert_eq!(app.column_at(5, 7), None, "outside every column");
    }

    #[test]
    fn a_flicked_wheel_glides_on_and_stops_at_the_end() {
        let names: Vec<String> = (0..40).map(|i| format!("f{i:02}")).collect();
        let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let mut app = app_in("glide", &refs);
        app.settings.wheel_speed = 2;
        app.settings.momentum = settings::Momentum::Long;
        let row = |app: &App| at(app).to_string();
        let start = row(&app);
        for _ in 0..3 {
            app.mouse(MouseEventKind::ScrollDown, 0, 0);
        }
        let after = row(&app);
        assert_ne!(start, after);
        let mut frames = 0;
        while app.glide(1.0 / 60.0) {
            frames += 1;
            assert!(frames < 600, "the glide ends");
        }
        assert!(row(&app) > after, "kept going after the last notch: {after} -> {}", row(&app));

        // A key halts it at once.
        for _ in 0..3 {
            app.mouse(MouseEventKind::ScrollUp, 0, 0);
        }
        app.key(KeyCode::Char('?'), KeyModifiers::NONE);
        let held = row(&app);
        assert!(!app.glide(1.0 / 60.0));
        assert_eq!(row(&app), held);

        // At the list's end it stops rather than spinning.
        app.key(KeyCode::Char('?'), KeyModifiers::NONE);
        app.key(KeyCode::Char('G'), KeyModifiers::NONE);
        for _ in 0..3 {
            app.mouse(MouseEventKind::ScrollDown, 0, 0);
        }
        let mut frames = 0;
        while app.glide(1.0 / 60.0) {
            frames += 1;
        }
        assert!(frames < 10, "stopped at the end after {frames} frames");
        assert_eq!(at(&app), "f39");
    }

    #[test]
    fn keys_keep_the_settings_in_step() {
        let mut app = app_in("keys-settings", &["alpha"]);
        typed(&mut app, "oO.");
        assert_eq!(app.settings.sort, Sort { key: SortKey::Modified, rev: true });
        assert!(app.settings.show_hidden && app.tree.show_hidden);
    }

    #[test]
    fn e_explodes_the_selected_folder_and_esc_stops_it() {
        let mut app = app_in("explode", &["file.txt"]);
        let d = app.tree.nodes[app.cursor].parent.unwrap();
        let base = app.tree.nodes[d].path.clone();
        for p in ["src/a/b", "src/c", "src/.cache/x"] {
            fs::create_dir_all(base.join(p)).unwrap();
        }
        app.reload();
        let src = app.tree.kids(d).into_iter().find(|&k| app.tree.nodes[k].name == "src").unwrap();
        app.set_cursor(src);
        typed(&mut app, "e");
        assert!(app.exploding.as_ref().is_some_and(|x| x.target == src));
        let t0 = Instant::now();
        while app.exploding.is_some() && t0.elapsed() < Duration::from_secs(5) {
            app.explode_poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        let open = |app: &App, rel: &str| {
            let p = base.join(rel);
            app.tree.nodes.iter().position(|n| n.path == p).is_some_and(|i| app.tree.nodes[i].expanded && app.tree.attached(i))
        };
        assert!(open(&app, "src") && open(&app, "src/a") && open(&app, "src/a/b") && open(&app, "src/c"));
        assert!(!open(&app, "src/.cache"), "hidden folders stay shut while dotfiles are hidden");
        assert_eq!(app.cursor, src, "the cursor stays on the exploded folder");
        assert!(app.note.as_ref().is_some_and(|n| n.0 == "opened 4 folders"), "{:?}", app.note);

        typed(&mut app, "e");
        assert!(app.key(KeyCode::Esc, KeyModifiers::NONE), "esc stops the explode, not tb");
        assert!(app.exploding.is_none());
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
