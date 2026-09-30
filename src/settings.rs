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

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Settings {
    pub row_spacing: u8,
    pub column_gap: u8,
    pub max_name: u8,
    pub sort: Sort,
    pub show_hidden: bool,
    pub accent: Accent,
    pub palette: Palette,
    pub legend: bool,
    pub speed: Speed,
    pub live: bool,
    pub ripples: bool,
    pub git: bool,
    pub dim_ignored: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            row_spacing: 0,
            column_gap: 3,
            max_name: 28,
            sort: Sort::default(),
            show_hidden: false,
            accent: Accent::Indigo,
            palette: Palette::Ember,
            legend: true,
            speed: Speed::Normal,
            live: true,
            ripples: true,
            git: true,
            dim_ignored: true,
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

pub const ITEMS: [Item; 14] = [
    Item { key: "row_spacing", label: "Row spacing", section: "Layout", help: "Blank rows between entries. More air, fewer entries on screen." },
    Item { key: "column_gap", label: "Column gap", section: "Layout", help: "Space between a column's longest name and the next column." },
    Item { key: "max_name", label: "Name width", section: "Layout", help: "Longer names are cut to this many columns with a …" },
    Item { key: "sort", label: "Sort by", section: "Order", help: "Same as o in the tree. Folders sort by what's inside them." },
    Item { key: "sort_reverse", label: "Reverse", section: "Order", help: "Same as O: oldest, smallest or z–a first." },
    Item { key: "show_hidden", label: "Dotfiles", section: "Order", help: "Same as . in the tree." },
    Item { key: "accent", label: "Accent", section: "Look", help: "Color of the lines, the selector and the highlights." },
    Item { key: "palette", label: "Heat colors", section: "Look", help: "Recency gradient. Aurora avoids red-green; mono is brightness only." },
    Item { key: "legend", label: "Legend", section: "Look", help: "The now ▮▮▮ old color key in the status bar." },
    Item { key: "speed", label: "Motion", section: "Look", help: "How fast folders unfurl and the view glides. Instant turns animation off." },
    Item { key: "live", label: "Live updates", section: "Behavior", help: "Re-list open folders about once a second as files change." },
    Item { key: "ripples", label: "Ripples", section: "Behavior", help: "Flash a live change and let it climb the tree." },
    Item { key: "git", label: "Git status", section: "Behavior", help: "Markers, branch and diffs inside git repos." },
    Item { key: "dim_ignored", label: "Dim ignored", section: "Behavior", help: "Fade files git ignores, like target/ and node_modules/." },
];

const SORTS: [SortKey; 4] = [SortKey::Name, SortKey::Modified, SortKey::Size, SortKey::Type];
const SPEEDS: [Speed; 4] = [Speed::Slow, Speed::Normal, Speed::Fast, Speed::Instant];
const ACCENTS: [Accent; 5] = [Accent::Indigo, Accent::Teal, Accent::Violet, Accent::Amber, Accent::Mono];
const PALETTES: [Palette; 3] = [Palette::Ember, Palette::Aurora, Palette::Mono];

/// Step through `all` from `cur`, wrapping.
fn cycle<T: PartialEq + Copy>(all: &[T], cur: T, dir: i32) -> T {
    let i = all.iter().position(|&x| x == cur).unwrap_or(0) as i32;
    all[(i + dir).rem_euclid(all.len() as i32) as usize]
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
            "sort" => sort_word(self.sort.key).into(),
            "sort_reverse" => on_off(self.sort.rev),
            "show_hidden" => if self.show_hidden { "shown".into() } else { "hidden".into() },
            "accent" => accent_word(self.accent).into(),
            "palette" => palette_word(self.palette).into(),
            "legend" => on_off(self.legend),
            "speed" => speed_word(self.speed).into(),
            "live" => on_off(self.live),
            "ripples" => on_off(self.ripples),
            "git" => on_off(self.git),
            "dim_ignored" => on_off(self.dim_ignored),
            _ => String::new(),
        }
    }

    /// The value as the config file stores it.
    pub fn store(&self, key: &str) -> String {
        match key {
            "row_spacing" | "column_gap" | "max_name" => self.show(key),
            "sort_reverse" => self.sort.rev.to_string(),
            "show_hidden" => self.show_hidden.to_string(),
            "legend" | "live" | "ripples" | "git" | "dim_ignored" => (self.show(key) == "on").to_string(),
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
            "sort" => self.sort = Sort { key: cycle(&SORTS, self.sort.key, dir), rev: false },
            "sort_reverse" => self.sort.rev ^= true,
            "show_hidden" => self.show_hidden ^= true,
            "accent" => self.accent = cycle(&ACCENTS, self.accent, dir),
            "palette" => self.palette = cycle(&PALETTES, self.palette, dir),
            "legend" => self.legend ^= true,
            "speed" => self.speed = cycle(&SPEEDS, self.speed, dir),
            "live" => self.live ^= true,
            "ripples" => self.ripples ^= true,
            "git" => self.git ^= true,
            "dim_ignored" => self.dim_ignored ^= true,
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
            "sort" => self.sort.key = d.sort.key,
            "sort_reverse" => self.sort.rev = d.sort.rev,
            "show_hidden" => self.show_hidden = d.show_hidden,
            "accent" => self.accent = d.accent,
            "palette" => self.palette = d.palette,
            "legend" => self.legend = d.legend,
            "speed" => self.speed = d.speed,
            "live" => self.live = d.live,
            "ripples" => self.ripples = d.ripples,
            "git" => self.git = d.git,
            "dim_ignored" => self.dim_ignored = d.dim_ignored,
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
            "sort" => self.sort.key = find(&SORTS, v, sort_word).ok_or(bad("name, modified, size, type"))?,
            "sort_reverse" => self.sort.rev = flag()?,
            "show_hidden" => self.show_hidden = flag()?,
            "accent" => self.accent = find(&ACCENTS, v, accent_word).ok_or(bad("indigo, teal, violet, amber, mono"))?,
            "palette" => self.palette = find(&PALETTES, v, palette_word).ok_or(bad("ember, aurora, mono"))?,
            "legend" => self.legend = flag()?,
            "speed" => self.speed = find(&SPEEDS, v, speed_word).ok_or(bad("slow, normal, fast, instant"))?,
            "live" => self.live = flag()?,
            "ripples" => self.ripples = flag()?,
            "git" => self.git = flag()?,
            "dim_ignored" => self.dim_ignored = flag()?,
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
        s.sort.rev = true;
        s.adjust("sort", 1);
        assert_eq!(s.sort, Sort { key: SortKey::Modified, rev: false }, "a new key starts in its natural direction");
    }
}
