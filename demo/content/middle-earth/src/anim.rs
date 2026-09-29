/// Eased walking speed: hobbits start slow, speed up after second breakfast.
pub fn pace(hour: f32) -> f32 {
    let t = ((hour - 9.0) / 3.0).clamp(0.0, 1.0);
    let ease = t * t * (3.0 - 2.0 * t);
    2.0 + 2.5 * ease // leagues per hour-ish
}
