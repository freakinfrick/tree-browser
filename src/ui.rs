//! Rendering: tree canvas with camera, status line, file preview popup.
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use ansi_to_tui::IntoText;
use ratatui::layout::{Constraint, Layout as RLayout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::layout::{glyph, layout};
use crate::App;

const LINE_ON: Color = Color::Rgb(90, 110, 200);
const LINE_OFF: Color = Color::Rgb(38, 45, 80);
const CURSOR_DOT: Color = Color::Rgb(235, 40, 40);

/// Age -> color, log-scale buckets from hot orange-red to cold blue.
pub fn heat(age: Duration) -> (u8, u8, u8) {
    const H: u64 = 3600;
    match age.as_secs() {
        s if s < H => (255, 85, 45),
        s if s < 24 * H => (255, 150, 50),
        s if s < 7 * 24 * H => (215, 170, 110),
        s if s < 30 * 24 * H => (150, 150, 150),
        s if s < 365 * 24 * H => (115, 125, 185),
        _ => (70, 85, 210),
    }
}

fn dim((r, g, b): (u8, u8, u8)) -> Color {
    let f = |c: u8| (c as f32 * 0.45) as u8;
    Color::Rgb(f(r), f(g), f(b))
}

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

pub fn draw(f: &mut Frame, app: &mut App) {
    let [canvas, status] = RLayout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(f.area());
    app.view = (canvas.width, canvas.height);

    let lay = layout(&app.tree, app.cursor, &|i| app.label(i));
    let (cx, cy, cw) = lay.cursor;
    app.target = (
        cx as f32 + cw as f32 / 2.0 - canvas.width as f32 * 0.4,
        cy as f32 - canvas.height as f32 / 2.0,
    );
    if app.cam.is_none() {
        app.cam = Some(app.target);
    }
    let (camx, camy) = app.cam.unwrap();
    let (ox, oy) = (camx.round() as i32, camy.round() as i32);
    let on_path: HashSet<usize> = app.tree.path_to(app.cursor).into_iter().collect();
    let to_screen = |x: i32, y: i32| -> Option<(u16, u16)> {
        let (sx, sy) = (x - ox, y - oy);
        (sx >= 0 && sy >= 0 && sx < canvas.width as i32 && sy < canvas.height as i32)
            .then(|| (canvas.x + sx as u16, canvas.y + sy as u16))
    };

    let buf = f.buffer_mut();
    for (&(x, y), &(mask, active)) in &lay.lines {
        if let Some(p) = to_screen(x, y) {
            buf[p].set_char(glyph(mask)).set_fg(if active { LINE_ON } else { LINE_OFF });
        }
    }
    let mut visible_dirs = Vec::new();
    for p in &lay.placed {
        let node = &app.tree.nodes[p.id];
        let Some((sx, sy)) = to_screen(p.x, p.y) else { continue };
        if node.is_dir {
            visible_dirs.push(node.path.clone());
        }
        let age = SystemTime::now().duration_since(app.heat(p.id)).unwrap_or_default();
        let mut style = if on_path.contains(&p.id) {
            Style::new().fg(Color::White).add_modifier(Modifier::BOLD)
        } else if p.active {
            Style::new().fg({
                let (r, g, b) = heat(age);
                Color::Rgb(r, g, b)
            })
        } else {
            Style::new().fg(dim(heat(age)))
        };
        if node.is_dir && !on_path.contains(&p.id) {
            style = style.add_modifier(Modifier::BOLD);
        }
        let room = (canvas.right() - sx) as usize;
        buf.set_stringn(sx, sy, &p.label, room, style);
    }
    if let Some(p) = to_screen(cx - 1, cy) {
        buf[p].set_char('●').set_fg(CURSOR_DOT);
    }
    for d in visible_dirs {
        app.mt.request(&d);
    }

    let cur = &app.tree.nodes[app.cursor];
    let incomplete = cur.is_dir && app.mt.cache.get(&cur.path).is_some_and(|c| !c.1);
    let left = format!(
        " {}  {}{}",
        cur.path.display(),
        ago(app.heat(app.cursor)),
        if incomplete { " (partial: walk capped)" } else { "" }
    );
    let hint = "hjkl move · space fold · enter open · - up · c collapse · r reload · q quit ";
    let hint_w = hint.chars().count();
    let show_hint = status.width as usize > hint_w + 40;
    let room = status.width as usize - if show_hint { hint_w + 1 } else { 0 };
    // Keep the tail of a long path: the leaf is what matters.
    let n = left.chars().count();
    let left = if n > room { format!("…{}", left.chars().skip(n - room + 1).collect::<String>()) } else { left };
    f.render_widget(Paragraph::new(Span::styled(left, Style::new().fg(Color::Gray))), status);
    if show_hint {
        let r = Rect { x: status.right() - hint_w as u16, width: hint_w as u16, ..status };
        f.render_widget(Paragraph::new(hint).style(Style::new().fg(Color::DarkGray)), r);
    }

    if let Some(pv) = &mut app.preview {
        let area = popup(f.area());
        pv.page = area.height.saturating_sub(2);
        let max = (pv.text.lines.len() as u16).saturating_sub(pv.page);
        pv.scroll = pv.scroll.min(max);
        let title = format!(" {} ", pv.title);
        let pos = format!(" {}/{} · q close ", pv.scroll + 1, pv.text.lines.len().max(1));
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(LINE_ON))
            .title(title)
            .title_bottom(Line::from(pos).right_aligned());
        f.render_widget(Clear, area);
        f.render_widget(Paragraph::new(pv.text.clone()).block(block).scroll((pv.scroll, 0)), area);
    }
}

pub fn popup(r: Rect) -> Rect {
    let w = (r.width as u32 * 9 / 10) as u16;
    let h = (r.height as u32 * 9 / 10) as u16;
    Rect { x: r.x + (r.width - w) / 2, y: r.y + (r.height - h) / 2, width: w, height: h }
}

pub struct Preview {
    pub title: String,
    pub text: Text<'static>,
    pub scroll: u16,
    pub page: u16,
}

fn run(cmd: &mut Command) -> Option<Vec<u8>> {
    let out = cmd.output().ok()?;
    (out.status.success() && !out.stdout.is_empty()).then_some(out.stdout)
}

impl Preview {
    /// Whole file, colored: glow for markdown, bat otherwise, plain text as last resort.
    pub fn load(path: &Path, width: u16) -> Preview {
        let w = width.saturating_sub(2).max(20).to_string();
        let mut head = Vec::new();
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let title = format!("{name} · {size} bytes");
        if let Ok(f) = std::fs::File::open(path) {
            let _ = f.take(8192).read_to_end(&mut head);
        }
        let text = if size == 0 {
            Text::from(vec![Line::from(""), Line::from("  (empty file)")])
        } else if head.contains(&0) {
            let kind = run(Command::new("file").arg("-b").arg(path))
                .map(|b| String::from_utf8_lossy(&b).trim().to_string())
                .unwrap_or_else(|| "binary".into());
            Text::from(vec![
                Line::from(""),
                Line::from(format!("  binary file · {size} bytes")),
                Line::from(format!("  {kind}")),
            ])
        } else {
            let is_md = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"));
            // glow is a snap and can't open paths outside $HOME: feed it stdin.
            let glow = || {
                let md = std::fs::read(path).ok()?;
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
            let raw = if is_md { glow().or_else(bat) } else { bat() }.unwrap_or_else(|| {
                let mut v = Vec::new();
                if let Ok(f) = std::fs::File::open(path) {
                    let _ = f.take(2 << 20).read_to_end(&mut v);
                }
                v
            });
            raw.into_text().unwrap_or_else(|_| Text::raw(String::from_utf8_lossy(&raw).into_owned()))
        };
        Preview { title, text, scroll: 0, page: 1 }
    }
}
