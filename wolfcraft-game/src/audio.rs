//! Sound effects: synthesised at start-up (no asset files) and played through rodio.

use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Source};

/// What a block sounds like (dig, step, place), like the material sets of the classic game.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mat {
    Stone,
    Wood,
    Dirt,
    Grass,
    Sand,
    Gravel,
    Glass,
    Wool,
    Leaves,
    Snow,
    Metal,
}

impl Mat {
    pub const ALL: [Mat; 11] = [Mat::Stone, Mat::Wood, Mat::Dirt, Mat::Grass, Mat::Sand, Mat::Gravel, Mat::Glass, Mat::Wool, Mat::Leaves, Mat::Snow, Mat::Metal];

    /// Sound material of a block.
    pub fn of(b: wolfcraft_engine::block::Block) -> Mat {
        use wolfcraft_engine::block::{Block as B, RenderKind, ToolKind};
        let name = b.info().name.to_lowercase();
        match b {
            B::Sand | B::RedSand => Mat::Sand,
            B::Gravel | B::Farmland => Mat::Gravel,
            B::Grass | B::DryGrass | B::SwampGrass | B::SnowyGrass | B::Podzol | B::MossBlock | B::TallGrass | B::Fern => Mat::Grass,
            B::Snow => Mat::Snow,
            B::Glass | B::Ice | B::PackedIce | B::Glowstone => Mat::Glass,
            _ if name.contains("листв") => Mat::Leaves,
            _ if name.contains("шерст") => Mat::Wool,
            _ if name.contains("стекл") || name.contains("панел") => Mat::Glass,
            _ if name.contains("железн") || name.contains("золот") || name.contains("медн") || name.contains("бронз") || name.contains("алмазн") || name.contains("урановый блок") => Mat::Metal,
            _ if b.info().render == RenderKind::Cross => Mat::Grass,
            _ => match b.info().tool {
                ToolKind::Axe => Mat::Wood,
                ToolKind::Shovel => Mat::Dirt,
                ToolKind::Pickaxe => Mat::Stone,
                _ => Mat::Dirt,
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    /// A block breaks.
    Break(Mat),
    /// A block is put down.
    Place(Mat),
    /// A footstep on this material.
    Step(Mat),
    /// One blow on a block while mining.
    Dig(Mat),
    Hurt,
    Eat,
    Splash,
    /// A blow that lands on a creature.
    Hit,
    /// An item picked up from the ground: a short bright pop.
    Pop,
    /// Dynamite fuse catching fire.
    Fuse,
    /// Grenade launcher / bazooka shot.
    Launch,
    Explosion,
    /// Gunshots: 0 pistol, 1 rifle, 2 shotgun, 3 sniper rifle.
    Shot(u8),
    /// A bullet hitting a creature: a short thwack.
    BulletHit,
    /// Armour put on: a leather rustle and a metal clink.
    Equip,
    /// An achievement: a bright rising chime.
    Achievement,
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
    fn unit(&mut self) -> f32 {
        self.next() * 0.5 + 0.5
    }
}

/// A biquad filter (RBJ cookbook): low-pass, high-pass or band-pass at `f` Hz.
#[derive(Clone, Copy)]
enum Filt {
    Low,
    High,
    Band,
}

fn filter(x: &[f32], kind: Filt, f: f32, q: f32) -> Vec<f32> {
    let w = std::f32::consts::TAU * (f / RATE as f32).clamp(0.0005, 0.49);
    let (sn, cs) = w.sin_cos();
    let alpha = sn / (2.0 * q);
    let (b0, b1, b2) = match kind {
        Filt::Low => ((1.0 - cs) / 2.0, 1.0 - cs, (1.0 - cs) / 2.0),
        Filt::High => ((1.0 + cs) / 2.0, -(1.0 + cs), (1.0 + cs) / 2.0),
        Filt::Band => (alpha, 0.0, -alpha),
    };
    let (a0, a1, a2) = (1.0 + alpha, -2.0 * cs, 1.0 - alpha);
    let (b0, b1, b2, a1, a2) = (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    x.iter()
        .map(|&v| {
            let y = b0 * v + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            x2 = x1;
            x1 = v;
            y2 = y1;
            y1 = y;
            y
        })
        .collect()
}

fn len_of(s: f32) -> usize {
    (s * RATE as f32).max(1.0) as usize
}

/// White noise with a fast attack and an exponential decay over `len_s`.
fn noise(len_s: f32, decay: f32, seed: u32) -> Vec<f32> {
    let n = len_of(len_s);
    let mut rng = Noise(seed | 1);
    (0..n).map(|i| rng.next() * env(i, n, decay)).collect()
}

fn env(i: usize, n: usize, decay: f32) -> f32 {
    let t = i as f32 / n as f32;
    (-t * decay).exp() * (1.0 - (-(i as f32) / 22.0).exp())
}

/// A body with a falling pitch: the "thump" under every impact.
fn thump(len_s: f32, f0: f32, f1: f32, decay: f32) -> Vec<f32> {
    let n = len_of(len_s);
    let mut ph = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            ph += (f1 + (f0 - f1) * (-t * 6.0).exp()) * std::f32::consts::TAU / RATE as f32;
            ph.sin() * env(i, n, decay)
        })
        .collect()
}

/// Struck object: inharmonic decaying partials (wood, metal, glass).
fn modal(len_s: f32, partials: &[(f32, f32, f32)]) -> Vec<f32> {
    let n = len_of(len_s);
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let atk = 1.0 - (-(i as f32) / 15.0).exp();
            partials.iter().map(|(f, a, d)| (t * f * std::f32::consts::TAU).sin() * a * (-t * d).exp()).sum::<f32>() * atk
        })
        .collect()
}

/// Little crunches scattered over `len_s` (gravel, snow, grass, chewing).
fn crunch(len_s: f32, count: usize, grain_s: f32, band: f32, q: f32, seed: u32) -> Vec<f32> {
    let n = len_of(len_s);
    let mut out = vec![0.0f32; n];
    let mut rng = Noise(seed | 1);
    for g in 0..count {
        let start = (rng.unit().powf(1.4) * n as f32 * 0.8) as usize;
        let amp = 0.4 + 0.6 * rng.unit();
        let f = band * (0.7 + 0.6 * rng.unit());
        let b = filter(&noise(grain_s * (0.5 + rng.unit()), 7.0, seed.wrapping_add(g as u32 * 7919)), Filt::Band, f, q);
        for (i, v) in b.iter().enumerate() {
            if let Some(o) = out.get_mut(start + i) {
                *o += v * amp * 2.5;
            }
        }
    }
    out
}

fn add(a: &mut Vec<f32>, b: &[f32], g: f32, at_s: f32) {
    let off = len_of(at_s).saturating_sub(1);
    if a.len() < off + b.len() {
        a.resize(off + b.len(), 0.0);
    }
    for (i, v) in b.iter().enumerate() {
        a[off + i] += v * g;
    }
}

fn gain(v: Vec<f32>, g: f32) -> Vec<f32> {
    v.into_iter().map(|x| x * g).collect()
}

/// A material's sound for an action (`kind` 0 break, 1 place, 2 step, 3 dig), variant `var`.
/// Every one: a low impact body plus the material's own texture, so blocks feel solid.
fn material(kind: u8, m: Mat, var: u32) -> Vec<f32> {
    let len = [0.34, 0.18, 0.13, 0.11][kind as usize];
    let loud = [1.0, 0.85, 0.42, 0.65][kind as usize];
    let seed = 1000 + kind as u32 * 131 + m as u32 * 17 + var * 4099;
    let mut v = Vec::new();
    let heavy = if kind == 2 { 0.7 } else { 1.0 };
    match m {
        Mat::Stone => {
            add(&mut v, &filter(&noise(0.03, 12.0, seed), Filt::Band, 2400.0, 1.2), 0.9, 0.0);
            add(&mut v, &filter(&noise(len, 9.0, seed + 1), Filt::Band, 900.0, 0.9), 0.6, 0.0);
            add(&mut v, &thump(len * 0.6, 160.0, 90.0, 9.0), 0.5 * heavy, 0.0);
            if kind == 0 {
                add(&mut v, &crunch(len, 10, 0.02, 1500.0, 1.5, seed + 2), 0.5, 0.03);
            }
        }
        Mat::Wood => {
            add(&mut v, &modal(len, &[(180.0, 0.55, 24.0), (447.0, 0.25, 30.0), (742.0, 0.12, 40.0)]), 1.0, 0.0);
            add(&mut v, &filter(&noise(0.04, 10.0, seed), Filt::Band, 1600.0, 1.0), 0.5, 0.0);
            if kind == 0 {
                add(&mut v, &crunch(len, 6, 0.03, 700.0, 1.2, seed + 3), 0.6, 0.04);
            }
        }
        Mat::Dirt => {
            add(&mut v, &thump(len * 0.7, 120.0, 70.0, 8.0), 0.8 * heavy, 0.0);
            add(&mut v, &filter(&noise(len, 7.0, seed), Filt::Low, 700.0, 0.7), 0.9, 0.0);
            add(&mut v, &crunch(len, 5, 0.015, 900.0, 1.0, seed + 1), 0.3, 0.02);
        }
        Mat::Grass => {
            add(&mut v, &thump(len * 0.5, 110.0, 70.0, 9.0), 0.5 * heavy, 0.0);
            add(&mut v, &crunch(len * 1.2, 16, 0.012, 3200.0, 1.2, seed), 0.55, 0.0);
            add(&mut v, &filter(&noise(len, 6.0, seed + 5), Filt::High, 2500.0, 0.7), 0.25, 0.0);
        }
        Mat::Sand => {
            add(&mut v, &filter(&noise(len * 1.3, 4.0, seed), Filt::Band, 1800.0, 0.6), 0.8, 0.0);
            add(&mut v, &thump(len * 0.4, 100.0, 60.0, 9.0), 0.35 * heavy, 0.0);
        }
        Mat::Gravel => {
            add(&mut v, &crunch(len * 1.3, 14, 0.02, 1300.0, 1.1, seed), 0.8, 0.0);
            add(&mut v, &thump(len * 0.5, 130.0, 80.0, 9.0), 0.5 * heavy, 0.0);
        }
        Mat::Glass => {
            add(&mut v, &modal(len, &[(1850.0, 0.35, 30.0), (2730.0, 0.22, 36.0), (3900.0, 0.12, 44.0)]), 1.0, 0.0);
            if kind == 0 {
                for i in 0..6 {
                    let f = 2100.0 + ((seed + i * 37) % 9) as f32 * 220.0;
                    add(&mut v, &modal(0.15, &[(f, 0.25, 40.0)]), 1.0, 0.02 + i as f32 * 0.03);
                }
                add(&mut v, &filter(&noise(0.2, 8.0, seed), Filt::High, 3000.0, 0.7), 0.4, 0.0);
            }
        }
        Mat::Wool => {
            add(&mut v, &filter(&noise(len * 0.8, 9.0, seed), Filt::Low, 400.0, 0.7), 1.2, 0.0);
        }
        Mat::Leaves => {
            add(&mut v, &crunch(len * 1.3, 22, 0.01, 4200.0, 1.0, seed), 0.6, 0.0);
            add(&mut v, &filter(&noise(len, 5.0, seed + 9), Filt::High, 3500.0, 0.7), 0.3, 0.0);
        }
        Mat::Snow => {
            add(&mut v, &crunch(len * 1.2, 12, 0.02, 1100.0, 3.0, seed), 0.7, 0.0);
            add(&mut v, &thump(len * 0.5, 100.0, 60.0, 9.0), 0.4 * heavy, 0.0);
        }
        Mat::Metal => {
            add(&mut v, &modal(len * 1.6, &[(510.0, 0.35, 9.0), (1340.0, 0.22, 12.0), (2190.0, 0.14, 16.0), (3020.0, 0.08, 20.0)]), 1.0, 0.0);
            add(&mut v, &filter(&noise(0.03, 12.0, seed), Filt::High, 2000.0, 0.8), 0.5, 0.0);
        }
    }
    // Same loudness for every material (steps quiet, breaks loud), never clipping.
    let peak = v.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1e-4);
    gain(v, loud * 0.9 / peak)
}

fn synth(s: Sfx, var: u32) -> Vec<f32> {
    let sd = var * 7717;
    match s {
        Sfx::Break(m) => material(0, m, var),
        Sfx::Place(m) => material(1, m, var),
        Sfx::Step(m) => material(2, m, var),
        Sfx::Dig(m) => material(3, m, var),
        // Being hurt: a punchy body blow and a short breathy grunt.
        Sfx::Hurt => {
            let mut v = thump(0.18, 150.0, 70.0, 7.0);
            add(&mut v, &filter(&noise(0.2, 6.0, 51 + sd), Filt::Band, 650.0, 2.0), 0.9, 0.01);
            add(&mut v, &filter(&noise(0.16, 6.0, 53 + sd), Filt::Band, 1200.0, 3.0), 0.5, 0.01);
            v
        }
        // Eating: three crunchy bites and a little swallow.
        Sfx::Eat => {
            let mut v = Vec::new();
            for i in 0..3u32 {
                add(&mut v, &crunch(0.09, 7, 0.012, 1600.0 + i as f32 * 200.0, 1.3, 61 + i * 11 + sd), 0.9, i as f32 * 0.13);
                add(&mut v, &filter(&noise(0.06, 9.0, 71 + i + sd), Filt::Low, 500.0, 1.0), 0.6, i as f32 * 0.13);
            }
            add(&mut v, &thump(0.08, 260.0, 180.0, 8.0), 0.25, 0.42);
            v
        }
        // Water: a splash of bright noise and a few rising bubbles.
        Sfx::Splash => {
            let mut v = filter(&noise(0.45, 4.0, 83 + sd), Filt::Band, 1400.0, 0.6);
            for i in 0..5u32 {
                let f = 500.0 + (i * 173 % 7) as f32 * 120.0;
                add(&mut v, &thump(0.06, f, f * 1.8, 6.0), 0.25, 0.05 + i as f32 * 0.05);
            }
            v
        }
        // A blow on a creature: a punchy low thump and a slap.
        Sfx::Hit => {
            let mut v = thump(0.14, 130.0, 60.0, 7.0);
            add(&mut v, &filter(&noise(0.05, 12.0, 97 + sd), Filt::Band, 1800.0, 0.9), 0.7, 0.0);
            v
        }
        // Pickup: a soft bubbly pop going up.
        Sfx::Pop => {
            let mut v = thump(0.08, 500.0, 1100.0, 6.0);
            add(&mut v, &filter(&noise(0.015, 10.0, 101), Filt::High, 3000.0, 0.7), 0.3, 0.0);
            gain(v, 0.5)
        }
        Sfx::Fuse => gain(filter(&noise(0.8, 1.2, 107), Filt::High, 3500.0, 0.6), 0.6),
        Sfx::Launch => {
            let mut v = filter(&noise(0.35, 5.0, 113 + sd), Filt::Low, 1500.0, 0.7);
            add(&mut v, &thump(0.3, 220.0, 70.0, 5.0), 0.8, 0.0);
            v
        }
        // Gunshot: a sharp crack, a body, a dry tail; heavier guns lower and longer.
        Sfx::Shot(k) => {
            let (len, body, tail) = match k {
                0 => (0.16, (520.0, 140.0), 0.10),
                1 => (0.12, (600.0, 180.0), 0.08),
                2 => (0.32, (260.0, 60.0), 0.30),
                _ => (0.45, (320.0, 50.0), 0.45),
            };
            let mut v = filter(&noise(0.03, 20.0, 201 + k as u32 + sd), Filt::High, 1800.0, 0.7);
            add(&mut v, &thump(len, body.0, body.1, 7.0), 0.9, 0.0);
            add(&mut v, &filter(&noise(len + tail, 3.0, 211 + k as u32), Filt::Low, 900.0, 0.7), 0.6, 0.01);
            v
        }
        Sfx::BulletHit => {
            let mut v = thump(0.07, 180.0, 90.0, 9.0);
            add(&mut v, &filter(&noise(0.05, 12.0, 223 + sd), Filt::Band, 2200.0, 1.0), 0.6, 0.0);
            v
        }
        // A blast: crack, deep falling boom, long rumble, falling debris.
        Sfx::Explosion => {
            let mut v = filter(&noise(0.12, 14.0, 127 + sd), Filt::High, 1200.0, 0.7);
            add(&mut v, &thump(1.2, 90.0, 24.0, 2.2), 1.4, 0.0);
            add(&mut v, &filter(&noise(1.9, 2.0, 131 + sd), Filt::Low, 220.0, 0.7), 2.0, 0.0);
            add(&mut v, &crunch(1.6, 40, 0.02, 1300.0, 1.0, 137 + sd), 0.35, 0.2);
            v
        }
        // Armour: leather rustle and a metal clink.
        Sfx::Equip => {
            let mut v = filter(&noise(0.22, 5.0, 301 + sd), Filt::Band, 1100.0, 0.8);
            add(&mut v, &modal(0.3, &[(1320.0, 0.25, 14.0), (2210.0, 0.15, 18.0)]), 0.8, 0.08);
            add(&mut v, &modal(0.3, &[(1180.0, 0.2, 14.0), (1990.0, 0.12, 18.0)]), 0.7, 0.16);
            v
        }
        // Achievement: three rising bell notes and a sparkle.
        Sfx::Achievement => {
            let mut v = Vec::new();
            for (i, f) in [784.0f32, 988.0, 1318.0].iter().enumerate() {
                add(&mut v, &modal(0.7, &[(*f, 0.4, 5.0), (f * 2.0, 0.12, 8.0), (f * 3.01, 0.06, 12.0)]), 1.0, i as f32 * 0.11);
            }
            add(&mut v, &filter(&noise(0.4, 6.0, 401), Filt::High, 6000.0, 0.7), 0.15, 0.3);
            gain(v, 0.6)
        }
    }
}

/// Master treatment for every effect: a short room reverb (feedback combs with a dark tail),
/// gentle tape-like saturation, peak normalisation and click-free fades. `room` 0..1.
fn master(dry: Vec<f32>, room: f32) -> Vec<f32> {
    let tail = (0.12 + 0.35 * room) * RATE as f32;
    let n = dry.len() + tail as usize;
    let mut wet = vec![0.0f32; n];
    for (delay, fb) in [(1116usize, 0.62f32), (1277, 0.58), (1491, 0.55), (1617, 0.52)] {
        let mut buf = vec![0.0f32; n];
        let mut lp = 0.0f32;
        for i in 0..n {
            let x = dry.get(i).copied().unwrap_or(0.0);
            let back = if i >= delay { buf[i - delay] } else { 0.0 };
            lp += (back - lp) * 0.45;
            buf[i] = x + lp * fb * room.max(0.2);
            wet[i] += buf[i] * 0.25;
        }
    }
    let mix_wet = 0.10 + 0.22 * room;
    let mut out: Vec<f32> = (0..n)
        .map(|i| {
            let d = dry.get(i).copied().unwrap_or(0.0);
            // `wet` holds the dry signal plus the echoes; keep only a share of the echoes.
            d + (wet[i] - d) * mix_wet
        })
        .collect();
    // Remove DC, saturate softly, normalise.
    let mean = out.iter().sum::<f32>() / n.max(1) as f32;
    for v in out.iter_mut() {
        *v = ((*v - mean) * 1.4).tanh();
    }
    // Keep each effect as loud as it was designed (steps quiet, blasts loud), never clipping.
    let dry_peak = dry.iter().fold(0.0f32, |m, v| m.max(v.abs())).clamp(1e-4, 0.95);
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-4);
    let k = dry_peak / peak;
    let fade_in = (0.002 * RATE as f32) as usize;
    let fade_out = (0.012 * RATE as f32) as usize;
    for (i, v) in out.iter_mut().enumerate() {
        let a = (i as f32 / fade_in as f32).min(1.0);
        let b = ((n - i) as f32 / fade_out as f32).min(1.0);
        *v *= k * a * b;
    }
    out
}

/// How much room sound an effect gets.
fn room_for(s: Sfx) -> f32 {
    match s {
        Sfx::Step(_) => 0.15,
        Sfx::Dig(_) | Sfx::Hit | Sfx::Pop | Sfx::Eat => 0.3,
        Sfx::Explosion => 1.0,
        Sfx::Launch | Sfx::Break(_) => 0.6,
        Sfx::Shot(_) => 0.75,
        Sfx::BulletHit => 0.2,
        Sfx::Equip => 0.25,
        Sfx::Achievement => 0.7,
        _ => 0.45,
    }
}

pub struct Audio {
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    /// Every effect in three variants (picked at random, so repeats differ like in MC).
    bufs: Vec<(Sfx, Vec<Vec<f32>>)>,
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
        let mut all = vec![Sfx::Hurt, Sfx::Eat, Sfx::Splash, Sfx::Hit, Sfx::Pop, Sfx::Fuse, Sfx::Launch, Sfx::Explosion, Sfx::Shot(0), Sfx::Shot(1), Sfx::Shot(2), Sfx::Shot(3), Sfx::BulletHit, Sfx::Equip, Sfx::Achievement];
        for m in Mat::ALL {
            all.extend([Sfx::Break(m), Sfx::Place(m), Sfx::Step(m), Sfx::Dig(m)]);
        }
        let bufs = all.into_iter().map(|s| (s, (0..3).map(|v| master(synth(s, v), room_for(s))).collect())).collect();
        Self { _stream: stream, handle, bufs, volume: 0.6, seed: 12345 }
    }

    /// Plays an effect; `pitch` varies it a little so repeats do not sound identical.
    /// The output, for other players' voices.
    pub fn handle(&self) -> Option<&OutputStreamHandle> {
        self.handle.as_ref()
    }

    pub fn play(&mut self, sfx: Sfx, gain: f32) {
        let Some(h) = &self.handle else { return };
        if self.volume <= 0.001 {
            return;
        }
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let Some((_, vars)) = self.bufs.iter().find(|(s, _)| *s == sfx) else { return };
        let data = &vars[(self.seed >> 8) as usize % vars.len()];
        let pitch = 0.92 + (self.seed >> 16) as f32 / 65536.0 * 0.16;
        let src = SamplesBuffer::new(1, RATE, data.clone()).amplify(self.volume * gain).speed(pitch);
        let _ = h.play_raw(src);
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_material_sound_is_audible_and_does_not_clip() {
        for m in Mat::ALL {
            for s in [Sfx::Break(m), Sfx::Place(m), Sfx::Step(m), Sfx::Dig(m)] {
                let v = synth(s, 0);
                let peak = v.iter().fold(0.0f32, |a, x| a.max(x.abs()));
                assert!(v.iter().all(|x| x.is_finite()), "{s:?}");
                assert!(peak > 0.02 && peak < 1.6, "{s:?} peak {peak}");
                assert!(v.len() < RATE as usize, "{s:?} short");
            }
        }
        for s in [Sfx::Hurt, Sfx::Eat, Sfx::Splash, Sfx::Hit, Sfx::Pop, Sfx::Explosion, Sfx::Shot(3), Sfx::Equip, Sfx::Achievement] {
            for var in 0..3 {
                let v = master(synth(s, var), room_for(s));
                assert!(!v.is_empty() && v.iter().all(|x| x.is_finite() && x.abs() <= 1.0), "{s:?}");
            }
        }
        assert_eq!(Mat::of(wolfcraft_engine::block::Block::Stone), Mat::Stone);
        assert_eq!(Mat::of(wolfcraft_engine::block::Block::OakPlanks), Mat::Wood);
        assert_eq!(Mat::of(wolfcraft_engine::block::Block::Sand), Mat::Sand);
        assert_eq!(Mat::of(wolfcraft_engine::block::Block::Grass), Mat::Grass);
        assert_eq!(Mat::of(wolfcraft_engine::block::Block::OakLeaves), Mat::Leaves);
    }
}
