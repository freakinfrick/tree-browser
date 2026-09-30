//! User settings: the `,` menu and `~/.config/tb/config.toml`.
//!
//! The file is flat `key = value` TOML. The menu writes one key at a time,
//! rewriting just that line (or appending it), so comments and hand edits
//! survive. Environment variables (TB_SORT, TB_LIVE, TB_GIT) still win for
//! the run they're set in, and aren't written back unless that setting is
//! changed in the menu.
use std::fs;
use std::path::{Path, PathBuf};

use crate::tree::{Sort, SortKey};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Speed {
    Slow,
    Normal,
    Fast,
    Instant,
}

impl Speed {
    /// Multiplier on frame time: springs and fades run this much faster.
    pub fn factor(self) -> f32 {
        match self {
            Speed::Slow => 0.5,
            Speed::Normal => 1.0,
            Speed::Fast => 2.0,
            Speed::Instant => 1000.0,
        }
    }
}

/// How far the wheel glides on after the last notch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Momentum {
    Off,
    Short,
    Medium,
    Long,
}

impl Momentum {
    /// Seconds for the glide's speed to fall to 1/e. 0 = no glide.
    pub fn tau(self) -> f32 {
        match self {
            Momentum::Off => 0.0,
            Momentum::Short => 0.12,
            Momentum::Medium => 0.25,
            Momentum::Long => 0.5,
        }
    }
}

/// Line and selector colors.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Accent {
    Indigo,
    Teal,
    Violet,
    Amber,
    Mono,
}

/// Recency gradient.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Palette {
    /// Red (now) through orange and grey to blue (years).
    Ember,
    /// Yellow through green and teal to purple, readable with red-green color blindness.
    Aurora,
    /// Bright white fading to dark grey.
    Mono,
}

/// How wide the tree's columns are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Columns {
    /// As wide as the column's longest name, up to the column width.
    Fit,
    /// Every column exactly the column width: a grid that never shifts.
    Equal,
}

/// What follows each name, dimmed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Details {
    Off,
    Age,
    Size,
    Both,
}

/// Age at which the heat gradient reaches its coldest color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeatRange {
    Day,
    Week,
    Month,
    Year,
    Years5,
}

impl HeatRange {
    pub fn secs(self) -> f32 {
        const DAY: f32 = 86400.0;
        match self {
            HeatRange::Day => DAY,
            HeatRange::Week => 7.0 * DAY,
            HeatRange::Month => 30.0 * DAY,
            HeatRange::Year => 365.0 * DAY,
            HeatRange::Years5 => 5.0 * 365.0 * DAY,
        }
    }
}

/// Box-drawing set for the tree's lines.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineStyle {
    Rounded,
    Square,
    Heavy,
    Double,
    Ascii,
}

/// Image and PDF previews.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Graphics {
    /// Whatever the terminal claims, except under multiplexers that claim too much.
    Auto,
    /// The terminal's claimed protocol, always.
    Pixels,
    /// Half-block characters, which work everywhere.
    Blocks,
    /// Captions only.
    Off,
}

/// What j and k move through.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StepThrough {
    /// The folder's own list.
    Folder,
    /// Every open folder's list in the cursor's column.
    Column,
    /// The whole open tree in reading order: into a folder, then on to its next sibling.
    Tree,
}

/// Which programs color text previews.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextPreview {
    /// glow for markdown, bat for the rest.
    Styled,
    Bat,
    /// The file as it is, no external programs.
    Plain,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Settings {
    pub row_spacing: u8,
    pub column_gap: u8,
    pub max_name: u8,
    pub columns: Columns,
    pub details: Details,
    pub sort: Sort,
    pub folders_first: bool,
    pub natural_sort: bool,
    pub show_hidden: bool,
    pub accent: Accent,
    pub palette: Palette,
    pub heat_range: HeatRange,
    pub lines: LineStyle,
    pub branch_offset: u8,
    pub legend: bool,
    pub speed: Speed,
    pub live: bool,
    pub ripples: bool,
    pub git: bool,
    pub dim_ignored: bool,
    pub explode_ignored: bool,
    pub step: StepThrough,
    pub mouse: bool,
    /// Rows the tree moves per wheel notch; text previews move three times that.
    pub wheel_speed: u8,
    pub momentum: Momentum,
    pub graphics: Graphics,
    pub preview: TextPreview,
    pub wrap: bool,
    pub remember: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            row_spacing: 0,
            column_gap: 3,
            max_name: 28,
            columns: Columns::Fit,
            details: Details::Off,
            sort: Sort::default(),
            folders_first: false,
            natural_sort: true,
            show_hidden: false,
            accent: Accent::Indigo,
            palette: Palette::Ember,
            heat_range: HeatRange::Years5,
            lines: LineStyle::Rounded,
            branch_offset: 0,
            legend: true,
            speed: Speed::Normal,
            live: true,
            ripples: true,
            git: true,
            dim_ignored: true,
            explode_ignored: false,
            step: StepThrough::Tree,
            mouse: true,
            wheel_speed: 1,
            momentum: Momentum::Short,
            graphics: Graphics::Auto,
            preview: TextPreview::Styled,
            wrap: true,
            remember: false,
        }
    }
}

/// One row of the menu.
pub struct Item {
    pub key: &'static str,
    pub label: &'static str,
    pub section: &'static str,
    pub help: &'static str,
}

pub const ITEMS: [Item; 30] = [
    Item { key: "row_spacing", label: "Row spacing", section: "Layout", help: "Blank rows between entries. More air, fewer entries on screen." },
    Item { key: "column_gap", label: "Column gap", section: "Layout", help: "Space between a column's longest name and the next column." },
    Item { key: "max_name", label: "Column width", section: "Layout", help: "Widest a column gets. Longer names are cut with a …" },
    Item { key: "columns", label: "Columns", section: "Layout", help: "Fit: as wide as the longest name. Equal: all the column width, a grid that holds still." },
    Item { key: "details", label: "Name details", section: "Layout", help: "Age and/or size after each name, dimmed. Folders count everything inside." },
    Item { key: "sort", label: "Sort by", section: "Order", help: "Same as o in the tree. Folders sort by what's inside them." },
    Item { key: "sort_reverse", label: "Reverse", section: "Order", help: "Same as O: oldest, smallest or z–a first." },
    Item { key: "folders_first", label: "Folders first", section: "Order", help: "Folders above files, whatever the sort." },
    Item { key: "natural_sort", label: "Natural sort", section: "Order", help: "Numbers in names count up: file2 before file10." },
    Item { key: "show_hidden", label: "Dotfiles", section: "Order", help: "Same as . in the tree." },
    Item { key: "accent", label: "Accent", section: "Look", help: "Color of the lines, the selector and the highlights." },
    Item { key: "palette", label: "Heat colors", section: "Look", help: "Recency gradient. Aurora avoids red-green; mono is brightness only." },
    Item { key: "heat_range", label: "Heat range", section: "Look", help: "Age that gets the coldest color. Short ranges tell apart the files of one busy week." },
    Item { key: "lines", label: "Tree lines", section: "Look", help: "Corners and branches: rounded, square, heavy, double or plain ASCII." },
    Item { key: "branch_offset", label: "Branch offset", section: "Look", help: "Line between each join and its name: 0 touches, 4 is a long reach." },
    Item { key: "legend", label: "Legend", section: "Look", help: "The now ▮▮▮ old color key in the status bar." },
    Item { key: "speed", label: "Motion", section: "Look", help: "How fast folders unfurl and the view glides. Instant turns animation off." },
    Item { key: "live", label: "Live updates", section: "Behavior", help: "Re-list open folders about once a second as files change." },
    Item { key: "ripples", label: "Ripples", section: "Behavior", help: "Flash a live change and let it climb the tree." },
    Item { key: "git", label: "Git status", section: "Behavior", help: "Markers, branch and diffs inside git repos." },
    Item { key: "dim_ignored", label: "Dim ignored", section: "Behavior", help: "Fade files git ignores, like target/ and node_modules/." },
    Item { key: "explode_ignored", label: "Explode ignored", section: "Behavior", help: "Let e open folders git ignores too. A folder you explode directly always opens." },
    Item { key: "step", label: "Step through", section: "Behavior", help: "What j and k walk: the folder, the column, or the whole open tree in reading order. Arrows and the wheel stay in the column." },
    Item { key: "mouse", label: "Mouse", section: "Behavior", help: "Off hands the mouse back to the terminal, so you can select text." },
    Item { key: "wheel_speed", label: "Wheel speed", section: "Behavior", help: "Entries per wheel notch. Text previews scroll three lines for each." },
    Item { key: "momentum", label: "Momentum", section: "Behavior", help: "How far a quick flick of the wheel glides on after you stop. Slow notches stay one step each." },
    Item { key: "graphics", label: "Image previews", section: "Behavior", help: "Pixels if the terminal can, half-blocks anywhere, or off. i in a preview flips it for this run." },
    Item { key: "preview", label: "Text preview", section: "Behavior", help: "Styled: glow for markdown, bat for code. Or bat for all, or plain text." },
    Item { key: "wrap", label: "Wrap lines", section: "Behavior", help: "Off cuts long lines at the edge of the preview." },
    Item { key: "remember", label: "Remember place", section: "Behavior", help: "Reopen the folders and selection you left, next time tb starts in the same folder." },
];

const SORTS: [SortKey; 4] = [SortKey::Name, SortKey::Modified, SortKey::Size, SortKey::Type];
const SPEEDS: [Speed; 4] = [Speed::Slow, Speed::Normal, Speed::Fast, Speed::Instant];
const WHEEL_SPEEDS: [u8; 4] = [1, 2, 3, 5];
const MOMENTA: [Momentum; 4] = [Momentum::Off, Momentum::Short, Momentum::Medium, Momentum::Long];
const ACCENTS: [Accent; 5] = [Accent::Indigo, Accent::Teal, Accent::Violet, Accent::Amber, Accent::Mono];
const PALETTES: [Palette; 3] = [Palette::Ember, Palette::Aurora, Palette::Mono];
const COLUMNS: [Columns; 2] = [Columns::Fit, Columns::Equal];
const DETAILS: [Details; 4] = [Details::Off, Details::Age, Details::Size, Details::Both];
const RANGES: [HeatRange; 5] = [HeatRange::Day, HeatRange::Week, HeatRange::Month, HeatRange::Year, HeatRange::Years5];
const LINES: [LineStyle; 5] = [LineStyle::Rounded, LineStyle::Square, LineStyle::Heavy, LineStyle::Double, LineStyle::Ascii];
const GRAPHICS: [Graphics; 4] = [Graphics::Auto, Graphics::Pixels, Graphics::Blocks, Graphics::Off];
const STEPS: [StepThrough; 3] = [StepThrough::Folder, StepThrough::Column, StepThrough::Tree];
const PREVIEWS: [TextPreview; 3] = [TextPreview::Styled, TextPreview::Bat, TextPreview::Plain];

/// Step through `all` from `cur`, wrapping.
fn cycle<T: PartialEq + Copy>(all: &[T], cur: T, dir: i32) -> T {
    let i = all.iter().position(|&x| x == cur).unwrap_or(0) as i32;
    all[(i + dir).rem_euclid(all.len() as i32) as usize]
}

/// Step through `all` from `cur`, stopping at the ends.
fn nudge<T: PartialEq + Copy>(all: &[T], cur: T, dir: i32) -> T {
    let i = all.iter().position(|&x| x == cur).unwrap_or(0) as i32;
    all[(i + dir).clamp(0, all.len() as i32 - 1) as usize]
}

fn sort_word(k: SortKey) -> &'static str {
    match k {
        SortKey::Name => "name",
        SortKey::Modified => "modified",
        SortKey::Size => "size",
        SortKey::Type => "type",
    }
}

fn speed_word(s: Speed) -> &'static str {
    match s {
        Speed::Slow => "slow",
        Speed::Normal => "normal",
        Speed::Fast => "fast",
        Speed::Instant => "instant",
    }
}

fn momentum_word(m: Momentum) -> &'static str {
    match m {
        Momentum::Off => "off",
        Momentum::Short => "short",
        Momentum::Medium => "medium",
        Momentum::Long => "long",
    }
}

fn accent_word(a: Accent) -> &'static str {
    match a {
        Accent::Indigo => "indigo",
        Accent::Teal => "teal",
        Accent::Violet => "violet",
        Accent::Amber => "amber",
        Accent::Mono => "mono",
    }
}

fn palette_word(p: Palette) -> &'static str {
    match p {
        Palette::Ember => "ember",
        Palette::Aurora => "aurora",
        Palette::Mono => "mono",
    }
}

fn columns_word(c: Columns) -> &'static str {
    match c {
        Columns::Fit => "fit",
        Columns::Equal => "equal",
    }
}

fn details_word(d: Details) -> &'static str {
    match d {
        Details::Off => "off",
        Details::Age => "age",
        Details::Size => "size",
        Details::Both => "both",
    }
}

fn range_word(r: HeatRange) -> &'static str {
    match r {
        HeatRange::Day => "day",
        HeatRange::Week => "week",
        HeatRange::Month => "month",
        HeatRange::Year => "year",
        HeatRange::Years5 => "5y",
    }
}

fn lines_word(l: LineStyle) -> &'static str {
    match l {
        LineStyle::Rounded => "rounded",
        LineStyle::Square => "square",
        LineStyle::Heavy => "heavy",
        LineStyle::Double => "double",
        LineStyle::Ascii => "ascii",
    }
}

fn graphics_word(g: Graphics) -> &'static str {
    match g {
        Graphics::Auto => "auto",
        Graphics::Pixels => "pixels",
        Graphics::Blocks => "blocks",
        Graphics::Off => "off",
    }
}

fn step_word(s: StepThrough) -> &'static str {
    match s {
        StepThrough::Folder => "folder",
        StepThrough::Column => "column",
        StepThrough::Tree => "tree",
    }
}

fn preview_word(p: TextPreview) -> &'static str {
    match p {
        TextPreview::Styled => "styled",
        TextPreview::Bat => "bat",
        TextPreview::Plain => "plain",
    }
}

fn find<T: Copy>(all: &[T], word: &str, name: fn(T) -> &'static str) -> Option<T> {
    all.iter().copied().find(|&x| name(x) == word)
}

fn on_off(b: bool) -> String {
    if b { "on".into() } else { "off".into() }
}

impl Settings {
    /// The value as the menu shows it.
    pub fn show(&self, key: &str) -> String {
        match key {
            "row_spacing" => self.row_spacing.to_string(),
            "column_gap" => self.column_gap.to_string(),
            "max_name" => self.max_name.to_string(),
            "columns" => columns_word(self.columns).into(),
            "details" => details_word(self.details).into(),
            "sort" => sort_word(self.sort.key).into(),
            "sort_reverse" => on_off(self.sort.rev),
            "folders_first" => on_off(self.folders_first),
            "natural_sort" => on_off(self.natural_sort),
            "show_hidden" => if self.show_hidden { "shown".into() } else { "hidden".into() },
            "accent" => accent_word(self.accent).into(),
            "palette" => palette_word(self.palette).into(),
            "heat_range" => range_word(self.heat_range).into(),
            "lines" => lines_word(self.lines).into(),
            "branch_offset" => self.branch_offset.to_string(),
            "legend" => on_off(self.legend),
            "speed" => speed_word(self.speed).into(),
            "live" => on_off(self.live),
            "ripples" => on_off(self.ripples),
            "git" => on_off(self.git),
            "dim_ignored" => on_off(self.dim_ignored),
            "explode_ignored" => on_off(self.explode_ignored),
            "step" => step_word(self.step).into(),
            "mouse" => on_off(self.mouse),
            "wheel_speed" => self.wheel_speed.to_string(),
            "momentum" => momentum_word(self.momentum).into(),
            "graphics" => graphics_word(self.graphics).into(),
            "preview" => preview_word(self.preview).into(),
            "wrap" => on_off(self.wrap),
            "remember" => on_off(self.remember),
            _ => String::new(),
        }
    }

    /// The value as the config file stores it.
    pub fn store(&self, key: &str) -> String {
        match key {
            "row_spacing" | "column_gap" | "max_name" | "branch_offset" | "wheel_speed" => self.show(key),
            "sort_reverse" => self.sort.rev.to_string(),
            "show_hidden" => self.show_hidden.to_string(),
            "legend" | "live" | "ripples" | "git" | "dim_ignored" | "folders_first" | "natural_sort" | "explode_ignored" | "mouse" | "wrap"
            | "remember" => (self.show(key) == "on").to_string(),
            _ => format!("\"{}\"", self.show(key)),
        }
    }

    /// Step a setting: numbers by one, choices to the next, switches flip.
    pub fn adjust(&mut self, key: &str, dir: i32) {
        let step = |v: u8, lo: u8, hi: u8, by: i32| (v as i32 + dir * by).clamp(lo as i32, hi as i32) as u8;
        match key {
            "row_spacing" => self.row_spacing = step(self.row_spacing, 0, 3, 1),
            "column_gap" => self.column_gap = step(self.column_gap, 3, 12, 1),
            "max_name" => self.max_name = step(self.max_name, 12, 60, 2),
            "columns" => self.columns = cycle(&COLUMNS, self.columns, dir),
            "details" => self.details = cycle(&DETAILS, self.details, dir),
            "sort" => self.sort = Sort { key: cycle(&SORTS, self.sort.key, dir), rev: false },
            "sort_reverse" => self.sort.rev ^= true,
            "folders_first" => self.folders_first ^= true,
            "natural_sort" => self.natural_sort ^= true,
            "show_hidden" => self.show_hidden ^= true,
            "accent" => self.accent = cycle(&ACCENTS, self.accent, dir),
            "palette" => self.palette = cycle(&PALETTES, self.palette, dir),
            "heat_range" => self.heat_range = cycle(&RANGES, self.heat_range, dir),
            "lines" => self.lines = cycle(&LINES, self.lines, dir),
            "branch_offset" => self.branch_offset = step(self.branch_offset, 0, 4, 1),
            "legend" => self.legend ^= true,
            "speed" => self.speed = cycle(&SPEEDS, self.speed, dir),
            "live" => self.live ^= true,
            "ripples" => self.ripples ^= true,
            "git" => self.git ^= true,
            "dim_ignored" => self.dim_ignored ^= true,
            "explode_ignored" => self.explode_ignored ^= true,
            "step" => self.step = cycle(&STEPS, self.step, dir),
            "mouse" => self.mouse ^= true,
            "wheel_speed" => self.wheel_speed = nudge(&WHEEL_SPEEDS, self.wheel_speed, dir),
            "momentum" => self.momentum = cycle(&MOMENTA, self.momentum, dir),
            "graphics" => self.graphics = cycle(&GRAPHICS, self.graphics, dir),
            "preview" => self.preview = cycle(&PREVIEWS, self.preview, dir),
            "wrap" => self.wrap ^= true,
            "remember" => self.remember ^= true,
            _ => {}
        }
    }

    /// Put one setting back to its default.
    pub fn reset(&mut self, key: &str) {
        let d = Settings::default();
        match key {
            "row_spacing" => self.row_spacing = d.row_spacing,
            "column_gap" => self.column_gap = d.column_gap,
            "max_name" => self.max_name = d.max_name,
            "columns" => self.columns = d.columns,
            "details" => self.details = d.details,
            "sort" => self.sort.key = d.sort.key,
            "sort_reverse" => self.sort.rev = d.sort.rev,
            "folders_first" => self.folders_first = d.folders_first,
            "natural_sort" => self.natural_sort = d.natural_sort,
            "show_hidden" => self.show_hidden = d.show_hidden,
            "accent" => self.accent = d.accent,
            "palette" => self.palette = d.palette,
            "heat_range" => self.heat_range = d.heat_range,
            "lines" => self.lines = d.lines,
            "branch_offset" => self.branch_offset = d.branch_offset,
            "legend" => self.legend = d.legend,
            "speed" => self.speed = d.speed,
            "live" => self.live = d.live,
            "ripples" => self.ripples = d.ripples,
            "git" => self.git = d.git,
            "dim_ignored" => self.dim_ignored = d.dim_ignored,
            "explode_ignored" => self.explode_ignored = d.explode_ignored,
            "step" => self.step = d.step,
            "mouse" => self.mouse = d.mouse,
            "wheel_speed" => self.wheel_speed = d.wheel_speed,
            "momentum" => self.momentum = d.momentum,
            "graphics" => self.graphics = d.graphics,
            "preview" => self.preview = d.preview,
            "wrap" => self.wrap = d.wrap,
            "remember" => self.remember = d.remember,
            _ => {}
        }
    }

    /// Set one key from the config file. Err explains a bad value.
    fn set(&mut self, key: &str, v: &str) -> Result<(), String> {
        let num = |lo: u8, hi: u8| -> Result<u8, String> {
            v.parse::<u8>().ok().filter(|n| (lo..=hi).contains(n)).ok_or(format!("{key} wants {lo} to {hi}"))
        };
        let flag = || v.parse::<bool>().map_err(|_| format!("{key} wants true or false"));
        let bad = |what: &str| format!("{key} wants one of {what}");
        match key {
            "row_spacing" => self.row_spacing = num(0, 3)?,
            "column_gap" => self.column_gap = num(3, 12)?,
            "max_name" => self.max_name = num(12, 60)?,
            "columns" => self.columns = find(&COLUMNS, v, columns_word).ok_or(bad("fit, equal"))?,
            "details" => self.details = find(&DETAILS, v, details_word).ok_or(bad("off, age, size, both"))?,
            "sort" => self.sort.key = find(&SORTS, v, sort_word).ok_or(bad("name, modified, size, type"))?,
            "sort_reverse" => self.sort.rev = flag()?,
            "folders_first" => self.folders_first = flag()?,
            "natural_sort" => self.natural_sort = flag()?,
            "show_hidden" => self.show_hidden = flag()?,
            "accent" => self.accent = find(&ACCENTS, v, accent_word).ok_or(bad("indigo, teal, violet, amber, mono"))?,
            "palette" => self.palette = find(&PALETTES, v, palette_word).ok_or(bad("ember, aurora, mono"))?,
            "heat_range" => self.heat_range = find(&RANGES, v, range_word).ok_or(bad("day, week, month, year, 5y"))?,
            "lines" => self.lines = find(&LINES, v, lines_word).ok_or(bad("rounded, square, heavy, double, ascii"))?,
            "branch_offset" => self.branch_offset = num(0, 4)?,
            "legend" => self.legend = flag()?,
            "speed" => self.speed = find(&SPEEDS, v, speed_word).ok_or(bad("slow, normal, fast, instant"))?,
            "live" => self.live = flag()?,
            "ripples" => self.ripples = flag()?,
            "git" => self.git = flag()?,
            "dim_ignored" => self.dim_ignored = flag()?,
            "explode_ignored" => self.explode_ignored = flag()?,
            "step" => self.step = find(&STEPS, v, step_word).ok_or(bad("folder, column, tree"))?,
            "mouse" => self.mouse = flag()?,
            "wheel_speed" => self.wheel_speed = v.parse().ok().filter(|n| WHEEL_SPEEDS.contains(n)).ok_or(bad("1, 2, 3, 5"))?,
            "momentum" => self.momentum = find(&MOMENTA, v, momentum_word).ok_or(bad("off, short, medium, long"))?,
            "graphics" => self.graphics = find(&GRAPHICS, v, graphics_word).ok_or(bad("auto, pixels, blocks, off"))?,
            "preview" => self.preview = find(&PREVIEWS, v, preview_word).ok_or(bad("styled, bat, plain"))?,
            "wrap" => self.wrap = flag()?,
            "remember" => self.remember = flag()?,
            _ => return Err(format!("unknown setting {key:?}")),
        }
        Ok(())
    }

    /// Read `key = value` lines. Returns the settings and a note per line it
    /// couldn't use; everything else still applies.
    pub fn parse(text: &str) -> (Settings, Vec<String>) {
        let mut s = Settings::default();
        let mut errs = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.split(" #").next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                errs.push(format!("line {}: expected key = value", n + 1));
                continue;
            };
            let v = v.trim().trim_matches('"');
            if let Err(e) = s.set(k.trim(), v) {
                errs.push(format!("line {}: {e}", n + 1));
            }
        }
        (s, errs)
    }

    /// Settings from the config file, or defaults if there isn't one.
    pub fn load(path: Option<&Path>) -> (Settings, Vec<String>) {
        match path.map(fs::read_to_string) {
            Some(Ok(text)) => Settings::parse(&text),
            _ => (Settings::default(), Vec::new()),
        }
    }

    /// Write one key to the config file, keeping every other line as it is.
    pub fn save(&self, path: &Path, key: &str) -> std::io::Result<()> {
        let old = fs::read_to_string(path).unwrap_or_else(|_| HEADER.to_string());
        let fresh = format!("{key} = {}", self.store(key));
        let mut found = false;
        let mut lines: Vec<String> = old
            .lines()
            .map(|l| {
                let is_key = l.split_once('=').is_some_and(|(k, _)| k.trim() == key) && !l.trim_start().starts_with('#');
                if is_key && !found {
                    found = true;
                    fresh.clone()
                } else {
                    l.to_string()
                }
            })
            .collect();
        if !found {
            lines.push(fresh);
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, lines.join("\n") + "\n")
    }
}

const HEADER: &str = "# tb settings. Press , in tb to change them, or edit this file.";

/// `$XDG_CONFIG_HOME/tb/config.toml`, else `~/.config/tb/config.toml`.
/// `TB_CONFIG` names another file.
pub fn path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("TB_CONFIG") {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("tb").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_values_and_reports_bad_lines() {
        let (s, errs) = Settings::parse(
            "# mine\nrow_spacing = 2\nsort = \"size\"  # biggest first\nsort_reverse = true\n\
             accent = \"teal\"\nmax_name = 99\nwho = 1\nlive = maybe\n",
        );
        assert_eq!(s.row_spacing, 2);
        assert_eq!(s.sort, Sort { key: SortKey::Size, rev: true });
        assert_eq!(s.accent, Accent::Teal);
        assert_eq!(s.max_name, 28, "out of range keeps the default");
        assert!(s.live);
        assert_eq!(errs.len(), 3, "{errs:?}");
        assert!(errs[0].starts_with("line 6: max_name"));
    }

    #[test]
    fn every_item_round_trips_through_the_file() {
        let mut s = Settings::default();
        for it in &ITEMS {
            s.adjust(it.key, 1);
        }
        let text: String = ITEMS.iter().map(|it| format!("{} = {}\n", it.key, s.store(it.key))).collect();
        let (back, errs) = Settings::parse(&text);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(back, s);
        for it in &ITEMS {
            s.reset(it.key);
        }
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn save_rewrites_one_line_and_keeps_the_rest() {
        let p = std::env::temp_dir().join(format!("tb-cfg-{}/tb/config.toml", std::process::id()));
        let _ = fs::remove_dir_all(p.parent().unwrap().parent().unwrap());
        let mut s = Settings { row_spacing: 1, ..Settings::default() };
        s.save(&p, "row_spacing").unwrap();
        fs::write(&p, fs::read_to_string(&p).unwrap() + "# my note\naccent = \"amber\"\n").unwrap();
        s.row_spacing = 3;
        s.save(&p, "row_spacing").unwrap();
        s.save(&p, "speed").unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.starts_with(HEADER));
        assert!(text.contains("row_spacing = 3\n# my note\naccent = \"amber\"\nspeed = \"normal\"\n"), "{text}");
        assert_eq!(text.matches("row_spacing").count(), 1);
        let (back, errs) = Settings::load(Some(&p));
        assert!(errs.is_empty());
        assert_eq!((back.row_spacing, back.accent), (3, Accent::Amber));
    }

    #[test]
    fn numbers_clamp_and_choices_wrap() {
        let mut s = Settings::default();
        s.adjust("row_spacing", -1);
        assert_eq!(s.row_spacing, 0);
        s.adjust("column_gap", -1);
        assert_eq!(s.column_gap, 3);
        s.adjust("accent", -1);
        assert_eq!(s.accent, Accent::Mono);
        s.wheel_speed = 3;
        s.adjust("wheel_speed", 1);
        s.adjust("wheel_speed", 1);
        assert_eq!(s.wheel_speed, 5, "3 → 5, then holds");
        s.sort.rev = true;
        s.adjust("sort", 1);
        assert_eq!(s.sort, Sort { key: SortKey::Modified, rev: false }, "a new key starts in its natural direction");
    }
}
