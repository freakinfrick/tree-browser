//! tb — horizontal tree file browser. Color = recency (recursive for dirs).
mod anim;
mod layout;
mod media;
mod mtime;
mod shell;
mod tree;
mod ui;

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
    /// Signal bead running along the line toward the cursor (world x).
    pub bead: Option<Damped>,
    /// Cursor (id, world x) last frame, to launch the bead from.
    pub last_cursor: Option<(usize, i32)>,
    pub help: bool,
    pub help_anim: f32,
    pub view: (u16, u16),
    /// Last frame's clickable labels: (screen x, y, width, node).
    pub hits: Vec<(i32, i32, i32, usize)>,
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
    history: Vec<String>,
    /// Steps back into history (0 = the fresh line).
    hist_at: usize,
    /// Command queued for the main loop, which owns the terminal.
    pending: Option<Run>,
    /// Quit with q: the wrapper cd's the shell to work_dir().
    cd_on_quit: bool,
    /// Started with --cwd-file, so q really does cd (status bar says so).
    pub can_cd: bool,
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
            preview: None,
            epoch: 0,
            lay: None,
            picker: None,
            detected: ratatui_image::picker::ProtocolType::Halfblocks,
            graphics_shown: false,
            repaint: false,
            prompt: None,
            history: Vec::new(),
            hist_at: 0,
            pending: None,
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

    /// Copy fresh mtime-cache results onto the nodes.
    fn absorb(&mut self) {
        for n in self.tree.nodes.iter_mut().filter(|n| n.is_dir) {
            n.rec = self.mt.cache.get(&n.path).map(|c| (c.0, c.1, c.3));
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

    /// Open preview that is not already closing.
    fn open_preview(&mut self) -> Option<&mut Preview> {
        self.preview.as_mut().filter(|p| !p.closing)
    }

    /// Returns false to quit.
    fn key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        if self.prompt.is_some() {
            self.prompt_key(code, mods);
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
            KeyCode::Char('.') => self.tree.show_hidden ^= true,
            KeyCode::Char('c') => self.collapse_others(),
            KeyCode::Char('r') => self.reload(),
            KeyCode::Char('?') => self.help ^= true,
            KeyCode::Char('!') => {
                self.prompt = Some(String::new());
                self.hist_at = 0;
            }
            KeyCode::Char('s') => self.pending = Some(Run::Shell),
            _ => {}
        }
        true
    }

    fn mouse(&mut self, kind: MouseEventKind, col: u16, row: u16) {
        // Clicks would silently change the folder the typed command runs in.
        if self.prompt.is_some() {
            return;
        }
        if let Some(pv) = self.open_preview() {
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
             color = last modified (dirs: newest anything inside): red = minutes, orange = hours, tan = days, grey = weeks, blue = years"
        );
        return Ok(());
    }
    let start = std::fs::canonicalize(&arg)?;
    let mut app = App::new(start);
    app.can_cd = cwd_file.is_some();
    let mut term = ratatui::init();
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
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
                    // Keys typed after Enter belong to the command, not the tree.
                    if app.pending.is_some() {
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
