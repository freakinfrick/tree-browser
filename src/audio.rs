//! Audio previews: the file plays through the default output device while the popup
//! is open, with a waveform read on a worker thread and a scrubber over it.
//! Playback needs the `audio` feature (on by default); without it the popup says so.
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use ratatui::layout::Rect;

/// Extensions previewed as audio (what the bundled symphonia decoders read).
const AUDIO_EXT: &[&str] = &["mp3", "flac", "wav", "wave", "ogg", "oga", "m4a", "m4b", "aac"];

/// Seconds of audio per waveform peak.
pub const WAVE_STEP: f32 = 0.05;
/// Stop reading the waveform past this many peaks (10 hours).
#[cfg(feature = "audio")]
const WAVE_MAX: usize = 720_000;

/// Worker -> popup: a run of new peaks, then (with the exact length) the last run.
type Chunk = (Vec<f32>, Option<Duration>);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Playing,
    Paused,
    Ended,
    /// Nothing to play through (no device, can't decode, built without audio).
    Silent,
}

pub struct Audio {
    #[cfg(feature = "audio")]
    out: Option<Out>,
    #[cfg_attr(not(feature = "audio"), allow(dead_code))]
    path: PathBuf,
    /// "mp3 · 44.1 kHz · stereo"
    pub info: String,
    /// Why nothing plays, if it doesn't.
    pub err: Option<String>,
    /// Peak (0..1) per WAVE_STEP seconds, filled in as the worker reads.
    pub wave: Vec<f32>,
    pub wave_done: bool,
    wave_rx: Option<Receiver<Chunk>>,
    /// Length from the container, else from the worker once it has read to the end.
    total: Option<Duration>,
    /// 0..=1, shown as a percent; the gain applied is its square (closer to how loud it sounds).
    pub volume: f32,
    pub muted: bool,
    /// Screen rows the waveform and scrubber were drawn on last frame, for the mouse.
    pub bar: Rect,
    /// Held last so the output stream is gone before stderr comes back.
    _hush: Hush,
}

#[cfg(feature = "audio")]
struct Out {
    player: rodio::Player,
    _sink: rodio::MixerDeviceSink,
}

pub fn is_audio(path: &Path) -> bool {
    let e = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    AUDIO_EXT.contains(&e.as_str())
}

/// 83 s -> "1:23", 3723 s -> "1:02:03".
pub fn clock(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// Loudest peak under each of `cols` columns, with `wave` spread over `total` peaks.
pub fn columns(wave: &[f32], total: usize, cols: usize) -> Vec<f32> {
    let total = total.max(wave.len()).max(1);
    (0..cols)
        .map(|c| {
            let lo = c * total / cols;
            let hi = ((c + 1) * total / cols).max(lo + 1);
            wave.get(lo..hi.min(wave.len())).map_or(0.0, |s| s.iter().copied().fold(0.0, f32::max))
        })
        .collect()
}

impl Audio {
    /// Audio preview for sound files, None for everything else. Starts playing at once.
    pub fn open(path: &Path) -> Option<Audio> {
        if !is_audio(path) {
            return None;
        }
        let mut a = Audio {
            #[cfg(feature = "audio")]
            out: None,
            path: path.to_path_buf(),
            info: String::new(),
            err: None,
            wave: Vec::new(),
            wave_done: false,
            wave_rx: None,
            total: None,
            volume: 1.0,
            muted: false,
            bar: Rect::default(),
            _hush: Hush::new(),
        };
        a.start();
        Some(a)
    }

    pub fn total(&self) -> Option<Duration> {
        self.total
    }

    /// Take the worker's new peaks; true if anything arrived.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.wave_rx else { return false };
        let mut any = false;
        loop {
            match rx.try_recv() {
                Ok((chunk, end)) => {
                    any = true;
                    self.wave.extend(chunk);
                    if let Some(t) = end {
                        self.total = Some(t);
                        self.wave_done = true;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.wave_done = true;
                    self.wave_rx = None;
                    break;
                }
            }
        }
        any
    }

    /// Fraction of the way through, 0..1 (0 while the length is unknown).
    pub fn progress(&self) -> f32 {
        match self.total {
            Some(t) if !t.is_zero() => (self.pos().as_secs_f32() / t.as_secs_f32()).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    pub fn seek_by(&mut self, secs: f32) {
        let to = self.pos().as_secs_f32() + secs;
        self.seek(Duration::from_secs_f32(to.max(0.0)));
    }

    /// Jump to a fraction of the length (0..1).
    pub fn seek_frac(&mut self, f: f32) {
        if let Some(t) = self.total {
            self.seek(t.mul_f32(f.clamp(0.0, 1.0)));
        }
    }

    pub fn volume_by(&mut self, d: f32) {
        self.volume = ((self.volume + d) * 10.0).round().clamp(0.0, 10.0) / 10.0;
        self.muted = false;
        self.apply_volume();
    }

    pub fn toggle_mute(&mut self) {
        self.muted ^= true;
        self.apply_volume();
    }
}

#[cfg(feature = "audio")]
impl Audio {
    fn decoder(path: &Path) -> Result<rodio::Decoder<std::io::BufReader<std::fs::File>>, String> {
        let f = std::fs::File::open(path).map_err(|e| format!("{e}"))?;
        rodio::Decoder::try_from(f).map_err(|e| format!("{e}"))
    }

    fn start(&mut self) {
        use rodio::Source;
        let dec = match Self::decoder(&self.path) {
            Ok(d) => d,
            Err(e) => {
                self.err = Some(format!("can't decode · {e}"));
                self.wave_done = true;
                return;
            }
        };
        let ext = self.path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let rate = dec.sample_rate().get();
        let ch = match dec.channels().get() {
            1 => "mono".to_string(),
            2 => "stereo".to_string(),
            n => format!("{n} ch"),
        };
        self.info = format!("{ext} · {} kHz · {ch}", rate as f32 / 1000.0);
        self.total = dec.total_duration();

        let (tx, rx) = std::sync::mpsc::channel();
        let path = self.path.clone();
        std::thread::spawn(move || scan(&path, &tx));
        self.wave_rx = Some(rx);

        let sink = rodio::DeviceSinkBuilder::from_default_device()
            .and_then(|b| b.with_error_callback(|_| {}).open_stream());
        match sink {
            Ok(mut sink) => {
                sink.log_on_drop(false);
                let player = rodio::Player::connect_new(sink.mixer());
                player.append(dec);
                self.out = Some(Out { player, _sink: sink });
            }
            Err(e) => self.err = Some(format!("no audio output · {e}")),
        }
    }

    pub fn state(&self) -> State {
        match &self.out {
            None => State::Silent,
            Some(o) if o.player.empty() => State::Ended,
            Some(o) if o.player.is_paused() => State::Paused,
            Some(_) => State::Playing,
        }
    }

    pub fn pos(&self) -> Duration {
        match (&self.out, self.state()) {
            (_, State::Ended) => self.total.unwrap_or_default(),
            (Some(o), _) => o.player.get_pos(),
            (None, _) => Duration::ZERO,
        }
    }

    /// Play / pause; at the end, play again from the start.
    pub fn toggle(&mut self) {
        match self.state() {
            State::Ended => self.seek(Duration::ZERO),
            State::Paused => self.out.as_ref().unwrap().player.play(),
            State::Playing => self.out.as_ref().unwrap().player.pause(),
            State::Silent => {}
        }
    }

    pub fn pause(&mut self) {
        if let Some(o) = &self.out {
            o.player.pause();
        }
    }

    pub fn seek(&mut self, to: Duration) {
        let to = self.total.map_or(to, |t| to.min(t));
        let Some(o) = &self.out else { return };
        // Played out: the decoder is gone, queue a fresh one. Paused stays paused.
        if o.player.empty() {
            match Self::decoder(&self.path) {
                Ok(d) => o.player.append(d),
                Err(_) => return,
            }
        }
        let _ = o.player.try_seek(to);
    }

    fn apply_volume(&self) {
        if let Some(o) = &self.out {
            o.player.set_volume(if self.muted { 0.0 } else { self.volume * self.volume });
        }
    }
}

#[cfg(not(feature = "audio"))]
impl Audio {
    fn start(&mut self) {
        self.err = Some("tb was built without the audio feature".into());
        self.wave_done = true;
    }
    pub fn state(&self) -> State {
        State::Silent
    }
    pub fn pos(&self) -> Duration {
        Duration::ZERO
    }
    pub fn toggle(&mut self) {}
    pub fn pause(&mut self) {}
    pub fn seek(&mut self, _to: Duration) {}
    fn apply_volume(&self) {}
}

/// Read the whole file for its waveform, sending new peaks every quarter second so a
/// long file draws in as it goes. Stops early once the popup (the receiver) is gone.
#[cfg(feature = "audio")]
fn scan(path: &Path, tx: &std::sync::mpsc::Sender<Chunk>) {
    use rodio::Source;
    let Ok(dec) = Audio::decoder(path) else { return };
    let (rate, ch) = (dec.sample_rate().get() as u64, dec.channels().get() as u64);
    let per = ((rate as f32 * WAVE_STEP) as usize * ch as usize).max(1);
    let (mut chunk, mut peak, mut n, mut frames, mut sent) = (Vec::new(), 0f32, 0, 0u64, 0);
    let mut last = std::time::Instant::now();
    for s in dec {
        peak = peak.max(s.abs());
        n += 1;
        if n == per {
            frames += (per as u64) / ch;
            chunk.push(peak.min(1.0));
            (peak, n) = (0.0, 0);
            if sent + chunk.len() >= WAVE_MAX {
                break;
            }
            if last.elapsed() >= Duration::from_millis(250) {
                sent += chunk.len();
                if tx.send((std::mem::take(&mut chunk), None)).is_err() {
                    return;
                }
                last = std::time::Instant::now();
            }
        }
    }
    if n > 0 {
        frames += n as u64 / ch;
        chunk.push(peak.min(1.0));
    }
    let _ = tx.send((chunk, Some(Duration::from_secs_f64(frames as f64 / rate as f64))));
}

/// Keeps stderr pointed at /dev/null while any audio preview is open: ALSA and the
/// decoders print straight to it (underruns, odd headers), which would scribble over
/// the TUI. Counted, so a new preview replacing an old one stays quiet throughout.
struct Hush;

static HUSHED: std::sync::Mutex<(usize, i32)> = std::sync::Mutex::new((0, -1));

impl Hush {
    fn new() -> Hush {
        let mut h = HUSHED.lock().unwrap_or_else(|e| e.into_inner());
        if h.0 == 0 {
            unsafe {
                let saved = libc::dup(2);
                let null = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY);
                if saved >= 0 && null >= 0 {
                    libc::dup2(null, 2);
                    h.1 = saved;
                } else if saved >= 0 {
                    libc::close(saved);
                }
                if null >= 0 {
                    libc::close(null);
                }
            }
        }
        h.0 += 1;
        Hush
    }
}

impl Drop for Hush {
    fn drop(&mut self) {
        let mut h = HUSHED.lock().unwrap_or_else(|e| e.into_inner());
        h.0 = h.0.saturating_sub(1);
        if h.0 == 0 {
            unhush_locked(&mut h);
        }
    }
}

fn unhush_locked(h: &mut (usize, i32)) {
    if h.1 >= 0 {
        unsafe {
            libc::dup2(h.1, 2);
            libc::close(h.1);
        }
        h.1 = -1;
    }
}

/// Give stderr back now (panic hook), whatever is still open.
pub fn unhush() {
    if let Ok(mut h) = HUSHED.try_lock() {
        unhush_locked(&mut h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_formats() {
        assert_eq!(clock(Duration::from_secs(0)), "0:00");
        assert_eq!(clock(Duration::from_secs(83)), "1:23");
        assert_eq!(clock(Duration::from_secs(3723)), "1:02:03");
    }

    #[test]
    fn columns_take_the_loudest_peak_and_leave_unread_space_empty() {
        assert_eq!(columns(&[0.1, 0.9, 0.3, 0.2], 4, 2), [0.9, 0.3]);
        assert_eq!(columns(&[0.5, 0.7], 4, 2), [0.7, 0.0], "second half not read yet");
        assert_eq!(columns(&[0.5], 1, 3), [0.5, 0.5, 0.5], "fewer peaks than columns stretch");
    }

    #[test]
    fn only_sound_files_open() {
        assert!(Audio::open(Path::new("notes.txt")).is_none());
        assert!(is_audio(Path::new("Song.MP3")));
    }

    /// 16-bit mono PCM WAV of `secs` seconds: a quiet first half, a loud second half.
    #[cfg(feature = "audio")]
    fn wav(path: &Path, rate: u32, secs: u32) {
        let n = rate * secs;
        let mut b = Vec::new();
        b.extend(b"RIFF");
        b.extend((36 + n * 2).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(rate.to_le_bytes());
        b.extend((rate * 2).to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend((n * 2).to_le_bytes());
        for i in 0..n {
            let amp = if i < n / 2 { 3000.0 } else { 30000.0 };
            let s = (amp * (i as f32 * 0.3).sin()) as i16;
            b.extend(s.to_le_bytes());
        }
        std::fs::write(path, b).unwrap();
    }

    #[cfg(feature = "audio")]
    #[test]
    fn waveform_reads_the_whole_file_and_its_length() {
        let p = std::env::temp_dir().join(format!("tb-audio-{}.wav", std::process::id()));
        wav(&p, 8000, 2);
        let mut a = Audio::open(&p).unwrap();
        assert_eq!(a.info, "wav · 8 kHz · mono");
        assert_eq!(a.total().map(|t| t.as_millis()), Some(2000));
        for _ in 0..500 {
            a.poll();
            if a.wave_done {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(a.wave_done);
        assert_eq!(a.wave.len(), 40, "2 s at 50 ms a peak");
        assert!(a.wave[5] < 0.15 && a.wave[35] > 0.85, "quiet half, loud half: {:?}", a.wave);
        // Whether or not this machine has an output device, the preview holds together.
        a.seek_frac(0.5);
        a.volume_by(-0.2);
        assert_eq!(a.volume, 0.8);
        let _ = std::fs::remove_file(&p);
    }
}
