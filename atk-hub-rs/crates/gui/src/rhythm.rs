//! Music rhythm: captures what the speakers play, turns it into a spectrum
//! and drives keyboard per-key lighting and audio-reactive wallpapers.

use crate::live::{SharedKeyboard, lock_err};
use crate::user_data_dir;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// Number of spectrum bands (matches the original's `resolution: 64`).
pub const BANDS: usize = 64;
const FFT: usize = 2048;
const FRAME: Duration = Duration::from_millis(33);

/// User settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RhythmPrefs {
    /// `spectrum`, `center`, `pulse`, `wave` or `ripple`.
    pub effect: String,
    /// `gradient`, `rainbow`, `cycle` or `single`.
    pub palette: String,
    /// Colour for the `single` palette.
    pub color: [u8; 3],
    /// Input gain in percent (10-300).
    pub sensitivity: u32,
    /// How fast bars fall (1-60, higher is faster).
    pub gravity: u32,
    /// Drive the keyboard backlight.
    pub keyboard: bool,
    /// Start automatically with the app.
    pub autostart: bool,
}

impl Default for RhythmPrefs {
    fn default() -> Self {
        Self {
            effect: "spectrum".into(),
            palette: "gradient".into(),
            color: [0, 170, 255],
            sensitivity: 100,
            gravity: 20,
            keyboard: true,
            autostart: false,
        }
    }
}

fn prefs_path() -> std::path::PathBuf {
    user_data_dir().join("atk-hub-rs-rhythm.json")
}

fn prefs() -> &'static Mutex<RhythmPrefs> {
    static PREFS: OnceLock<Mutex<RhythmPrefs>> = OnceLock::new();
    PREFS.get_or_init(|| {
        Mutex::new(
            std::fs::read_to_string(prefs_path())
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default(),
        )
    })
}

/// Whether the user switched music rhythm on.
static REQUESTED: AtomicBool = AtomicBool::new(false);
/// Whether the capture thread is alive.
static RUNNING: AtomicBool = AtomicBool::new(false);

/* ---------------------------------------------------------------- analysis -- */

fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * PI / len as f32;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (sin, cos) = (angle * k as f32).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * cos - im[b] * sin;
                let ti = re[b] * sin + im[b] * cos;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// Log-spaced band magnitudes (0..1) of the newest `FFT` samples.
fn bands(samples: &[f32], rate: u32, gain: f32) -> [f32; BANDS] {
    let mut re = vec![0.0; FFT];
    let mut im = vec![0.0; FFT];
    let offset = samples.len().saturating_sub(FFT);
    for (i, &s) in samples[offset..].iter().enumerate() {
        let window = 0.5 - 0.5 * (2.0 * PI * i as f32 / (FFT - 1) as f32).cos();
        re[i] = s * window;
    }
    fft(&mut re, &mut im);
    let bin_hz = rate.max(1) as f32 / FFT as f32;
    let (lo, hi) = (40.0f32, 16_000.0f32.min(rate as f32 / 2.0));
    let mut out = [0.0; BANDS];
    for (band, value) in out.iter_mut().enumerate() {
        let f0 = lo * (hi / lo).powf(band as f32 / BANDS as f32);
        let f1 = lo * (hi / lo).powf((band + 1) as f32 / BANDS as f32);
        let b0 = ((f0 / bin_hz) as usize).max(1);
        let b1 = ((f1 / bin_hz) as usize).max(b0 + 1).min(FFT / 2);
        let peak = (b0..b1)
            .map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt())
            .fold(0.0f32, f32::max);
        // Normalise to the window's coherent gain, tilt up the highs and map
        // roughly 60 dB of range onto 0..1.
        let amplitude = peak * 4.0 / FFT as f32 * (1.0 + band as f32 / 16.0) * gain;
        let db = 20.0 * amplitude.max(1e-6).log10();
        *value = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    }
    out
}

/// One analysed frame.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    /// Left channel bands.
    pub left: [f32; BANDS],
    /// Right channel bands.
    pub right: [f32; BANDS],
    /// Mean of both channels.
    pub mono: [f32; BANDS],
    /// Bass, mids and treble levels.
    pub low: f32,
    /// Mid level.
    pub mid: f32,
    /// Treble level.
    pub high: f32,
    /// Overall level.
    pub energy: f32,
    /// Bass onset this frame.
    pub beat: bool,
}

struct Analyzer {
    left: [f32; BANDS],
    right: [f32; BANDS],
    bass_avg: f32,
    since_beat: u32,
}

impl Analyzer {
    fn new() -> Self {
        Self {
            left: [0.0; BANDS],
            right: [0.0; BANDS],
            bass_avg: 0.0,
            since_beat: 99,
        }
    }

    fn step(&mut self, l: &[f32], r: &[f32], rate: u32, prefs: &RhythmPrefs) -> Frame {
        let gain = prefs.sensitivity.clamp(10, 300) as f32 / 100.0;
        let fall = prefs.gravity.clamp(1, 60) as f32 / 400.0;
        let nl = bands(l, rate, gain);
        let nr = bands(r, rate, gain);
        for i in 0..BANDS {
            self.left[i] = nl[i].max(self.left[i] - fall);
            self.right[i] = nr[i].max(self.right[i] - fall);
        }
        let mut mono = [0.0; BANDS];
        for (value, (l, r)) in mono.iter_mut().zip(self.left.iter().zip(&self.right)) {
            *value = (l + r) / 2.0;
        }
        let avg = |range: std::ops::Range<usize>| {
            let len = range.len() as f32;
            mono[range].iter().sum::<f32>() / len
        };
        let (low, mid, high) = (avg(0..10), avg(10..40), avg(40..BANDS));
        let energy = avg(0..BANDS);
        let bass = avg(0..6);
        self.since_beat = self.since_beat.saturating_add(1);
        let beat = bass > 0.25 && bass > self.bass_avg * 1.35 && self.since_beat > 6;
        if beat {
            self.since_beat = 0;
        }
        self.bass_avg = self.bass_avg * 0.9 + bass * 0.1;
        Frame {
            left: self.left,
            right: self.right,
            mono,
            low,
            mid,
            high,
            energy,
            beat,
        }
    }
}

/* ----------------------------------------------------------------- effects -- */

fn hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let to = |value: f32| ((value + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    [to(r), to(g), to(b)]
}

fn scale(color: [u8; 3], level: f32) -> [u8; 3] {
    color.map(|c| (f32::from(c) * level.clamp(0.0, 1.0)).round() as u8)
}

/// Resamples `values` to `n` columns by taking each group's peak.
fn columns(values: &[f32], n: usize) -> Vec<f32> {
    let n = n.max(1);
    (0..n)
        .map(|c| {
            let a = c * values.len() / n;
            let b = ((c + 1) * values.len() / n).max(a + 1).min(values.len());
            values[a..b].iter().copied().fold(0.0, f32::max)
        })
        .collect()
}

/// Effect state that persists between frames.
pub struct Painter {
    time: f32,
    flash: f32,
    ripples: Vec<f32>,
}

impl Painter {
    /// A fresh painter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            time: 0.0,
            flash: 0.0,
            ripples: Vec::new(),
        }
    }

    fn palette(&self, prefs: &RhythmPrefs, x: f32, level: f32) -> [u8; 3] {
        match prefs.palette.as_str() {
            "rainbow" => hsv(x * 0.85, 1.0, 1.0),
            "cycle" => hsv(self.time * 0.08 + x * 0.15, 1.0, 1.0),
            "single" => prefs.color,
            // Green at the bottom through yellow to red at the peak.
            _ => hsv(0.33 * (1.0 - level.clamp(0.0, 1.0)), 1.0, 1.0),
        }
    }

    /// Colours for a `rows x cols` keyboard, row-major.
    pub fn paint(
        &mut self,
        frame: &Frame,
        prefs: &RhythmPrefs,
        rows: usize,
        cols: usize,
    ) -> Vec<[u8; 3]> {
        self.time += FRAME.as_secs_f32();
        self.flash = if frame.beat {
            1.0
        } else {
            (self.flash - 0.12).max(0.0)
        };
        if frame.beat {
            self.ripples.push(0.0);
        }
        for r in &mut self.ripples {
            *r += 0.06 + frame.energy * 0.05;
        }
        self.ripples.retain(|r| *r < 1.6);
        let (rows, cols) = (rows.max(1), cols.max(1));
        let level_cols = match prefs.effect.as_str() {
            "center" => {
                let half = columns(&frame.mono, cols.div_ceil(2));
                (0..cols)
                    .map(|c| {
                        let mid = (cols - 1) as f32 / 2.0;
                        let d = ((c as f32 - mid).abs()).round() as usize;
                        half[d.min(half.len() - 1)]
                    })
                    .collect()
            }
            _ => columns(&frame.mono, cols),
        };
        let mut out = Vec::with_capacity(rows * cols);
        for row in 0..rows {
            for (col, &level) in level_cols.iter().enumerate() {
                let x = col as f32 / (cols - 1).max(1) as f32;
                let height = (rows - row) as f32 / rows as f32; // 1 at the top row
                let color = match prefs.effect.as_str() {
                    "pulse" => {
                        let base = self.palette(prefs, x, frame.energy);
                        let lit = scale(base, 0.15 + frame.energy * 1.4);
                        let white = scale([255, 255, 255], self.flash * 0.6);
                        [0, 1, 2].map(|i| lit[i].saturating_add(white[i]))
                    }
                    "wave" => {
                        let base = match prefs.palette.as_str() {
                            "single" => prefs.color,
                            _ => hsv(x + self.time * 0.15, 1.0, 1.0),
                        };
                        scale(base, 0.1 + level * 1.2)
                    }
                    "ripple" => {
                        let (cx, cy) = (x - 0.5, (row as f32 + 0.5) / rows as f32 - 0.5);
                        let dist = (cx * cx + cy * cy * 0.5).sqrt() * 2.0;
                        let glow = self
                            .ripples
                            .iter()
                            .map(|r| (1.0 - (dist - r).abs() * 4.0).max(0.0) * (1.6 - r) / 1.6)
                            .fold(0.0, f32::max);
                        let base = self.palette(prefs, dist / 1.4, dist / 1.4);
                        scale(base, 0.08 + frame.low * 0.3 + glow)
                    }
                    // Spectrum bars rising from the bottom row.
                    _ => {
                        let threshold = height - 0.5 / rows as f32;
                        if level >= threshold {
                            self.palette(prefs, x, height)
                        } else {
                            [0, 0, 0]
                        }
                    }
                };
                out.push(color);
            }
        }
        out
    }
}

impl Default for Painter {
    fn default() -> Self {
        Self::new()
    }
}

/* ------------------------------------------------------------------ engine -- */

struct Buffers {
    left: VecDeque<f32>,
    right: VecDeque<f32>,
    rate: u32,
}

/// Starts the capture and render threads if they are not running.
fn ensure_engine(app: &AppHandle) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let buffers = Arc::new(Mutex::new(Buffers {
        left: VecDeque::with_capacity(FFT * 2),
        right: VecDeque::with_capacity(FFT * 2),
        rate: 48_000,
    }));
    let stop = Arc::new(AtomicBool::new(false));

    #[cfg(windows)]
    {
        let buffers = Arc::clone(&buffers);
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let result = atk_input::loopback::capture(&stop, |l, r, rate| {
                    if let Ok(mut b) = buffers.lock() {
                        b.rate = rate;
                        b.left.extend(l);
                        b.right.extend(r);
                        let extra = b.left.len().saturating_sub(FFT);
                        b.left.drain(..extra);
                        let extra = b.right.len().saturating_sub(FFT);
                        b.right.drain(..extra);
                    }
                });
                if result.is_err() {
                    // Default device changed or vanished: retry shortly.
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        });
    }

    let app = app.clone();
    std::thread::spawn(move || {
        let keyboard: SharedKeyboard = Arc::clone(&app.state::<crate::HubState>().keyboard);
        let mut analyzer = Analyzer::new();
        let mut painter = Painter::new();
        let mut streaming = false;
        let mut saved_light = None;
        let mut next = Instant::now();
        let mut last_open = Instant::now() - Duration::from_secs(10);
        loop {
            let wallpapers = crate::livewall::has_html_wallpaper();
            let requested = REQUESTED.load(Ordering::SeqCst);
            if !requested && !wallpapers {
                break;
            }
            let current = prefs().lock().map(|p| p.clone()).unwrap_or_default();
            let (l, r, rate) = match buffers.lock() {
                Ok(b) => (
                    b.left.iter().copied().collect::<Vec<_>>(),
                    b.right.iter().copied().collect::<Vec<_>>(),
                    b.rate,
                ),
                Err(_) => break,
            };
            let frame = analyzer.step(&l, &r, rate, &current);
            if requested {
                let _ = app.emit_to(
                    "main",
                    "rhythm:frame",
                    serde_json::json!({
                        "bands": frame.mono.to_vec(),
                        "beat": frame.beat,
                        "energy": frame.energy,
                        "low": frame.low,
                        "mid": frame.mid,
                        "high": frame.high,
                    }),
                );
            }
            if wallpapers {
                let mut spectrum = frame.left.to_vec();
                spectrum.extend_from_slice(&frame.right);
                crate::livewall::forward_audio(&app, &spectrum);
            }
            let want_kb = requested && current.keyboard;
            if (want_kb || streaming)
                && let Ok(mut guard) = keyboard.try_lock()
            {
                if guard.is_none() && want_kb && last_open.elapsed() > Duration::from_secs(2) {
                    last_open = Instant::now();
                    *guard = atk_hid::keyboard::Keyboard::open_first()
                        .ok()
                        .map(|(kb, _)| kb);
                }
                if let Some(kb) = guard.as_mut() {
                    if want_kb && !streaming {
                        saved_light = kb.lighting().ok();
                        streaming = kb.begin_rgb_stream().is_ok();
                    }
                    if want_kb && streaming {
                        let (rows, cols) =
                            (usize::from(kb.model().rows), usize::from(kb.model().cols));
                        let colors = painter.paint(&frame, &current, rows, cols);
                        if kb.stream_rgb(&colors).is_err() {
                            *guard = None;
                            streaming = false;
                        }
                    } else if !want_kb && streaming {
                        if let Some(light) = saved_light.take() {
                            let _ = kb.set_lighting(light);
                        }
                        streaming = false;
                    }
                }
            }
            next += FRAME;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else {
                next = now;
            }
        }
        if streaming
            && let Some(light) = saved_light
            && let Ok(mut guard) = keyboard.lock()
            && let Some(kb) = guard.as_mut()
        {
            let _ = kb.set_lighting(light);
        }
        stop.store(true, Ordering::SeqCst);
        RUNNING.store(false, Ordering::SeqCst);
        // A start request may have raced with this shutdown.
        if REQUESTED.load(Ordering::SeqCst) || crate::livewall::has_html_wallpaper() {
            ensure_engine(&app);
        }
    });
}

/// Keeps capture running while an HTML wallpaper needs the spectrum.
pub fn wallpaper_needs_audio(app: &AppHandle) {
    ensure_engine(app);
}

/// Starts music rhythm on launch when the user asked for it.
pub fn autostart(app: &AppHandle) {
    if prefs().lock().is_ok_and(|p| p.autostart) {
        REQUESTED.store(true, Ordering::SeqCst);
        ensure_engine(app);
    }
}

#[tauri::command]
pub fn rhythm_get() -> Result<serde_json::Value, String> {
    let p = prefs().lock().map_err(lock_err)?.clone();
    #[cfg(windows)]
    let device = atk_input::loopback::default_output_name();
    #[cfg(not(windows))]
    let device: Option<String> = None;
    Ok(serde_json::json!({
        "prefs": p,
        "running": REQUESTED.load(Ordering::SeqCst),
        "device": device,
    }))
}

#[tauri::command]
pub fn rhythm_set(settings: RhythmPrefs) -> Result<serde_json::Value, String> {
    let text = serde_json::to_string_pretty(&settings).map_err(|error| error.to_string())?;
    std::fs::write(prefs_path(), text).map_err(|error| error.to_string())?;
    *prefs().lock().map_err(lock_err)? = settings;
    rhythm_get()
}

#[tauri::command]
pub fn rhythm_start(app: AppHandle) -> Result<serde_json::Value, String> {
    REQUESTED.store(true, Ordering::SeqCst);
    ensure_engine(&app);
    rhythm_get()
}

#[tauri::command]
pub fn rhythm_stop() -> Result<serde_json::Value, String> {
    REQUESTED.store(false, Ordering::SeqCst);
    rhythm_get()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, rate: u32) -> Vec<f32> {
        (0..FFT)
            .map(|i| (2.0 * PI * freq * i as f32 / rate as f32).sin() * 0.5)
            .collect()
    }

    #[test]
    fn tone_peaks_in_matching_band() {
        let low = bands(&tone(100.0, 48_000), 48_000, 1.0);
        let high = bands(&tone(5_000.0, 48_000), 48_000, 1.0);
        let argmax = |b: &[f32; BANDS]| (0..BANDS).max_by(|&x, &y| b[x].total_cmp(&b[y])).unwrap();
        assert!(argmax(&low) < 16, "100 Hz peaked at {}", argmax(&low));
        assert!(argmax(&high) > 40, "5 kHz peaked at {}", argmax(&high));
        assert!(
            bands(&vec![0.0; FFT], 48_000, 1.0)
                .iter()
                .all(|v| *v == 0.0)
        );
    }

    #[test]
    fn spectrum_effect_lights_bottom_rows_first() {
        let mut frame = Frame {
            left: [0.0; BANDS],
            right: [0.0; BANDS],
            mono: [0.0; BANDS],
            low: 0.0,
            mid: 0.0,
            high: 0.0,
            energy: 0.0,
            beat: false,
        };
        frame.mono[..32].fill(0.5);
        let colors = Painter::new().paint(&frame, &RhythmPrefs::default(), 4, 2);
        // Left column at half height: bottom two rows lit, top two dark.
        assert_eq!(colors[0], [0, 0, 0]);
        assert_eq!(colors[2], [0, 0, 0]);
        assert_ne!(colors[4], [0, 0, 0]);
        assert_ne!(colors[6], [0, 0, 0]);
        // Right column silent.
        assert_eq!(colors[7], [0, 0, 0]);
    }

    #[test]
    fn hsv_primaries() {
        assert_eq!(hsv(0.0, 1.0, 1.0), [255, 0, 0]);
        assert_eq!(hsv(1.0 / 3.0, 1.0, 1.0), [0, 255, 0]);
    }
}
