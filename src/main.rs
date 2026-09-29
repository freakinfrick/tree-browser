//! tb — horizontal tree file browser. Color = recency (recursive for dirs).
mod layout;
mod mtime;
mod tree;
mod ui;

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use mtime::Mtime;
use tree::Tree;
use ui::Preview;

pub struct App {
    pub tree: Tree,
    pub cursor: usize,
    pub mt: Mtime,
    pub cam: Option<(f32, f32)>,
    pub target: (f32, f32),
    pub view: (u16, u16),
    pub preview: Option<Preview>,
}

impl App {
    fn new(start: PathBuf) -> App {
        // Root at the parent so the start dir's siblings show on the left, like the clip.
        let root = start.parent().map(PathBuf::from).unwrap_or_else(|| start.clone());
        let mut tree = Tree::new(&root);
        tree.load(tree.root);
        tree.nodes[tree.root].expanded = true;
        let start_id = tree
            .kids(tree.root)
            .iter()
            .copied()
            .find(|&k| tree.nodes[k].path == start)
            .unwrap_or(tree.root);
        tree.nodes[tree.root].last = Some(start_id);
        let mut app = App { tree, cursor: start_id, mt: Mtime::spawn(), cam: None, target: (0.0, 0.0), view: (80, 24), preview: None };
        app.enter();
        app
    }

    /// Newest mtime at or under the node.
    pub fn heat(&self, id: usize) -> SystemTime {
        let n = &self.tree.nodes[id];
        if n.is_dir
            && let Some(&(t, _)) = self.mt.cache.get(&n.path) {
                return t.max(n.mtime);
            }
        n.mtime
    }

    pub fn label(&self, id: usize) -> String {
        let n = &self.tree.nodes[id];
        let partial = n.is_dir && self.mt.cache.get(&n.path).is_some_and(|c| !c.1);
        if partial { format!("{}~", n.name) } else { n.name.clone() }
    }

    fn siblings(&self) -> Vec<usize> {
        match self.tree.nodes[self.cursor].parent {
            Some(p) => self.tree.kids(p).to_vec(),
            None => vec![self.cursor],
        }
    }

    fn set_cursor(&mut self, id: usize) {
        self.cursor = id;
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
        let again = self.tree.kids(id).iter().copied().find(|&k| self.tree.nodes[k].path == name);
        if id != self.cursor {
            self.set_cursor(again.or(self.tree.kids(id).first().copied()).unwrap_or(id));
        }
    }

    /// Returns false to quit.
    fn key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        if let Some(pv) = &mut self.preview {
            let page = pv.page.max(1);
            match code {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => self.preview = None,
                KeyCode::Char('j') | KeyCode::Down => pv.scroll = pv.scroll.saturating_add(1),
                KeyCode::Char('k') | KeyCode::Up => pv.scroll = pv.scroll.saturating_sub(1),
                KeyCode::PageDown | KeyCode::Char(' ') => pv.scroll = pv.scroll.saturating_add(page),
                KeyCode::Char('d') if mods.contains(KeyModifiers::CONTROL) => pv.scroll = pv.scroll.saturating_add(page / 2),
                KeyCode::Char('u') if mods.contains(KeyModifiers::CONTROL) => pv.scroll = pv.scroll.saturating_sub(page / 2),
                KeyCode::PageUp => pv.scroll = pv.scroll.saturating_sub(page),
                KeyCode::Char('g') | KeyCode::Home => pv.scroll = 0,
                KeyCode::Char('G') | KeyCode::End => pv.scroll = u16::MAX,
                _ => {}
            }
            return true;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return false,
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
            }
            KeyCode::Char('c') => self.collapse_others(),
            KeyCode::Char('r') => self.reload(),
            _ => {}
        }
        true
    }

    /// Ease the camera toward its target. Returns true while still moving.
    fn tick_camera(&mut self) -> bool {
        let Some((x, y)) = self.cam else { return false };
        let (tx, ty) = self.target;
        let (dx, dy) = (tx - x, ty - y);
        if dx.abs() < 0.5 && dy.abs() < 0.5 {
            let moved = (x, y) != (tx, ty);
            self.cam = Some((tx, ty));
            return moved;
        }
        self.cam = Some((x + dx * 0.22, y + dy * 0.22));
        true
    }
}

fn main() -> std::io::Result<()> {
    let arg = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    if arg == "-h" || arg == "--help" {
        println!("usage: tb [DIR]\n\nhjkl/arrows move · enter/l open · space fold · - reroot up · c collapse others · r reload · q quit\ncolor = last modified (dirs: newest anything inside): red/orange = hours, tan = days, grey = weeks, blue = older");
        return Ok(());
    }
    let start = std::fs::canonicalize(&arg)?;
    let mut app = App::new(start);
    let mut term = ratatui::init();
    let mut dirty = true;
    let res = loop {
        if dirty
            && let Err(e) = term.draw(|f| ui::draw(f, &mut app)) {
                break Err(e);
            }
        dirty = false;
        match event::poll(Duration::from_millis(16)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                    if !app.key(k.code, k.modifiers) {
                        break Ok(());
                    }
                    dirty = true;
                }
                Ok(Event::Resize(..)) => dirty = true,
                Ok(_) => {}
                Err(e) => break Err(e),
            },
            Ok(false) => {}
            Err(e) => break Err(e),
        }
        dirty |= app.mt.poll();
        dirty |= app.tick_camera();
    };
    ratatui::restore();
    res
}
