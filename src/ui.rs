//! Rendering: animated tree canvas, status bar, preview popup, help overlay.
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, OwnedFd};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Instant, SystemTime};

use ansi_to_tui::IntoText;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout as RLayout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::anim::{approach, heat, heat_stops, mix, pulse, to_color, Damped, Rgb, RECOLOR};
use crate::layout::{cell_glyph, layout_with, lines, ACTIVE, DOWN, ROUTE, UP};
use crate::audio::{self, Audio, State};
use crate::git::St;
use crate::media::Media;
use crate::tree::Sort;
use crate::settings::TextPreview;
use crate::App;

/// The fixed colors of one ground: everything but heat and accent.
pub struct GroundColors {
    pub bg: Rgb,
    bar: Rgb,
    pop: Rgb,
    /// Status bar and popup text.
    text: Rgb,
    /// Names on the cursor path.
    route_text: Rgb,
    flash: Rgb,
    dot: Rgb,
    muted: Rgb,
    /// Ignored names at dim floor 10; floor 0 sits nearly on the background.
    ignored: Rgb,
    match_bg: Rgb,
    /// A live change's flash, and the ember behind the name that changed.
    ripple: Rgb,
    ripple_bg: Rgb,
    /// Untracked, staged, modified, conflict.
    git: [Rgb; 4],
    /// Where a cell with the terminal's own foreground fades from.
    default_fg: Rgb,
}

const DARK: GroundColors = GroundColors {
    bg: [9.0, 10.0, 15.0],
    bar: [17.0, 19.0, 29.0],
    pop: [13.0, 15.0, 23.0],
    text: [238.0, 240.0, 250.0],
    route_text: [238.0, 240.0, 250.0],
    flash: [235.0, 242.0, 255.0],
    dot: [255.0, 58.0, 58.0],
    muted: [110.0, 118.0, 150.0],
    ignored: [214.0, 216.0, 224.0],
    match_bg: [92.0, 70.0, 22.0],
    ripple: [255.0, 200.0, 150.0],
    ripple_bg: [110.0, 38.0, 28.0],
    git: [[110.0, 200.0, 225.0], [120.0, 215.0, 130.0], [240.0, 200.0, 90.0], [255.0, 90.0, 120.0]],
    default_fg: [200.0; 3],
};

/// The Ent look: ink on parchment.
const PARCHMENT: GroundColors = GroundColors {
    bg: [242.0, 232.0, 207.0],
    bar: [226.0, 212.0, 178.0],
    pop: [236.0, 224.0, 194.0],
    text: [40.0, 32.0, 24.0],
    route_text: [24.0, 76.0, 38.0],
    flash: [20.0, 16.0, 12.0],
    dot: [190.0, 30.0, 30.0],
    muted: [130.0, 118.0, 100.0],
    ignored: [80.0, 74.0, 66.0],
    match_bg: [236.0, 204.0, 120.0],
    ripple: [190.0, 70.0, 20.0],
    ripple_bg: [240.0, 196.0, 160.0],
    git: [[30.0, 118.0, 150.0], [40.0, 130.0, 50.0], [176.0, 120.0, 0.0], [180.0, 30.0, 60.0]],
    default_fg: [40.0, 32.0, 24.0],
};

static GROUND: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_ground(g: crate::settings::Ground) {
    GROUND.store(g as u8, std::sync::atomic::Ordering::Relaxed);
}

fn paper() -> bool {
    GROUND.load(std::sync::atomic::Ordering::Relaxed) == crate::settings::Ground::Parchment as u8
}

/// The ground in use.
pub fn gr() -> &'static GroundColors {
    if paper() {
        &PARCHMENT
    } else {
        &DARK
    }
}
/// Line and selector colors for one accent.
pub struct AccentColors {
    /// Branches off the cursor path.
    dim: Rgb,
    /// Branches hanging off the line.
    active: Rgb,
    /// The line itself, keys, highlights.
    route: Rgb,
    /// Selector background.
    pill: Rgb,
}

const ACCENTS: [AccentColors; 5] = [
    // Indigo
    AccentColors { dim: [33.0, 39.0, 68.0], active: [68.0, 86.0, 168.0], route: [140.0, 168.0, 255.0], pill: [34.0, 39.0, 72.0] },
    // Teal
    AccentColors { dim: [24.0, 50.0, 54.0], active: [44.0, 124.0, 126.0], route: [110.0, 222.0, 212.0], pill: [22.0, 52.0, 56.0] },
    // Violet
    AccentColors { dim: [45.0, 33.0, 68.0], active: [108.0, 70.0, 170.0], route: [198.0, 152.0, 255.0], pill: [48.0, 32.0, 74.0] },
    // Amber
    AccentColors { dim: [58.0, 44.0, 24.0], active: [150.0, 108.0, 40.0], route: [255.0, 198.0, 110.0], pill: [62.0, 46.0, 22.0] },
    // Mono
    AccentColors { dim: [44.0, 44.0, 50.0], active: [100.0, 100.0, 110.0], route: [212.0, 212.0, 222.0], pill: [46.0, 46.0, 54.0] },
];

/// Parchment's only accent: bark lines, a forest-green path, a moss selector.
const FOREST: AccentColors =
    AccentColors { dim: [176.0, 158.0, 130.0], active: [96.0, 120.0, 70.0], route: [34.0, 92.0, 48.0], pill: [208.0, 222.0, 182.0] };

static ACCENT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_accent(a: crate::settings::Accent) {
    ACCENT.store(a as u8, std::sync::atomic::Ordering::Relaxed);
}

fn acc() -> &'static AccentColors {
    if paper() {
        return &FOREST;
    }
    &ACCENTS[ACCENT.load(std::sync::atomic::Ordering::Relaxed) as usize % ACCENTS.len()]
}

/// How much heat tints an ignored name's grey.
const IGNORED_TINT: f32 = 0.12;

/// An ignored name's color: a grey from nearly the background (floor 0) to
/// the ground's ignored grey (floor 10), with a hint of its heat.
fn ignored_shade(g: &GroundColors, h: Rgb, floor: u8) -> Rgb {
    let grey = mix(g.bg, g.ignored, 0.1 + 0.08 * floor.min(10) as f32);
    mix(grey, h, IGNORED_TINT)
}
/// A name off the cursor's line: each off-line dim step below 10 fades 0.08,
/// so 0 leaves a fifth of the color.
fn off_line(g: &GroundColors, h: Rgb, focus_dim: u8) -> Rgb {
    mix(h, g.bg, 0.08 * (10 - focus_dim.min(10)) as f32)
}

/// Git marker colors.
fn git_color(st: St) -> Rgb {
    let g = gr();
    match st {
        St::Ignored => g.muted,
        St::Untracked => g.git[0],
        St::Staged => g.git[1],
        St::Modified => g.git[2],
        St::Conflict => g.git[3],
    }
}

/// Seconds for the camera to (mostly) arrive; slower than nodes so the eye
/// sees the tree move before the view recenters.
const CAM: f32 = 0.16;
const POPUP_T: f32 = 0.085;

pub fn ago(t: SystemTime) -> String {
    let s = SystemTime::now().duration_since(t).unwrap_or_default().as_secs();
    match s {
        s if s < 60 => format!("{s}s ago"),
        s if s < 3600 => format!("{}m ago", s / 60),
        s if s < 86400 => format!("{}h ago", s / 3600),
        s if s < 86400 * 365 => format!("{}d ago", s / 86400),
        s => format!("{:.1}y ago", s as f64 / (86400.0 * 365.0)),
    }
}

fn human(n: u64) -> String {
    let mut v = n as f64;
    for u in ["B", "KB", "MB", "GB", "TB"] {
        if v < 1024.0 || u == "TB" {
            return if u == "B" { format!("{n} B") } else { format!("{v:.1} {u}") };
        }
        v /= 1024.0;
    }
    unreachable!()
}

fn age_of(t: SystemTime) -> f32 {
    SystemTime::now().duration_since(t).unwrap_or_default().as_secs_f32()
}

/// Break lines longer than `width` columns, for plain previews (the preview
/// pane clips rather than wraps).
fn wrap_text(raw: &[u8], width: usize) -> Vec<u8> {
    let text = String::from_utf8_lossy(raw);
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let mut w = 0;
        for c in line.chars() {
            let cw = c.width().unwrap_or(0);
            if w + cw > width && c != '\n' {
                out.push('\n');
                w = 0;
            }
            out.push(c);
            w += cw;
        }
    }
    out.into_bytes()
}

/// Write `text` at world-relative screen coords, clipping on every side.
fn put(buf: &mut Buffer, area: Rect, sx: i32, sy: i32, text: &str, style: Style) {
    if sy < 0 || sy >= area.height as i32 || sx >= area.width as i32 {
        return;
    }
    let mut col = sx;
    let mut s = String::new();
    for c in text.chars() {
        let w = c.width().unwrap_or(0) as i32;
        if col >= 0 {
            s.push(c);
        } else if col + w > 0 {
            s.push(' ');
        }
        col += w;
    }
    let x0 = sx.max(0);
    let room = (area.width as i32 - x0) as usize;
    buf.set_stringn(area.x + x0 as u16, area.y + sy as u16, s, room, style);
}

fn tint(buf: &mut Buffer, r: Rect, bg: Rgb) {
    for y in r.top()..r.bottom() {
        for x in r.left()..r.right() {
            buf[(x, y)].set_bg(to_color(bg));
        }
    }
}

/// Pull every cell outside `keep` toward the background by `d` (0..1).
fn dim_backdrop(buf: &mut Buffer, keep: Rect, d: f32) {
    let area = buf.area;
    let fade = |c: Color, dflt: Rgb| -> Color {
        let rgb = match c {
            Color::Rgb(r, g, b) => [r as f32, g as f32, b as f32],
            _ => dflt,
        };
        to_color(mix(rgb, gr().bg, d))
    };
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if keep.contains((x, y).into()) {
                continue;
            }
            let cell = &mut buf[(x, y)];
            let (fg, bg) = (cell.fg, cell.bg);
            cell.set_fg(fade(fg, gr().default_fg)).set_bg(fade(bg, gr().bg));
        }
    }
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    let l = |x: u16, y: u16| (x as f32 + (y as f32 - x as f32) * t).round() as u16;
    Rect { x: l(a.x, b.x), y: l(a.y, b.y), width: l(a.width, b.width), height: l(a.height, b.height) }
}

/// Draw one frame. Returns true while anything is still animating.
pub fn frame(f: &mut Frame, app: &mut App, dt: f32) -> bool {
    let [canvas, status] = RLayout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(f.area());
    app.view = (canvas.width, canvas.height);

    // Relayout only when the model changed; animation frames reuse it.
    let fresh = app.lay.as_ref().is_none_or(|(g, _)| *g != app.epoch) || app.lay_rows != canvas.height;
    let (epoch, lay) = match app.lay.take() {
        Some(l) if !fresh => l,
        _ => {
            app.lay_rows = canvas.height;
            let mut pipes = std::mem::take(&mut app.pipes);
            let l = layout_with(&app.tree, app.cursor, &|i| app.label(i), app.spacing(), &mut pipes);
            app.pipes = pipes;
            (app.epoch, l)
        }
    };
    let tree = &app.tree;
    let mut moving = app.scene.sync(&lay, &|id| tree.nodes[id].parent, dt);
    let route: HashSet<usize> = app.tree.path_to(app.cursor).into_iter().collect();

    // Color targets: heat, dimmed off-route, white on route. Recomputed on
    // relayout only; the per-frame loop just cross-fades toward them.
    if fresh {
        let now = SystemTime::now();
        for p in &lay.placed {
            let target = if route.contains(&p.id) {
                gr().route_text
            } else {
                let age = now.duration_since(app.heat_of(p.id)).unwrap_or_default().as_secs_f32();
                let h = heat(age);
                if p.active { h } else { off_line(gr(), h, app.settings.focus_dim) }
            };
            let Some(a) = app.scene.nodes.get_mut(&p.id) else { continue };
            a.target = target;
            if a.rgb == [0.0; 3] {
                a.rgb = a.target;
            }
        }
    }
    for a in app.scene.nodes.values_mut().filter(|a| !a.ghost) {
        for (c, t) in a.rgb.iter_mut().zip(a.target) {
            moving |= approach(c, t, RECOLOR, dt);
        }
    }

    // Live-change ripples: brightness per node this frame.
    let tick = std::time::Instant::now();
    app.ripples.retain(|_, r| tick < r.start + crate::RIPPLE_LIFE);
    moving |= !app.ripples.is_empty();
    let since = |start: std::time::Instant| match tick.checked_duration_since(start) {
        Some(d) => d.as_secs_f32(),
        None => -start.duration_since(tick).as_secs_f32(),
    };
    let glow: std::collections::HashMap<usize, f32> =
        app.ripples.iter().map(|(&id, r)| (id, pulse(since(r.start)) * r.strength)).collect();

    // Camera: anchored to the cursor column's left edge (not the label's
    // center, which would pan on every j/k to a name of another length).
    let (cx, cy, cw) = lay.cursor;
    let mut tx = cx as f32 - canvas.width as f32 * 0.38;
    // Held after the wheel took over another column: stay put, only nudging
    // enough to keep the cursor's pill on screen.
    if let Some(h) = &mut app.cam_hold {
        *h = h.max((cx + cw + 1) as f32 - canvas.width as f32).min((cx - 2) as f32);
        tx = *h;
    }
    app.cam_tx = tx;
    let ty = cy as f32 - canvas.height as f32 / 2.0;
    let cam = app.cam.get_or_insert((Damped::new(tx), Damped::new(ty)));
    moving |= cam.0.step(tx, CAM, dt);
    moving |= cam.1.step(ty, CAM, dt);
    let (ox, oy) = (cam.0.v.round() as i32, cam.1.v.round() as i32);

    // The selector is pinned to the line (the cursor's target slot); items
    // slide into it and the camera pan carries horizontal motion, so the
    // selector itself never animates.
    let (pill_x, pill_y, pill_w) = (cx, cy, cw);

    let buf = f.buffer_mut();
    tint(buf, canvas, gr().bg);

    app.cols = lay.cols.iter().map(|&(lo, hi, id)| (lo - ox, hi - ox, id)).collect();

    // Hover cue: a faint pill on the column the wheel would take over.
    if let Some(p) = app.hover().and_then(|id| lay.placed.iter().find(|p| p.id == id)) {
        let (w, sy) = (p.label.width() as i32, p.y - oy);
        for x in (p.x - 2)..=(p.x + w) {
            let sx = x - ox;
            if sx >= 0 && sy >= 0 && sx < canvas.width as i32 && sy < canvas.height as i32 {
                buf[(canvas.x + sx as u16, canvas.y + sy as u16)].set_bg(to_color(mix(acc().pill, gr().bg, 0.55)));
            }
        }
    }

    // Pill first (bg only), so lines and labels draw over it.
    for x in (pill_x - 2)..=(pill_x + pill_w) {
        let (sx, sy) = (x - ox, pill_y - oy);
        if sx >= 0 && sy >= 0 && sx < canvas.width as i32 && sy < canvas.height as i32 {
            buf[(canvas.x + sx as u16, canvas.y + sy as u16)].set_bg(to_color(acc().pill));
        }
    }

    let scene = &app.scene;
    let pos = |id: usize| {
        scene.nodes.get(&id).filter(|a| !a.ghost && a.alpha > 0.05).map(|a| {
            let (x, y) = a.pos();
            (x, y, a.w)
        })
    };
    let spine: HashSet<usize> = crate::layout::spine(&app.tree, app.cursor).into_iter().collect();
    // Sap: each branch is tinted by the recursive heat of the dir it grows
    // from, so recent work glows through the wiring.
    let now = SystemTime::now();
    let mut sap: std::collections::HashMap<usize, Rgb> = std::collections::HashMap::new();
    let tree = &app.tree;
    let mut sap_of = |owner: usize| {
        *sap.entry(owner).or_insert_with(|| heat(now.duration_since(tree.heat(owner)).unwrap_or_default().as_secs_f32()))
    };
    // A ripple crossing the elbow from a node up to its folder, lit halfway
    // between the two: (folder, x range between their labels, the two rows, glow).
    let wires: Vec<(usize, i32, i32, i32, i32, f32)> = app
        .ripples
        .iter()
        .filter_map(|(&id, r)| {
            let p = tree.nodes[id].parent?;
            let ((cx, cy, _), (px, py, pw)) = (pos(id)?, pos(p)?);
            let g = pulse(since(r.start + crate::RIPPLE_STEP / 2)) * r.strength;
            (g > 0.01).then_some((p, px + pw, cx, py, cy, g))
        })
        .collect();
    let ignored = |id: usize| app.settings.dim_ignored && app.git_state(id) == Some(St::Ignored);
    for ((x, y), cell) in lines(&lay.blocks, &pos, &route, &spine, &ignored) {
        let (sx, sy) = (x - ox, y - oy);
        if sx < 0 || sy < 0 || sx >= canvas.width as i32 || sy >= canvas.height as i32 {
            continue;
        }
        let h = sap_of(cell.owner);
        let c = match cell.emph {
            ROUTE => mix(acc().route, h, 0.18),
            ACTIVE => mix(acc().active, h, 0.42),
            _ => mix(acc().dim, mix(h, gr().bg, 0.55), 0.35),
        };
        // Only this branch's elbow: its two rows, and the trunk between them.
        let lit = wires.iter().filter(|w| {
            w.0 == cell.owner
                && (w.1..w.2).contains(&x)
                && (w.3.min(w.4)..=w.3.max(w.4)).contains(&y)
                && (y == w.3 || y == w.4 || cell.mask & (UP | DOWN) != 0)
        });
        let c = match lit.map(|w| w.5).reduce(f32::max) {
            Some(g) => mix(c, gr().ripple, 0.9 * g),
            None => c,
        };
        buf[(canvas.x + sx as u16, canvas.y + sy as u16)].set_char(cell_glyph(&cell)).set_fg(to_color(c));
    }

    // Ghosts under live nodes.
    let mut order: Vec<(&usize, &crate::anim::NodeAnim)> = scene.nodes.iter().collect();
    order.sort_by_key(|(_, a)| !a.ghost);
    app.hits.clear();
    let mut visible_dirs = Vec::new();
    // While finding, the matched text lights up in the cursor's column.
    let finding = app.search.as_ref().map(|s| s.query.as_str()).filter(|q| !q.is_empty());
    let found = finding.map(|q| app.matches(q, &app.siblings())).unwrap_or_default();
    for (&id, a) in order {
        let (x, y) = a.pos();
        let (sx, sy) = (x - ox, y - oy);
        if sy < 0 || sy >= canvas.height as i32 || sx + a.w < 0 || sx >= canvas.width as i32 {
            continue;
        }
        let node = &app.tree.nodes[id];
        let g = if a.ghost { 0.0 } else { glow.get(&id).copied().unwrap_or(0.0) };
        let git = if a.ghost { None } else { app.git_state(id) };
        // Ignored by git: grey at the dim floor's lightness, unless it's on the cursor path.
        let quiet = git == Some(St::Ignored) && app.settings.dim_ignored && !route.contains(&id);
        let rgb = if quiet { ignored_shade(gr(), a.rgb, app.settings.dim_floor) } else { a.rgb };
        let mut style = Style::new().fg(to_color(mix(gr().bg, mix(rgb, gr().ripple, g), a.alpha)));
        if node.is_dir || route.contains(&id) {
            style = style.add_modifier(Modifier::BOLD);
        }
        // The pill keeps the cursor's row; everywhere else an ember shows behind the name.
        if g > 0.02 && id != app.cursor {
            style = style.bg(to_color(mix(gr().bg, gr().ripple_bg, g * a.alpha)));
        }
        let (name, details) = a.label.split_at(a.name.min(a.label.len()));
        put(buf, canvas, sx, sy, name, style);
        if !details.is_empty() {
            let st = Style::new().fg(to_color(mix(gr().bg, rgb, a.alpha * 0.5)));
            put(buf, canvas, sx + name.width() as i32, sy, details, st);
        }
        if let Some((_, spans)) = finding.filter(|_| !a.ghost && found.contains(&id)).and_then(|q| crate::fuzzy(name, q)) {
            let lit = Style::new().fg(to_color(mix(gr().bg, gr().flash, a.alpha))).bg(to_color(mix(gr().bg, gr().match_bg, a.alpha)));
            for r in spans {
                put(buf, canvas, sx + a.label[..r.start].width() as i32, sy, &a.label[r], lit.add_modifier(Modifier::BOLD));
            }
        }
        // Bud: a closed folder that may still hold something.
        let bud = node.is_dir && !node.expanded && node.children.as_ref().is_none_or(|k| !k.is_empty());
        // Git: a file's marker sits where a folder's bud would; a closed
        // folder's bud takes the loudest state inside it.
        let git = git.filter(|&s| s != St::Ignored);
        let spinning = app.exploding.as_ref().filter(|x| x.target == id && !a.ghost);
        if let Some(x) = spinning {
            // `e` at work: the bud turns into a spinner until everything unfurls.
            let st = Style::new().fg(to_color(mix(gr().bg, acc().route, a.alpha))).add_modifier(Modifier::BOLD);
            put(buf, canvas, sx + a.w + 1, sy, spinner(x.born), st);
        } else if bud && !a.ghost {
            let c = match git {
                Some(s) => mix(gr().bg, git_color(s), a.alpha),
                None => mix(gr().bg, rgb, a.alpha * 0.55),
            };
            put(buf, canvas, sx + a.w + 1, sy, "›", Style::new().fg(to_color(c)));
        } else if let (Some(s), false) = (git, node.is_dir) {
            let st = Style::new().fg(to_color(mix(gr().bg, git_color(s), a.alpha))).add_modifier(Modifier::BOLD);
            put(buf, canvas, sx + a.w + 1, sy, s.glyph(), st);
        }
        if !a.ghost {
            app.hits.push((sx, sy, a.w, id));
            if node.is_dir {
                visible_dirs.push(node.path.clone());
            }
        }
    }
    // Signal bead: on every move a light sweeps along the line into the
    // cursor, lighting wire and labels as it passes, with a motion-blur tail
    // whose length follows its speed. Drawn last so it passes over labels.
    if app.last_cursor.is_some_and(|(id, _)| id != app.cursor) {
        let (_, lx) = app.last_cursor.unwrap();
        let start = if lx != cx { lx - 2 } else { cx - 10 };
        app.bead = Some(Damped::new(start as f32));
    }
    app.last_cursor = Some((app.cursor, cx));
    let bead_target = (cx - 2) as f32;
    if let Some(b) = &mut app.bead {
        let running = b.step(bead_target, 0.13, dt);
        moving |= running;
        let head = b.v.round() as i32;
        let dir = if b.vel >= 0.0 { -1 } else { 1 };
        let tail = (b.vel.abs() * 0.05).clamp(1.0, 12.0) as i32;
        let row = cy - oy;
        for i in 0..=tail {
            let x = head + dir * i - ox;
            if x < 0 || x >= canvas.width as i32 || row < 0 || row >= canvas.height as i32 {
                continue;
            }
            let fade = 1.0 - i as f32 / (tail + 1) as f32;
            let cellref = &mut buf[(canvas.x + x as u16, canvas.y + row as u16)];
            if cellref.symbol() == " " {
                continue; // gaps stay dark; the light rides wire and names
            }
            cellref.set_fg(to_color(mix(acc().route, gr().flash, fade)));
            if i == 0 {
                cellref.set_bg(to_color(mix(gr().bg, acc().route, 0.35)));
            }
        }
        if !running {
            app.bead = None;
        }
    }

    for d in visible_dirs {
        app.mt.request(&d);
    }

    status_bar(f, app, status);

    // Help overlay.
    moving |= approach(&mut app.help_anim, if app.help { 1.0 } else { 0.0 }, 0.06, dt);
    if app.help_anim > 0.01 {
        help(f, app.help_anim);
    }
    moving |= app.exploding.is_some();
    moving |= approach(&mut app.menu_anim, if app.menu.is_some() { 1.0 } else { 0.0 }, 0.05, dt);
    if app.menu_anim > 0.01 {
        settings_panel(f, app, canvas, app.menu_anim);
    }

    // Preview popup: grows out of the cursor row, backdrop dims behind it.
    let full = popup(f.area());
    let fx = (pill_x - 2 - ox).clamp(0, canvas.width as i32 - 1) as u16;
    let from = Rect {
        x: canvas.x + fx,
        y: canvas.y + (pill_y - oy).clamp(0, canvas.height as i32 - 1) as u16,
        // Right after a shrink the camera still frames the old width: keep it on screen.
        width: (pill_w as u16 + 3).min(canvas.width - fx),
        height: 1,
    };
    if let Some(pv) = &mut app.preview {
        if let Some(rx) = &pv.pending {
            match rx.try_recv() {
                Ok(t) => (pv.text, pv.pending) = (t, None),
                Err(TryRecvError::Empty) => moving = true,
                Err(TryRecvError::Disconnected) => {
                    (pv.text, pv.pending) = (Text::from(vec![Line::from(""), Line::from("  (preview failed)")]), None)
                }
            }
        }
        if let Some(a) = &mut pv.audio {
            // The sound stops as the popup starts to fold, not after.
            if pv.closing {
                a.pause();
            }
            a.poll();
        }
        let target = if pv.closing { 0.0 } else { 1.0 };
        moving |= pv.open.step(target, POPUP_T, dt);
        let p = pv.open.v.clamp(0.0, 1.0);
        let area = lerp_rect(from, full, p);
        pv.page = full.height.saturating_sub(2);
        let max = (pv.text.lines.len() as i32 - pv.page as i32).max(0);
        pv.scroll = pv.scroll.clamp(0, max);
        moving |= pv.sy.step(pv.scroll as f32, 0.07, dt);
        dim_backdrop(f.buffer_mut(), area, 0.55 * p);
        if area.width > 4 && area.height > 2 {
            let mode = app.picker.as_ref().map(|p| format!("i {} · ", crate::media::label(p))).unwrap_or_default();
            let pos_label = match (&pv.media, &pv.audio) {
                (_, Some(a)) => match a.total() {
                    Some(t) => format!(" {} / {} ", audio::clock(a.pos()), audio::clock(t)),
                    None => format!(" {} ", audio::clock(a.pos())),
                },
                (Some(m), _) if m.pages > 1 => format!(" {mode}page {}/{} ", m.page + 1, m.pages),
                (Some(m), _) => m.dims.map(|(w, h)| format!(" {mode}{w}×{h} ")).unwrap_or_default(),
                (None, None) => {
                    let d = match (&pv.diff, pv.showing_diff) {
                        (None, _) => "",
                        (Some(_), false) => "d diff · ",
                        (Some(_), true) => "d file · ",
                    };
                    format!(" {d}{}/{} ", pv.scroll + 1, pv.text.lines.len().max(1))
                }
            };
            let title = if pv.showing_diff { format!(" {} · diff ", pv.title) } else { format!(" {} ", pv.title) };
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(to_color(mix(gr().pop, acc().route, p))))
                .style(Style::new().fg(to_color(gr().text)).bg(to_color(gr().pop)))
                .title(Span::styled(title, Style::new().fg(to_color(gr().text)).add_modifier(Modifier::BOLD)))
                .title_bottom(Line::from(Span::styled(pos_label, Style::new().fg(to_color(gr().muted)))).right_aligned());
            f.render_widget(Clear, area);
            // Pixels only once the popup has landed: graphics protocols can't follow the grow animation.
            let settled = p > 0.97 && !pv.closing;
            if let Some(a) = &mut pv.audio {
                f.render_widget(block, area);
                a.bar = Rect::default();
                if p > 0.9 {
                    audio_panel(f.buffer_mut(), a, area.inner(Margin { vertical: 1, horizontal: 2 }));
                }
            } else if let (Some(m), Some(picker), true) = (&mut pv.media, &app.picker, settled) {
                f.render_widget(block, area);
                if m.err.is_none() {
                    m.render(f, area.inner(Margin { vertical: 1, horizontal: 1 }), picker, gr().pop);
                } else {
                    f.render_widget(Paragraph::new(pv.text.clone()), area.inner(Margin { vertical: 1, horizontal: 1 }));
                }
                app.graphics_shown = true;
            } else {
                let sy = pv.sy.v.round().max(0.0) as u16;
                if pv.pending.is_some() {
                    let spin = spinner(pv.born);
                    let row = Line::from(vec![
                        Span::styled(format!("{spin} "), Style::new().fg(to_color(acc().route))),
                        Span::styled(pv.loading, Style::new().fg(to_color(gr().muted))),
                    ])
                    .centered();
                    let pad = vec![Line::from(""); (area.height as usize).saturating_sub(3) / 2];
                    let text = Text::from(pad.into_iter().chain([row]).collect::<Vec<_>>());
                    f.render_widget(Paragraph::new(text).block(block), area);
                } else {
                    // Only the visible rows: cloning a whole long file every frame is what lags.
                    let rows = area.height.saturating_sub(2) as usize;
                    let lines: Vec<Line> = pv.text.lines.iter().skip(sy as usize).take(rows).cloned().collect();
                    f.render_widget(Paragraph::new(lines).block(block), area);
                }
                if max > 0 && p > 0.9 && pv.media.is_none() && pv.audio.is_none() {
                    let mut st = ScrollbarState::new(max as usize).position(sy as usize);
                    f.render_stateful_widget(
                        Scrollbar::new(ScrollbarOrientation::VerticalRight)
                            .begin_symbol(None)
                            .end_symbol(None)
                            .track_symbol(Some("│"))
                            .track_style(Style::new().fg(to_color(acc().dim)))
                            .thumb_symbol("┃")
                            .thumb_style(Style::new().fg(to_color(acc().route))),
                        area.inner(Margin { vertical: 1, horizontal: 0 }),
                        &mut st,
                    );
                }
            }
        }
    }
    if app.preview.as_ref().is_some_and(|p| p.closing && p.open.v < 0.03) {
        app.preview = None;
        // Sixel/iTerm2 pixels can outlive the cells they were drawn over.
        app.repaint |= std::mem::take(&mut app.graphics_shown);
    }
    app.lay = Some((epoch, lay));
    moving
}

fn status_bar(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    tint(buf, area, gr().bar);
    if let Some(line) = &app.prompt {
        return prompt_bar(f, app, area, line);
    }
    if let Some(s) = &app.search {
        return search_bar(f, app, area, &s.query);
    }
    let cur = &app.tree.nodes[app.cursor];
    let t = app.heat_of(app.cursor);
    let partial = cur.rec.is_some_and(|c| !c.1);
    let meta = if cur.is_dir {
        let items = match &cur.children {
            Some(_) => format!("{} items", app.tree.kids(app.cursor).len()),
            None => "folder".into(),
        };
        match cur.rec {
            Some(r) => format!("{items} · {}{}", human(r.3), if r.1 { "" } else { "+" }),
            None => items,
        }
    } else {
        human(cur.size)
    };

    // Right side: explode · note · git · sort · meta · age swatch · legend · help key.
    let muted = Style::new().fg(to_color(gr().muted));
    let mut right = Vec::new();
    if let Some(x) = &app.exploding {
        right.push(Span::styled(format!("{} ", spinner(x.born)), Style::new().fg(to_color(acc().route))));
        let n = if x.folders == 0 { "reading".into() } else { format!("{} folders", x.folders) };
        right.push(Span::styled(format!("exploding · {n} · "), Style::new().fg(to_color(gr().text))));
        right.push(Span::styled("esc", Style::new().fg(to_color(acc().route)).add_modifier(Modifier::BOLD)));
        right.push(Span::styled(" stops · ", muted));
    } else if let Some((msg, _)) = app.note.as_ref().filter(|n| n.1.elapsed() < crate::NOTE) {
        right.push(Span::styled(format!("{msg} · "), Style::new().fg(to_color(gr().text))));
    }
    if let Some((top, repo)) = app.git.as_ref().and_then(|g| g.repo_of(&cur.path)) {
        right.push(Span::styled("⎇ ", Style::new().fg(to_color(acc().route))));
        right.push(Span::styled(format!("{} · ", repo.branch), Style::new().fg(to_color(gr().text))));
        if let Some(st) = repo.get(top, &cur.path) {
            right.push(Span::styled(format!("{} · ", st.describe()), Style::new().fg(to_color(git_color(st)))));
        }
    }
    if app.tree.sort != Sort::default() {
        right.push(Span::styled("⇅ ", Style::new().fg(to_color(acc().route))));
        right.push(Span::styled(format!("{} · ", app.tree.sort.describe()), Style::new().fg(to_color(gr().text))));
    }
    right.extend([
        Span::styled(format!("{meta} · "), muted),
        Span::styled("● ", Style::new().fg(to_color(heat(age_of(t))))),
        Span::styled(format!("{}{}", ago(t), if partial { " (partial)" } else { "" }), Style::new().fg(to_color(gr().text))),
        Span::styled("   ", muted),
    ]);
    // Tight bar: the color legend goes first (it's in `?` too), then the shell keys,
    // so the breadcrumb keeps room.
    let key = Style::new().fg(to_color(acc().route)).add_modifier(Modifier::BOLD);
    let q = if app.can_cd { " cd  " } else { " quit  " };
    let hints = [("!", " cmd  "), ("s", " shell  "), ("q", q), ("?", " keys ")];
    let hw: usize = hints.iter().map(|(k, d)| k.width() + d.width()).sum();
    let legend_w = "now ".len() + heat_stops().len() + " old   ".len();
    let base: usize = right.iter().map(|s| s.width()).sum::<usize>() + 24;
    let w = area.width as usize;
    let (legend, shown) = if w >= base + legend_w + hw {
        (true, &hints[..])
    } else if w >= base + hw {
        (false, &hints[..])
    } else {
        // Still tight: the legend goes too rather than squeezing the name.
        (w >= base + legend_w + hints[3].0.width() + hints[3].1.width(), &hints[3..])
    };
    if legend && app.settings.legend {
        right.push(Span::styled("now ", muted));
        for &(_, c) in heat_stops() {
            right.push(Span::styled("▮", Style::new().fg(to_color(c))));
        }
        right.push(Span::styled(" old   ", muted));
    }
    for (k, d) in shown {
        right.push(Span::styled(*k, key));
        right.push(Span::styled(*d, muted));
    }
    let rw: usize = right.iter().map(|s| s.width()).sum();

    // Left side: breadcrumb root › … › leaf, trimmed from the front.
    let names: Vec<String> = app.tree.path_to(app.cursor).iter().map(|&i| app.tree.nodes[i].name.clone()).collect();
    let room = (area.width as usize).saturating_sub(rw + 2);
    let mut start = 0;
    let width_from = |s: usize| names[s..].iter().map(|n| n.width() + 3).sum::<usize>() + if s > 0 { 4 } else { 0 };
    while start + 1 < names.len() && width_from(start) > room {
        start += 1;
    }
    let mut left = vec![Span::raw(" ")];
    if start > 0 {
        left.push(Span::styled("… › ", muted));
    }
    for (i, n) in names[start..].iter().enumerate() {
        let leaf = start + i == names.len() - 1;
        if i > 0 {
            left.push(Span::styled(" › ", Style::new().fg(to_color(acc().active))));
        }
        left.push(if leaf {
            Span::styled(n.clone(), Style::new().fg(to_color(gr().text)).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(n.clone(), muted)
        });
    }
    f.render_widget(Paragraph::new(Line::from(left)), area);
    if area.width as usize > rw {
        let r = Rect { x: area.right() - rw as u16, width: rw as u16, ..area };
        f.render_widget(Paragraph::new(Line::from(right)), r);
    }
}

/// `!` command line: `folder $ text█`, scrolled so the end stays visible.
fn prompt_bar(f: &mut Frame, app: &App, area: Rect, line: &str) {
    let dir = app.work_dir();
    let name = dir.file_name().map_or_else(|| dir.to_string_lossy(), |n| n.to_string_lossy());
    let head = format!(" {name} $ ");
    let hint = "  enter run · esc cancel · $f = selection ";
    let room = (area.width as usize).saturating_sub(head.width() + 1 + hint.width());
    let mut shown = line;
    while shown.width() > room {
        let mut c = shown.chars();
        c.next();
        shown = c.as_str();
    }
    let mut spans = vec![
        Span::styled(head, Style::new().fg(to_color(acc().route)).add_modifier(Modifier::BOLD)),
        Span::styled(shown, Style::new().fg(to_color(gr().text))),
        Span::styled(" ", Style::new().add_modifier(Modifier::REVERSED)),
    ];
    if room > 0 {
        spans.push(Span::styled(hint, Style::new().fg(to_color(gr().muted))));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// `/` query: `/ text█  2/5`, or "no match" when nothing in the column matches it.
fn search_bar(f: &mut Frame, app: &App, area: Rect, q: &str) {
    let (at, n) = app.match_pos(q);
    let count = match (q.is_empty(), n) {
        (true, _) => String::new(),
        (false, 0) => "  no match".into(),
        _ => format!("  {at}/{n}"),
    };
    let hint = if q.is_empty() || n > 0 { "  tab ↑↓ cycle · enter stay · esc back " } else { "  esc back " };
    let spans = vec![
        Span::styled(" / ", Style::new().fg(to_color(acc().route)).add_modifier(Modifier::BOLD)),
        Span::styled(q, Style::new().fg(to_color(gr().text))),
        Span::styled(" ", Style::new().add_modifier(Modifier::REVERSED)),
        Span::styled(count, Style::new().fg(to_color(if n > 0 { acc().route } else { gr().dot }))),
        Span::styled(hint, Style::new().fg(to_color(gr().muted))),
    ];
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

const KEYS: [(&str, &str); 24] = [
    ("h j k l / arrows", "move · j k walk the open tree, ↑ ↓ the column"),
    ("l / enter", "open folder · preview file"),
    ("space / tab", "fold / unfold"),
    ("J K / pgup pgdn", "jump 10 (pgup pgdn in the column)"),
    ("g G", "first / last sibling"),
    ("/ n N", "fuzzy find in column · next / previous"),
    ("tab ↑↓ while /", "cycle matches · Caps = exact case"),
    ("-", "re-root one level up"),
    ("c C", "fold this folder · collapse other branches"),
    ("e", "explode: open every folder inside · esc stops"),
    (".", "show / hide dotfiles"),
    ("o O", "sort: name · newest · largest · type · reverse"),
    ("r", "reload (open folders update live)"),
    ("mouse", "click select · click again open"),
    ("wheel", "scroll the column under the pointer"),
    ("preview", "j k · space · ctrl-d/u · g G · q"),
    ("d in a preview", "git diff ⇄ file · M + ? ! marks"),
    ("image / pdf", "j k page · ↑ ↓ image · i pixels ⇄ blocks"),
    ("audio", "space pause · ← → seek · ↑ ↓ volume · 0-9"),
    ("!", "run a command here ($f = selection)"),
    ("s", "shell here · exit / ctrl-d returns"),
    (",", "settings: layout · colors · mouse · previews · …"),
    ("?", "toggle this help"),
    ("q / esc", "quit (q + tb.bash: cd there)"),
];

/// Settings menu, docked on the right so the tree stays in view and every
/// change shows as it's made. Slides in with `t`.
fn settings_panel(f: &mut Frame, app: &App, canvas: Rect, t: f32) {
    use crate::settings::ITEMS;
    let w = 52u16.min(canvas.width);
    let x = canvas.right() - (w as f32 * t).round() as u16;
    let r = Rect { x, y: canvas.y, width: canvas.right() - x, height: canvas.height };
    if r.width < 3 || r.height < 3 {
        return;
    }
    let fade = |c: Rgb| to_color(mix(gr().pop, c, t));
    let inner = (w as usize).saturating_sub(4);
    let sel = app.menu.unwrap_or(0);

    let mut body: Vec<Line> = Vec::new();
    let mut sel_line = 0;
    let mut section = "";
    for (i, it) in ITEMS.iter().enumerate() {
        if it.section != section {
            section = it.section;
            if !body.is_empty() {
                body.push(Line::from(""));
            }
            body.push(Line::from(Span::styled(format!(" {}", section.to_uppercase()), Style::new().fg(fade(gr().muted)).add_modifier(Modifier::BOLD))));
        }
        let on = i == sel;
        if on {
            sel_line = body.len();
        }
        let value = app.settings.show(it.key);
        // A swatch next to the choices that are colors.
        let swatch: Vec<Span> = match it.key {
            "accent" => vec![Span::styled(" ━━", Style::new().fg(fade(acc().route)))],
            "palette" => {
                let mut v = vec![Span::raw(" ")];
                v.extend(heat_stops().iter().map(|&(_, c)| Span::styled("▮", Style::new().fg(fade(c)))));
                v
            }
            _ => Vec::new(),
        };
        let sw: usize = swatch.iter().map(|s| s.width()).sum();
        let (l, rr) = if on { ("‹ ", " ›") } else { ("  ", "  ") };
        let vw = value.width() + 4 + sw;
        let pad = inner.saturating_sub(it.label.width() + 2 + vw);
        let bg = if on { mix(gr().pop, acc().pill, t) } else { gr().pop };
        let base = Style::new().bg(to_color(bg));
        let label = if on { base.fg(fade(gr().text)).add_modifier(Modifier::BOLD) } else { base.fg(fade(mix(gr().muted, gr().text, 0.5))) };
        let mut spans = vec![
            Span::styled(format!("  {}", it.label), label),
            Span::styled(" ".repeat(pad), base),
            Span::styled(l, base.fg(fade(acc().route))),
            Span::styled(value, base.fg(fade(gr().text))),
        ];
        spans.extend(swatch.into_iter().map(|s| s.patch_style(base)));
        spans.push(Span::styled(rr, base.fg(fade(acc().route))));
        body.push(Line::from(spans));
    }

    // Footer: what the selected setting does, any trouble with the file, where it lives, keys.
    let mut foot: Vec<Line> = vec![Line::from("")];
    let mut words = String::new();
    for word in ITEMS[sel].help.split(' ') {
        if !words.is_empty() && words.width() + 1 + word.width() > inner {
            foot.push(Line::from(Span::styled(format!("  {words}"), Style::new().fg(fade(gr().text)))));
            words.clear();
        }
        if !words.is_empty() {
            words.push(' ');
        }
        words.push_str(word);
    }
    foot.push(Line::from(Span::styled(format!("  {words}"), Style::new().fg(fade(gr().text)))));
    foot.push(Line::from(""));
    let trouble = app.save_err.iter().chain(app.config_errs.iter().take(2));
    for e in trouble {
        foot.push(Line::from(Span::styled(format!("  {}", crate::layout::truncate_to(e, inner)), Style::new().fg(fade(gr().dot)))));
    }
    let place = match &app.config {
        Some(p) => tilde(p),
        None => "not saved: no home directory".into(),
    };
    foot.push(Line::from(Span::styled(format!("  {}", crate::layout::truncate_to(&place, inner)), Style::new().fg(fade(gr().muted)))));
    let key = Style::new().fg(fade(acc().route)).add_modifier(Modifier::BOLD);
    let m = Style::new().fg(fade(gr().muted));
    foot.push(Line::from(vec![
        Span::styled("  j k", key),
        Span::styled(" move  ", m),
        Span::styled("h l", key),
        Span::styled(" change  ", m),
        Span::styled("r", key),
        Span::styled(" reset  ", m),
        Span::styled("esc", key),
        Span::styled(" close", m),
    ]));

    // Scroll the list so the selection stays in view above the footer.
    let rows = (r.height as usize).saturating_sub(2);
    let room = rows.saturating_sub(foot.len()).max(1);
    let skip = (sel_line + 1).saturating_sub(room).min(body.len().saturating_sub(room));
    let mut lines: Vec<Line> = body.into_iter().skip(skip).take(room).collect();
    while lines.len() + foot.len() < rows {
        lines.push(Line::from(""));
    }
    lines.extend(foot);

    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(fade(acc().route)))
                .style(Style::new().fg(to_color(gr().text)).bg(to_color(gr().pop)))
                .title(Span::styled(" settings ", Style::new().fg(fade(gr().text)).add_modifier(Modifier::BOLD))),
        ),
        r,
    );
}

fn help(f: &mut Frame, t: f32) {
    let w = 64u16.min(f.area().width);
    let full_h = (KEYS.len() as u16 + 6).min(f.area().height);
    let h = ((full_h as f32) * t).round().max(1.0) as u16;
    let a = f.area();
    let r = Rect { x: a.x + (a.width - w) / 2, y: a.y + (a.height - full_h) / 2, width: w, height: h };
    let fade = |c: Rgb| to_color(mix(gr().pop, c, t));
    let mut lines = vec![Line::from("")];
    for (k, d) in KEYS {
        lines.push(Line::from(vec![
            Span::styled(format!("  {k:>17}  "), Style::new().fg(fade(acc().route)).add_modifier(Modifier::BOLD)),
            Span::styled(d, Style::new().fg(fade(gr().text))),
        ]));
    }
    lines.push(Line::from(""));
    let mut legend = vec![Span::styled("  color = last change inside  now ", Style::new().fg(fade(gr().muted)))];
    for &(_, c) in heat_stops() {
        legend.push(Span::styled("▮", Style::new().fg(fade(c))));
    }
    legend.push(Span::styled(" 5y", Style::new().fg(fade(gr().muted))));
    lines.push(Line::from(legend));
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(fade(acc().route)))
                .style(Style::new().fg(to_color(gr().text)).bg(to_color(gr().pop)))
                .title(Span::styled(" treebeard ", Style::new().fg(fade(gr().text)).add_modifier(Modifier::BOLD))),
        ),
        r,
    );
}

const EIGHTHS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Audio popup body: state and format, the waveform with the played part lit,
/// a scrubber under it, times, volume, and the keys. Records where the
/// waveform and scrubber landed so the mouse can seek on them.
fn audio_panel(buf: &mut Buffer, a: &mut Audio, r: Rect) {
    if r.width < 12 || r.height < 3 {
        return;
    }
    let muted = Style::new().fg(to_color(gr().muted));
    let wave_h = r.height.saturating_sub(9).clamp(1, 10);
    // Header, gap, waveform, scrubber, times, gap, volume, gap, keys.
    let body = wave_h + 8;
    let mut y = r.y + r.height.saturating_sub(body) / 2;
    let w = r.width as usize;
    let put_line = |buf: &mut Buffer, y: u16, spans: Vec<Span>| {
        if y < r.bottom() {
            buf.set_line(r.x, y, &Line::from(spans), r.width);
        }
    };
    let right = |buf: &mut Buffer, y: u16, text: &str, st: Style| {
        let tw = text.width() as u16;
        if y < r.bottom() && tw < r.width {
            buf.set_string(r.right() - tw, y, text, st);
        }
    };

    let state = a.state();
    let (glyph, word, color) = match state {
        State::Playing => ("▶", "playing", acc().route),
        State::Paused => ("‖", "paused", gr().text),
        State::Ended => ("■", "ended · space plays again", gr().muted),
        State::Silent => ("·", "silent", gr().dot),
    };
    let bold = Style::new().fg(to_color(color)).add_modifier(Modifier::BOLD);
    put_line(buf, y, vec![Span::styled(format!("{glyph} "), bold), Span::styled(word, bold)]);
    if word.width() + a.info.width() + 4 <= w {
        right(buf, y, &a.info, muted);
    }
    y += 1;
    if let Some(e) = &a.err {
        put_line(buf, y, vec![Span::styled(e.clone(), Style::new().fg(to_color(gr().dot)))]);
    }
    y += 1;

    // Waveform: eighth-block bars from the bottom, one column per slice of the file.
    let total_peaks = a.total().map(|t| (t.as_secs_f32() / audio::WAVE_STEP).ceil() as usize);
    let prog = a.progress();
    let head = (prog * (w - 1) as f32).round() as usize;
    a.bar = Rect { x: r.x, y, width: r.width, height: wave_h + 1 };
    match total_peaks {
        Some(n) if !a.wave.is_empty() || a.wave_done => {
            let cols = audio::columns(&a.wave, n, w);
            let read = if a.wave_done { w } else { (a.wave.len() * w / n.max(1)).min(w) };
            let played = to_color(mix(acc().route, gr().flash, 0.25));
            let ahead = to_color(mix(acc().active, gr().pop, 0.2));
            for (x, v) in cols.iter().enumerate() {
                let level = if x < read { ((v * (wave_h * 8) as f32).round() as usize).max(1) } else { 0 };
                let fg = if x == head && state != State::Silent {
                    to_color(gr().flash)
                } else if x < head {
                    played
                } else {
                    ahead
                };
                for row in 0..wave_h as usize {
                    let fill = level.saturating_sub(row * 8).min(8);
                    let cy = y + wave_h - 1 - row as u16;
                    if cy >= r.bottom() {
                        continue;
                    }
                    let ch = if fill == 0 && row == 0 && x >= read { '·' } else { EIGHTHS[fill] };
                    let fg = if x >= read { to_color(acc().dim) } else { fg };
                    buf[(r.x + x as u16, cy)].set_char(ch).set_fg(fg);
                }
            }
        }
        _ => {
            let msg = if a.wave_done { "no waveform" } else { "reading waveform…" };
            let mid = y + (wave_h - 1) / 2;
            if mid < r.bottom() {
                buf.set_string(r.x + (r.width.saturating_sub(msg.width() as u16)) / 2, mid, msg, muted);
            }
        }
    }
    y += wave_h;

    // Scrubber.
    for x in (0..w).filter(|_| y < r.bottom()) {
        let (ch, c) = match a.total() {
            Some(_) if x < head => ('━', acc().route),
            Some(_) if x == head => ('●', gr().flash),
            _ => ('─', acc().dim),
        };
        buf[(r.x + x as u16, y)].set_char(ch).set_fg(to_color(c));
    }
    y += 1;
    put_line(buf, y, vec![Span::styled(audio::clock(a.pos()), Style::new().fg(to_color(gr().text)))]);
    if let Some(t) = a.total() {
        let left = t.saturating_sub(a.pos());
        right(buf, y, &format!("-{} / {}", audio::clock(left), audio::clock(t)), muted);
    }
    y += 2;

    // Volume.
    let mut vol = vec![Span::styled("vol ", muted)];
    if a.muted {
        vol.push(Span::styled("muted", Style::new().fg(to_color(gr().dot))));
    } else {
        let on = (a.volume * 10.0).round() as usize;
        vol.push(Span::styled("▮".repeat(on), Style::new().fg(to_color(acc().route))));
        vol.push(Span::styled("▮".repeat(10 - on), Style::new().fg(to_color(acc().dim))));
        vol.push(Span::styled(format!(" {}%", on * 10), muted));
    }
    put_line(buf, y, vol);
    y += 2;

    // Keys, dropping from the end until they fit.
    let key = Style::new().fg(to_color(acc().route)).add_modifier(Modifier::BOLD);
    let hints: &[(&str, &str)] = &[
        ("space", " pause  "),
        ("← →", " 5s  "),
        ("⇧← ⇧→", " 30s  "),
        ("0-9", " jump  "),
        ("↑ ↓", " volume  "),
        ("m", " mute  "),
        ("q", " close"),
    ];
    let mut n = hints.len();
    while n > 1 && hints[..n].iter().map(|(k, d)| k.width() + d.width()).sum::<usize>() > w {
        n -= 1;
    }
    let spans = hints[..n].iter().flat_map(|(k, d)| [Span::styled(*k, key), Span::styled(*d, muted)]).collect();
    put_line(buf, y, spans);
}

/// `path` with the home directory shown as `~`.
pub fn tilde(path: &std::path::Path) -> String {
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty()).map(std::path::PathBuf::from);
    match home.as_deref().and_then(|h| path.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// Braille spinner (ratatui's throbber set).
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The spinner frame for something that started at `born`.
fn spinner(born: Instant) -> &'static str {
    SPINNER[(born.elapsed().as_millis() / 80) as usize % SPINNER.len()]
}

pub fn popup(r: Rect) -> Rect {
    let w = (r.width as u32 * 9 / 10) as u16;
    let h = (r.height as u32 * 9 / 10) as u16;
    Rect { x: r.x + (r.width - w) / 2, y: r.y + (r.height - h) / 2, width: w, height: h }
}

pub struct Preview {
    pub title: String,
    pub text: Text<'static>,
    /// Target scroll row; `sy` eases toward it.
    pub scroll: i32,
    pub sy: Damped,
    pub page: u16,
    /// 0 = collapsed onto the cursor row, 1 = fully open.
    pub open: Damped,
    pub closing: bool,
    /// Image / PDF pages; `text` is then only the caption shown while it can't draw.
    pub media: Option<Media>,
    /// Sound files: playing while the popup is open.
    pub audio: Option<Audio>,
    /// Text still rendering on a worker (glow on a long file takes a while); spinner meanwhile.
    pub pending: Option<Receiver<Text<'static>>>,
    pub born: Instant,
    /// Spinner caption while pending.
    pub loading: &'static str,
    pub path: std::path::PathBuf,
    /// Repo top when git has changes for this file: `d` shows the diff.
    pub diff: Option<std::path::PathBuf>,
    pub showing_diff: bool,
    /// Whichever of file and diff isn't showing, once both exist.
    alt: Option<Text<'static>>,
}

/// The file's changes against HEAD (staged and not), colored by git.
fn diff_text(top: &Path, path: &Path) -> Text<'static> {
    let git = |args: &[&str]| {
        run(Command::new("git")
            .args(["--no-optional-locks", "-c", "color.diff=always", "diff"])
            .args(args)
            .arg("--")
            .arg(path)
            .current_dir(top)
            .stdin(Stdio::null())
            .stderr(Stdio::null()))
    };
    // No commits yet: there's no HEAD, so show what's staged.
    match git(&["HEAD"]).or_else(|| git(&["--cached"])) {
        Some(out) => out.into_text().unwrap_or_default(),
        None => Text::from(vec![Line::from(""), Line::from("  (no changes against HEAD)")]),
    }
}

fn run(cmd: &mut Command) -> Option<Vec<u8>> {
    let out = cmd.output().ok()?;
    (out.status.success() && !out.stdout.is_empty()).then_some(out.stdout)
}

/// Run `cmd` with `input` on stdin and a pseudo-terminal as stdout. glow only
/// uses its real 256-color palette when it sees a terminal; piped, it drops to 16 colors.
fn run_tty(mut cmd: Command, input: Vec<u8>, width: u16) -> Option<Vec<u8>> {
    let (mut m, mut s) = (0, 0);
    let mut ws = libc::winsize { ws_row: 50, ws_col: width, ws_xpixel: 0, ws_ypixel: 0 };
    unsafe {
        // *mut, not *const: macOS declares termp/winp mutable, Linux takes either.
        if libc::openpty(&mut m, &mut s, std::ptr::null_mut(), std::ptr::null_mut(), &raw mut ws) != 0 {
            return None;
        }
        // Raw: no \n -> \r\n translation, no echo.
        let mut t = std::mem::zeroed::<libc::termios>();
        libc::tcgetattr(s, &mut t);
        libc::cfmakeraw(&mut t);
        libc::tcsetattr(s, libc::TCSANOW, &t);
    }
    let (mut master, slave) = unsafe { (File::from_raw_fd(m), OwnedFd::from_raw_fd(s)) };
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::from(slave)).stderr(Stdio::null()).spawn().ok()?;
    // Our copy of the slave lives in `cmd`; drop it so the master sees EOF when the child exits.
    drop(cmd);
    let mut stdin = child.stdin.take()?;
    let feed = std::thread::spawn(move || stdin.write_all(&input));
    let mut out = Vec::new();
    // Linux reports the closed slave as EIO: that's the end of output, not a failure.
    let _ = master.read_to_end(&mut out);
    let _ = feed.join();
    let ok = child.wait().ok()?.success();
    (ok && !out.is_empty()).then_some(out)
}

fn is_md(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}

/// First 2 MiB of a file.
fn head(path: &Path) -> Vec<u8> {
    let mut v = Vec::new();
    if let Ok(f) = std::fs::File::open(path) {
        let _ = f.take(2 << 20).read_to_end(&mut v);
    }
    v
}

impl Preview {
    /// Whole file, colored: glow for markdown, bat otherwise, plain text as last resort.
    pub fn load(path: &Path, width: u16, how: TextPreview, wrap: bool) -> Preview {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let title = format!("{name} · {}", human(size));
        let mut first = Vec::new();
        if let Ok(f) = std::fs::File::open(path) {
            let _ = f.take(8192).read_to_end(&mut first);
        }
        let media = if size == 0 { None } else { Media::open(path) };
        let audio = if size == 0 || media.is_some() { None } else { Audio::open(path) };
        let mut pending = None;
        let text = if audio.is_some() {
            Text::default()
        } else if let Some(m) = &media {
            let what = match (&m.err, m.dims) {
                (Some(e), _) => format!("  can't render · {e}"),
                (None, Some((w, h))) => format!("  {w}×{h}"),
                (None, None) => String::new(),
            };
            Text::from(vec![Line::from(""), Line::from(what)])
        } else if size == 0 {
            Text::from(vec![Line::from(""), Line::from("  (empty file)")])
        } else {
            let (tx, rx) = mpsc::channel();
            let path = path.to_path_buf();
            std::thread::spawn(move || tx.send(Preview::render(&path, width, size, &first, how, wrap)));
            pending = Some(rx);
            Text::default()
        };
        Preview {
            title,
            text,
            scroll: 0,
            sy: Damped::new(0.0),
            page: 1,
            open: Damped::new(0.0),
            closing: false,
            media,
            audio,
            pending,
            born: Instant::now(),
            loading: if is_md(path) { "rendering markdown" } else { "loading" },
            path: path.to_path_buf(),
            diff: None,
            showing_diff: false,
            alt: None,
        }
    }

    /// `d`: flip between the file and its diff against HEAD.
    pub fn toggle_diff(&mut self) {
        let Some(top) = self.diff.clone() else { return };
        if self.pending.is_some() {
            return;
        }
        match self.alt.take() {
            Some(other) => self.alt = Some(std::mem::replace(&mut self.text, other)),
            None => {
                self.alt = Some(std::mem::take(&mut self.text));
                let (tx, rx) = mpsc::channel();
                let path = self.path.clone();
                std::thread::spawn(move || tx.send(diff_text(&top, &path)));
                self.pending = Some(rx);
                self.loading = "diffing";
                self.born = Instant::now();
            }
        }
        self.showing_diff ^= true;
        self.scroll = 0;
        self.sy = Damped::new(0.0);
    }

    /// Text files, colored: glow for markdown, bat otherwise, plain text as last
    /// resort (or as asked). Runs on a worker thread.
    fn render(path: &Path, width: u16, size: u64, first: &[u8], how: TextPreview, wrap: bool) -> Text<'static> {
        let w = width.saturating_sub(3).max(20);
        if first.contains(&0) {
            let kind = run(Command::new("file").arg("-b").arg(path))
                .map(|b| String::from_utf8_lossy(&b).trim().to_string())
                .unwrap_or_else(|| "binary".into());
            return Text::from(vec![
                Line::from(""),
                Line::from(format!("  binary file · {}", human(size))),
                Line::from(format!("  {kind}")),
            ]);
        }
        {
            // glow is a snap and can't open paths outside $HOME: feed it stdin.
            let glow = || {
                let mut cmd = Command::new("glow");
                cmd.args(["-s", if paper() { "light" } else { "dark" }, "-w", &w.to_string(), "-"]);
                run_tty(cmd, head(path), w)
            };
            let bat = || {
                let wrap = if wrap { "--wrap=character" } else { "--wrap=never" };
                let mut cmd = Command::new("bat");
                // Dark themes paint pale text that vanishes on parchment.
                if paper() {
                    cmd.arg("--theme=GitHub");
                }
                run(cmd
                    .args(["--color=always", "--style=numbers", "--paging=never", wrap])
                    .args(["--line-range", ":5000", "--terminal-width", &w.to_string()])
                    .arg(path))
            };
            let plain = || if wrap { wrap_text(&head(path), w as usize) } else { head(path) };
            let raw = match how {
                TextPreview::Styled if is_md(path) => glow().or_else(bat),
                TextPreview::Styled | TextPreview::Bat => bat(),
                TextPreview::Plain => None,
            }
            .unwrap_or_else(plain);
            let mut t = raw.into_text().unwrap_or_else(|_| Text::raw(String::from_utf8_lossy(&raw).into_owned()));
            // Reset/black backgrounds from glow/bat would punch holes in the popup.
            let clear = |st: &mut Style| {
                if matches!(st.bg, Some(Color::Reset | Color::Black)) {
                    st.bg = None;
                }
            };
            for line in &mut t.lines {
                clear(&mut line.style);
                for span in &mut line.spans {
                    clear(&mut span.style);
                }
            }
            t
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tty_output_is_raw_and_complete() {
        // 200 KiB through the pty: nothing lost, no \n -> \r\n translation.
        let input: Vec<u8> = (0..20_000).flat_map(|i| format!("line {i:05}\n").into_bytes()).collect();
        let out = run_tty(Command::new("cat"), input.clone(), 80).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn glow_gets_its_256_color_palette() {
        if Command::new("glow").arg("--version").output().is_err() {
            return;
        }
        let mut glow = Command::new("glow");
        glow.args(["-s", "dark", "-w", "60", "-"]);
        let out = run_tty(glow, b"# Title\n\nbody\n".to_vec(), 60);
        let out = String::from_utf8_lossy(&out.unwrap()).into_owned();
        assert!(out.contains("38;5;"), "expected 256-color escapes, got {out:?}");
    }

    /// sRGB channel (0-255) to linear light.
    fn linear(v: f32) -> f32 {
        let v = v / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    }

    fn luminance(c: Rgb) -> f32 {
        let [r, g, b] = c.map(linear);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// CIE L*a*b* of an sRGB color.
    fn lab(c: Rgb) -> [f32; 3] {
        let [r, g, b] = c.map(linear);
        let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
        let (x, y, z) = (
            f((0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047),
            f(luminance(c)),
            f((0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883),
        );
        [116.0 * y - 16.0, 500.0 * (x - y), 200.0 * (y - z)]
    }

    fn delta_e(a: Rgb, b: Rgb) -> f32 {
        let (a, b) = (lab(a), lab(b));
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    /// WCAG contrast ratio of two colors, either way round.
    fn contrast(c: Rgb, bg: Rgb) -> f32 {
        let (a, b) = (luminance(c) + 0.05, luminance(bg) + 0.05);
        a.max(b) / a.min(b)
    }

    #[test]
    fn every_palette_stays_readable_and_apart_from_ignored_and_cursor_colors() {
        use crate::anim::{heat_scaled, range_scale, stops_of};
        use crate::settings::{Palette, Settings, PALETTES, PAPER_PALETTES};
        let d = Settings::default();
        let scale = range_scale(d.heat_range.secs());
        let ages = || std::iter::successors(Some(1.0_f32), |a| Some(a * 1.25)).take_while(|&a| a <= 1.5 * d.heat_range.secs());
        let grounds = [(&DARK, &PALETTES[..]), (&PARCHMENT, &PAPER_PALETTES[..])];
        for (g, p) in grounds.into_iter().flat_map(|(g, ps)| ps.iter().map(move |&p| (g, p))) {
            for age in ages() {
                let on = heat_scaled(stops_of(p), age, scale);
                let off = off_line(g, on, d.focus_dim);
                let c = contrast(on, g.bg);
                assert!(c >= 2.8, "{p:?}, {age:.0}s old: on-line name too faint ({c:.1}:1)");
                // Mono is all greys: only the thin lines mark its ignored names, and
                // only bold marks the cursor path.
                if p == Palette::Mono {
                    continue;
                }
                for c in [on, off] {
                    let de = delta_e(c, ignored_shade(g, c, d.dim_floor));
                    assert!(de >= 15.0, "{p:?}, {age:.0}s old: reads as ignored (dE {de:.1})");
                }
                let de = delta_e(on, g.route_text);
                assert!(de >= 15.0, "{p:?}, {age:.0}s old: reads as the cursor path (dE {de:.1})");
            }
        }
    }
}
