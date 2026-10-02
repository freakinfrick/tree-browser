//! Image and PDF previews. Pixels go through ratatui-image, which uses kitty /
//! sixel / iTerm2 graphics when the terminal answers the query. Without pixels tb draws
//! block glyphs itself: quadrants (2x2 sub-pixels per cell) or sextants (2x3), or
//! leaves half-blocks to the crate.
use std::path::{Path, PathBuf};
use std::process::Command;

use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Frame;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{FilterType, Resize, StatefulImage};

use crate::settings::BlockGlyphs;

/// Extensions previewed as pictures. Anything the image crate can't decode goes
/// through ImageMagick (svg, ico, heic, ...).
const IMAGE_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "ico", "svg", "svgz", "avif", "heic", "tga", "ppm",
    "pgm", "pnm", "qoi", "xcf", "psd",
];

/// Long side of a rendered PDF page, in pixels.
const PDF_PX: &str = "1600";

pub struct Media {
    path: PathBuf,
    pdf: bool,
    pub pages: usize,
    /// 0-based page on screen (always 0 for images).
    pub page: usize,
    /// Decoded current page; the protocol is built from it at draw time (needs the picker).
    img: Option<DynamicImage>,
    proto: Option<StatefulProtocol>,
    /// Quadrant cells, built instead of `proto` when there are no pixels.
    quads: Option<(BlockGlyphs, Vec<Quad>)>,
    /// Area and cell size the protocol was built for.
    built: (Rect, Rect),
    /// Pixel size of the current page.
    pub dims: Option<(u32, u32)>,
    pub err: Option<String>,
}

fn ext(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn stdout(cmd: &mut Command) -> Result<Vec<u8>, String> {
    let out = cmd.output().map_err(|e| format!("{e}"))?;
    if out.status.success() && !out.stdout.is_empty() {
        Ok(out.stdout)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("no output").to_string())
    }
}

/// Pixels (whatever the terminal claimed) <-> half-blocks. Multiplexers like herdr
/// claim kitty support for every client, including ones that can't show it.
pub fn toggle(p: &mut Picker, detected: ProtocolType) {
    let next = if p.protocol_type() == ProtocolType::Halfblocks { detected } else { ProtocolType::Halfblocks };
    p.set_protocol_type(next);
}

pub fn label(p: &Picker) -> &'static str {
    match p.protocol_type() {
        ProtocolType::Halfblocks => "blocks",
        ProtocolType::Sixel => "sixel",
        ProtocolType::Kitty => "kitty",
        ProtocolType::Iterm2 => "iterm2",
    }
}

/// Picker override for testing and for terminals that answer the query wrongly:
/// TB_GRAPHICS=halfblocks|sixel|kitty|iterm2|off.
/// Returns the picker and what the terminal claimed (for `i` to switch back to).
pub fn picker() -> Option<(Picker, ProtocolType)> {
    let force = std::env::var("TB_GRAPHICS").unwrap_or_default().to_ascii_lowercase();
    if force == "off" {
        return None;
    }
    // tmux < 3.3 swallows the query; the crate then times out after 1 s but leaves its
    // reader thread on stdin, eating keystrokes. tmux can't pass pixels through anyway.
    let mut p = if std::env::var_os("TMUX").is_some() {
        let mut p = Picker::from_fontsize((10, 20));
        p.set_protocol_type(ProtocolType::Halfblocks);
        p
    } else {
        Picker::from_query_stdio().unwrap_or_else(|_| Picker::from_fontsize((10, 20)))
    };
    let detected = p.protocol_type();
    match force.as_str() {
        "halfblocks" => p.set_protocol_type(ProtocolType::Halfblocks),
        "sixel" => p.set_protocol_type(ProtocolType::Sixel),
        "kitty" => p.set_protocol_type(ProtocolType::Kitty),
        "iterm2" => p.set_protocol_type(ProtocolType::Iterm2),
        // herdr answers "kitty" for every attached client, whatever terminal it
        // draws into (Konsole 21.12 then prints the image data as text). Start safe;
        // `i` switches to the claimed protocol.
        _ if std::env::var_os("HERDR_ENV").is_some() => p.set_protocol_type(ProtocolType::Halfblocks),
        _ => {}
    }
    Some((p, detected))
}

/// Whether the image viewer opens this file: images and PDFs.
pub fn shows(path: &Path) -> bool {
    let e = ext(path);
    e == "pdf" || IMAGE_EXT.contains(&e.as_str())
}

impl Media {
    /// Media preview for image and PDF files, None for everything else.
    pub fn open(path: &Path) -> Option<Media> {
        if !shows(path) {
            return None;
        }
        let pdf = ext(path) == "pdf";
        let mut m =
            Media { path: path.to_path_buf(), pdf, pages: 1, page: 0, img: None, proto: None, quads: None, built: Default::default(), dims: None, err: None };
        if pdf {
            m.pages = stdout(Command::new("pdfinfo").arg(path))
                .ok()
                .and_then(|o| {
                    String::from_utf8_lossy(&o)
                        .lines()
                        .find_map(|l| l.strip_prefix("Pages:").and_then(|n| n.trim().parse().ok()))
                })
                .unwrap_or(1);
        }
        m.load();
        Some(m)
    }

    /// Flip to page `n` (clamped); decodes now so the next frame just encodes.
    pub fn goto(&mut self, n: isize) {
        let n = n.clamp(0, self.pages as isize - 1) as usize;
        if n != self.page {
            self.page = n;
            self.load();
        }
    }

    pub fn flip(&mut self, d: isize) {
        self.goto(self.page as isize + d);
    }

    /// Drop the encoded page so the next draw re-encodes (protocol changed).
    pub fn reencode(&mut self) {
        self.proto = None;
        self.quads = None;
    }

    fn load(&mut self) {
        self.reencode();
        match self.decode() {
            Ok(img) => {
                self.dims = Some((img.width(), img.height()));
                self.img = Some(img);
                self.err = None;
            }
            Err(e) => {
                self.img = None;
                self.err = Some(e);
            }
        }
    }

    fn decode(&self) -> Result<DynamicImage, String> {
        let bytes = if self.pdf {
            let n = (self.page + 1).to_string();
            stdout(
                Command::new("pdftoppm")
                    .args(["-f", &n, "-l", &n, "-png", "-singlefile", "-scale-to", PDF_PX])
                    .arg(&self.path),
            )
            .map_err(|e| format!("pdftoppm: {e}"))?
        } else {
            let direct = (!ext(&self.path).starts_with("svg"))
                .then(|| image::ImageReader::open(&self.path).ok()?.with_guessed_format().ok()?.decode().ok())
                .flatten();
            if let Some(img) = direct {
                return Ok(img);
            }
            // [0]: first frame/layer only. Transparent background so the popup shows through.
            let mut src = self.path.as_os_str().to_owned();
            src.push("[0]");
            stdout(Command::new("convert").args(["-background", "none", "-density", "150"]).arg(src).arg("png:-"))
                .map_err(|e| format!("convert: {e}"))?
        };
        image::load_from_memory(&bytes).map_err(|e| format!("{e}"))
    }

    /// Draw the current page scaled to fit `area`, centered. `bg` shows through
    /// transparent pixels in block mode.
    pub fn render(&mut self, f: &mut Frame, area: Rect, picker: &Picker, glyphs: BlockGlyphs, bg: [f32; 3]) {
        let quad = picker.protocol_type() == ProtocolType::Halfblocks && glyphs != BlockGlyphs::Half;
        let rows = if glyphs == BlockGlyphs::Sextants { 3 } else { 2 };
        let built = if quad { self.quads.as_ref().is_some_and(|q| q.0 == glyphs) } else { self.proto.is_some() };
        if !built || self.built.0 != area {
            let Some(img) = &self.img else { return };
            // Scale to whole cells, cropping the sliver (< 1 cell) that doesn't fit:
            // a partly covered edge cell would otherwise blend with transparent black.
            let (fw, fh) = (picker.font_size().0 as f32, picker.font_size().1 as f32);
            let s = (area.width as f32 * fw / img.width() as f32).min(area.height as f32 * fh / img.height() as f32);
            let cw = ((img.width() as f32 * s / fw) as u16).clamp(1, area.width);
            let ch = ((img.height() as f32 * s / fh) as u16).clamp(1, area.height);
            let cells = Rect { x: area.x + (area.width - cw) / 2, y: area.y + (area.height - ch) / 2, width: cw, height: ch };
            if quad {
                let fitted = img.resize_to_fill(cw as u32 * fw as u32, ch as u32 * fh as u32, FilterType::Lanczos3);
                let sub = fitted.resize_exact(cw as u32 * 2, ch as u32 * rows, FilterType::Lanczos3);
                self.quads = Some((glyphs, blocks(&sub.to_rgba8(), bg, rows)));
            } else {
                let fitted = img.resize_to_fill(cw as u32 * fw as u32, ch as u32 * fh as u32, FilterType::Triangle);
                self.proto = Some(picker.new_resize_protocol(fitted));
            }
            self.built = (area, cells);
        }
        let r = self.built.1;
        if quad {
            let Some((_, quads)) = &self.quads else { return };
            let buf = f.buffer_mut();
            for (i, q) in quads.iter().enumerate() {
                let (x, y) = (r.x + (i % r.width as usize) as u16, r.y + (i / r.width as usize) as u16);
                if let Some(c) = buf.cell_mut((x, y)) {
                    let rgb = |p: [u8; 3]| Color::Rgb(p[0], p[1], p[2]);
                    c.set_char(q.ch).set_fg(rgb(q.fg)).set_bg(rgb(q.bg));
                }
            }
            return;
        }
        let Some(proto) = &mut self.proto else { return };
        f.render_stateful_widget(StatefulImage::default().resize(Resize::Fit(None)), r, proto);
    }
}

/// One cell of a quadrant-block image.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Quad {
    ch: char,
    fg: [u8; 3],
    bg: [u8; 3],
}

/// Quadrant glyphs by which sub-pixels take the foreground color, bits in reading
/// order (0 top-left, 1 top-right, 2 bottom-left). The last sub-pixel is always
/// background, which covers every split into two groups without the complements.
const QUADS: [char; 8] = [' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛'];

/// Sextant glyph for a 2x3 mask, bits in reading order. U+1FB00.. holds every
/// pattern except the four that already exist as block elements.
fn sextant(m: usize) -> char {
    match m {
        0 => ' ',
        21 => '▌',
        42 => '▐',
        63 => '█',
        _ => char::from_u32(0x1FB00 + m as u32 - 1 - (m > 21) as u32 - (m > 42) as u32).unwrap_or(' '),
    }
}

/// `img` at 2 x `rows` pixels per cell (2 = quadrants, 3 = sextants) -> one glyph and
/// two colors per cell, choosing the split of the sub-pixels whose two group
/// averages leave the least error.
fn blocks(img: &image::RgbaImage, bg: [f32; 3], rows: u32) -> Vec<Quad> {
    let (w, h) = (img.width() / 2, img.height() / rows);
    let n = 2 * rows as usize;
    let px = |x: u32, y: u32| {
        let p = img.get_pixel(x, y).0;
        let a = p[3] as f32 / 255.0;
        [0, 1, 2].map(|i| p[i] as f32 * a + bg[i] * (1.0 - a))
    };
    let mut out = Vec::with_capacity((w * h) as usize);
    for cy in 0..h {
        for cx in 0..w {
            let p: Vec<[f32; 3]> = (0..n as u32).map(|i| px(cx * 2 + i % 2, cy * rows + i / 2)).collect();
            let mut best = (f32::MAX, 0, [0.0; 3], [0.0; 3]);
            for mask in 0..1usize << (n - 1) {
                let mean = |on: bool| {
                    let (mut s, mut k) = ([0.0f32; 3], 0.0);
                    for (i, c) in p.iter().enumerate() {
                        if (mask >> i & 1 == 1) == on {
                            (0..3).for_each(|j| s[j] += c[j]);
                            k += 1.0;
                        }
                    }
                    if k == 0.0 { s } else { s.map(|v| v / k) }
                };
                let (fg, bk) = (mean(true), mean(false));
                let err: f32 = p
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let m = if mask >> i & 1 == 1 { fg } else { bk };
                        (0..3).map(|j| (c[j] - m[j]).powi(2)).sum::<f32>()
                    })
                    .sum();
                if err < best.0 {
                    best = (err, mask, if mask == 0 { bk } else { fg }, bk);
                }
            }
            let u8s = |c: [f32; 3]| c.map(|v| v.round().clamp(0.0, 255.0) as u8);
            let ch = if rows == 3 { sextant(best.1) } else { QUADS[best.1] };
            out.push(Quad { ch, fg: u8s(best.2), bg: u8s(best.3) });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(px: [[u8; 4]; 4]) -> image::RgbaImage {
        let mut i = image::RgbaImage::new(2, 2);
        for (n, p) in px.into_iter().enumerate() {
            i.put_pixel(n as u32 % 2, n as u32 / 2, image::Rgba(p));
        }
        i
    }

    #[test]
    fn quadrant_picks_the_exact_split() {
        let (w, k) = ([255, 255, 255, 255], [0, 0, 0, 255]);
        // White top-left + bottom-left, black right column: left half block.
        let q = blocks(&img([w, k, w, k]), [0.0; 3], 2);
        assert_eq!(q, vec![Quad { ch: '▌', fg: [255; 3], bg: [0; 3] }]);
        // Diagonal.
        assert_eq!(blocks(&img([k, w, w, k]), [0.0; 3], 2)[0].ch, '▞');
        // Only bottom-right differs: the other three take the foreground.
        assert_eq!(blocks(&img([w, w, w, k]), [0.0; 3], 2)[0], Quad { ch: '▛', fg: [255; 3], bg: [0; 3] });
    }

    #[test]
    fn sextant_codepoints() {
        assert_eq!(sextant(1), '\u{1FB00}'); // top-left only
        assert_eq!(sextant(20), '\u{1FB13}'); // sextants 3+5
        assert_eq!(sextant(22), '\u{1FB14}'); // 2+3+5, after the skipped left half
        assert_eq!(sextant(62), '\u{1FB3B}');
        // Top row white, rest black: upper third.
        let mut i = image::RgbaImage::from_pixel(2, 3, image::Rgba([0, 0, 0, 255]));
        i.put_pixel(0, 0, image::Rgba([255; 4]));
        i.put_pixel(1, 0, image::Rgba([255; 4]));
        assert_eq!(blocks(&i, [0.0; 3], 3)[0], Quad { ch: sextant(3), fg: [255; 3], bg: [0; 3] });
    }

    #[test]
    fn quadrant_flat_cell_is_a_space_and_alpha_shows_bg() {
        let clear = [9, 9, 9, 0];
        let q = blocks(&img([clear; 4]), [10.0, 20.0, 30.0], 2);
        assert_eq!(q, vec![Quad { ch: ' ', fg: [10, 20, 30], bg: [10, 20, 30] }]);
    }
}
