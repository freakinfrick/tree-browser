//! Pure horizontal-tree layout, in two passes:
//!
//! 1. `layout`: tree + cursor -> target world positions for every visible node,
//!    plus the parent->children blocks. Each depth is a column. An expanded
//!    dir's children form a contiguous block in the next column, centered on
//!    the parent row and pushed down past the previous block.
//! 2. `lines`: blocks + *any* positions -> connector cells. Taking positions as
//!    a function lets the renderer feed animated positions, so connectors
//!    stretch and grow with the motion instead of snapping.
//!
//! Each parent's elbow gets its own trunk column so trunks never merge.
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::SystemTime;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::settings::{Columns, Details, LineStyle};
use crate::tree::Tree;

pub const MAXW: usize = 28;

/// Room in the layout, from the settings.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Spacing {
    /// Blank rows between entries.
    pub rows: i32,
    /// Columns between a column's widest name and the next column. At least
    /// 3: the cell after a name holds its bud or git marker, and trunks
    /// start past that.
    pub gap: i32,
    /// Names are cut to this many columns.
    pub maxw: usize,
    pub columns: Columns,
    pub details: Details,
    /// Line cells between each join and the name it leads to.
    pub branch: i32,
}

impl Default for Spacing {
    fn default() -> Spacing {
        Spacing { rows: 0, gap: 3, maxw: MAXW, columns: Columns::Fit, details: Details::Off, branch: 0 }
    }
}
pub const UP: u8 = 1;
pub const DOWN: u8 = 2;
pub const LEFT: u8 = 4;
pub const RIGHT: u8 = 8;

/// Line emphasis, highest wins per cell.
pub const DIM: u8 = 0;
pub const ACTIVE: u8 = 1;
pub const ROUTE: u8 = 2;

pub struct Placed {
    pub id: usize,
    pub x: i32,
    pub y: i32,
    /// Name, then (with name details on) padding and the details.
    pub label: String,
    /// Bytes of `label` that are the name.
    pub name: usize,
    /// Block hangs off the cursor path (not dimmed).
    pub active: bool,
}

pub struct Block {
    pub parent: usize,
    pub kids: Vec<usize>,
    /// Trunk index within its column gap; trunk sits at `bar_x - 1 - k`.
    pub k: i32,
    /// Branch offset: line cells between the joins and the kids' names.
    pub off: i32,
}

#[derive(Default)]
pub struct Layout {
    pub placed: Vec<Placed>,
    pub blocks: Vec<Block>,
    /// Cursor (x, y, width) at target position.
    pub cursor: (i32, i32, i32),
    /// One band per column: world x range [lo, hi) and its line node, None
    /// past the line's end (open folders off the line reach further). A band
    /// spans the pill margin plus the gap to the next column.
    pub cols: Vec<(i32, i32, Option<usize>)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cell {
    /// Direction bits (UP/DOWN/LEFT/RIGHT).
    pub mask: u8,
    /// Bits contributed by the route; horizontal ones draw as a double tube
    /// (except on heavy and double lines, where the route is color alone).
    pub route: u8,
    /// Bits only ignored entries contribute. On heavy and double lines
    /// these draw light, so ignored branches look thinner.
    pub thin: u8,
    /// Highest emphasis touching the cell.
    pub emph: u8,
    /// Parent dir of the branch that owns the cell (for its heat tint).
    pub owner: usize,
}

pub type Lines = HashMap<(i32, i32), Cell>;

/// Glyph per direction mask, one row per `LineStyle`.
const GLYPHS: [[char; 16]; 5] = [
    ['·', '│', '│', '│', '─', '╯', '╮', '┤', '─', '╰', '╭', '├', '─', '┴', '┬', '┼'],
    ['·', '│', '│', '│', '─', '┘', '┐', '┤', '─', '└', '┌', '├', '─', '┴', '┬', '┼'],
    ['·', '┃', '┃', '┃', '━', '┛', '┓', '┫', '━', '┗', '┏', '┣', '━', '┻', '┳', '╋'],
    ['·', '║', '║', '║', '═', '╝', '╗', '╣', '═', '╚', '╔', '╠', '═', '╩', '╦', '╬'],
    ['.', '|', '|', '|', '-', '+', '+', '+', '-', '+', '+', '+', '-', '+', '+', '+'],
];

static STYLE: AtomicU8 = AtomicU8::new(0);

pub fn set_line_style(s: LineStyle) {
    STYLE.store(s as u8, Ordering::Relaxed);
}

pub fn cell_glyph(c: &Cell) -> char {
    styled_cell_glyph(STYLE.load(Ordering::Relaxed) as usize, c)
}

/// Heavy or double lines where a light run crosses: [vertical heavy/double,
/// horizontal light], then [vertical light, horizontal heavy/double].
const MIXED: [[[char; 16]; 2]; 2] = [
    [
        ['·', '┃', '┃', '┃', '─', '┚', '┒', '┨', '─', '┖', '┎', '┠', '─', '┸', '┰', '╂'],
        ['·', '│', '│', '│', '━', '┙', '┑', '┥', '━', '┕', '┍', '┝', '━', '┷', '┯', '┿'],
    ],
    [
        ['·', '║', '║', '║', '─', '╜', '╖', '╢', '─', '╙', '╓', '╟', '─', '╨', '╥', '╫'],
        ['·', '│', '│', '│', '═', '╛', '╕', '╡', '═', '╘', '╒', '╞', '═', '╧', '╤', '╪'],
    ],
];

/// Glyph for a cell: the route's horizontal run is a double "tube"
/// (like the original's hollow cables); crossings keep the light verticals.
/// On heavy and double lines, runs that only ignored entries use go light.
fn styled_cell_glyph(style: usize, c: &Cell) -> char {
    let m = c.mask as usize & 15;
    if (style == 2 || style == 3) && c.thin != 0 {
        let axis = |bits: u8| c.mask & bits & !c.thin != 0 || c.mask & bits == 0;
        let set = &MIXED[style - 2];
        return match (axis(UP | DOWN), axis(LEFT | RIGHT)) {
            (true, true) => GLYPHS[style][m],
            (true, false) => set[0][m],
            (false, true) => set[1][m],
            (false, false) => GLYPHS[1][m],
        };
    }
    let plain = GLYPHS[style][m];
    if c.route & (LEFT | RIGHT) == 0 {
        return plain;
    }
    // Unicode has no double-horizontal joins with heavy verticals, the tube can't
    // stand out against double lines, and ASCII has no double set.
    match style {
        2 | 3 => return plain,
        4 => return if c.mask & (UP | DOWN) == 0 { '=' } else { '+' },
        _ => {}
    }
    match c.mask & 15 {
        15 => '╪',
        13 => '╧',
        14 => '╤',
        11 => '╞',
        9 => '╘',
        10 => '╒',
        7 => '╡',
        5 => '╛',
        6 => '╕',
        _ => '═',
    }
}


/// One column's labels and name lengths. Details sit in their own
/// right-aligned column after the names; equal columns pad every name to
/// the column width.
fn column_labels(tree: &Tree, level: &[(usize, i32, String, bool)], sp: Spacing, now: SystemTime) -> Vec<(String, usize)> {
    let details: Vec<String> = level.iter().map(|l| detail(tree, l.0, sp.details, now)).collect();
    if sp.details == Details::Off {
        return level.iter().map(|l| (l.2.clone(), l.2.len())).collect();
    }
    let namew = match sp.columns {
        Columns::Fit => level.iter().map(|l| l.2.width()).max().unwrap_or(0),
        Columns::Equal => sp.maxw,
    };
    let dw = details.iter().map(|d| d.width()).max().unwrap_or(0);
    level
        .iter()
        .zip(details)
        .map(|(l, d)| {
            let pad = namew - l.2.width() + 2 + dw - d.width();
            (format!("{}{}{d}", l.2, " ".repeat(pad)), l.2.len())
        })
        .collect()
}

/// Age and/or size, as the name details show them. A folder's size is
/// blank until its walk lands.
fn detail(tree: &Tree, id: usize, what: Details, now: SystemTime) -> String {
    let age = || short_age(now.duration_since(tree.heat(id)).unwrap_or_default().as_secs());
    let n = &tree.nodes[id];
    let size = || if n.is_dir && n.rec.is_none() { String::new() } else { short_size(tree.size(id)) };
    match what {
        Details::Off => String::new(),
        Details::Age => age(),
        Details::Size => size(),
        Details::Both => age_and_size(&age(), &size()),
    }
}

/// Both padded to four columns, so the dots line up down a column. A size
/// not known yet leaves its place blank.
fn age_and_size(age: &str, size: &str) -> String {
    let dot = if size.is_empty() { "   " } else { " · " };
    format!("{age:>4}{dot}{size:>4}")
}

/// `now 5m 3h 2d 4w 8mo 3y`.
pub fn short_age(secs: u64) -> String {
    const M: u64 = 60;
    const H: u64 = 60 * M;
    const D: u64 = 24 * H;
    match secs {
        s if s < M => "now".into(),
        s if s < H => format!("{}m", s / M),
        s if s < D => format!("{}h", s / H),
        s if s < 7 * D => format!("{}d", s / D),
        s if s < 30 * D => format!("{}w", s / (7 * D)),
        s if s < 365 * D => format!("{}mo", s / (30 * D)),
        s => format!("{}y", s / (365 * D)),
    }
}

/// `980B 4.2K 12M 1.3G`: at most four columns.
pub fn short_size(n: u64) -> String {
    let mut v = n as f64;
    for u in ["B", "K", "M", "G", "T"] {
        // 999.5 would round up to a fifth column.
        if v < 999.5 || u == "T" {
            return match u {
                "B" => format!("{n}B"),
                _ if v < 9.95 => format!("{v:.1}{u}"),
                _ => format!("{v:.0}{u}"),
            };
        }
        v /= 1024.0;
    }
    unreachable!()
}

pub fn truncate_to(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        w += c.width().unwrap_or(0);
        if w > max - 1 {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

/// The line: root..cursor, continued past the cursor through each expanded
/// dir's remembered child (or its first). Every spine node sits at y = 0.
pub fn spine(tree: &Tree, cursor: usize) -> Vec<usize> {
    let mut s = tree.path_to(cursor);
    let mut cur = cursor;
    while tree.nodes[cur].expanded {
        let kids = tree.kids(cur);
        let Some(next) = tree.nodes[cur].last.filter(|l| kids.contains(l)).or(kids.first().copied()) else { break };
        s.push(next);
        cur = next;
    }
    s
}

pub fn layout(tree: &Tree, cursor: usize, label_of: &dyn Fn(usize) -> String, sp: Spacing) -> Layout {
    let spine = spine(tree, cursor);
    let on_spine: HashSet<usize> = spine.iter().copied().collect();
    let mut out = Layout::default();
    // (id, y, label, active) for the current column, sorted by y.
    let cut = |s: String| truncate_to(&s, sp.maxw);
    let mut level = vec![(tree.root, 0, cut(label_of(tree.root)), true)];
    let mut x = 0i32;
    let now = SystemTime::now();
    for depth in 1.. {
        let labels = column_labels(tree, &level, sp, now);
        for ((id, y, _, active), (label, name)) in level.iter().zip(&labels) {
            if *id == cursor {
                out.cursor = (x, *y, label.width() as i32);
            }
            out.placed.push(Placed { id: *id, x, y: *y, label: label.clone(), name: *name, active: *active });
        }
        let widest = labels.iter().map(|l| l.0.width() as i32).max().unwrap_or(0);
        let colw = if sp.columns == Columns::Equal && sp.details == Details::Off { widest.max(sp.maxw as i32) } else { widest };
        let on_line = spine.get(depth - 1).filter(|s| level.iter().any(|l| l.0 == **s));
        let parents: Vec<_> = level
            .iter()
            .filter(|(id, ..)| tree.nodes[*id].expanded && !tree.kids(*id).is_empty())
            .collect();
        if parents.is_empty() {
            out.cols.push((x - 2, x + colw + 3, on_line.copied()));
            break;
        }
        let n = parents.len() as i32;
        let next_x = x + colw + n + sp.gap.max(3) + sp.branch.max(0);
        out.cols.push((x - 2, next_x - 2, on_line.copied()));

        // y0 per parent. The spine block is pinned so its spine child sits on
        // the line; blocks above pack upward from it, blocks below downward.
        let ideal = |py: i32, len: i32| py - (len - 1) / 2;
        let mut y0s = vec![0i32; parents.len()];
        let pin = parents.iter().position(|p| on_spine.contains(&p.0) && spine.get(depth).is_some());
        let (below_from, mut prev_end) = match pin {
            Some(i) => {
                let kids = tree.kids(parents[i].0);
                let idx = kids.iter().position(|k| *k == spine[depth]).unwrap_or(0) as i32;
                y0s[i] = -idx;
                let mut next_start = y0s[i];
                for j in (0..i).rev() {
                    let len = tree.kids(parents[j].0).len() as i32;
                    y0s[j] = ideal(parents[j].1, len).min(next_start - 1 - len);
                    next_start = y0s[j];
                }
                (i + 1, y0s[i] + tree.kids(parents[i].0).len() as i32 - 1)
            }
            None => (0, i32::MIN / 2),
        };
        for j in below_from..parents.len() {
            let len = tree.kids(parents[j].0).len() as i32;
            y0s[j] = ideal(parents[j].1, len).max(prev_end + 2);
            prev_end = y0s[j] + len - 1;
        }

        // Trunk slots, counted leftward from the bar. A block hanging below its
        // parent must sit right of every lower parent's trunk, so lower parents
        // go further left; a block raised above its parent is the mirror image.
        // Blocks going opposite ways never share rows, so each direction counts
        // from the bar on its own and every trunk hugs its block.
        let raised: Vec<bool> =
            parents.iter().zip(&y0s).map(|((pid, py, ..), y0)| y0 + tree.kids(*pid).len() as i32 - 1 < *py).collect();
        let ups = raised.iter().filter(|&&r| r).count() as i32;
        let (mut up, mut down) = (0, 0);
        let mut slots = Vec::with_capacity(raised.len());
        for &r in &raised {
            if r {
                slots.push(ups - 1 - up);
                up += 1;
            } else {
                slots.push(down);
                down += 1;
            }
        }
        let mut next = Vec::new();
        for (k, (pid, ..)) in parents.iter().enumerate() {
            let kids = tree.kids(*pid).to_vec();
            let slot = slots[k];
            let active = on_spine.contains(pid);
            for (i, &kid) in kids.iter().enumerate() {
                next.push((kid, y0s[k] + i as i32, cut(label_of(kid)), active));
            }
            out.blocks.push(Block { parent: *pid, kids, k: slot, off: sp.branch.max(0) });
        }
        next.sort_by_key(|n| n.1);
        x = next_x;
        level = next;
    }
    // Row spacing: lay out in rows, then spread them. Connectors are drawn
    // between whatever rows the nodes land on, so they stretch through the gaps.
    let pitch = 1 + sp.rows.max(0);
    if pitch > 1 {
        for p in &mut out.placed {
            p.y *= pitch;
        }
        out.cursor.1 *= pitch;
    }
    out
}

struct Canvas {
    cells: Lines,
    owner: usize,
    /// Whether what's drawn now leads to an entry git doesn't ignore.
    thick: bool,
    /// Per cell, the bits drawn while `thick`.
    thick_bits: HashMap<(i32, i32), u8>,
}

impl Canvas {
    fn add(&mut self, x: i32, y: i32, bits: u8, emph: u8) {
        let e = self.cells.entry((x, y)).or_insert(Cell { owner: self.owner, ..Cell::default() });
        e.mask |= bits;
        if self.thick {
            *self.thick_bits.entry((x, y)).or_default() |= bits;
        }
        if emph == ROUTE {
            e.route |= bits;
        }
        if emph >= e.emph {
            e.emph = emph;
            e.owner = self.owner;
        }
    }
    fn hseg(&mut self, x1: i32, x2: i32, y: i32, emph: u8) {
        let (a, b) = (x1.min(x2), x1.max(x2));
        for x in a..=b {
            let bits = if x > a { LEFT } else { 0 } | if x < b { RIGHT } else { 0 };
            self.add(x, y, bits, emph);
        }
    }
    fn vseg(&mut self, x: i32, y1: i32, y2: i32, emph: u8) {
        let (a, b) = (y1.min(y2), y1.max(y2));
        for y in a..=b {
            let bits = if y > a { UP } else { 0 } | if y < b { DOWN } else { 0 };
            self.add(x, y, bits, emph);
        }
    }
}

/// Connector cells for every block, from the given positions.
/// `pos(id)` -> (x, y, label width); None = node not drawn, block skipped.
/// `route` = root..cursor (brightest); `spine` = the whole line (active);
/// `ignored(id)` = git ignores it, so its branch may draw thinner.
pub fn lines(
    blocks: &[Block],
    pos: &dyn Fn(usize) -> Option<(i32, i32, i32)>,
    route: &HashSet<usize>,
    spine: &HashSet<usize>,
    ignored: &dyn Fn(usize) -> bool,
) -> Lines {
    let mut c = Canvas { cells: HashMap::new(), owner: 0, thick: true, thick_bits: HashMap::new() };
    for b in blocks {
        c.owner = b.parent;
        let Some((px, py, pw)) = pos(b.parent) else { continue };
        let kids: Vec<(usize, i32, i32)> =
            b.kids.iter().filter_map(|&k| pos(k).map(|(x, y, _)| (k, x, y))).collect();
        if kids.is_empty() {
            continue;
        }
        let start = px + pw;
        let bar_x = kids.iter().map(|k| k.1).min().unwrap() - 1 - b.off;
        if bar_x < start {
            continue; // still unfurling out of the parent label
        }
        let y0 = kids.iter().map(|k| k.2).min().unwrap();
        let y1 = kids.iter().map(|k| k.2).max().unwrap();
        let on = route.contains(&b.parent);
        let base = if spine.contains(&b.parent) { ACTIVE } else { DIM };
        let routed = kids.iter().find(|k| route.contains(&k.0)).map(|k| k.2);
        let top = if on && routed.is_some() { ROUTE } else { base };

        // A kid's own tick is thin if it's ignored; the bar and the join
        // into it stay thick while anything in the block isn't.
        let block_thick = kids.iter().any(|k| !ignored(k.0));
        for &(k, x, y) in &kids {
            c.thick = !ignored(k);
            let emph = if Some(y) == routed { top } else { base };
            // Offset 0 keeps the join on the name, even while kids slide in.
            if b.off > 0 && x - 1 > bar_x {
                c.hseg(bar_x, x - 1, y, emph);
                c.add(x - 1, y, RIGHT, emph);
            } else {
                c.add(bar_x, y, RIGHT, emph);
            }
        }
        c.thick = block_thick;
        c.vseg(bar_x, y0, y1, base);
        // Join the block at its top entry, from above or below; a parent level
        // with its block joins it straight across.
        let ty = if py > y1 { y0 } else { py.clamp(y0, y1) };
        if ty == py {
            c.hseg(start, bar_x, py, top);
        } else {
            let tx = (bar_x - 1 - b.k).max(start);
            c.hseg(start, tx, py, top);
            c.vseg(tx, py, ty, top);
            c.hseg(tx, bar_x, ty, top);
        }
        if let Some(ry) = routed.filter(|_| top == ROUTE) {
            c.vseg(bar_x, ty, ry, ROUTE);
        }
    }
    for (at, cell) in c.cells.iter_mut() {
        cell.thin = cell.mask & !c.thick_bits.get(at).copied().unwrap_or(0);
    }
    c.cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(name: &str, dirs: &[&str]) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("tb-layout-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for d in dirs {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        root
    }

    fn pos(l: &Layout, t: &Tree, name: &str) -> (i32, i32) {
        let p = l.placed.iter().find(|p| t.nodes[p.id].name == name).unwrap();
        (p.x, p.y)
    }

    fn name_of(t: &Tree) -> impl Fn(usize) -> String + '_ {
        |i| t.nodes[i].name.clone()
    }

    fn target_lines(l: &Layout, route: &HashSet<usize>) -> Lines {
        let m: HashMap<usize, (i32, i32, i32)> =
            l.placed.iter().map(|p| (p.id, (p.x, p.y, p.label.width() as i32))).collect();
        lines(&l.blocks, &|id| m.get(&id).copied(), route, route, &|_| false)
    }

    fn deep(name: &str) -> (Tree, usize, usize, usize) {
        let root = fixture(name, &["a/1/x", "a/2", "a/3", "b/9"]);
        let mut t = Tree::new(&root);
        t.load(0);
        t.nodes[0].expanded = true;
        let a = t.kids(0)[0];
        t.load(a);
        t.nodes[a].expanded = true;
        let one = t.kids(a)[0];
        let three = t.kids(a)[2];
        (t, a, one, three)
    }

    #[test]
    fn equal_columns_and_details_line_up() {
        let (t, a, _, three) = deep("equal");
        let fit = layout(&t, three, &name_of(&t), Spacing::default());
        let eq = layout(&t, three, &name_of(&t), Spacing { columns: Columns::Equal, maxw: 12, ..Spacing::default() });
        assert_eq!(pos(&fit, &t, "1").0 - pos(&fit, &t, "a").0, 1 + 1 + 3, "fit: name, one trunk, gap");
        assert_eq!(pos(&eq, &t, "1").0 - pos(&eq, &t, "a").0, 12 + 1 + 3, "equal: the column width");

        let det = layout(&t, three, &name_of(&t), Spacing { details: Details::Age, ..Spacing::default() });
        let row = |name: &str| det.placed.iter().find(|p| t.nodes[p.id].name == name).unwrap();
        assert_eq!(row("a").label, "a  now");
        assert_eq!(row("a").name, 1);
        assert_eq!(row("a").label.width(), row("b").label.width(), "details right-aligned");
        assert_eq!(det.cursor.2, row("3").label.width() as i32, "the pill covers the details");
        let _ = a;
    }

    #[test]
    fn branch_offset_reaches_from_join_to_name() {
        let (t, _, one, three) = deep("branch");
        let near = layout(&t, three, &name_of(&t), Spacing::default());
        let far = layout(&t, three, &name_of(&t), Spacing { branch: 2, ..Spacing::default() });
        assert_eq!(pos(&far, &t, "1").0 - pos(&near, &t, "1").0, 4, "2 more per column, two columns");
        let route: HashSet<usize> = t.path_to(one).into_iter().collect();
        let cells = target_lines(&far, &route);
        let (x, y) = pos(&far, &t, "2");
        let at = |dx: i32| cells[&(x - dx, y)].mask;
        assert_eq!((at(1), at(2)), (LEFT | RIGHT, LEFT | RIGHT), "the reach is a straight line");
        assert_eq!(at(3), UP | DOWN | RIGHT, "the join sits back on the trunk");
    }

    /// Cells two blocks both draw: a crossing or an overlap. Blocks never
    /// share a cell when the wiring is right.
    fn clashes(l: &Layout, route: &HashSet<usize>) -> Vec<(i32, i32)> {
        let m: HashMap<usize, (i32, i32, i32)> =
            l.placed.iter().map(|p| (p.id, (p.x, p.y, p.label.width() as i32))).collect();
        let mut seen: HashMap<(i32, i32), usize> = HashMap::new();
        let mut out = Vec::new();
        for b in &l.blocks {
            for cell in lines(std::slice::from_ref(b), &|id| m.get(&id).copied(), route, route, &|_| false).into_keys() {
                if seen.insert(cell, b.parent).is_some() {
                    out.push(cell);
                }
            }
        }
        out
    }

    #[test]
    fn wires_never_cross_in_a_fully_open_tree() {
        // Uneven fan-outs so blocks get pushed both above and below their parents.
        let mut dirs = Vec::new();
        for (i, n) in [1, 7, 2, 9, 1, 4, 12, 3, 1, 6].iter().enumerate() {
            for j in 0..*n {
                for k in 0..(i * j) % 4 {
                    dirs.push(format!("p{i:02}/c{j:02}/g{k}"));
                }
                dirs.push(format!("p{i:02}/c{j:02}"));
            }
        }
        let root = fixture("cross", &dirs.iter().map(String::as_str).collect::<Vec<_>>());
        let mut t = Tree::new(&root);
        let mut stack = vec![0];
        while let Some(id) = stack.pop() {
            t.load(id);
            t.nodes[id].expanded = true;
            stack.extend(t.kids(id));
        }
        let spacings = [
            Spacing::default(),
            Spacing { rows: 2, gap: 9, ..Spacing::default() },
            Spacing { branch: 3, ..Spacing::default() },
            Spacing { columns: Columns::Equal, details: Details::Age, maxw: 12, ..Spacing::default() },
        ];
        for sp in spacings {
            let mut bad = Vec::new();
            for cursor in 0..t.nodes.len() {
                let l = layout(&t, cursor, &name_of(&t), sp);
                let route: HashSet<usize> = t.path_to(cursor).into_iter().collect();
                let c = clashes(&l, &route);
                let m: HashMap<usize, (i32, i32)> = l.placed.iter().map(|p| (p.id, (p.x, p.y))).collect();
                let cells = target_lines(&l, &route);
                for b in &l.blocks {
                    let py = m[&b.parent].1;
                    let (bx, top) = (m[&b.kids[0]].0 - 1 - b.off, m[&b.kids[0]].1);
                    let bottom = m[b.kids.last().unwrap()].1;
                    if py < top || py > bottom {
                        assert!(cells[&(bx, top)].mask & LEFT != 0, "{sp:?}: {} joins its block at the top", t.nodes[b.parent].name);
                    }
                }
                if !c.is_empty() {
                    bad.push((t.nodes[cursor].name.clone(), c.len()));
                }
            }
            assert!(bad.is_empty(), "{sp:?}: {} of {} cursors cross wires: {:?}", bad.len(), t.nodes.len(), &bad[..bad.len().min(8)]);
        }
    }

    #[test]
    fn short_ages_and_sizes() {
        let d = 86400;
        let ages: Vec<String> = [5, 300, 7200, 3 * d, 15 * d, 100 * d, 800 * d].map(short_age).into();
        assert_eq!(ages, ["now", "5m", "2h", "3d", "2w", "3mo", "2y"]);
        let sizes: Vec<String> = [980, 4300, 12 << 20, 1395864371, 1023 * 1024].map(short_size).into();
        assert_eq!(sizes, ["980B", "4.2K", "12M", "1.3G", "1.0M"]);
    }

    #[test]
    fn every_line_style_draws_bars_and_joins() {
        for row in &GLYPHS {
            let g = |m: u8| row[m as usize];
            assert_eq!((g(UP), g(DOWN)), (g(UP | DOWN), g(UP | DOWN)), "one vertical bar");
            assert_eq!((g(LEFT), g(RIGHT)), (g(LEFT | RIGHT), g(LEFT | RIGHT)), "one horizontal bar");
            assert_ne!(g(UP | DOWN), g(LEFT | RIGHT));
            assert_ne!(g(UP | DOWN | RIGHT), g(UP | DOWN), "a join isn't a bar");
        }
        assert!(GLYPHS[4].iter().all(char::is_ascii), "ascii style is ascii");
        let join = Cell { mask: LEFT | RIGHT | DOWN, route: LEFT | RIGHT, ..Cell::default() };
        assert_eq!(styled_cell_glyph(0, &join), '╤', "rounded: the route is a double tube");
        assert_eq!(styled_cell_glyph(2, &join), '┳', "heavy: joins match the heavy branches");
    }

    #[test]
    fn age_and_size_line_up() {
        let (a, b) = (age_and_size("1d", "331K"), age_and_size("11mo", "11K"));
        assert_eq!((a.width(), a.find('·')), (b.width(), b.find('·')));
        let walking = age_and_size("1d", "");
        assert_eq!((walking.width(), walking.find("1d")), (a.width(), a.find("1d")), "age stays in its place");
    }

    #[test]
    fn spacing_spreads_rows_widens_gaps_and_cuts_names() {
        let (t, _, one, three) = deep("spacing");
        let tight = layout(&t, three, &name_of(&t), Spacing::default());
        let airy = layout(&t, three, &name_of(&t), Spacing { rows: 2, gap: 7, ..Spacing::default() });
        for name in ["1", "2", "3", "a", "b"] {
            assert_eq!(pos(&airy, &t, name).1, pos(&tight, &t, name).1 * 3, "{name}: every row three apart");
        }
        assert_eq!(airy.cursor.1, tight.cursor.1 * 3);
        assert_eq!(pos(&airy, &t, "1").0 - pos(&tight, &t, "1").0, 8, "4 more gap per column, two columns");
        let route: HashSet<usize> = t.path_to(one).into_iter().collect();
        let cells = target_lines(&airy, &route);
        let (x, y) = pos(&airy, &t, "2");
        assert!(cells.contains_key(&(x - 1, y - 1)) && cells.contains_key(&(x - 1, y - 2)), "trunk runs through the blank rows");
        let long = fixture("spacing-long", &["a-very-long-folder-name"]);
        let mut t = Tree::new(&long);
        t.load(0);
        t.nodes[0].expanded = true;
        let l = layout(&t, t.kids(0)[0], &name_of(&t), Spacing { maxw: 12, ..Spacing::default() });
        assert_eq!(l.placed[1].label, "a-very-long…", "12 columns, ellipsis included");
    }

    #[test]
    fn spine_is_one_straight_line() {
        let (mut t, _, one, _) = deep("straight");
        t.load(one);
        t.nodes[one].expanded = true;
        let l = layout(&t, one, &name_of(&t), Spacing::default());
        for id in spine(&t, one) {
            let p = l.placed.iter().find(|p| p.id == id).unwrap();
            assert_eq!(p.y, 0, "{} on the line", t.nodes[id].name);
        }
        assert_eq!(l.cursor.1, 0);
        assert_eq!(pos(&l, &t, "x").1, 0, "line continues past the cursor");
    }

    #[test]
    fn column_scrolls_about_the_line() {
        let (t, _, one, three) = deep("scroll");
        let at1 = layout(&t, one, &name_of(&t), Spacing::default());
        let at3 = layout(&t, three, &name_of(&t), Spacing::default());
        assert_eq!((pos(&at1, &t, "1").1, pos(&at3, &t, "3").1), (0, 0), "selection always on the line");
        assert_eq!(pos(&at1, &t, "1").1 - pos(&at3, &t, "1").1, 2, "column slid up by two rows");
        assert_eq!(pos(&at3, &t, "a").1, 0, "parent column did not move");
    }

    #[test]
    fn column_bands_follow_the_line() {
        let (mut t, a, one, three) = deep("bands");
        t.load(one);
        t.nodes[one].expanded = true;
        let l = layout(&t, one, &name_of(&t), Spacing::default());
        let ids: Vec<usize> = l.cols.iter().filter_map(|c| c.2).collect();
        assert_eq!(ids, spine(&t, one), "a band per line column, ancestors and descendants");
        assert_eq!(ids[1], a);
        for w in l.cols.windows(2) {
            assert_eq!(w[0].1, w[1].0, "bands tile with no gaps");
        }
        for (lo, hi, id) in l.cols.iter().filter_map(|c| Some((c.0, c.1, c.2?))) {
            let p = l.placed.iter().find(|p| p.id == id).unwrap();
            assert!(lo < p.x && p.x < hi, "{} inside its band", t.nodes[id].name);
        }
        // The cursor on 3: 1 stays open off the line, so x's column has a band too.
        let back = layout(&t, three, &name_of(&t), Spacing::default());
        let last = back.cols.last().unwrap();
        let (x, _) = pos(&back, &t, "x");
        assert!(last.2.is_none() && last.0 < x && x < last.1, "a band past the line's end, with no line node");
    }

    #[test]
    fn blocks_above_the_line_do_not_overlap_it() {
        let root = fixture("above", &["a/1", "a/2", "a/3", "a/4", "b/5", "b/6", "b/7"]);
        let mut t = Tree::new(&root);
        t.load(0);
        t.nodes[0].expanded = true;
        for &k in &t.kids(0).to_vec() {
            t.load(k);
            t.nodes[k].expanded = true;
        }
        let b = t.kids(0)[1];
        let l = layout(&t, b, &name_of(&t), Spacing::default()); // spine through b; a's block sits above it
        let (y4, y5) = (pos(&l, &t, "4").1, pos(&l, &t, "5").1);
        assert_eq!(y5, 0, "b's first child on the line");
        assert!(y4 <= y5 - 2, "a's block ends above b's with a gap: {y4} vs {y5}");
    }

    #[test]
    fn sibling_blocks_do_not_overlap_and_trunks_differ() {
        let root = fixture("overlap", &["a/1", "a/2", "a/3", "a/4", "b/5", "b/6", "b/7", "b/8", "c/9"]);
        let mut t = Tree::new(&root);
        t.load(0);
        t.nodes[0].expanded = true;
        for &k in &t.kids(0).to_vec() {
            t.load(k);
            t.nodes[k].expanded = true;
        }
        let l = layout(&t, 0, &name_of(&t), Spacing::default());
        let ys: Vec<i32> = ["1", "2", "3", "4", "5", "6", "7", "8"].iter().map(|n| pos(&l, &t, n).1).collect();
        for w in ys.windows(2) {
            assert!(w[1] > w[0], "rows strictly increase: {ys:?}");
        }
        assert!(ys[4] - ys[3] >= 2, "blank row between blocks");
        // a's block is level with a (no trunk); b and c route down via
        // their own trunk columns, which must not merge.
        let bx = pos(&l, &t, "1").0 - 1;
        let ax = pos(&l, &t, "a").0;
        let trunk_xs: HashSet<i32> = target_lines(&l, &HashSet::new())
            .iter()
            .filter(|((x, _), c)| *x > ax && *x < bx && c.mask & (UP | DOWN) != 0)
            .map(|((x, _), _)| *x)
            .collect();
        assert_eq!(trunk_xs.len(), 2, "one trunk per displaced parent: {trunk_xs:?}");
    }

    #[test]
    fn route_emphasis_reaches_cursor_row() {
        let root = fixture("route", &["a/1", "a/2", "a/3"]);
        let mut t = Tree::new(&root);
        t.load(0);
        t.nodes[0].expanded = true;
        let a = t.kids(0)[0];
        t.load(a);
        t.nodes[a].expanded = true;
        let three = t.kids(a)[2];
        let l = layout(&t, three, &name_of(&t), Spacing::default());
        let route: HashSet<usize> = t.path_to(three).into_iter().collect();
        let ln = target_lines(&l, &route);
        let (x3, y3) = pos(&l, &t, "3");
        assert_eq!(ln[&(x3 - 1, y3)].emph, ROUTE, "bar tick at cursor row is on the route");
        let (x1, y1) = pos(&l, &t, "1");
        assert_eq!(ln[&(x1 - 1, y1)].emph, ACTIVE, "sibling tick is not");
    }

    #[test]
    fn unfurling_block_draws_nothing() {
        let root = fixture("unfurl", &["a/1"]);
        let mut t = Tree::new(&root);
        t.load(0);
        t.nodes[0].expanded = true;
        let l = layout(&t, 0, &name_of(&t), Spacing::default());
        // Kid sitting inside the parent label (mid-animation): no connector.
        let ln = lines(&l.blocks, &|id| Some(if id == 0 { (0, 0, 10) } else { (5, 0, 1) }), &HashSet::new(), &HashSet::new(), &|_| false);
        assert!(ln.is_empty());
    }

    #[test]
    fn route_is_a_tube_and_crossings_join() {
        let c = |mask, route| Cell { mask, route, emph: ROUTE, ..Cell::default() };
        assert_eq!(cell_glyph(&c(LEFT | RIGHT, LEFT | RIGHT)), '═');
        assert_eq!(cell_glyph(&c(UP | DOWN | LEFT | RIGHT, LEFT | RIGHT)), '╪', "light bar through the tube");
        assert_eq!(cell_glyph(&c(DOWN | RIGHT, RIGHT)), '╒');
        assert_eq!(cell_glyph(&c(UP | DOWN, 0)), '│', "no route bits: light");
    }

    #[test]
    fn ignored_branches_draw_thin_on_double_and_heavy() {
        let c = |mask, thin| Cell { mask, thin, ..Cell::default() };
        let tick = c(UP | DOWN | RIGHT, RIGHT);
        assert_eq!(styled_cell_glyph(3, &tick), '╟', "double bar, light tick");
        assert_eq!(styled_cell_glyph(2, &tick), '┠', "heavy bar, light tick");
        assert_eq!(styled_cell_glyph(3, &c(UP | RIGHT, RIGHT)), '╙', "last kid ignored");
        assert_eq!(styled_cell_glyph(3, &c(LEFT | RIGHT, LEFT | RIGHT)), '─');
        assert_eq!(styled_cell_glyph(3, &c(UP | DOWN | RIGHT, UP | DOWN | RIGHT)), '├', "all ignored: all light");
        assert_eq!(styled_cell_glyph(3, &c(UP | DOWN | RIGHT, 0)), '╠');
        assert_eq!(styled_cell_glyph(0, &tick), '├', "rounded has no thinner line");
    }

    #[test]
    fn ignored_kid_gets_a_thin_tick_on_a_thick_bar() {
        let root = fixture("thin", &["a", "b", "c"]);
        let mut t = Tree::new(&root);
        t.load(0);
        t.nodes[0].expanded = true;
        let l = layout(&t, 0, &name_of(&t), Spacing::default());
        let m: HashMap<usize, (i32, i32, i32)> = l.placed.iter().map(|p| (p.id, (p.x, p.y, p.label.width() as i32))).collect();
        let b = t.kids(0)[1];
        let ln = lines(&l.blocks, &|id| m.get(&id).copied(), &HashSet::new(), &HashSet::new(), &|id| id == b);
        let (xb, yb) = pos(&l, &t, "b");
        let (xa, ya) = pos(&l, &t, "a");
        assert_eq!(ln[&(xb - 1, yb)].thin, RIGHT, "b's tick is thin, the bar through it isn't");
        assert_eq!(ln[&(xa - 1, ya)].thin, 0);
        let all = lines(&l.blocks, &|id| m.get(&id).copied(), &HashSet::new(), &HashSet::new(), &|id| id != 0);
        assert!(all.values().all(|c| c.thin == c.mask), "a block of only ignored kids is all thin");
    }

    #[test]
    fn cells_remember_their_branch() {
        let root = fixture("owner", &["a/1", "b/2"]);
        let mut t = Tree::new(&root);
        t.load(0);
        t.nodes[0].expanded = true;
        let (a, b) = (t.kids(0)[0], t.kids(0)[1]);
        for k in [a, b] {
            t.load(k);
            t.nodes[k].expanded = true;
        }
        let l = layout(&t, a, &name_of(&t), Spacing::default());
        let ln = target_lines(&l, &HashSet::new());
        let (x2, y2) = pos(&l, &t, "2");
        assert_eq!(ln[&(x2 - 1, y2)].owner, b, "tick into b's block belongs to b");
    }

    #[test]
    fn dotfiles_hidden_unless_shown_or_on_the_path() {
        let root = fixture("hidden", &[".secret/inner", "plain"]);
        let mut t = Tree::new(&root);
        t.load(0);
        t.nodes[0].expanded = true;
        let names = |t: &Tree| t.kids(0).iter().map(|&k| t.nodes[k].name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&t), ["plain"]);
        let secret = t.nodes[0].children.as_ref().unwrap().iter().copied().find(|&k| t.nodes[k].name == ".secret").unwrap();
        t.reveal = t.path_to(secret).into_iter().collect();
        assert_eq!(names(&t), [".secret", "plain"], "cursor path stays visible");
        t.reveal.clear();
        t.show_hidden = true;
        assert_eq!(names(&t), [".secret", "plain"]);
    }

    #[test]
    fn truncate_adds_ellipsis() {
        let s = "x".repeat(40);
        let t = truncate_to(&s, MAXW);
        assert_eq!(t.width(), MAXW);
        assert!(t.ends_with('…'));
        assert_eq!(truncate_to("short", MAXW), "short");
    }
}
