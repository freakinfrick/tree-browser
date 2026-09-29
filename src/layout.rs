//! Pure horizontal-tree layout: tree + cursor -> world-space labels and line cells.
//!
//! Each depth is a column. An expanded dir's children form a contiguous block in
//! the next column, centered on the parent row and pushed down past the previous
//! block. Parent and block are joined by an elbow connector whose vertical trunk
//! gets its own x per parent, so trunks never merge.
use std::collections::{HashMap, HashSet};
use unicode_width::UnicodeWidthStr;

use crate::tree::Tree;

pub const MAXW: usize = 28;
pub const UP: u8 = 1;
pub const DOWN: u8 = 2;
pub const LEFT: u8 = 4;
pub const RIGHT: u8 = 8;

pub struct Placed {
    pub id: usize,
    pub x: i32,
    pub y: i32,
    pub label: String,
    /// Block hangs off the cursor path (not dimmed).
    pub active: bool,
}

#[derive(Default)]
pub struct Layout {
    pub placed: Vec<Placed>,
    /// (x, y) -> (direction mask, active)
    pub lines: HashMap<(i32, i32), (u8, bool)>,
    /// Cursor (x, y, width).
    pub cursor: (i32, i32, i32),
}

pub fn glyph(mask: u8) -> char {
    [' ', '│', '│', '│', '─', '┘', '┐', '┤', '─', '└', '┌', '├', '─', '┴', '┬', '┼'][mask as usize & 15]
}

/// Truncate to MAXW display columns with a trailing ellipsis.
pub fn truncate(s: &str) -> String {
    truncate_to(s, MAXW)
}

pub fn truncate_to(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        if out.width() + c.to_string().width() > max - 1 {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

impl Layout {
    fn add(&mut self, x: i32, y: i32, bits: u8, active: bool) {
        let e = self.lines.entry((x, y)).or_insert((0, false));
        e.0 |= bits;
        e.1 |= active;
    }
    fn hseg(&mut self, x1: i32, x2: i32, y: i32, active: bool) {
        let (a, b) = (x1.min(x2), x1.max(x2));
        for x in a..=b {
            let bits = if x > a { LEFT } else { 0 } | if x < b { RIGHT } else { 0 };
            self.add(x, y, bits, active);
        }
    }
    fn vseg(&mut self, x: i32, y1: i32, y2: i32, active: bool) {
        let (a, b) = (y1.min(y2), y1.max(y2));
        for y in a..=b {
            let bits = if y > a { UP } else { 0 } | if y < b { DOWN } else { 0 };
            self.add(x, y, bits, active);
        }
    }
}

pub fn layout(tree: &Tree, cursor: usize, label_of: &dyn Fn(usize) -> String) -> Layout {
    let on_path: HashSet<usize> = tree.path_to(cursor).into_iter().collect();
    let mut out = Layout::default();
    // (id, y, label, active) for the current column.
    let mut level = vec![(tree.root, 0, truncate(&label_of(tree.root)), true)];
    let mut x = 0i32;
    loop {
        for (id, y, label, active) in &level {
            if *id == cursor {
                out.cursor = (x, *y, label.width() as i32);
            }
            out.placed.push(Placed { id: *id, x, y: *y, label: label.clone(), active: *active });
        }
        let parents: Vec<_> = level
            .iter()
            .filter(|(id, ..)| tree.nodes[*id].expanded && !tree.kids(*id).is_empty())
            .collect();
        if parents.is_empty() {
            break;
        }
        let colw = level.iter().map(|l| l.2.width() as i32).max().unwrap_or(0);
        let n = parents.len() as i32;
        let next_x = x + colw + n + 3;
        let bar_x = next_x - 1;
        let mut next = Vec::new();
        let mut prev_end = i32::MIN / 2;
        for (k, (pid, py, plabel, _)) in parents.iter().enumerate() {
            let kids = tree.kids(*pid);
            let len = kids.len() as i32;
            let y0 = (py - (len - 1) / 2).max(prev_end + 2);
            prev_end = y0 + len - 1;
            let active = on_path.contains(pid);
            for (i, &kid) in kids.iter().enumerate() {
                next.push((kid, y0 + i as i32, truncate(&label_of(kid)), active));
                out.add(bar_x, y0 + i as i32, RIGHT, active);
            }
            out.vseg(bar_x, y0, prev_end, active);
            let start = x + plabel.width() as i32;
            let ty = (*py).clamp(y0, prev_end);
            if ty == *py {
                out.hseg(start, bar_x, *py, active);
            } else {
                let tx = next_x - 2 - k as i32;
                out.hseg(start, tx, *py, active);
                out.vseg(tx, *py, ty, active);
                out.hseg(tx, bar_x, ty, active);
            }
        }
        x = next_x;
        level = next;
    }
    out
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

    #[test]
    fn block_centered_on_parent() {
        let root = fixture("center", &["a/1", "a/2", "a/3"]);
        let mut t = Tree::new(&root);
        t.load(0);
        t.nodes[0].expanded = true;
        let a = t.kids(0)[0];
        t.load(a);
        t.nodes[a].expanded = true;
        let l = layout(&t, a, &name_of(&t));
        let (_, ay) = pos(&l, &t, "a");
        let (_, y2) = pos(&l, &t, "2");
        assert_eq!(ay, y2, "middle child sits on parent row");
        assert_eq!(pos(&l, &t, "1").1, ay - 1, "blocks may go above parent (negative y ok)");
        assert!(ay == 0 && pos(&l, &t, "1").1 < 0);
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
        let l = layout(&t, 0, &name_of(&t));
        let ys: Vec<i32> = ["1", "2", "3", "4", "5", "6", "7", "8"].iter().map(|n| pos(&l, &t, n).1).collect();
        for w in ys.windows(2) {
            assert!(w[1] > w[0], "rows strictly increase: {ys:?}");
        }
        assert!(ys[4] - ys[3] >= 2, "blank row between blocks");
        // a's block is level with a (no trunk); b and c route down via
        // their own trunk columns, which must not merge.
        let bx = pos(&l, &t, "1").0 - 1;
        let ax = pos(&l, &t, "a").0;
        let trunk_xs: HashSet<i32> = l
            .lines
            .iter()
            .filter(|((x, _), (m, _))| *x > ax && *x < bx && m & (UP | DOWN) != 0)
            .map(|((x, _), _)| *x)
            .collect();
        assert_eq!(trunk_xs.len(), 2, "one trunk per displaced parent: {trunk_xs:?}");
    }

    #[test]
    fn truncate_adds_ellipsis() {
        let s = "x".repeat(40);
        let t = truncate(&s);
        assert_eq!(t.width(), MAXW);
        assert!(t.ends_with('…'));
        assert_eq!(truncate("short"), "short");
    }
}
