//! Sound effects: synthesised at start-up (no asset files) and played through rodio.

use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Source};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    Break,
    Place,
    Step,
    Hurt,
    Eat,
    Splash,
    /// A single hit on a block while mining.
    Hit,
}

const RATE: u32 = 44100;

struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Noise burst through a one-pole low-pass (`tone` 0..1: dull..bright) with an exponential decay.
fn burst(len_s: f32, tone: f32, decay: f32, seed: u32) -> Vec<f32> {
    let n = (len_s * RATE as f32) as usize;
    let mut rng = Noise(seed | 1);
    let mut lp = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            lp += (rng.next() - lp) * tone;
            lp * (-t * decay).exp() * (1.0 - (-(i as f32) / 40.0).exp())
        })
        .collect()
}

/// Sine sweep from `f0` to `f1` Hz with a decay.
fn sweep(len_s: f32, f0: f32, f1: f32, decay: f32) -> Vec<f32> {
    let n = (len_s * RATE as f32) as usize;
    let mut ph = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            ph += (f0 + (f1 - f0) * t) * std::f32::consts::TAU / RATE as f32;
            ph.sin() * (-t * decay).exp()
        })
        .collect()
}

fn mix(a: &[f32], b: &[f32], gb: f32) -> Vec<f32> {
    (0..a.len().max(b.len())).map(|i| a.get(i).copied().unwrap_or(0.0) + b.get(i).copied().unwrap_or(0.0) * gb).collect()
}

fn synth(s: Sfx) -> Vec<f32> {
    match s {
        Sfx::Break => mix(&burst(0.20, 0.35, 6.0, 11), &sweep(0.12, 170.0, 90.0, 5.0), 0.5),
        Sfx::Place => mix(&burst(0.09, 0.25, 7.0, 23), &sweep(0.10, 150.0, 80.0, 6.0), 0.9),
        Sfx::Step => burst(0.08, 0.18, 9.0, 37).into_iter().map(|v| v * 0.7).collect(),
        Sfx::Hurt => mix(&sweep(0.28, 260.0, 110.0, 3.5), &burst(0.2, 0.5, 6.0, 51), 0.35),
        Sfx::Eat => {
            let mut v = burst(0.05, 0.5, 8.0, 61);
            v.extend(burst(0.05, 0.45, 8.0, 67));
            v.extend(burst(0.06, 0.4, 8.0, 71));
            v
        }
        Sfx::Splash => burst(0.35, 0.3, 4.0, 83),
        Sfx::Hit => mix(&burst(0.06, 0.45, 9.0, 97), &sweep(0.05, 210.0, 140.0, 8.0), 0.4),
    }
}

pub struct Audio {
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    bufs: Vec<(Sfx, Vec<f32>)>,
    pub volume: f32,
    seed: u32,
}

impl Audio {
    pub fn new() -> Self {
        let (stream, handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(e) => {
                log::warn!("audio disabled: {e}");
                (None, None)
            }
        };
        let bufs = [Sfx::Break, Sfx::Place, Sfx::Step, Sfx::Hurt, Sfx::Eat, Sfx::Splash, Sfx::Hit].into_iter().map(|s| (s, synth(s))).collect();
        Self { _stream: stream, handle, bufs, volume: 0.6, seed: 12345 }
    }

    /// Plays an effect; `pitch` varies it a little so repeats do not sound identical.
    pub fn play(&mut self, sfx: Sfx, gain: f32) {
        let Some(h) = &self.handle else { return };
        if self.volume <= 0.001 {
            return;
        }
        let Some((_, data)) = self.bufs.iter().find(|(s, _)| *s == sfx) else { return };
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let pitch = 0.9 + (self.seed >> 16) as f32 / 65536.0 * 0.25;
        let src = SamplesBuffer::new(1, RATE, data.clone()).amplify(self.volume * gain).speed(pitch);
        let _ = h.play_raw(src);
    }
}