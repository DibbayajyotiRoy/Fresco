//! Signal processing for the audio-reactive visualiser (WIDGETS_ROADMAP W3 / §6.1).
//!
//! Pure maths over `f32` sample buffers — no I/O, no globals, no threads — so the
//! whole audio pipeline can be unit-tested off-device and stays platform-neutral
//! (same "brain code" principle as [`crate::schedule`]). Capture lives elsewhere;
//! this module only ever sees a slice of samples.
//!
//! Everything here is dependency-free on purpose. A radix-2 FFT is ~100 lines and
//! the maintainer prefers small testable code over a crate we would then have to
//! audit, license-check and ship in the Flatpak.
//!
//! # Pipeline
//!
//! ```text
//! samples ─► ring buffer ─► window ─► FFT ─► magnitude ─► log-spaced bands
//!                                                              │
//!                              0..1 bar heights ◄─ attack/release ◄─ dB + floor
//! ```
//!
//! # Power model
//!
//! The visualiser is the one widget that legitimately redraws continuously, so its
//! cost has to be *bounded and measurable*. Two properties make that true:
//!
//! * Cost per [`SpectrumAnalyzer::process`] call is fixed — `O(N log N)` at a fixed
//!   `N`, with every table (twiddles, bit-reversal, window, band edges) precomputed
//!   once in [`SpectrumAnalyzer::new`]. No allocation happens on the hot path.
//! * [`SpectrumAnalyzer::is_silent`] lets the daemon stop redrawing entirely when
//!   nothing is playing. "No audio ⇒ no FFT ⇒ no redraw" is rule 6 of the power
//!   model, and it is only enforceable if this module answers the question.

use std::f32::consts::TAU;

/// Largest FFT size we will plan for.
///
/// Well beyond anything a visualiser needs (2^16 at 44.1 kHz is a 1.5 s window,
/// which would look like treacle), and it keeps the bit-reversal table inside a
/// `u32` so the plan stays half the size it would otherwise be.
const MAX_FFT_SIZE: usize = 1 << 16;

/// Magnitude floor used before taking a logarithm, ≈ -240 dB.
///
/// Digital silence is exactly 0.0 and `log10(0)` is `-inf`, which would poison
/// every downstream average and clamp. Clamping first keeps the pipeline free of
/// non-finite values without a single branch on NaN.
const MAG_EPSILON: f32 = 1e-12;

/// A band below this is sub-pixel on any realistic bar height, i.e. visually zero.
const BAND_EPSILON: f32 = 1e-3;

// ---------------------------------------------------------------------------
// FFT
// ---------------------------------------------------------------------------

/// A reusable radix-2 Cooley-Tukey FFT plan for a fixed size.
///
/// The visualiser calls the FFT tens of times a second at a *fixed* `N`, so the
/// twiddle factors and the bit-reversal permutation are computed once here rather
/// than per call. That turns each transform into pure arithmetic over preallocated
/// tables — no `sin`/`cos` calls and no allocation on the hot path.
///
/// Twiddles are stored as a single half-length table indexed with a stride instead
/// of one table per butterfly stage: stage `len` needs `exp(-2πi·j/len)`, which is
/// `table[j · N/len]`. One table, no per-stage bookkeeping, better cache behaviour.
#[derive(Clone, Debug)]
pub struct FftPlan {
    n: usize,
    /// `cos(-2πk/n)` for `k` in `0..n/2`.
    cos: Vec<f32>,
    /// `sin(-2πk/n)` for `k` in `0..n/2`.
    sin: Vec<f32>,
    /// `rev[i]` is `i` with its `log2(n)` low bits reversed.
    rev: Vec<u32>,
}

impl FftPlan {
    /// Builds a plan for `n` points.
    ///
    /// Returns `None` unless `n` is a power of two with `2 <= n <= 65536`. Radix-2
    /// Cooley-Tukey is only defined for power-of-two lengths, and returning an
    /// `Option` rather than panicking means a bad value in a config file degrades
    /// the widget instead of taking the daemon down.
    #[must_use]
    pub fn new(n: usize) -> Option<Self> {
        if n < 2 || !n.is_power_of_two() || n > MAX_FFT_SIZE {
            return None;
        }
        let half = n / 2;
        let mut cos = Vec::with_capacity(half);
        let mut sin = Vec::with_capacity(half);
        for k in 0..half {
            // Computed in f64 and narrowed: at N = 65536 the f32 rounding of the
            // angle itself is the dominant error term, and it is free to avoid.
            let angle = -2.0 * std::f64::consts::PI * (k as f64) / (n as f64);
            cos.push(angle.cos() as f32);
            sin.push(angle.sin() as f32);
        }

        // Bit-reversal permutation. `reverse_bits` reverses the whole word, so we
        // shift the result back down to the low `log2(n)` bits.
        let shift = usize::BITS - n.trailing_zeros();
        let rev = (0..n).map(|i| (i.reverse_bits() >> shift) as u32).collect();

        Some(Self { n, cos, sin, rev })
    }

    /// Number of points this plan transforms.
    #[must_use]
    pub fn len(&self) -> usize {
        self.n
    }

    /// Always `false` — a plan is never empty (`n >= 2`). Present because clippy
    /// reasonably insists that anything with `len` also has `is_empty`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Number of usable magnitude bins, `n / 2` (DC up to but excluding Nyquist).
    ///
    /// For real input the spectrum is conjugate-symmetric, so the upper half is a
    /// mirror and carries no extra information.
    #[must_use]
    pub fn bins(&self) -> usize {
        self.n / 2
    }

    /// In-place forward complex FFT (sign convention `exp(-2πi·k·t/N)`, unscaled).
    ///
    /// `re` and `im` must both be exactly [`FftPlan::len`] long. A mismatch trips a
    /// `debug_assert` in debug builds and is a no-op in release — a visualiser must
    /// never panic the daemon over a buffer-size bug, and leaving the buffers alone
    /// is strictly better than writing garbage into them.
    pub fn run(&self, re: &mut [f32], im: &mut [f32]) {
        debug_assert_eq!(re.len(), self.n, "FftPlan::run: re has wrong length");
        debug_assert_eq!(im.len(), self.n, "FftPlan::run: im has wrong length");
        if re.len() != self.n || im.len() != self.n {
            return;
        }

        // 1. Reorder into bit-reversed index order so the butterflies below can run
        //    in place, reading and writing the same slots.
        for (i, &r) in self.rev.iter().enumerate() {
            let j = r as usize;
            // Guard with `i < j` so each pair is swapped exactly once.
            if i < j {
                re.swap(i, j);
                im.swap(i, j);
            }
        }

        // 2. Iterative butterflies, doubling the transform length each stage.
        let mut len = 2;
        while len <= self.n {
            let half = len / 2;
            let stride = self.n / len;
            let mut base = 0;
            while base < self.n {
                for j in 0..half {
                    let t = j * stride;
                    let (wr, wi) = (self.cos[t], self.sin[t]);
                    let a = base + j;
                    let b = a + half;
                    // (wr + i·wi) · (re[b] + i·im[b])
                    let xr = re[b] * wr - im[b] * wi;
                    let xi = re[b] * wi + im[b] * wr;
                    re[b] = re[a] - xr;
                    im[b] = im[a] - xi;
                    re[a] += xr;
                    im[a] += xi;
                }
                base += len;
            }
            len <<= 1;
        }
    }
}

/// One-shot in-place forward FFT — builds a throwaway [`FftPlan`] per call.
///
/// The buffers must be equal-length powers of two, `2..=65536`. Convenient for
/// tests and one-off analysis; **do not** use it on the visualiser hot path, where
/// building the twiddle tables would dwarf the transform itself. Keep an [`FftPlan`]
/// there instead.
///
/// A bad length trips a `debug_assert` in debug builds and leaves the buffers
/// untouched in release, for the same reason as [`FftPlan::run`].
pub fn fft_in_place(re: &mut [f32], im: &mut [f32]) {
    debug_assert_eq!(re.len(), im.len(), "fft_in_place: length mismatch");
    debug_assert!(
        re.len() >= 2 && re.len().is_power_of_two() && re.len() <= MAX_FFT_SIZE,
        "fft_in_place: length must be a power of two in 2..=65536"
    );
    if re.len() != im.len() {
        return;
    }
    if let Some(plan) = FftPlan::new(re.len()) {
        plan.run(re, im);
    }
}

// ---------------------------------------------------------------------------
// Windowing
// ---------------------------------------------------------------------------

/// Analysis window applied before the FFT.
///
/// **Why window at all:** the FFT assumes the buffer repeats forever. Any tone that
/// is not an exact whole number of cycles long therefore has a step discontinuity at
/// the wrap point, and that step smears energy across *every* bin (spectral leakage).
/// Untreated, a steady note makes the bars shimmer and neighbouring bars flicker in
/// sympathy as the phase drifts frame to frame. Tapering the buffer to zero at both
/// ends removes the discontinuity and the bars go still.
///
/// The trade is main-lobe width (frequency resolution) against sidelobe level
/// (leakage). [`Window::Hann`] is the default because it is the best-behaved
/// general-purpose compromise for music.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Window {
    /// Raised cosine. -31 dB sidelobes falling fast, 4-bin main lobe. Default.
    #[default]
    Hann,
    /// Like Hann but with a non-zero pedestal: lower first sidelobe (-43 dB), worse
    /// far-field rejection. Slightly crisper on isolated tones.
    Hamming,
    /// 4-term, -92 dB sidelobes. Effectively zero leakage at the cost of a main lobe
    /// twice as wide as Hann — use it when a loud bass note is bleeding into the mids.
    BlackmanHarris,
    /// No window. Only correct when the buffer already contains a whole number of
    /// cycles, which real audio never does. Present for tests and for callers who
    /// have already windowed.
    Rect,
}

impl Window {
    /// Cosine-sum coefficients `[a0, a1, a2, a3]` for `w[i] = a0 - a1·cos(θ) +
    /// a2·cos(2θ) - a3·cos(3θ)`, with `θ = 2πi/(N-1)`.
    const fn coefficients(self) -> [f32; 4] {
        match self {
            Window::Hann => [0.5, 0.5, 0.0, 0.0],
            Window::Hamming => [0.54, 0.46, 0.0, 0.0],
            Window::BlackmanHarris => [0.35875, 0.48829, 0.14128, 0.01168],
            Window::Rect => [1.0, 0.0, 0.0, 0.0],
        }
    }

    /// Coherent gain — the window's mean value, i.e. the factor by which it shrinks
    /// a sinusoid's peak bin.
    ///
    /// For a cosine-sum window this is exactly `a0`. Dividing magnitudes by it means
    /// a full-scale sine reads 0 dBFS whichever window is selected, so
    /// [`SpectrumConfig::noise_floor_db`] keeps the same meaning when the user
    /// changes windows. Without this, switching Hann → Blackman-Harris would drop
    /// every bar by 3 dB for no visible reason.
    #[must_use]
    pub fn coherent_gain(self) -> f32 {
        self.coefficients()[0]
    }

    /// The window value at sample `i` of `n`.
    #[must_use]
    pub fn value(self, i: usize, n: usize) -> f32 {
        if self == Window::Rect {
            return 1.0;
        }
        // A 1-point window has no interior to taper, and `n - 1` would divide by
        // zero. The only sane value is 1.0.
        if n < 2 {
            return 1.0;
        }
        let [a0, a1, a2, a3] = self.coefficients();
        // Symmetric (denominator `n - 1`) rather than periodic (`n`): the difference
        // is one sample in ~1000 and symmetric windows are what the textbook
        // coherent-gain figures above assume.
        let theta = TAU * (i as f32) / ((n - 1) as f32);
        a0 - a1 * theta.cos() + a2 * (2.0 * theta).cos() - a3 * (3.0 * theta).cos()
    }
}

/// Multiplies `buf` in place by the window `w`.
///
/// Recomputes the window on every call. That is fine for one-off use, but
/// [`SpectrumAnalyzer`] precomputes the table instead — three `cos` calls per sample
/// per frame is real money at 60 Hz.
pub fn apply_window(buf: &mut [f32], w: Window) {
    if w == Window::Rect {
        return;
    }
    let n = buf.len();
    for (i, s) in buf.iter_mut().enumerate() {
        *s *= w.value(i, n);
    }
}

// ---------------------------------------------------------------------------
// Level metering
// ---------------------------------------------------------------------------

/// Root-mean-square level of `samples`.
///
/// Tracks perceived loudness far better than [`peak`], and it is ~1 % of the cost of
/// an FFT — enough on its own for visualiser themes that are a single pulsing shape.
/// Returns `0.0` for an empty slice (no samples means no level; never NaN).
#[must_use]
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}

/// Largest absolute sample value in `samples`, or `0.0` if empty.
///
/// Use for clip indicators and snappy transient-driven effects. `f32::max` is used
/// rather than a comparison so a stray NaN in the capture buffer cannot propagate.
#[must_use]
pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |acc, s| acc.max(s.abs()))
}

// ---------------------------------------------------------------------------
// Spectrum
// ---------------------------------------------------------------------------

/// Tuning for [`SpectrumAnalyzer`].
///
/// Values are sanitised by [`SpectrumAnalyzer::new`], so a hand-edited config file
/// can never produce a panic or a degenerate analyser; read
/// [`SpectrumAnalyzer::config`] back to see what was actually used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectrumConfig {
    /// Sample rate of the incoming audio, in Hz.
    pub sample_rate: u32,
    /// FFT window length in samples. Rounded up to a power of two, clamped to
    /// `64..=65536`.
    ///
    /// 1024 at 44.1 kHz is a 23 ms window: long enough to resolve ~43 Hz bins, short
    /// enough that the bars still track a fast drum pattern. Doubling it buys bass
    /// resolution and costs responsiveness.
    pub fft_size: usize,
    /// Number of output bars. Clamped to `1..=512`.
    pub bands: usize,
    /// Lowest frequency shown, in Hz. Below ~40 Hz there is little musical content
    /// and a lot of rumble, and at `fft_size = 1024` a single bin is already 43 Hz
    /// wide, so asking for less buys nothing.
    pub min_hz: f32,
    /// Highest frequency shown, in Hz. Clamped below Nyquist. 16 kHz rather than the
    /// full 22.05 kHz because the top octave is nearly always empty in lossy-encoded
    /// music, and empty bars at the right-hand edge look broken.
    pub max_hz: f32,
    /// Analysis window. See [`Window`].
    pub window: Window,
    /// Rise time constant in **seconds**. Short, so a transient is on screen the
    /// frame it happens.
    pub attack: f32,
    /// Fall time constant in **seconds**. Long, so bars glide down instead of
    /// strobing.
    pub release: f32,
    /// Level mapped to a zero-height bar, in dBFS (negative). Also the threshold
    /// [`SpectrumAnalyzer::is_silent`] compares the overall level against.
    ///
    /// -70 dBFS is quiet enough to sit under room tone and hiss, high enough that a
    /// paused player is reported silent immediately.
    pub noise_floor_db: f32,
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            fft_size: 1024,
            bands: 32,
            min_hz: 40.0,
            max_hz: 16_000.0,
            window: Window::Hann,
            // ~20 ms rise / ~250 ms fall. These are one-pole time constants: after
            // one `attack` the bar has covered 63 % of the jump, after three it is
            // within 5 %. The asymmetry is the whole trick — symmetric smoothing
            // either lags the beat or strobes, never neither.
            attack: 0.020,
            release: 0.250,
            noise_floor_db: -70.0,
        }
    }
}

/// Streaming FFT spectrum analyser producing smoothed, log-spaced bar heights.
///
/// Owns a ring buffer of the last `fft_size` samples, so callers may push whatever
/// chunk size their audio backend hands them (PipeWire quanta of 256, 512, 1024 …)
/// without doing any buffering of their own.
///
/// All state is preallocated in [`SpectrumAnalyzer::new`]; [`SpectrumAnalyzer::process`]
/// allocates nothing.
#[derive(Clone, Debug)]
pub struct SpectrumAnalyzer {
    cfg: SpectrumConfig,
    plan: FftPlan,
    /// Precomputed analysis window, `fft_size` long.
    window: Vec<f32>,
    /// Ring buffer of the most recent `fft_size` samples.
    history: Vec<f32>,
    /// Write cursor into `history`; also the index of the *oldest* sample.
    cursor: usize,
    /// Scratch real/imaginary buffers for the transform.
    re: Vec<f32>,
    im: Vec<f32>,
    /// Half-open `[lo, hi)` bin range per band.
    edges: Vec<(usize, usize)>,
    /// Smoothed output, `bands` long, each in `0.0..=1.0`.
    out: Vec<f32>,
    /// RMS of the most recent analysis window, in dBFS.
    level_db: f32,
}

impl SpectrumAnalyzer {
    /// Builds an analyser, sanitising `cfg`.
    #[must_use]
    pub fn new(cfg: SpectrumConfig) -> Self {
        let cfg = sanitize(cfg);
        // `sanitize` guarantees a valid power-of-two size, so the plan cannot fail;
        // the fallback keeps this constructor total rather than adding an `unwrap`.
        let plan = FftPlan::new(cfg.fft_size)
            .unwrap_or_else(|| FftPlan::new(1024).expect("1024 is a valid FFT size"));
        let n = plan.len();
        let window = (0..n).map(|i| cfg.window.value(i, n)).collect();
        let edges = band_edges(&cfg, n);
        Self {
            cfg,
            plan,
            window,
            history: vec![0.0; n],
            cursor: 0,
            re: vec![0.0; n],
            im: vec![0.0; n],
            edges,
            out: vec![0.0; cfg.bands],
            level_db: MAG_EPSILON.log10() * 20.0,
        }
    }

    /// The sanitised configuration actually in use.
    #[must_use]
    pub fn config(&self) -> &SpectrumConfig {
        &self.cfg
    }

    /// Most recent bar heights without advancing the analyser.
    #[must_use]
    pub fn bands(&self) -> &[f32] {
        &self.out
    }

    /// RMS level of the most recent analysis window, in dBFS (negative).
    ///
    /// Roughly -240 dB before any audio has been seen and for digital silence.
    #[must_use]
    pub fn level_db(&self) -> f32 {
        self.level_db
    }

    /// Half-open FFT bin range `[lo, hi)` covered by `band`, or `None` if out of range.
    #[must_use]
    pub fn band_bins(&self, band: usize) -> Option<(usize, usize)> {
        self.edges.get(band).copied()
    }

    /// Frequency range in Hz covered by `band`, or `None` if out of range. Useful
    /// for axis labels and tooltips.
    #[must_use]
    pub fn band_range_hz(&self, band: usize) -> Option<(f32, f32)> {
        let (lo, hi) = self.band_bins(band)?;
        let bin_hz = self.cfg.sample_rate as f32 / self.plan.len() as f32;
        Some((lo as f32 * bin_hz, hi as f32 * bin_hz))
    }

    /// Whether the signal is effectively silent **and** the bars have finished falling.
    ///
    /// This is the power-model hook: while it returns `true` the daemon can stop
    /// redrawing the visualiser entirely (roadmap rule 6, "no audio ⇒ no FFT ⇒ no
    /// redraw"). Note that it is deliberately *not* just a level test — a level test
    /// alone would cut the redraw the instant the music stopped and freeze the bars
    /// mid-air. Requiring every band to have decayed to zero first means the release
    /// envelope plays out, the bars land at zero, and only then does the widget go
    /// idle.
    ///
    /// The exponential fall means that costs a bounded tail: from a full-height bar
    /// it takes about seven [`SpectrumConfig::release`] time constants — ~1.7 s at
    /// the defaults — for this to turn `true` after the audio stops.
    #[must_use]
    pub fn is_silent(&self) -> bool {
        self.level_db <= self.cfg.noise_floor_db && self.out.iter().all(|v| *v < BAND_EPSILON)
    }

    /// Clears the history, the smoothing state and the reported level.
    ///
    /// Call it when the capture stream is restarted, so stale audio from before the
    /// gap cannot flash onto the bars.
    pub fn reset(&mut self) {
        self.history.fill(0.0);
        self.cursor = 0;
        self.out.fill(0.0);
        self.level_db = MAG_EPSILON.log10() * 20.0;
    }

    /// Feeds newly captured `samples` and returns the updated bar heights.
    ///
    /// `samples` is the audio that arrived *since the last call*, in `-1.0..=1.0`;
    /// its length sets the time step used for smoothing, so the envelope is
    /// wall-clock correct whatever chunk size the backend delivers. An empty slice
    /// is a no-op (no elapsed time, nothing to integrate) and returns the previous
    /// frame unchanged.
    ///
    /// The returned slice is [`SpectrumConfig::bands`] long, each value in
    /// `0.0..=1.0`, ready to multiply by a bar height.
    pub fn process(&mut self, samples: &[f32]) -> &[f32] {
        if samples.is_empty() {
            return &self.out;
        }
        self.push(samples);
        self.fill_scratch();

        // Level is measured on the raw window, before the taper — the window would
        // otherwise knock ~3 dB off the reading and make the silence threshold
        // depend on which window is selected.
        self.level_db = 20.0 * rms(&self.re).max(MAG_EPSILON).log10();

        for (s, w) in self.re.iter_mut().zip(self.window.iter()) {
            *s *= *w;
        }
        self.im.fill(0.0);
        self.plan.run(&mut self.re, &mut self.im);

        let dt = samples.len() as f32 / self.cfg.sample_rate as f32;
        self.reduce_and_smooth(dt);
        &self.out
    }

    /// Copies `samples` into the ring buffer, keeping only the newest `fft_size`.
    fn push(&mut self, samples: &[f32]) {
        let n = self.history.len();
        // A chunk longer than the window overwrites it completely; take the tail and
        // realign the cursor instead of wrapping around several times.
        if samples.len() >= n {
            self.history.copy_from_slice(&samples[samples.len() - n..]);
            self.cursor = 0;
            return;
        }
        let tail = n - self.cursor;
        if samples.len() <= tail {
            self.history[self.cursor..self.cursor + samples.len()].copy_from_slice(samples);
            self.cursor = (self.cursor + samples.len()) % n;
        } else {
            let (head, rest) = samples.split_at(tail);
            self.history[self.cursor..].copy_from_slice(head);
            self.history[..rest.len()].copy_from_slice(rest);
            self.cursor = rest.len();
        }
    }

    /// Unrolls the ring buffer into `re` in chronological order.
    fn fill_scratch(&mut self) {
        let n = self.history.len();
        let split = n - self.cursor;
        self.re[..split].copy_from_slice(&self.history[self.cursor..]);
        self.re[split..].copy_from_slice(&self.history[..self.cursor]);
    }

    /// Groups bins into bands, converts to dB, normalises and smooths.
    fn reduce_and_smooth(&mut self, dt: f32) {
        // One-pole coefficients derived from the elapsed time rather than fixed per
        // frame: `1 - exp(-dt/tau)`. A fixed coefficient would make the bars rise
        // and fall at a speed that depends on the capture buffer size, so plugging in
        // a different sound card would visibly change the animation.
        let a_att = coefficient(dt, self.cfg.attack);
        let a_rel = coefficient(dt, self.cfg.release);

        let scale = 1.0 / ((self.plan.len() as f32 / 2.0) * self.cfg.window.coherent_gain());
        let floor = self.cfg.noise_floor_db;
        // `sanitize` guarantees `floor < 0`, so this span is strictly positive.
        let span = -floor;

        for (band, &(lo, hi)) in self.edges.iter().enumerate() {
            // Peak-hold across the band, not an average. A band near 10 kHz spans
            // dozens of bins while a bass band spans one; averaging would divide a
            // pure tone's energy by the band width, so the same note would render
            // tall in the bass and invisible in the treble purely because of how the
            // bands were cut. Taking the strongest bin makes bar height mean "the
            // loudest thing in this frequency range", which is what a viewer reads
            // off the display anyway.
            let mut mag = 0.0f32;
            for k in lo..hi {
                let m = self.re[k] * self.re[k] + self.im[k] * self.im[k];
                mag = mag.max(m);
            }
            // One sqrt per band instead of one per bin.
            let mag = mag.sqrt() * scale;

            // dB, not linear. Music has a huge dynamic range; on a linear scale the
            // bars sit pinned at the bottom until a peak slams them to the top.
            // Hearing is logarithmic, so a dB axis is also what "looks like the
            // music sounds".
            let db = 20.0 * mag.max(MAG_EPSILON).log10();
            let target = ((db - floor) / span).clamp(0.0, 1.0);

            let prev = self.out[band];
            // Fast attack, slow release. Rising instantly is what makes the display
            // feel locked to the beat; falling slowly is what stops it strobing.
            let a = if target > prev { a_att } else { a_rel };
            let next = prev + (target - prev) * a;
            // A one-pole decay approaches zero but never reaches it. Snapping the
            // sub-pixel tail to exactly zero bounds how long it takes `is_silent` to
            // become true after the music stops — without it the daemon would keep
            // redrawing invisible bars forever.
            self.out[band] = if next < BAND_EPSILON && target < BAND_EPSILON {
                0.0
            } else {
                next
            };
        }
    }
}

/// One-pole smoothing coefficient for a time step `dt` and time constant `tau`.
fn coefficient(dt: f32, tau: f32) -> f32 {
    // tau == 0 means "no smoothing": jump straight to the target.
    if tau <= 0.0 {
        return 1.0;
    }
    (1.0 - (-dt / tau).exp()).clamp(0.0, 1.0)
}

/// Clamps a config into a range the analyser can actually honour.
fn sanitize(mut cfg: SpectrumConfig) -> SpectrumConfig {
    cfg.sample_rate = cfg.sample_rate.clamp(8_000, 384_000);
    cfg.fft_size = cfg.fft_size.clamp(64, MAX_FFT_SIZE).next_power_of_two();
    cfg.bands = cfg.bands.clamp(1, 512);

    let nyquist = cfg.sample_rate as f32 / 2.0;
    // NaN-safe: `is_finite` first, because comparisons against NaN are all false and
    // would let a NaN slip through a bare clamp.
    if !cfg.min_hz.is_finite() || cfg.min_hz <= 0.0 {
        cfg.min_hz = 20.0;
    }
    if !cfg.max_hz.is_finite() || cfg.max_hz <= 0.0 {
        cfg.max_hz = nyquist;
    }
    cfg.min_hz = cfg.min_hz.clamp(1.0, nyquist * 0.5);
    cfg.max_hz = cfg.max_hz.clamp(cfg.min_hz * 2.0, nyquist);

    if !cfg.attack.is_finite() || cfg.attack < 0.0 {
        cfg.attack = 0.0;
    }
    if !cfg.release.is_finite() || cfg.release < 0.0 {
        cfg.release = 0.0;
    }
    // A non-negative floor would make the normalisation span zero or negative.
    if !cfg.noise_floor_db.is_finite() || cfg.noise_floor_db >= 0.0 {
        cfg.noise_floor_db = -70.0;
    }
    cfg
}

/// Half-open `[lo, hi)` bin ranges for each band, logarithmically spaced.
///
/// **Why logarithmic:** pitch is logarithmic — the octave 40-80 Hz occupies as much
/// musical space as 5-10 kHz, but only 1/125th as much of a linear frequency axis.
/// Split 20 Hz-16 kHz into 32 *linear* slices and the first slice swallows the bass,
/// every kick drum drives one bar, and the remaining 31 sit almost still. Geometric
/// spacing gives every octave the same number of bars, which is what makes a
/// visualiser look like it is responding to the music rather than to the maths.
///
/// Two constraints are then forced on top:
///
/// * Bands never overlap and never repeat a bin (`lo` starts where the previous
///   `hi` ended), so energy is not double-counted into adjacent bars.
/// * Every band gets at least one bin. At `fft_size = 1024` a bin is 43 Hz, so the
///   lowest dozen geometric bands all round to the same bin; without this they would
///   be empty and permanently dark. The consequence is real and worth knowing: at
///   N = 1024 the bass bands are resolution-limited to one bin each and are
///   effectively linearly spaced. Raise `fft_size` to 4096 to get true log spacing
///   down low.
fn band_edges(cfg: &SpectrumConfig, n: usize) -> Vec<(usize, usize)> {
    let max_bin = n / 2;
    let bin_of = |hz: f32| -> usize {
        let b = (hz * n as f32 / cfg.sample_rate as f32).round();
        // Bin 0 is DC. A sound card's DC offset is not music, and letting it through
        // would peg the lowest bar at a constant height forever, so bands start at 1.
        (b as usize).clamp(1, max_bin.saturating_sub(1))
    };

    let ratio = (cfg.max_hz / cfg.min_hz).powf(1.0 / cfg.bands as f32);
    let mut edges = Vec::with_capacity(cfg.bands);
    let mut prev_hi = bin_of(cfg.min_hz);
    let mut hz = cfg.min_hz;
    for _ in 0..cfg.bands {
        hz *= ratio;
        // Clamp `lo` so there is always room for at least one bin below `max_bin`,
        // even when the caller asked for more bands than the FFT has bins.
        let lo = prev_hi.min(max_bin - 1);
        let hi = bin_of(hz).max(lo + 1).min(max_bin);
        edges.push((lo, hi));
        prev_hi = hi;
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers ------------------------------------------------------------

    /// Textbook O(n²) DFT, same sign convention as [`FftPlan::run`]. Deliberately
    /// written straight from the definition so it shares no code — and therefore no
    /// bugs — with the implementation under test.
    fn naive_dft(re: &[f32], im: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let n = re.len();
        let mut or = vec![0.0f64; n];
        let mut oi = vec![0.0f64; n];
        for (k, (ork, oik)) in or.iter_mut().zip(oi.iter_mut()).enumerate() {
            for t in 0..n {
                let angle = -2.0 * std::f64::consts::PI * (k as f64) * (t as f64) / (n as f64);
                let (s, c) = angle.sin_cos();
                *ork += f64::from(re[t]) * c - f64::from(im[t]) * s;
                *oik += f64::from(re[t]) * s + f64::from(im[t]) * c;
            }
        }
        (
            or.iter().map(|v| *v as f32).collect(),
            oi.iter().map(|v| *v as f32).collect(),
        )
    }

    /// Deterministic pseudo-random noise in -1..1 (xorshift; no rand dependency).
    fn noise(n: usize, seed: u32) -> Vec<f32> {
        let mut s = seed | 1;
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s as f32 / u32::MAX as f32) * 2.0 - 1.0
            })
            .collect()
    }

    fn sine(n: usize, freq_hz: f32, sample_rate: f32, amp: f32) -> Vec<f32> {
        (0..n)
            .map(|i| amp * (TAU * freq_hz * i as f32 / sample_rate).sin())
            .collect()
    }

    fn magnitudes(re: &[f32], im: &[f32]) -> Vec<f32> {
        re.iter()
            .zip(im)
            .map(|(r, i)| (r * r + i * i).sqrt())
            .collect()
    }

    fn argmax(v: &[f32]) -> usize {
        v.iter()
            .enumerate()
            .fold((0usize, f32::NEG_INFINITY), |best, (i, &x)| {
                if x > best.1 {
                    (i, x)
                } else {
                    best
                }
            })
            .0
    }

    // -- FFT ----------------------------------------------------------------

    #[test]
    fn plan_rejects_non_power_of_two_and_tiny_sizes() {
        for bad in [0, 1, 3, 5, 6, 7, 100, 1000, 1023, 1025] {
            assert!(FftPlan::new(bad).is_none(), "n={bad} should be rejected");
        }
        assert!(FftPlan::new(MAX_FFT_SIZE * 2).is_none());
        for good in [2, 4, 8, 64, 1024, MAX_FFT_SIZE] {
            let p = FftPlan::new(good).unwrap_or_else(|| panic!("n={good} should be accepted"));
            assert_eq!(p.len(), good);
            assert_eq!(p.bins(), good / 2);
            assert!(!p.is_empty());
        }
    }

    #[test]
    fn dc_signal_puts_all_energy_in_bin_zero() {
        let n = 64;
        let mut re = vec![1.0f32; n];
        let mut im = vec![0.0f32; n];
        fft_in_place(&mut re, &mut im);
        // Sum of n ones lands entirely in bin 0.
        assert!((re[0] - n as f32).abs() < 1e-3, "bin0 re = {}", re[0]);
        assert!(im[0].abs() < 1e-3);
        for k in 1..n {
            assert!(
                re[k].abs() < 1e-3 && im[k].abs() < 1e-3,
                "bin {k} should be empty, got {} + {}i",
                re[k],
                im[k]
            );
        }
    }

    /// Headline correctness test: a 440 Hz sine sampled at 44100 Hz with N = 1024
    /// must peak in bin round(440 · 1024 / 44100) = 10.
    #[test]
    fn sine_440hz_lands_in_the_expected_bin() {
        const N: usize = 1024;
        const SR: f32 = 44_100.0;
        const F: f32 = 440.0;

        let expected = (F * N as f32 / SR).round() as usize;
        assert_eq!(expected, 10, "bin arithmetic changed");

        let mut re = sine(N, F, SR, 1.0);
        apply_window(&mut re, Window::Hann);
        let mut im = vec![0.0f32; N];
        FftPlan::new(N).unwrap().run(&mut re, &mut im);

        let mags = magnitudes(&re, &im);
        // Search the usable half only; the upper half is the conjugate mirror.
        let peak_bin = argmax(&mags[..N / 2]);
        assert_eq!(peak_bin, expected, "440 Hz should peak in bin {expected}");

        // And it should dominate: everything more than 2 bins away is ≥ 20 dB down.
        let peak_mag = mags[peak_bin];
        for (k, m) in mags[..N / 2].iter().enumerate() {
            if k.abs_diff(peak_bin) > 2 {
                assert!(
                    *m < peak_mag * 0.1,
                    "bin {k} = {m} is not 20 dB below the peak {peak_mag}"
                );
            }
        }
    }

    /// Strongest available check: the fast transform must agree with the definition.
    #[test]
    fn fft_matches_naive_dft() {
        for n in [2usize, 4, 8, 16, 32, 64, 128] {
            let re0 = noise(n, 0x1234_5678);
            let im0 = noise(n, 0x9E37_79B9);
            let (want_re, want_im) = naive_dft(&re0, &im0);

            let mut re = re0.clone();
            let mut im = im0.clone();
            FftPlan::new(n).unwrap().run(&mut re, &mut im);

            for k in 0..n {
                // Error grows like sqrt(n) with f32 accumulation; this bound is
                // ~3 orders of magnitude tighter than any real disagreement would be.
                let tol = 1e-3 * (n as f32).sqrt();
                assert!(
                    (re[k] - want_re[k]).abs() < tol && (im[k] - want_im[k]).abs() < tol,
                    "n={n} bin {k}: fft {} + {}i vs dft {} + {}i",
                    re[k],
                    im[k],
                    want_re[k],
                    want_im[k]
                );
            }
        }
    }

    /// Parseval: total energy is preserved (scaled by N for an unnormalised FFT).
    #[test]
    fn parseval_energy_is_conserved() {
        let n = 256;
        let re0 = noise(n, 0xDEAD_BEEF);
        let im0 = vec![0.0f32; n];
        let time_energy: f64 = re0.iter().map(|v| f64::from(*v) * f64::from(*v)).sum();

        let mut re = re0.clone();
        let mut im = im0.clone();
        FftPlan::new(n).unwrap().run(&mut re, &mut im);
        let freq_energy: f64 = re
            .iter()
            .zip(&im)
            .map(|(r, i)| f64::from(*r) * f64::from(*r) + f64::from(*i) * f64::from(*i))
            .sum();

        let want = time_energy * n as f64;
        assert!(
            (freq_energy - want).abs() / want < 1e-4,
            "parseval: {freq_energy} vs {want}"
        );
    }

    /// Forward → conjugate → forward → conjugate → /N reconstructs the input, which
    /// exercises the twiddles and the bit-reversal in both directions.
    #[test]
    fn conjugate_roundtrip_reconstructs_input() {
        let n = 512;
        let re0 = noise(n, 0x0BAD_F00D);
        let mut re = re0.clone();
        let mut im = vec![0.0f32; n];
        let plan = FftPlan::new(n).unwrap();

        plan.run(&mut re, &mut im);
        for v in im.iter_mut() {
            *v = -*v;
        }
        plan.run(&mut re, &mut im);
        for (r, i) in re.iter_mut().zip(im.iter_mut()) {
            *r /= n as f32;
            *i /= -(n as f32);
        }

        for k in 0..n {
            assert!(
                (re[k] - re0[k]).abs() < 1e-4,
                "sample {k}: {} vs {}",
                re[k],
                re0[k]
            );
            assert!(im[k].abs() < 1e-4, "sample {k} imaginary residue {}", im[k]);
        }
    }

    #[test]
    fn plan_and_one_shot_agree() {
        let n = 128;
        let base = noise(n, 0xC0FF_EE00);
        let (mut a_re, mut a_im) = (base.clone(), vec![0.0f32; n]);
        let (mut b_re, mut b_im) = (base.clone(), vec![0.0f32; n]);
        FftPlan::new(n).unwrap().run(&mut a_re, &mut a_im);
        fft_in_place(&mut b_re, &mut b_im);
        assert_eq!(a_re, b_re);
        assert_eq!(a_im, b_im);
    }

    #[test]
    fn run_with_wrong_length_is_a_noop_not_a_panic() {
        // Release-build behaviour; in debug the `debug_assert` would fire first, so
        // only the non-debug path is exercised here.
        if cfg!(debug_assertions) {
            return;
        }
        let plan = FftPlan::new(8).unwrap();
        let mut re = vec![1.0f32; 4];
        let mut im = vec![0.0f32; 4];
        plan.run(&mut re, &mut im);
        assert_eq!(re, vec![1.0f32; 4]);
    }

    // -- windows ------------------------------------------------------------

    #[test]
    fn window_shapes_are_correct() {
        let n = 64;
        // Hann tapers to exactly zero at both ends and peaks at 1.0 in the middle.
        assert!(Window::Hann.value(0, n).abs() < 1e-6);
        assert!(Window::Hann.value(n - 1, n).abs() < 1e-6);
        assert!((Window::Hann.value(n / 2, n) - 1.0).abs() < 1e-2);
        // Hamming sits on a 0.08 pedestal — that is the whole point of it.
        assert!((Window::Hamming.value(0, n) - 0.08).abs() < 1e-5);
        // Blackman-Harris is ~0 at the ends too.
        assert!(Window::BlackmanHarris.value(0, n).abs() < 1e-4);
        // Rect is flat.
        for i in [0, 1, n / 2, n - 1] {
            assert_eq!(Window::Rect.value(i, n), 1.0);
        }
        // Every window is symmetric.
        for w in [Window::Hann, Window::Hamming, Window::BlackmanHarris] {
            for i in 0..n {
                let d = (w.value(i, n) - w.value(n - 1 - i, n)).abs();
                assert!(d < 1e-6, "{w:?} asymmetric at {i}: {d}");
            }
        }
        assert_eq!(Window::default(), Window::Hann);
        assert_eq!(Window::Rect.coherent_gain(), 1.0);
        assert_eq!(Window::Hann.coherent_gain(), 0.5);
    }

    #[test]
    fn apply_window_survives_degenerate_buffers() {
        let mut empty: Vec<f32> = vec![];
        apply_window(&mut empty, Window::Hann);
        assert!(empty.is_empty());

        let mut one = vec![1.0f32];
        apply_window(&mut one, Window::BlackmanHarris);
        assert_eq!(one, vec![1.0f32]); // nothing to taper

        let mut rect = vec![0.5f32; 8];
        apply_window(&mut rect, Window::Rect);
        assert_eq!(rect, vec![0.5f32; 8]);
    }

    /// The reason windowing exists: a tone that is *not* on a bin centre leaks all
    /// over the spectrum with a rectangular window.
    ///
    /// Two different measures, because the windows trade against each other:
    /// *concentration* (energy within ±1 bin) rewards a narrow main lobe, and
    /// *leakage* (energy beyond ±4 bins) rewards low sidelobes. Hann wins the first
    /// against Rect; Blackman-Harris wins the second against Hann while deliberately
    /// losing the first, which is exactly the documented trade.
    #[test]
    fn windowing_reduces_spectral_leakage() {
        const N: usize = 1024;
        const SR: f32 = 44_100.0;

        // Returns (concentration within ±1 bin, leakage beyond ±4 bins).
        let spread = |freq_hz: f32, w: Window| -> (f32, f32) {
            let mut re = sine(N, freq_hz, SR, 1.0);
            apply_window(&mut re, w);
            let mut im = vec![0.0f32; N];
            FftPlan::new(N).unwrap().run(&mut re, &mut im);
            let power: Vec<f32> = re
                .iter()
                .zip(&im)
                .take(N / 2)
                .map(|(r, i)| r * r + i * i)
                .collect();
            let peak_bin = argmax(&power);
            let total: f32 = power.iter().sum();
            let mut near = 0.0;
            let mut far = 0.0;
            for (k, p) in power.iter().enumerate() {
                match k.abs_diff(peak_bin) {
                    0..=1 => near += *p,
                    2..=4 => {}
                    _ => far += *p,
                }
            }
            (near / total, far / total)
        };

        // Exactly on bin 10.5 — the worst case for a rectangular window.
        let off_centre = 10.5 * SR / N as f32;
        let (rect_near, rect_far) = spread(off_centre, Window::Rect);
        let (hann_near, hann_far) = spread(off_centre, Window::Hann);
        let (bh_near, bh_far) = spread(off_centre, Window::BlackmanHarris);

        assert!(
            hann_near > rect_near + 0.1,
            "Hann ({hann_near}) should confine much more energy than Rect ({rect_near})"
        );
        assert!(
            hann_far < rect_far * 0.2,
            "Hann far-field leakage ({hann_far}) should be far below Rect's ({rect_far})"
        );
        assert!(
            bh_far < hann_far,
            "Blackman-Harris ({bh_far}) should leak less than Hann ({hann_far})"
        );
        // ...and it pays for that with a wider main lobe.
        assert!(
            bh_near < hann_near,
            "Blackman-Harris main lobe ({bh_near}) should be wider than Hann's ({hann_near})"
        );

        // On a bin centre the rectangular window is perfect, so it must NOT lose
        // there — this guards against the test above passing for the wrong reason.
        let on_centre = 10.0 * SR / N as f32;
        assert!(spread(on_centre, Window::Rect).0 > 0.99);
    }

    // -- levels -------------------------------------------------------------

    #[test]
    fn rms_and_peak_on_known_inputs() {
        assert_eq!(rms(&[1.0, 1.0, 1.0, 1.0]), 1.0);
        assert_eq!(rms(&[-1.0, 1.0, -1.0, 1.0]), 1.0);
        assert_eq!(rms(&[0.0, 0.0]), 0.0);
        // RMS of a full-scale sine is 1/sqrt(2).
        let s = sine(4096, 100.0, 8000.0, 1.0);
        assert!((rms(&s) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);

        assert_eq!(peak(&[0.1, -0.9, 0.3]), 0.9);
        assert_eq!(peak(&[0.0; 16]), 0.0);
        assert!((peak(&s) - 1.0).abs() < 1e-2);
    }

    #[test]
    fn rms_and_peak_handle_empty_and_nan() {
        assert_eq!(rms(&[]), 0.0);
        assert_eq!(peak(&[]), 0.0);
        assert!(rms(&[]).is_finite());
        assert!(peak(&[]).is_finite());
        // A stray NaN from the capture layer must not become the answer.
        assert_eq!(peak(&[0.5, f32::NAN, 0.25]), 0.5);
    }

    // -- spectrum -----------------------------------------------------------

    fn analyzer() -> SpectrumAnalyzer {
        SpectrumAnalyzer::new(SpectrumConfig::default())
    }

    /// Feeds `frames` chunks of `fft_size` samples of a steady tone.
    fn feed_tone(a: &mut SpectrumAnalyzer, freq_hz: f32, amp: f32, frames: usize) {
        let n = a.config().fft_size;
        let sr = a.config().sample_rate as f32;
        for f in 0..frames {
            // Keep phase continuous across chunks so the tone really is steady.
            let chunk: Vec<f32> = (0..n)
                .map(|i| {
                    let t = (f * n + i) as f32;
                    amp * (TAU * freq_hz * t / sr).sin()
                })
                .collect();
            a.process(&chunk);
        }
    }

    #[test]
    fn defaults_are_the_documented_ones() {
        let c = SpectrumConfig::default();
        assert_eq!(c.sample_rate, 44_100);
        assert_eq!(c.fft_size, 1024);
        assert_eq!(c.bands, 32);
        assert_eq!(c.window, Window::Hann);
        assert_eq!(analyzer().bands().len(), 32);
    }

    #[test]
    fn config_is_sanitised() {
        let a = SpectrumAnalyzer::new(SpectrumConfig {
            fft_size: 1000,    // not a power of two
            bands: 0,          // impossible
            min_hz: -5.0,      // impossible
            max_hz: 900_000.0, // above Nyquist
            attack: -1.0,
            release: f32::NAN,
            noise_floor_db: 12.0, // must be negative
            ..Default::default()
        });
        let c = *a.config();
        assert_eq!(c.fft_size, 1024);
        assert_eq!(c.bands, 1);
        assert!(c.min_hz > 0.0 && c.min_hz < c.max_hz);
        assert!(c.max_hz <= c.sample_rate as f32 / 2.0);
        assert_eq!(c.attack, 0.0);
        assert_eq!(c.release, 0.0);
        assert!(c.noise_floor_db < 0.0);
        assert_eq!(a.bands().len(), 1);

        // Absurd band counts must still produce well-formed, in-range edges.
        let wide = SpectrumAnalyzer::new(SpectrumConfig {
            fft_size: 64,
            bands: 512,
            ..Default::default()
        });
        for b in 0..wide.config().bands {
            let (lo, hi) = wide.band_bins(b).unwrap();
            assert!(
                lo < hi && hi <= wide.config().fft_size / 2,
                "band {b}: {lo}..{hi}"
            );
        }
    }

    #[test]
    fn bands_are_log_spaced_and_widths_do_not_shrink() {
        let a = analyzer();
        let mut widths = Vec::new();
        let mut prev_hi = 0;
        for b in 0..a.config().bands {
            let (lo, hi) = a.band_bins(b).unwrap();
            assert!(lo >= 1, "band {b} must skip DC");
            assert!(lo < hi, "band {b} must own at least one bin");
            if b > 0 {
                assert_eq!(
                    lo,
                    prev_hi,
                    "band {b} must start where band {} ended",
                    b - 1
                );
            }
            prev_hi = hi;
            widths.push(hi - lo);
        }
        // Non-decreasing: log spacing means each band is at least as wide as the one
        // below it (the low bands are pinned at one bin by FFT resolution).
        for w in widths.windows(2) {
            assert!(w[1] >= w[0], "widths must not shrink: {widths:?}");
        }
        // And genuinely wider at the top — this is what "log spaced" buys.
        assert!(
            *widths.last().unwrap() > widths[0] * 8,
            "top band should be much wider than the bottom: {widths:?}"
        );

        // Frequency ranges are contiguous and ascending.
        let (lo_hz, _) = a.band_range_hz(0).unwrap();
        let (_, hi_hz) = a.band_range_hz(a.config().bands - 1).unwrap();
        assert!(lo_hz < hi_hz && hi_hz <= a.config().sample_rate as f32 / 2.0);
        assert!(a.band_range_hz(999).is_none());
        assert!(a.band_bins(999).is_none());

        // A larger FFT resolves the bass, so the bottom bands stop being pinned.
        let big = SpectrumAnalyzer::new(SpectrumConfig {
            fft_size: 8192,
            ..Default::default()
        });
        let (lo, hi) = big.band_bins(0).unwrap();
        assert!(hi - lo >= 1);
        let (tlo, thi) = big.band_bins(big.config().bands - 1).unwrap();
        assert!(thi - tlo > hi - lo);
    }

    #[test]
    fn bass_tone_lights_low_bands_only() {
        let mut a = analyzer();
        feed_tone(&mut a, 60.0, 0.9, 60);
        let bands = a.bands().to_vec();
        let hot = argmax(&bands);
        assert!(
            hot <= 2,
            "60 Hz should light a bottom band, got {hot}: {bands:?}"
        );
        assert!(bands[hot] > 0.5, "bass bar too short: {bands:?}");
        for (i, v) in bands.iter().enumerate().skip(10) {
            assert!(
                *v < 0.1,
                "band {i} should be dark for a 60 Hz tone: {bands:?}"
            );
        }
    }

    #[test]
    fn treble_tone_lights_high_bands_only() {
        let mut a = analyzer();
        feed_tone(&mut a, 8_000.0, 0.9, 60);
        let bands = a.bands().to_vec();
        let hot = argmax(&bands);
        let (lo_hz, hi_hz) = a.band_range_hz(hot).unwrap();
        assert!(
            lo_hz <= 8_000.0 && hi_hz >= 8_000.0,
            "hottest band {hot} covers {lo_hz}..{hi_hz} Hz, not 8 kHz: {bands:?}"
        );
        assert!(bands[hot] > 0.5, "treble bar too short: {bands:?}");
        for (i, v) in bands.iter().enumerate().take(20) {
            assert!(
                *v < 0.1,
                "band {i} should be dark for an 8 kHz tone: {bands:?}"
            );
        }
    }

    #[test]
    fn full_scale_tone_reaches_the_top_of_the_scale() {
        let mut a = analyzer();
        // Exactly on bin 10 so no leakage confuses the amplitude calibration.
        let f = 10.0 * 44_100.0 / 1024.0;
        feed_tone(&mut a, f, 1.0, 200);
        let v = a.bands()[argmax(a.bands())];
        assert!(v > 0.97, "0 dBFS sine should peg a bar, got {v}");
        // Halving the amplitude is -6 dB, i.e. 6/70 of the scale.
        let mut b = analyzer();
        feed_tone(&mut b, f, 0.5, 200);
        let v2 = b.bands()[argmax(b.bands())];
        let drop = (v - v2) * 70.0;
        assert!(
            (drop - 6.0).abs() < 0.7,
            "-6 dB should drop ~6/70 of scale, got {drop} dB"
        );
    }

    #[test]
    fn all_bands_stay_inside_the_unit_range() {
        let mut a = analyzer();
        // Includes samples well outside -1..1, which a badly configured capture can
        // deliver; the bars must clamp, not overshoot the widget.
        for seed in 1..12u32 {
            let chunk: Vec<f32> = noise(1024, seed.wrapping_mul(2_654_435_761))
                .iter()
                .map(|v| v * 4.0)
                .collect();
            for v in a.process(&chunk) {
                assert!((0.0..=1.0).contains(v), "band out of range: {v}");
                assert!(v.is_finite());
            }
        }
    }

    #[test]
    fn attack_is_fast_and_release_is_slow() {
        let cfg = SpectrumConfig::default();
        let n = cfg.fft_size;
        let dt = n as f32 / cfg.sample_rate as f32; // ~23.2 ms per frame
        let f = 10.0 * 44_100.0 / 1024.0;

        // Steady-state height for this tone.
        let mut steady = analyzer();
        feed_tone(&mut steady, f, 0.8, 200);
        let hot = argmax(steady.bands());
        let s = steady.bands()[hot];
        assert!(s > 0.5);

        // Attack: one frame (~1.16 time constants) covers most of the jump...
        let mut a = analyzer();
        feed_tone(&mut a, f, 0.8, 1);
        let after_1 = a.bands()[hot];
        assert!(
            after_1 > 0.5 * s && after_1 < 0.95 * s,
            "one frame should cover ~2/3 of the rise: {after_1} of {s}"
        );
        // ...and three frames are essentially there.
        feed_tone(&mut a, f, 0.8, 2);
        assert!(a.bands()[hot] > 0.9 * s, "3 frames should be ~there");

        // Release: from steady state, silence for one release time constant should
        // leave roughly 1/e of the height, and three should be near zero.
        let frames_per_tau = (cfg.release / dt).round() as usize;
        let mut r = steady.clone();
        for _ in 0..frames_per_tau {
            r.process(&vec![0.0; n]);
        }
        let after_tau = r.bands()[hot];
        assert!(
            after_tau > 0.2 * s && after_tau < 0.55 * s,
            "after one release tau expected ~1/e of {s}, got {after_tau}"
        );
        for _ in 0..frames_per_tau * 2 {
            r.process(&vec![0.0; n]);
        }
        assert!(r.bands()[hot] < 0.1 * s, "three taus should be near zero");

        // Release really is slower than attack: the same elapsed time recovers far
        // more of a rise than it sheds of a fall.
        assert!(cfg.release > cfg.attack * 5.0);
    }

    #[test]
    fn smoothing_is_frame_size_independent() {
        // The same wall-clock amount of audio must produce (nearly) the same height
        // whether it arrives as one big chunk or several small ones.
        let f = 10.0 * 44_100.0 / 1024.0;
        let sr = 44_100.0;
        let total = 4096;
        let sig = sine(total, f, sr, 0.8);

        let mut big = analyzer();
        big.process(&sig);
        let mut small = analyzer();
        for chunk in sig.chunks(256) {
            small.process(chunk);
        }
        let hot = argmax(big.bands());
        let d = (big.bands()[hot] - small.bands()[hot]).abs();
        assert!(d < 0.1, "chunking changed the height by {d}");
    }

    #[test]
    fn a_single_loud_frame_does_not_stick() {
        let mut a = analyzer();
        feed_tone(&mut a, 1_000.0, 1.0, 1);
        assert!(a.bands().iter().any(|v| *v > 0.3), "the hit should show");

        // ~23 ms of audio per frame. After 1 s (four release constants) the bars are
        // already visually gone...
        for _ in 0..43 {
            a.process(&vec![0.0; 1024]);
        }
        for (i, v) in a.bands().iter().enumerate() {
            assert!(*v < 0.02, "band {i} still visible at {v} after 1 s");
        }
        // ...and the snap-to-zero puts them at exactly zero well inside 2 s, so the
        // daemon really can stop redrawing rather than chasing a decaying tail.
        for _ in 0..43 {
            a.process(&vec![0.0; 1024]);
        }
        for (i, v) in a.bands().iter().enumerate() {
            assert_eq!(*v, 0.0, "band {i} stuck at {v}");
        }
        assert!(a.is_silent(), "should be idle again after the decay");
    }

    #[test]
    fn silence_is_detected_and_gates_redraws() {
        let mut a = analyzer();
        // Nothing seen yet ⇒ silent, so the daemon never starts a redraw loop.
        assert!(a.is_silent());
        assert!(a.level_db() < -200.0);

        // Digital silence stays silent.
        a.process(&vec![0.0; 1024]);
        assert!(a.is_silent());
        assert!(a.bands().iter().all(|v| *v == 0.0));

        // Music is not silent, and stays not-silent while the bars fall.
        feed_tone(&mut a, 440.0, 0.7, 5);
        assert!(!a.is_silent());
        a.process(&vec![0.0; 1024]);
        assert!(
            !a.is_silent(),
            "must keep redrawing while the release envelope is still visible"
        );
        assert!(a.level_db() < -200.0, "level itself is already silent");

        // Empty input changes nothing at all.
        let before = a.bands().to_vec();
        assert_eq!(a.process(&[]), before.as_slice());
    }

    #[test]
    fn quiet_signal_near_the_noise_floor_behaves_as_documented() {
        let sr = 44_100.0;
        let f = 10.0 * sr / 1024.0;

        // The floor is on RMS, so a sine needs amplitude = sqrt(2)·10^(floor/20)
        // to sit exactly on it. Comfortably under that must read as silence...
        let floor_amp = std::f32::consts::SQRT_2 * 10f32.powf(-70.0 / 20.0);
        let mut quiet = analyzer();
        feed_tone(&mut quiet, f, floor_amp * 0.5, 30);
        assert!(quiet.level_db() < -70.0);
        assert!(
            quiet.is_silent(),
            "below the floor is silence: {:?}",
            quiet.bands()
        );

        // ...and comfortably above it must not, even though the bars are tiny.
        let mut audible = analyzer();
        feed_tone(&mut audible, f, floor_amp * 8.0, 30);
        assert!(audible.level_db() > -70.0);
        assert!(!audible.is_silent());
        let v = audible.bands()[argmax(audible.bands())];
        assert!(
            v > 0.0 && v < 0.35,
            "a whisper should be a short bar, got {v}"
        );
    }

    #[test]
    fn reset_clears_history_and_smoothing() {
        let mut a = analyzer();
        feed_tone(&mut a, 440.0, 1.0, 20);
        assert!(!a.is_silent());
        a.reset();
        assert!(a.bands().iter().all(|v| *v == 0.0));
        assert!(a.is_silent());
        // And the old audio is gone from the ring, not just from the output.
        a.process(&vec![0.0; 64]);
        assert!(a.bands().iter().all(|v| *v == 0.0));
    }

    #[test]
    fn ring_buffer_wraps_correctly() {
        // Chunk sizes that do not divide the window exercise every branch of `push`.
        let mut a = SpectrumAnalyzer::new(SpectrumConfig {
            fft_size: 256,
            ..Default::default()
        });
        let f = 8.0 * 44_100.0 / 256.0;
        let sig = sine(256 * 8, f, 44_100.0, 0.9);
        for chunk in sig.chunks(100) {
            a.process(chunk);
        }
        let hot = argmax(a.bands());
        let (lo_hz, hi_hz) = a.band_range_hz(hot).unwrap();
        assert!(
            lo_hz <= f && hi_hz >= f,
            "wrapped ring lost the tone: hottest band covers {lo_hz}..{hi_hz}, want {f}"
        );

        // An over-long chunk (more than one window) must keep the newest samples.
        let mut b = SpectrumAnalyzer::new(SpectrumConfig {
            fft_size: 256,
            ..Default::default()
        });
        let mut long = vec![0.0f32; 1024];
        long.extend(sine(256, f, 44_100.0, 0.9));
        b.process(&long);
        assert!(
            !b.is_silent(),
            "tail of the chunk should be what is analysed"
        );
    }
}
