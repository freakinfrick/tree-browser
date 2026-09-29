//! tb — horizontal tree file browser. Color = recency (recursive for dirs).
mod anim;
mod layout;
mod mtime;
mod tree;
mod ui;

use std::io::stdout;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton,
    MouseEventKind,
};
use ratatui::crossterm::execute;

use anim::{Damped, Scene};
use mtime::Mtime;
use tree::Tree;
use ui::Preview;

/// Frame budget while animating. Idle = no frames at all.
const FRAME: Duration = Duration::from_micros(16_667);

pub struct App {
    pub tree: Tree,
    pub cursor: usize,
    pub mt: Mtime,
    pub scene: Scene,
    pub cam: Option<(Damped, Damped)>,
    /// Route flash on navigation, decays 1 -> 0.
    pub flash: f32,
    pub help: bool,
    pub help_anim: f32,
    pub view: (u16, u16),
    /// Last frame's clickable labels: (screen x, y, width, node).
    pub hits: Vec<(i32, i32, i32, usize)>,
    pub preview: Option<Preview>,
    /// Bumped whenever tree, cursor, or heat data change; layout is cached per epoch.
    pub epoch: u64,
    pub lay: Option<(u64, layout::Layout)>,
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
        let mut app = App {
            tree,
            cursor: start_id,
            mt: Mtime::spawn(),
            scene: Scene::default(),
            cam: None,
            flash: 0.0,
            help: false,
            help_anim: 0.0,
            view: (80, 24),
            hits: Vec::new(),
            preview: None,
            epoch: 0,
            lay: None,
        };
        app.enter();
        app
    }

    /// Newest mtime at or under the node.
    pub fn heat_of(&self, id: usize) -> SystemTime {
        let n = &self.tree.nodes[id];
        n.rec.map_or(n.mtime, |(t, _)| t.max(n.mtime))
    }

    pub fn label(&self, id: usize) -> String {
        let n = &self.tree.nodes[id];
        let partial = n.rec.is_some_and(|c| !c.1);
        // Truncate before the marker so long names keep it.
        if partial { format!("{}~", layout::truncate_to(&n.name, layout::MAXW - 1)) } else { n.name.clone() }
    }

    /// Copy fresh mtime-cache results onto the nodes.
    fn absorb(&mut self) {
        for n in self.tree.nodes.iter_mut().filter(|n| n.is_dir) {
            n.rec = self.mt.cache.get(&n.path).map(|c| (c.0, c.1));
        }
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
        self.absorb();
        let again = self.tree.kids(id).iter().copied().find(|&k| self.tree.nodes[k].path == name);
        if id != self.cursor {
            self.set_cursor(again.or(self.tree.kids(id).first().copied()).unwrap_or(id));
        }
    }

    /// Open preview that is not already closing.
    fn open_preview(&mut self) -> Option<&mut Preview> {
        self.preview.as_mut().filter(|p| !p.closing)
    }

    /// Returns false to quit.
    fn key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        if let Some(pv) = self.open_preview() {
            let page = pv.page.max(1) as i32;
            let ctrl = mods.contains(KeyModifiers::CONTROL);
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
                self.flash = 1.0;
            }
            KeyCode::Char('c') => self.collapse_others(),
            KeyCode::Char('r') => self.reload(),
            KeyCode::Char('?') => self.help ^= true,
            _ => {}
        }
        true
    }

    fn mouse(&mut self, kind: MouseEventKind, col: u16, row: u16) {
        if let Some(pv) = self.open_preview() {
            match kind {
                MouseEventKind::ScrollDown => pv.scroll += 3,
                MouseEventKind::ScrollUp => pv.scroll -= 3,
                MouseEventKind::Down(MouseButton::Left) => pv.closing = true,
                _ => {}
            }
            return;
        }
        match kind {
            MouseEventKind::ScrollDown => self.step(1),
            MouseEventKind::ScrollUp => self.step(-1),
            MouseEventKind::Down(MouseButton::Left) => {
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
}

fn restore() {
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
}

fn main() -> std::io::Result<()> {
    let arg = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    if arg == "-h" || arg == "--help" {
        println!(
            "usage: tb [DIR]\n\nhjkl/arrows move · enter/l open · space fold · - reroot up · c collapse others · r reload · ? help · q quit\n\
             color = last modified (dirs: newest anything inside): red = minutes, orange = hours, tan = days, grey = weeks, blue = years"
        );
        return Ok(());
    }
    let start = std::fs::canonicalize(&arg)?;
    let mut app = App::new(start);
    let mut term = ratatui::init();
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), DisableMouseCapture);
        hook(info);
    }));
    execute!(stdout(), EnableMouseCapture)?;

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
            let mut moving = false;
            if let Err(e) = term.draw(|f| moving = ui::frame(f, &mut app, dt)) {
                break Err(e);
            }
            animating = moving;
            dirty = false;
        }
        // Sleep until the next frame is due, or indefinitely-ish when idle
        // (wake periodically to pick up background mtime results).
        let timeout = if animating { FRAME.saturating_sub(last.elapsed()) } else { Duration::from_millis(100) };
        match event::poll(timeout) {
            Ok(true) => {}
            Ok(false) => {
                if app.mt.poll() {
                    app.absorb();
                    app.epoch += 1;
                    dirty = true;
                }
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
                }
                Ok(Event::Mouse(m)) if m.kind != MouseEventKind::Moved => app.mouse(m.kind, m.column, m.row),
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
        app.epoch += 1;
        if app.mt.poll() {
            app.absorb();
        }
    };
    restore();
    res
}
