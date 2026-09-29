//! Eased walking speed.
//!
//! Hobbits start slow, speed up after second breakfast (09:00), and are
//! effectively running by elevenses. The curve is a smoothstep so the plot
//! does not stutter between meals. See `the-shire/bag-end/second-breakfast.md`
//! for the meal that powers it.

/// Walking pace in leagues per hour-ish, given the hour of the day.
///
/// `0.0` = dawdling before second breakfast; `1.0` = the Black Riders are
/// behind us.
pub fn pace(hour: f32) -> f32 {
    let t = ((hour - 9.0) / 3.0).clamp(0.0, 1.0);
    let ease = t * t * (3.0 - 2.0 * t); // smoothstep: fast after 09:00, never before
    2.0 + 2.5 * ease // leagues per hour-ish
}
