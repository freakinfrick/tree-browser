//! Motion: critically damped springs, per-node animation, heat gradient.
//!
//! Every visible node owns a spring toward its layout target. New nodes spawn
//! at their nearest animated ancestor's label end and spring out (so an
//! expand unfurls from the parent); nodes that leave the layout become ghosts
//! that spring back into their nearest surviving ancestor while fading out.
//! Connectors are drawn from the animated positions, so they follow along.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::time::Instant;

use crate::layout::Layout;
use crate::settings::{Palette, ALL_PALETTES};

/// Unity-style SmoothDamp: critically damped, no overshoot, and velocity
/// carries over when the target moves mid-flight (no restart jerk).
#[derive(Clone, Copy, Debug)]
pub struct Damped {
    pub v: f32,
    pub vel: f32,
}

impl Damped {
    pub fn new(v: f32) -> Damped {
        Damped { v, vel: 0.0 }
    }

    /// Advance toward `target`. Returns true while still moving.
    pub fn step(&mut self, target: f32, smooth: f32, dt: f32) -> bool {
        let omega = 2.0 / smooth.max(1e-4);
        let x = omega * dt;
        let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
        let change = self.v - target;
        let temp = (self.vel + omega * change) * dt;
        self.vel = (self.vel - omega * temp) * exp;
        let mut out = target + (change + temp) * exp;
        if (target > self.v) == (out > target) {
            out = target;
            self.vel = 0.0;
        }
        self.v = out;
        if (self.v - target).abs() < 0.02 && self.vel.abs() < 0.2 {
            self.v = target;
            self.vel = 0.0;
            return false;
        }
        true
    }
}

/// Frame-rate independent exponential approach. Returns true while moving.
pub fn approach(cur: &mut f32, target: f32, tau: f32, dt: f32) -> bool {
    *cur += (target - *cur) * (1.0 - (-dt / tau).exp());
    if (*cur - target).abs() < 0.004 {
        *cur = target;
        return false;
    }
    true
}

/// Wheel momentum. Each notch moves its rows at once, so slow notches stay
/// exact; notches in quick succession set a speed that coasts on after the
/// last one, decaying with time constant `tau`, so a flick carries further.
#[derive(Clone, Copy, Debug, Default)]
pub struct Glide {
    /// Rows per second, signed.
    vel: f32,
    /// Rows travelled but not yet taken as whole steps.
    owed: f32,
    /// When and which way the last notch went.
    last: Option<(Instant, bool)>,
}

impl Glide {
    /// Notches further apart than this don't build speed.
    const QUICK: f32 = 0.2;
    /// Speed cap, rows per second: terminals deliver bursts of notches at once.
    const MAX: f32 = 150.0;
    /// Below this, rows per second, the glide stops.
    const MIN: f32 = 3.0;

    /// A notch of `rows` (negative = up) at `now`. Returns the rows to move at once.
    pub fn notch(&mut self, rows: i32, tau: f32, now: Instant) -> i32 {
        let down = rows > 0;
        let gap = self.last.filter(|l| l.1 == down).map(|l| (now - l.0).as_secs_f32());
        self.last = Some((now, down));
        self.vel = match gap {
            Some(g) if tau > 0.0 && g < Self::QUICK => {
                let v = (rows as f32 / g.max(0.01)).clamp(-Self::MAX, Self::MAX);
                // Smooth over the last two gaps; a first quick pair counts in full.
                if self.vel == 0.0 { v } else { (self.vel + v) / 2.0 }
            }
            _ => 0.0,
        };
        if self.vel == 0.0 {
            self.owed = 0.0;
        }
        rows
    }

    /// Advance `dt` seconds. Returns whole rows to move now.
    pub fn tick(&mut self, tau: f32, dt: f32) -> i32 {
        if self.vel == 0.0 || tau <= 0.0 {
            // Keep `last`: frames run between the notches of a flick.
            (self.vel, self.owed) = (0.0, 0.0);
            return 0;
        }
        // Exact integral of the decay over dt: frame-rate independent.
        let k = (-dt / tau).exp();
        self.owed += self.vel * tau * (1.0 - k);
        self.vel *= k;
        if self.vel.abs() < Self::MIN {
            self.vel = 0.0;
        }
        let n = self.owed.trunc();
        self.owed -= n;
        n as i32
    }

    /// Drop any glide: a key, a click, the end of the list.
    pub fn stop(&mut self) {
        *self = Glide::default();
    }

    pub fn moving(&self) -> bool {
        self.vel != 0.0
    }
}

pub type Rgb = [f32; 3];

/// Live-change ripple brightness `t` seconds after it reaches a node
/// (negative = not there yet): a quick flash, then an exponential fade.
pub fn pulse(t: f32) -> f32 {
    const RISE: f32 = 0.07;
    const FADE: f32 = 0.45;
    if t < 0.0 {
        0.0
    } else if t < RISE {
        t / RISE
    } else {
        (-(t - RISE) / FADE).exp()
    }
}

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

pub fn to_color(c: Rgb) -> ratatui::style::Color {
    let q = |v: f32| v.clamp(0.0, 255.0).round() as u8;
    ratatui::style::Color::Rgb(q(c[0]), q(c[1]), q(c[2]))
}

/// (age seconds, color) stops; interpolated in log-time.
/// Recency gradients: (age in seconds, color) stops, blended in log-time.
pub type Stops = [(f32, Rgb); 7];

const EMBER: Stops = [
    (60.0, [255.0, 70.0, 60.0]),
    (3600.0, [255.0, 118.0, 48.0]),
    (86400.0, [255.0, 172.0, 64.0]),
    (7.0 * 86400.0, [240.0, 190.0, 70.0]),
    // Mauve, not grey: short heat ranges squeeze this stop to hours (about
    // 12 at a month), and a grey there reads as a git-ignored name.
    (30.0 * 86400.0, [196.0, 112.0, 170.0]),
    (365.0 * 86400.0, [108.0, 118.0, 196.0]),
    (5.0 * 365.0 * 86400.0, [66.0, 80.0, 214.0]),
];

/// Viridis-like: brightness carries recency, no red-green contrast needed.
/// The old end is lifted off viridis's near-black so the oldest names stay readable.
const AURORA: Stops = [
    (60.0, [253.0, 231.0, 37.0]),
    (3600.0, [170.0, 220.0, 50.0]),
    (86400.0, [84.0, 197.0, 104.0]),
    (7.0 * 86400.0, [36.0, 166.0, 136.0]),
    (30.0 * 86400.0, [44.0, 136.0, 156.0]),
    (365.0 * 86400.0, [70.0, 106.0, 170.0]),
    (5.0 * 365.0 * 86400.0, [96.0, 80.0, 170.0]),
];

/// Magma-like: brightness and hue both carry recency, red-green safe.
const MAGMA: Stops = [
    (60.0, [255.0, 248.0, 180.0]),
    (3600.0, [255.0, 190.0, 120.0]),
    (86400.0, [250.0, 120.0, 96.0]),
    (7.0 * 86400.0, [222.0, 72.0, 120.0]),
    (30.0 * 86400.0, [184.0, 70.0, 160.0]),
    (365.0 * 86400.0, [146.0, 86.0, 196.0]),
    (5.0 * 365.0 * 86400.0, [120.0, 96.0, 210.0]),
];

/// Hue alone carries recency here: the aqua end would outshine new work if
/// it kept its natural brightness, so the whole ramp sits at one level.
const NEON: Stops = [
    (60.0, [255.0, 140.0, 200.0]),
    (3600.0, [255.0, 110.0, 225.0]),
    (86400.0, [215.0, 120.0, 255.0]),
    (7.0 * 86400.0, [160.0, 135.0, 255.0]),
    (30.0 * 86400.0, [110.0, 160.0, 250.0]),
    (365.0 * 86400.0, [70.0, 170.0, 225.0]),
    (5.0 * 365.0 * 86400.0, [50.0, 165.0, 185.0]),
];

/// Starts at pale cyan, not white: white is the cursor path's color.
const GLACIER: Stops = [
    (60.0, [180.0, 240.0, 255.0]),
    (3600.0, [130.0, 220.0, 255.0]),
    (86400.0, [90.0, 200.0, 242.0]),
    (7.0 * 86400.0, [60.0, 170.0, 228.0]),
    (30.0 * 86400.0, [70.0, 136.0, 230.0]),
    (365.0 * 86400.0, [96.0, 112.0, 226.0]),
    (5.0 * 365.0 * 86400.0, [118.0, 100.0, 214.0]),
];

/// Brightness only, like mono, but warm, so names never match the ignored
/// grey, and starting at cream so new work never matches the cursor path.
const SEPIA: Stops = [
    (60.0, [255.0, 226.0, 170.0]),
    (3600.0, [248.0, 210.0, 150.0]),
    (86400.0, [234.0, 188.0, 118.0]),
    (7.0 * 86400.0, [214.0, 160.0, 90.0]),
    (30.0 * 86400.0, [192.0, 134.0, 80.0]),
    (365.0 * 86400.0, [170.0, 114.0, 78.0]),
    (5.0 * 365.0 * 86400.0, [152.0, 100.0, 80.0]),
];

const MONO: Stops = [
    (60.0, [250.0, 250.0, 250.0]),
    (3600.0, [220.0, 220.0, 226.0]),
    (86400.0, [190.0, 190.0, 198.0]),
    (7.0 * 86400.0, [160.0, 160.0, 170.0]),
    (30.0 * 86400.0, [132.0, 132.0, 142.0]),
    (365.0 * 86400.0, [108.0, 108.0, 118.0]),
    (5.0 * 365.0 * 86400.0, [90.0, 90.0, 100.0]),
];

/// Parchment: new growth is leaf green, then gold, rust, and dark bark.
/// On paper, dark reads loudest, so the oldest names carry the most ink.
const GROWTH: Stops = [
    (60.0, [34.0, 128.0, 40.0]),
    (3600.0, [78.0, 124.0, 18.0]),
    (86400.0, [140.0, 116.0, 0.0]),
    (7.0 * 86400.0, [170.0, 104.0, 0.0]),
    (30.0 * 86400.0, [166.0, 70.0, 28.0]),
    (365.0 * 86400.0, [120.0, 64.0, 36.0]),
    (5.0 * 365.0 * 86400.0, [70.0, 44.0, 30.0]),
];

/// Parchment: fresh ink is near black and browns as it fades, so new work
/// stays the boldest.
const INK: Stops = [
    (60.0, [28.0, 22.0, 18.0]),
    (3600.0, [56.0, 36.0, 26.0]),
    (86400.0, [88.0, 50.0, 24.0]),
    (7.0 * 86400.0, [116.0, 66.0, 22.0]),
    (30.0 * 86400.0, [136.0, 82.0, 30.0]),
    (365.0 * 86400.0, [146.0, 94.0, 40.0]),
    (5.0 * 365.0 * 86400.0, [150.0, 102.0, 50.0]),
];

/// Vellum: rubric vermilion, then gold leaf and verdigris, down to iron-gall
/// violet. On paper the oldest names carry the most ink.
const MANUSCRIPT: Stops = [
    (60.0, [196.0, 40.0, 30.0]),
    (3600.0, [186.0, 74.0, 14.0]),
    (86400.0, [164.0, 108.0, 0.0]),
    (7.0 * 86400.0, [106.0, 112.0, 18.0]),
    (30.0 * 86400.0, [38.0, 106.0, 60.0]),
    (365.0 * 86400.0, [32.0, 80.0, 96.0]),
    (5.0 * 365.0 * 86400.0, [46.0, 40.0, 70.0]),
];

/// Foxed: fresh edits glint gold, then cool through olive into deep moss.
const GILDED: Stops = [
    (60.0, [160.0, 108.0, 0.0]),
    (3600.0, [146.0, 110.0, 0.0]),
    (86400.0, [120.0, 118.0, 20.0]),
    (7.0 * 86400.0, [68.0, 116.0, 40.0]),
    (30.0 * 86400.0, [46.0, 98.0, 60.0]),
    (365.0 * 86400.0, [38.0, 80.0, 66.0]),
    (5.0 * 365.0 * 86400.0, [30.0, 62.0, 58.0]),
];

/// Ledger: ink reversed. Iron-gall ink goes on pale and darkens as it
/// oxidizes, so new names are light brown and the oldest near black.
const IRON_GALL: Stops = [
    (60.0, [150.0, 102.0, 50.0]),
    (3600.0, [146.0, 94.0, 40.0]),
    (86400.0, [136.0, 82.0, 30.0]),
    (7.0 * 86400.0, [116.0, 66.0, 22.0]),
    (30.0 * 86400.0, [88.0, 50.0, 24.0]),
    (365.0 * 86400.0, [56.0, 36.0, 26.0]),
    (5.0 * 365.0 * 86400.0, [28.0, 22.0, 18.0]),
];

pub fn stops_of(p: Palette) -> &'static Stops {
    match p {
        Palette::Ember => &EMBER,
        Palette::Magma => &MAGMA,
        Palette::Neon => &NEON,
        Palette::Aurora => &AURORA,
        Palette::Glacier => &GLACIER,
        Palette::Sepia => &SEPIA,
        Palette::Mono => &MONO,
        Palette::Growth => &GROWTH,
        Palette::Ink => &INK,
        Palette::Manuscript => &MANUSCRIPT,
        Palette::Gilded => &GILDED,
        Palette::IronGall => &IRON_GALL,
    }
}

static PALETTE: AtomicU8 = AtomicU8::new(0);

pub fn set_palette(p: Palette) {
    // `ALL_PALETTES` is in declaration order, so the discriminant indexes it.
    PALETTE.store(p as u8, Ordering::Relaxed);
}

/// Multiplier on age before the gradient lookup, as f32 bits (1.0 = the
/// gradient's own 5-year span).
static HEAT_SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000);

/// Stretch the gradient so `range` seconds reaches its coldest color.
pub fn set_heat_range(range: f32) {
    HEAT_SCALE.store(range_scale(range).to_bits(), Ordering::Relaxed);
}

pub fn range_scale(range: f32) -> f32 {
    heat_stops()[heat_stops().len() - 1].0 / range
}

/// The gradient in use.
pub fn heat_stops() -> &'static Stops {
    stops_of(ALL_PALETTES[PALETTE.load(Ordering::Relaxed) as usize % ALL_PALETTES.len()])
}

pub fn heat(age_secs: f32) -> Rgb {
    heat_scaled(heat_stops(), age_secs, f32::from_bits(HEAT_SCALE.load(Ordering::Relaxed)))
}

/// `heat` on given stops, with the age stretched by `scale` (see `set_heat_range`).
pub fn heat_scaled(stops: &Stops, age_secs: f32, scale: f32) -> Rgb {
    let a = (age_secs * scale).max(1.0).ln();
    if a <= stops[0].0.ln() {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        let (t0, t1) = (w[0].0.ln(), w[1].0.ln());
        if a <= t1 {
            let t = (a - t0) / (t1 - t0);
            // Smoothstep so bands blend without visible kinks.
            return mix(w[0].1, w[1].1, t * t * (3.0 - 2.0 * t));
        }
    }
    stops[stops.len() - 1].1
}

pub struct NodeAnim {
    pub x: Damped,
    pub y: Damped,
    pub alpha: f32,
    pub rgb: Rgb,
    /// Color the renderer is fading toward.
    pub target: Rgb,
    pub label: String,
    /// Bytes of `label` that are the name; the rest is name details.
    pub name: usize,
    pub w: i32,
    pub ghost: bool,
    /// Target (x, y) this frame; ghosts retarget each frame.
    pub tx: f32,
    pub ty: f32,
}

impl NodeAnim {
    pub fn pos(&self) -> (i32, i32) {
        (self.x.v.round() as i32, self.y.v.round() as i32)
    }
}

pub const MOVE: f32 = 0.11;
pub const FADE_IN: f32 = 0.07;
pub const FADE_OUT: f32 = 0.05;
pub const RECOLOR: f32 = 0.12;

#[derive(Default)]
pub struct Scene {
    pub nodes: HashMap<usize, NodeAnim>,
}

impl Scene {
    /// Retarget every node to the layout, spawn new ones, ghost departed ones,
    /// and advance positions + alpha. Colors are advanced by the renderer
    /// (it owns the color targets). Returns true while anything moves.
    pub fn sync(&mut self, lay: &Layout, parent_of: &dyn Fn(usize) -> Option<usize>, dt: f32) -> bool {
        let mut live = std::collections::HashSet::with_capacity(lay.placed.len());
        let origin = |nodes: &HashMap<usize, NodeAnim>, mut id: usize| -> Option<(f32, f32)> {
            while let Some(p) = parent_of(id) {
                if let Some(a) = nodes.get(&p) {
                    return Some((a.x.v + a.w as f32, a.y.v));
                }
                id = p;
            }
            None
        };
        // Placed is column-ordered, so a parent spawns before its kids and
        // the kids unfurl from the parent's (itself moving) spawn point.
        for p in &lay.placed {
            live.insert(p.id);
            let w = unicode_width::UnicodeWidthStr::width(p.label.as_str()) as i32;
            if let Some(a) = self.nodes.get_mut(&p.id) {
                a.tx = p.x as f32;
                a.ty = p.y as f32;
                a.label.clone_from(&p.label);
                a.name = p.name;
                a.w = w;
                a.ghost = false;
                continue;
            }
            let (sx, sy) = origin(&self.nodes, p.id).unwrap_or((p.x as f32, p.y as f32));
            self.nodes.insert(
                p.id,
                NodeAnim {
                    x: Damped::new(sx),
                    y: Damped::new(sy),
                    alpha: 0.0,
                    rgb: [0.0; 3],
                    target: [0.0; 3],
                    label: p.label.clone(),
                    name: p.name,
                    w,
                    ghost: false,
                    tx: p.x as f32,
                    ty: p.y as f32,
                },
            );
        }
        // Ghosts collapse into their nearest surviving ancestor.
        let ghosts: Vec<usize> = self.nodes.keys().copied().filter(|id| !live.contains(id)).collect();
        for id in ghosts {
            let o = origin(&self.nodes, id);
            let a = self.nodes.get_mut(&id).unwrap();
            a.ghost = true;
            if let Some((ox, oy)) = o {
                a.tx = ox;
                a.ty = oy;
            }
        }
        let mut moving = false;
        self.nodes.retain(|_, a| {
            moving |= a.x.step(a.tx, MOVE, dt);
            moving |= a.y.step(a.ty, MOVE, dt);
            if a.ghost {
                approach(&mut a.alpha, 0.0, FADE_OUT, dt);
                moving = true;
                a.alpha > 0.02
            } else {
                moving |= approach(&mut a.alpha, 1.0, FADE_IN, dt);
                true
            }
        });
        moving
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Rows a glide covers once left to run out, at 60 fps.
    fn coast(g: &mut Glide, tau: f32) -> i32 {
        let mut n = 0;
        for _ in 0..600 {
            n += g.tick(tau, 1.0 / 60.0);
        }
        assert!(!g.moving(), "comes to rest");
        n
    }

    #[test]
    fn slow_notches_step_exactly_and_flicks_glide_on() {
        let t0 = Instant::now();
        let ms = |m: u64| t0 + Duration::from_millis(m);
        let mut g = Glide::default();
        assert_eq!(g.notch(1, 0.12, ms(0)), 1, "a notch moves at once");
        assert_eq!(g.notch(1, 0.12, ms(400)), 1);
        assert_eq!(coast(&mut g, 0.12), 0, "slow notches don't glide");

        // Five notches 20 ms apart, frames between: 50 rows/s, then short momentum coasts ~6 rows.
        let mut moved = 0;
        for i in 0..5 {
            moved += g.notch(1, 0.12, ms(1000 + 20 * i));
            if i < 4 {
                moved += g.tick(0.12, 0.02);
            }
        }
        assert!(moved > 5, "frames between the notches carry it along: {moved}");
        let short = coast(&mut g, 0.12);
        assert!((4..=7).contains(&short), "{short}");
        for i in 0..5 {
            g.notch(-1, 0.5, ms(2000 + 20 * i));
        }
        let long = coast(&mut g, 0.5);
        assert!(long < -short * 3, "long glides further, and upward: {long}");

        for i in 0..5 {
            g.notch(1, 0.0, ms(3000 + 20 * i));
        }
        assert!(!g.moving(), "momentum off");
    }

    #[test]
    fn a_reversed_notch_or_a_stop_kills_the_glide() {
        let t0 = Instant::now();
        let mut g = Glide::default();
        g.notch(1, 0.25, t0);
        g.notch(1, 0.25, t0 + Duration::from_millis(20));
        assert!(g.moving());
        g.notch(-1, 0.25, t0 + Duration::from_millis(40));
        assert!(!g.moving(), "turning the wheel back halts it");
        g.notch(1, 0.25, t0 + Duration::from_millis(60));
        g.notch(1, 0.25, t0 + Duration::from_millis(61));
        assert!(g.tick(0.25, 1.0 / 60.0) <= 3, "a burst in one read is capped, not a jump");
        g.stop();
        assert_eq!(coast(&mut g, 0.25), 0);
    }

    #[test]
    fn pulse_flashes_then_fades_out() {
        assert_eq!(pulse(-0.1), 0.0);
        assert!(pulse(0.035) > 0.4 && pulse(0.035) < 0.6);
        assert!((pulse(0.07) - 1.0).abs() < 1e-4);
        assert!(pulse(0.5) < pulse(0.2));
        assert!(pulse(2.2) < 0.01, "gone by the time the ripple is dropped");
    }

    #[test]
    fn damped_settles_without_overshoot() {
        let mut d = Damped::new(0.0);
        let mut max: f32 = 0.0;
        let mut frames = 0;
        while d.step(10.0, MOVE, 1.0 / 60.0) {
            max = max.max(d.v);
            frames += 1;
            assert!(frames < 120, "settles within 2s");
        }
        assert!(max <= 10.0, "no overshoot: {max}");
        assert_eq!(d.v, 10.0);
        // ~0.11 smooth time: most of the way in a handful of frames.
        assert!(frames > 5 && frames < 60, "frames={frames}");
    }

    #[test]
    fn damped_is_framerate_independent() {
        let run = |fps: f32| {
            let mut d = Damped::new(0.0);
            let dt = 1.0 / fps;
            for _ in 0..(0.1 * fps) as usize {
                d.step(10.0, MOVE, dt);
            }
            d.v
        };
        assert!((run(30.0) - run(120.0)).abs() < 0.6);
    }

    #[test]
    fn heat_range_stretches_the_gradient() {
        let coldest = heat_stops()[heat_stops().len() - 1].1;
        // Pure: the global scale is shared with tests running alongside.
        assert_eq!(heat_scaled(heat_stops(), 86400.0, range_scale(86400.0)), coldest, "a day range: a day old is as cold as it gets");
        assert_ne!(heat_scaled(heat_stops(), 86400.0, range_scale(5.0 * 365.0 * 86400.0)), coldest);
    }

    #[test]
    fn palettes_are_listed_in_declaration_order() {
        for (i, &p) in ALL_PALETTES.iter().enumerate() {
            assert_eq!(p as usize, i, "{p:?}");
        }
    }

    #[test]
    fn ember_red_never_rises_with_age() {
        let r = |s: f32| heat_scaled(&EMBER, s, 1.0)[0];
        let mut last = f32::MAX;
        for s in [1.0, 600.0, 7200.0, 2.0 * 86400.0, 20.0 * 86400.0, 200.0 * 86400.0, 3e9] {
            let v = r(s);
            assert!(v <= last + 0.01, "red never rises with age at {s}");
            last = v;
        }
    }
}
