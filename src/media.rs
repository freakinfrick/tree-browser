//! Image and PDF previews. Pixels go through ratatui-image, which uses kitty /
//! sixel / iTerm2 graphics when the terminal answers the query, half-blocks otherwise.
use std::path::{Path, PathBuf};
use std::process::Command;

use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui::Frame;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{FilterType, Resize, StatefulImage};

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
pub fn picker() -> Option<Picker> {
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
    match force.as_str() {
        "halfblocks" => p.set_protocol_type(ProtocolType::Halfblocks),
        "sixel" => p.set_protocol_type(ProtocolType::Sixel),
        "kitty" => p.set_protocol_type(ProtocolType::Kitty),
        "iterm2" => p.set_protocol_type(ProtocolType::Iterm2),
        _ => {}
    }
    Some(p)
}

impl Media {
    /// Media preview for image and PDF files, None for everything else.
    pub fn open(path: &Path) -> Option<Media> {
        let e = ext(path);
        let pdf = e == "pdf";
        if !pdf && !IMAGE_EXT.contains(&e.as_str()) {
            return None;
        }
        let mut m =
            Media { path: path.to_path_buf(), pdf, pages: 1, page: 0, img: None, proto: None, dims: None, err: None };
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
    }

    fn load(&mut self) {
        self.proto = None;
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

    /// Draw the current page scaled to fit `area`, centered.
    pub fn render(&mut self, f: &mut Frame, area: Rect, picker: &Picker) {
        if self.proto.is_none() {
            let Some(img) = &self.img else { return };
            self.proto = Some(picker.new_resize_protocol(img.clone()));
        }
        let Some(proto) = &mut self.proto else { return };
        let resize = Resize::Scale(Some(FilterType::Triangle));
        let fit = proto.size_for(resize.clone(), area);
        let r = Rect {
            x: area.x + area.width.saturating_sub(fit.width) / 2,
            y: area.y + area.height.saturating_sub(fit.height) / 2,
            ..fit
        };
        f.render_stateful_widget(StatefulImage::default().resize(resize), r, proto);
    }
}
