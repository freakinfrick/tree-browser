//! Motion: critically damped springs, per-node animation, heat gradient.
//!
//! Every visible node owns a spring toward its layout target. New nodes spawn
//! at their nearest animated ancestor's label end and spring out (so an
//! expand unfurls from the parent); nodes that leave the layout become ghosts
//! that spring back into their nearest surviving ancestor while fading out.
//! Connectors are drawn from the animated positions, so they follow along.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::layout::Layout;
use crate::settings::Palette;

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
type Stops = [(f32, Rgb); 7];

const EMBER: Stops = [
    (60.0, [255.0, 70.0, 60.0]),
    (3600.0, [255.0, 118.0, 48.0]),
    (86400.0, [255.0, 172.0, 64.0]),
    (7.0 * 86400.0, [218.0, 180.0, 128.0]),
    (30.0 * 86400.0, [150.0, 152.0, 166.0]),
    (365.0 * 86400.0, [108.0, 118.0, 196.0]),
    (5.0 * 365.0 * 86400.0, [66.0, 80.0, 214.0]),
];

/// Viridis-like: brightness carries recency, no red-green contrast needed.
const AURORA: Stops = [
    (60.0, [253.0, 231.0, 37.0]),
    (3600.0, [170.0, 220.0, 50.0]),
    (86400.0, [84.0, 197.0, 104.0]),
    (7.0 * 86400.0, [34.0, 163.0, 132.0]),
    (30.0 * 86400.0, [38.0, 128.0, 142.0]),
    (365.0 * 86400.0, [56.0, 92.0, 142.0]),
    (5.0 * 365.0 * 86400.0, [72.0, 52.0, 128.0]),
];

const MONO: Stops = [
    (60.0, [250.0, 250.0, 250.0]),
    (3600.0, [218.0, 218.0, 224.0]),
    (86400.0, [184.0, 184.0, 192.0]),
    (7.0 * 86400.0, [150.0, 150.0, 160.0]),
    (30.0 * 86400.0, [118.0, 118.0, 128.0]),
    (365.0 * 86400.0, [90.0, 90.0, 100.0]),
    (5.0 * 365.0 * 86400.0, [66.0, 66.0, 76.0]),
];

static PALETTE: AtomicU8 = AtomicU8::new(0);

pub fn set_palette(p: Palette) {
    PALETTE.store(p as u8, Ordering::Relaxed);
}

/// The gradient in use.
pub fn heat_stops() -> &'static Stops {
    match PALETTE.load(Ordering::Relaxed) {
        1 => &AURORA,
        2 => &MONO,
        _ => &EMBER,
    }
}

pub fn heat(age_secs: f32) -> Rgb {
    let stops = heat_stops();
    let a = age_secs.max(1.0).ln();
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
    fn heat_is_monotone_hot_to_cold() {
        let r = |s: f32| heat(s)[0];
        let mut last = f32::MAX;
        for s in [1.0, 600.0, 7200.0, 2.0 * 86400.0, 20.0 * 86400.0, 200.0 * 86400.0, 3e9] {
            let v = r(s);
            assert!(v <= last + 0.01, "red never rises with age at {s}");
            last = v;
        }
    }
}
