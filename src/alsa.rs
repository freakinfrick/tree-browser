//! Linux/BSD sound output through the system's libasound, opened with dlopen at run time,
//! so building tb needs no ALSA headers and a machine without the library stays quiet.
//! One thread owns the PCM handle (ALSA handles aren't safe to share); the preview
//! steers it through atomics and reads back the position it publishes.
use std::ffi::{CStr, c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use rodio::Source;

use crate::audio::{Audio, Dec};

const STREAM_PLAYBACK: c_uint = 0;
const ACCESS_RW_INTERLEAVED: c_uint = 3;
const FORMAT_S16_LE: c_int = 2;
/// Device buffer. A write blocks until there's room, so this is also how late a key lands.
const LATENCY_US: c_uint = 100_000;

type Pcm = *mut c_void;

/// The handful of libasound calls tb makes, resolved once and kept for the process.
macro_rules! lib {
    ($($field:ident = $sym:literal: $ty:ty;)*) => {
        struct Lib {
            $($field: $ty,)*
        }

        fn load() -> Option<Lib> {
            // SAFETY: each symbol is cast to the signature alsa/pcm.h gives it; the
            // library is never closed, so the pointers stay valid for the whole process.
            unsafe {
                let h = libc::dlopen(c"libasound.so.2".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
                if h.is_null() {
                    return None;
                }
                Some(Lib {
                    $($field: {
                        let p = libc::dlsym(h, $sym.as_ptr());
                        if p.is_null() {
                            return None;
                        }
                        std::mem::transmute::<*mut c_void, $ty>(p)
                    },)*
                })
            }
        }
    };
}

lib! {
    open = c"snd_pcm_open": unsafe extern "C" fn(*mut Pcm, *const c_char, c_uint, c_int) -> c_int;
    set_params = c"snd_pcm_set_params": unsafe extern "C" fn(Pcm, c_int, c_uint, c_uint, c_uint, c_int, c_uint) -> c_int;
    writei = c"snd_pcm_writei": unsafe extern "C" fn(Pcm, *const c_void, c_ulong) -> c_long;
    recover = c"snd_pcm_recover": unsafe extern "C" fn(Pcm, c_int, c_int) -> c_int;
    delay = c"snd_pcm_delay": unsafe extern "C" fn(Pcm, *mut c_long) -> c_int;
    drop = c"snd_pcm_drop": unsafe extern "C" fn(Pcm) -> c_int;
    prepare = c"snd_pcm_prepare": unsafe extern "C" fn(Pcm) -> c_int;
    drain = c"snd_pcm_drain": unsafe extern "C" fn(Pcm) -> c_int;
    close = c"snd_pcm_close": unsafe extern "C" fn(Pcm) -> c_int;
    strerror = c"snd_strerror": unsafe extern "C" fn(c_int) -> *const c_char;
}

fn lib() -> Result<&'static Lib, String> {
    static LIB: OnceLock<Option<Lib>> = OnceLock::new();
    LIB.get_or_init(load).as_ref().ok_or_else(|| "libasound.so.2 not found".into())
}

impl Lib {
    fn err(&self, code: c_int) -> String {
        // SAFETY: snd_strerror returns a static string for any code.
        unsafe { CStr::from_ptr((self.strerror)(code)) }.to_string_lossy().into_owned()
    }
}

/// An open PCM, closed on drop. Moved to the playback thread and only touched there.
struct Handle(Pcm, &'static Lib);

// SAFETY: the handle is used from one thread at a time (the playback thread owns it).
unsafe impl Send for Handle {}

impl Handle {
    fn open(name: &CStr, rate: u32, channels: u16) -> Result<Handle, String> {
        let lib = lib()?;
        let mut pcm: Pcm = std::ptr::null_mut();
        // SAFETY: plain libasound calls on a handle we own.
        unsafe {
            let e = (lib.open)(&mut pcm, name.as_ptr(), STREAM_PLAYBACK, 0);
            if e < 0 {
                return Err(lib.err(e));
            }
            let e = (lib.set_params)(pcm, FORMAT_S16_LE, ACCESS_RW_INTERLEAVED, channels.into(), rate, 1, LATENCY_US);
            if e < 0 {
                (lib.close)(pcm);
                return Err(lib.err(e));
            }
        }
        Ok(Handle(pcm, lib))
    }

    /// Write every frame of `buf` (interleaved), recovering from underruns. False if the
    /// device is gone for good.
    fn write(&self, buf: &[i16], channels: usize) -> bool {
        let mut off = 0;
        while off < buf.len() {
            let frames = ((buf.len() - off) / channels) as c_ulong;
            // SAFETY: `buf[off..]` holds at least `frames` whole frames.
            let r = unsafe { (self.1.writei)(self.0, buf[off..].as_ptr().cast(), frames) };
            if r < 0 {
                if unsafe { (self.1.recover)(self.0, r as c_int, 1) } < 0 {
                    return false;
                }
                continue;
            }
            off += r as usize * channels;
        }
        true
    }

    /// Frames written but not yet heard (0 if the device can't say).
    fn delay(&self) -> u64 {
        let mut d: c_long = 0;
        let ok = unsafe { (self.1.delay)(self.0, &mut d) } >= 0;
        if ok { d.max(0) as u64 } else { 0 }
    }

    /// Throw away what's queued and get ready for new frames.
    fn flush(&self) {
        unsafe {
            (self.1.drop)(self.0);
            (self.1.prepare)(self.0);
        }
    }

    /// Wait for what's queued to play out.
    fn drain(&self) {
        unsafe {
            (self.1.drain)(self.0);
            (self.1.prepare)(self.0);
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            (self.1.drop)(self.0);
            (self.1.close)(self.0);
        }
    }
}

/// What the preview and the playback thread share.
#[derive(Default)]
struct Shared {
    paused: AtomicBool,
    ended: AtomicBool,
    /// The device failed mid-play; nothing more will sound.
    dead: AtomicBool,
    quit: AtomicBool,
    /// f32 bits.
    gain: AtomicU32,
    /// Frames heard from the start of the file.
    pos: AtomicU64,
    seek: Mutex<Option<Duration>>,
}

/// A file playing through the default device, driven like rodio's Player.
pub struct Out {
    shared: Arc<Shared>,
    rate: u32,
    thread: Option<JoinHandle<()>>,
}

impl Out {
    pub fn open(path: &Path, dec: Dec) -> Result<Out, String> {
        Out::open_on(c"default", path, dec)
    }

    fn open_on(device: &CStr, path: &Path, dec: Dec) -> Result<Out, String> {
        let (rate, channels) = (dec.sample_rate().get(), dec.channels().get());
        let pcm = Handle::open(device, rate, channels)?;
        let shared = Arc::new(Shared { gain: AtomicU32::new(1f32.to_bits()), ..Default::default() });
        let (s, path) = (shared.clone(), path.to_path_buf());
        let thread = std::thread::spawn(move || run(pcm, dec, &path, rate, channels.into(), &s));
        Ok(Out { shared, rate, thread: Some(thread) })
    }

    pub fn is_ended(&self) -> bool {
        self.shared.ended.load(Relaxed) || self.shared.dead.load(Relaxed)
    }

    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Relaxed)
    }

    pub fn play(&self) {
        self.shared.paused.store(false, Relaxed);
    }

    pub fn pause(&self) {
        self.shared.paused.store(true, Relaxed);
    }

    pub fn pos(&self) -> Duration {
        Duration::from_secs_f64(self.shared.pos.load(Relaxed) as f64 / self.rate as f64)
    }

    /// Jump to `to`, from the end too. Paused stays paused.
    pub fn seek(&self, to: Duration) {
        if self.shared.dead.load(Relaxed) {
            return;
        }
        *self.shared.seek.lock().unwrap_or_else(|e| e.into_inner()) = Some(to);
        // Show the new spot at once rather than after the thread's next pass.
        self.shared.pos.store((to.as_secs_f64() * self.rate as f64) as u64, Relaxed);
        self.shared.ended.store(false, Relaxed);
    }

    pub fn set_volume(&self, gain: f32) {
        self.shared.gain.store(gain.to_bits(), Relaxed);
    }
}

impl Drop for Out {
    fn drop(&mut self) {
        self.shared.quit.store(true, Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Move `dec` to `to`, reopening the file if the decoder can't seek from where it is.
fn reposition(dec: &mut Dec, path: &Path, to: Duration) {
    if dec.try_seek(to).is_ok() {
        return;
    }
    if let Ok(mut d) = Audio::decoder(path) {
        let _ = d.try_seek(to);
        *dec = d;
    }
}

/// The playback thread: decode 20 ms at a time into the device until told to quit.
/// Pausing drops what the device holds and rewinds the decoder to what was heard,
/// so pause and seek take effect at once and the clock never runs ahead of the sound.
fn run(pcm: Handle, mut dec: Dec, path: &Path, rate: u32, channels: usize, s: &Shared) {
    let idle = || std::thread::sleep(Duration::from_millis(10));
    let chunk = (rate as usize / 50).max(1) * channels;
    let mut buf: Vec<i16> = Vec::with_capacity(chunk);
    // Frames handed to the device, counted from the start of the file.
    let mut written: u64 = 0;
    // The device holds frames not yet heard.
    let mut queued = false;
    let frames = |d: Duration| (d.as_secs_f64() * rate as f64) as u64;
    loop {
        if s.quit.load(Relaxed) {
            return;
        }
        let want = s.seek.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(to) = want {
            if queued {
                pcm.flush();
                queued = false;
            }
            reposition(&mut dec, path, to);
            written = frames(to);
            // The end branch may have raced this seek in; the seek wins.
            s.pos.store(written, Relaxed);
            s.ended.store(false, Relaxed);
            continue;
        }
        if s.paused.load(Relaxed) {
            if queued {
                let heard = written.saturating_sub(pcm.delay());
                pcm.flush();
                queued = false;
                reposition(&mut dec, path, Duration::from_secs_f64(heard as f64 / rate as f64));
                written = heard;
                s.pos.store(heard, Relaxed);
            }
            idle();
            continue;
        }
        if s.ended.load(Relaxed) {
            idle();
            continue;
        }
        let gain = f32::from_bits(s.gain.load(Relaxed));
        buf.clear();
        buf.extend(dec.by_ref().take(chunk).map(|x| ((x * gain).clamp(-1.0, 1.0) * i16::MAX as f32) as i16));
        buf.truncate(buf.len() / channels * channels);
        if buf.is_empty() {
            if queued {
                pcm.drain();
                queued = false;
            }
            if s.seek.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
                s.pos.store(written, Relaxed);
                s.ended.store(true, Relaxed);
            }
            continue;
        }
        if !pcm.write(&buf, channels) {
            s.dead.store(true, Relaxed);
            return;
        }
        queued = true;
        written += (buf.len() / channels) as u64;
        // A seek that landed meanwhile has already published its own position.
        if s.seek.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
            s.pos.store(written.saturating_sub(pcm.delay()), Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ALSA's `null` device takes any format and discards it, so the whole path runs
    /// without speakers. Skipped where libasound isn't installed.
    #[test]
    fn plays_pauses_and_seeks_on_the_null_device() {
        if lib().is_err() {
            eprintln!("skipped: libasound.so.2 not found");
            return;
        }
        let p = std::env::temp_dir().join(format!("tb-alsa-{}.wav", std::process::id()));
        crate::audio::tests::wav(&p, 8000, 2);
        let out = Out::open_on(c"null", &p, Audio::decoder(&p).unwrap()).unwrap();
        out.pause();
        let mut held = out.pos();
        for _ in 0..100 {
            std::thread::sleep(Duration::from_millis(20));
            if out.pos() == held {
                break;
            }
            held = out.pos();
        }
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(out.pos(), held, "paused clock stands still");
        out.seek(Duration::from_millis(1500));
        assert_eq!(out.pos(), Duration::from_millis(1500), "seek shows at once");
        assert!(out.is_paused() && !out.is_ended());
        out.play();
        for _ in 0..300 {
            if out.is_ended() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(out.is_ended(), "played out the last half second");
        assert_eq!(out.pos(), Duration::from_secs(2));
        out.seek(Duration::ZERO);
        assert!(!out.is_ended(), "seek from the end plays again");
        drop(out);
        let _ = std::fs::remove_file(&p);
    }

    /// Real output through the default device (PipeWire / PulseAudio / hardware): the
    /// clock follows the sound. Needs a sound server, so run by hand with --ignored.
    #[test]
    #[ignore]
    fn clock_follows_the_default_device() {
        let p = std::env::temp_dir().join(format!("tb-alsa-dev-{}.wav", std::process::id()));
        crate::audio::tests::wav(&p, 8000, 2);
        let out = Out::open(&p, Audio::decoder(&p).unwrap()).unwrap();
        std::thread::sleep(Duration::from_millis(1000));
        let at = out.pos().as_secs_f32();
        assert!((0.7..1.2).contains(&at), "1 s in, the clock says {at} s");
        out.pause();
        std::thread::sleep(Duration::from_millis(100));
        let held = out.pos();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(out.pos(), held);
        drop(out);
        let _ = std::fs::remove_file(&p);
    }
}
