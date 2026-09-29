//! Rendering: animated tree canvas, status bar, preview popup, help overlay.
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::SystemTime;

use ansi_to_tui::IntoText;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout as RLayout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::anim::{approach, heat, mix, to_color, Damped, Rgb, HEAT, RECOLOR};
use crate::layout::{glyph, layout, lines, ACTIVE, ROUTE};
use crate::App;

pub const BG: Rgb = [9.0, 10.0, 15.0];
const BAR_BG: Rgb = [17.0, 19.0, 29.0];
const POP_BG: Rgb = [13.0, 15.0, 23.0];
const LINE_DIM: Rgb = [33.0, 39.0, 68.0];
const LINE_ACTIVE: Rgb = [68.0, 86.0, 168.0];
const LINE_ROUTE: Rgb = [140.0, 168.0, 255.0];
const FLASH: Rgb = [235.0, 242.0, 255.0];
const ROUTE_TEXT: Rgb = [238.0, 240.0, 250.0];
const PILL: Rgb = [34.0, 39.0, 72.0];
const DOT: Rgb = [255.0, 58.0, 58.0];
const MUTED: Rgb = [110.0, 118.0, 150.0];

/// Seconds for the camera to (mostly) arrive; slower than nodes so the eye
/// sees the tree move before the view recenters.
const CAM: f32 = 0.16;
const PILL_T: f32 = 0.05;
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
        to_color(mix(rgb, BG, d))
    };
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if keep.contains((x, y).into()) {
                continue;
            }
            let cell = &mut buf[(x, y)];
            let (fg, bg) = (cell.fg, cell.bg);
            cell.set_fg(fade(fg, [200.0; 3])).set_bg(fade(bg, BG));
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
    let fresh = app.lay.as_ref().is_none_or(|(g, _)| *g != app.epoch);
    let (epoch, lay) = match app.lay.take() {
        Some(l) if !fresh => l,
        _ => (app.epoch, layout(&app.tree, app.cursor, &|i| app.label(i))),
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
                ROUTE_TEXT
            } else {
                let age = now.duration_since(app.heat_of(p.id)).unwrap_or_default().as_secs_f32();
                let h = heat(age);
                if p.active { h } else { mix(h, BG, 0.6) }
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
    moving |= approach(&mut app.flash, 0.0, 0.22, dt);

    // Camera: cursor at 40% across, vertically centered.
    let (cx, cy, cw) = lay.cursor;
    let tx = cx as f32 + cw as f32 / 2.0 - canvas.width as f32 * 0.4;
    let ty = cy as f32 - canvas.height as f32 / 2.0;
    let cam = app.cam.get_or_insert((Damped::new(tx), Damped::new(ty)));
    moving |= cam.0.step(tx, CAM, dt);
    moving |= cam.1.step(ty, CAM, dt);
    let (ox, oy) = (cam.0.v.round() as i32, cam.1.v.round() as i32);

    // The selector is pinned to the line (the cursor's target slot); items
    // slide into it. Only its x/width ease, when moving along the line.
    let (px, py, pw) = (cx as f32, cy as f32, cw as f32);
    let pill = app.pill.get_or_insert((Damped::new(px), Damped::new(py), Damped::new(pw)));
    moving |= pill.0.step(px, PILL_T, dt);
    moving |= pill.1.step(py, PILL_T, dt);
    moving |= pill.2.step(pw, PILL_T, dt);
    let (pill_x, pill_y, pill_w) = (pill.0.v.round() as i32, pill.1.v.round() as i32, pill.2.v.round() as i32);

    let buf = f.buffer_mut();
    tint(buf, canvas, BG);

    // Pill first (bg only), so lines and labels draw over it.
    for x in (pill_x - 2)..=(pill_x + pill_w) {
        let (sx, sy) = (x - ox, pill_y - oy);
        if sx >= 0 && sy >= 0 && sx < canvas.width as i32 && sy < canvas.height as i32 {
            buf[(canvas.x + sx as u16, canvas.y + sy as u16)].set_bg(to_color(PILL));
        }
    }

    let scene = &app.scene;
    let pos = |id: usize| {
        scene.nodes.get(&id).filter(|a| !a.ghost && a.alpha > 0.05).map(|a| {
            let (x, y) = a.pos();
            (x, y, a.w)
        })
    };
    let route_rgb = mix(LINE_ROUTE, FLASH, app.flash);
    let spine: HashSet<usize> = crate::layout::spine(&app.tree, app.cursor).into_iter().collect();
    for ((x, y), (mask, emph)) in lines(&lay.blocks, &pos, &route, &spine) {
        let (sx, sy) = (x - ox, y - oy);
        if sx < 0 || sy < 0 || sx >= canvas.width as i32 || sy >= canvas.height as i32 {
            continue;
        }
        let c = match emph {
            ROUTE => route_rgb,
            ACTIVE => LINE_ACTIVE,
            _ => LINE_DIM,
        };
        buf[(canvas.x + sx as u16, canvas.y + sy as u16)].set_char(glyph(mask)).set_fg(to_color(c));
    }

    // Ghosts under live nodes.
    let mut order: Vec<(&usize, &crate::anim::NodeAnim)> = scene.nodes.iter().collect();
    order.sort_by_key(|(_, a)| !a.ghost);
    app.hits.clear();
    let mut visible_dirs = Vec::new();
    for (&id, a) in order {
        let (x, y) = a.pos();
        let (sx, sy) = (x - ox, y - oy);
        if sy < 0 || sy >= canvas.height as i32 || sx + a.w < 0 || sx >= canvas.width as i32 {
            continue;
        }
        let node = &app.tree.nodes[id];
        let mut style = Style::new().fg(to_color(mix(BG, a.rgb, a.alpha)));
        if node.is_dir || route.contains(&id) {
            style = style.add_modifier(Modifier::BOLD);
        }
        put(buf, canvas, sx, sy, &a.label, style);
        if !a.ghost {
            app.hits.push((sx, sy, a.w, id));
            if node.is_dir {
                visible_dirs.push(node.path.clone());
            }
        }
    }
    let (dsx, dsy) = (pill_x - 1 - ox, pill_y - oy);
    if dsx >= 0 && dsy >= 0 && dsx < canvas.width as i32 && dsy < canvas.height as i32 {
        let dot = mix(DOT, [255.0, 190.0, 190.0], app.flash);
        buf[(canvas.x + dsx as u16, canvas.y + dsy as u16)].set_char('●').set_fg(to_color(dot));
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

    // Preview popup: grows out of the cursor row, backdrop dims behind it.
    let full = popup(f.area());
    let from = Rect {
        x: canvas.x + (pill_x - 2 - ox).clamp(0, canvas.width as i32 - 1) as u16,
        y: canvas.y + (pill_y - oy).clamp(0, canvas.height as i32 - 1) as u16,
        width: (pill_w as u16 + 3).min(canvas.width),
        height: 1,
    };
    if let Some(pv) = &mut app.preview {
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
            let pos_label = format!(" {}/{} ", pv.scroll + 1, pv.text.lines.len().max(1));
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(to_color(mix(POP_BG, LINE_ROUTE, p))))
                .style(Style::new().bg(to_color(POP_BG)))
                .title(Span::styled(format!(" {} ", pv.title), Style::new().fg(to_color(ROUTE_TEXT)).add_modifier(Modifier::BOLD)))
                .title_bottom(Line::from(Span::styled(pos_label, Style::new().fg(to_color(MUTED)))).right_aligned());
            f.render_widget(Clear, area);
            let sy = pv.sy.v.round().max(0.0) as u16;
            f.render_widget(Paragraph::new(pv.text.clone()).block(block).scroll((sy, 0)), area);
            if max > 0 && p > 0.9 {
                let mut st = ScrollbarState::new(max as usize).position(sy as usize);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(None)
                        .end_symbol(None)
                        .track_symbol(Some("│"))
                        .track_style(Style::new().fg(to_color(LINE_DIM)))
                        .thumb_symbol("┃")
                        .thumb_style(Style::new().fg(to_color(LINE_ROUTE))),
                    area.inner(Margin { vertical: 1, horizontal: 0 }),
                    &mut st,
                );
            }
        }
    }
    if app.preview.as_ref().is_some_and(|p| p.closing && p.open.v < 0.03) {
        app.preview = None;
    }
    app.lay = Some((epoch, lay));
    moving
}

fn status_bar(f: &mut Frame, app: &App, area: Rect) {
    let buf = f.buffer_mut();
    tint(buf, area, BAR_BG);
    let cur = &app.tree.nodes[app.cursor];
    let t = app.heat_of(app.cursor);
    let partial = cur.rec.is_some_and(|c| !c.1);
    let meta = if cur.is_dir {
        match &cur.children {
            Some(k) => format!("{} items", k.len()),
            None => "folder".into(),
        }
    } else {
        std::fs::symlink_metadata(&cur.path).map(|m| human(m.len())).unwrap_or_default()
    };

    // Right side: meta · age swatch · legend · help key.
    let muted = Style::new().fg(to_color(MUTED));
    let mut right = vec![
        Span::styled(format!("{meta} · "), muted),
        Span::styled("● ", Style::new().fg(to_color(heat(age_of(t))))),
        Span::styled(format!("{}{}", ago(t), if partial { " (partial)" } else { "" }), Style::new().fg(to_color(ROUTE_TEXT))),
        Span::styled("   now ", muted),
    ];
    for (_, c) in HEAT {
        right.push(Span::styled("▮", Style::new().fg(to_color(c))));
    }
    right.push(Span::styled(" old   ? keys ", muted));
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
            left.push(Span::styled(" › ", Style::new().fg(to_color(LINE_ACTIVE))));
        }
        left.push(if leaf {
            Span::styled(n.clone(), Style::new().fg(to_color(ROUTE_TEXT)).add_modifier(Modifier::BOLD))
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

const KEYS: [(&str, &str); 12] = [
    ("h j k l / arrows", "move"),
    ("l / enter", "open folder · preview file"),
    ("space / tab", "fold / unfold"),
    ("J K / pgup pgdn", "jump 10"),
    ("g G", "first / last sibling"),
    ("-", "re-root one level up"),
    ("c", "collapse other branches"),
    ("r", "reload"),
    ("mouse", "click select · click again open · wheel"),
    ("preview", "j k · space · ctrl-d/u · g G · q"),
    ("?", "toggle this help"),
    ("q", "quit"),
];

fn help(f: &mut Frame, t: f32) {
    let w = 64u16.min(f.area().width);
    let full_h = (KEYS.len() as u16 + 6).min(f.area().height);
    let h = ((full_h as f32) * t).round().max(1.0) as u16;
    let a = f.area();
    let r = Rect { x: a.x + (a.width - w) / 2, y: a.y + (a.height - full_h) / 2, width: w, height: h };
    let fade = |c: Rgb| to_color(mix(POP_BG, c, t));
    let mut lines = vec![Line::from("")];
    for (k, d) in KEYS {
        lines.push(Line::from(vec![
            Span::styled(format!("  {k:>17}  "), Style::new().fg(fade(LINE_ROUTE)).add_modifier(Modifier::BOLD)),
            Span::styled(d, Style::new().fg(fade(ROUTE_TEXT))),
        ]));
    }
    lines.push(Line::from(""));
    let mut legend = vec![Span::styled("  color = last change inside  now ", Style::new().fg(fade(MUTED)))];
    for (_, c) in HEAT {
        legend.push(Span::styled("▮", Style::new().fg(fade(c))));
    }
    legend.push(Span::styled(" 5y", Style::new().fg(fade(MUTED))));
    lines.push(Line::from(legend));
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(fade(LINE_ROUTE)))
                .style(Style::new().bg(to_color(POP_BG)))
                .title(Span::styled(" tb ", Style::new().fg(fade(ROUTE_TEXT)).add_modifier(Modifier::BOLD))),
        ),
        r,
    );
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
}

fn run(cmd: &mut Command) -> Option<Vec<u8>> {
    let out = cmd.output().ok()?;
    (out.status.success() && !out.stdout.is_empty()).then_some(out.stdout)
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
    pub fn load(path: &Path, width: u16) -> Preview {
        let w = width.saturating_sub(3).max(20).to_string();
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let title = format!("{name} · {}", human(size));
        let mut first = Vec::new();
        if let Ok(f) = std::fs::File::open(path) {
            let _ = f.take(8192).read_to_end(&mut first);
        }
        let text = if size == 0 {
            Text::from(vec![Line::from(""), Line::from("  (empty file)")])
        } else if first.contains(&0) {
            let kind = run(Command::new("file").arg("-b").arg(path))
                .map(|b| String::from_utf8_lossy(&b).trim().to_string())
                .unwrap_or_else(|| "binary".into());
            Text::from(vec![
                Line::from(""),
                Line::from(format!("  binary file · {}", human(size))),
                Line::from(format!("  {kind}")),
            ])
        } else {
            let is_md = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"));
            // glow is a snap and can't open paths outside $HOME: feed it stdin.
            let glow = || {
                let md = head(path);
                let mut child = Command::new("glow")
                    .args(["-s", "dark", "-w", &w, "-"])
                    .env("CLICOLOR_FORCE", "1")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .ok()?;
                child.stdin.take()?.write_all(&md).ok()?;
                let out = child.wait_with_output().ok()?;
                (out.status.success() && !out.stdout.is_empty()).then_some(out.stdout)
            };
            let bat = || {
                run(Command::new("bat")
                    .args(["--color=always", "--style=numbers", "--paging=never", "--wrap=character"])
                    .args(["--line-range", ":5000", "--terminal-width", &w])
                    .arg(path))
            };
            let raw = if is_md { glow().or_else(bat) } else { bat() }.unwrap_or_else(|| head(path));
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
        };
        Preview { title, text, scroll: 0, sy: Damped::new(0.0), page: 1, open: Damped::new(0.0), closing: false }
    }
}
