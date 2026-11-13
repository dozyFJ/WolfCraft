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

/// Granular texture: `count` tiny clicks of noise scattered over `len_s`, each through a
/// low-pass of `tone`, fading out with `decay` (gravel crunch, rustle, snow).
fn grains(len_s: f32, count: usize, grain_s: f32, tone: f32, decay: f32, seed: u32) -> Vec<f32> {
    let n = (len_s * RATE as f32) as usize;
    let mut out = vec![0.0f32; n];
    let mut rng = Noise(seed | 1);
    for g in 0..count {
        let start = ((rng.next() * 0.5 + 0.5) * (n as f32) * 0.85) as usize;
        let amp = 0.5 + 0.5 * (rng.next() * 0.5 + 0.5);
        let b = burst(grain_s * (0.6 + 0.8 * (rng.next() * 0.5 + 0.5)), tone, 6.0, seed.wrapping_add(g as u32 * 977));
        let fade = (-(start as f32 / n as f32) * decay).exp();
        for (i, v) in b.iter().enumerate() {
            if let Some(o) = out.get_mut(start + i) {
                *o += v * amp * fade;
            }
        }
    }
    out
}

/// Several decaying sine partials (a struck body: wood knock, metal clang, glass ping).
fn ring(len_s: f32, partials: &[(f32, f32)], decay: f32) -> Vec<f32> {
    let n = (len_s * RATE as f32) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let env = (-t / len_s * decay).exp() * (1.0 - (-(i as f32) / 30.0).exp());
            partials.iter().map(|(f, a)| (t * f * std::f32::consts::TAU).sin() * a).sum::<f32>() * env
        })
        .collect()
}

fn gain(v: Vec<f32>, g: f32) -> Vec<f32> {
    v.into_iter().map(|x| x * g).collect()
}

/// A material's sound for an action. Length and weight: break > place > dig > step.
fn material(kind: u8, m: Mat) -> Vec<f32> {
    // kind: 0 break, 1 place, 2 step, 3 dig.
    let len = [0.30, 0.16, 0.12, 0.10][kind as usize];
    let loud = [1.0, 0.85, 0.45, 0.7][kind as usize];
    let seed = 1000 + kind as u32 * 131 + m as u32 * 17;
    let v = match m {
        // Sharp stony crack: bright short noise and a dry click.
        Mat::Stone => mix(&burst(len * 0.8, 0.55, 9.0, seed), &ring(len * 0.5, &[(420.0, 0.35), (980.0, 0.15)], 10.0), 0.6),
        // Hollow wooden knock: low resonances with a soft woody noise.
        Mat::Wood => mix(&ring(len, &[(190.0, 0.6), (380.0, 0.25), (610.0, 0.12)], 7.0), &burst(len * 0.6, 0.22, 8.0, seed), 0.6),
        // Dull earthy thud with a little crumble.
        Mat::Dirt => mix(&burst(len, 0.12, 7.0, seed), &grains(len, 6, 0.012, 0.25, 4.0, seed + 1), 0.5),
        // Grassy rustle: many light, bright grains.
        Mat::Grass => mix(&grains(len * 1.2, 18, 0.010, 0.55, 3.5, seed), &burst(len * 0.7, 0.10, 8.0, seed + 2), 0.6),
        // Sand: a soft hiss.
        Mat::Sand => gain(grains(len * 1.3, 28, 0.010, 0.75, 2.5, seed), 0.6),
        // Gravel: a coarse crunch of pebbles.
        Mat::Gravel => mix(&grains(len * 1.2, 14, 0.018, 0.35, 3.0, seed), &burst(len * 0.5, 0.18, 9.0, seed + 3), 0.5),
        // Glass: a bright clink; breaking adds a shower of tinkles.
        Mat::Glass => {
            let mut v = ring(len, &[(1850.0, 0.35), (2730.0, 0.22), (3900.0, 0.12)], 9.0);
            if kind == 0 {
                for (i, f) in [2200.0f32, 3100.0, 2650.0, 3600.0, 2900.0].iter().enumerate() {
                    let off = (i as f32 * 0.035 * RATE as f32) as usize;
                    let p = ring(0.12, &[(*f, 0.25)], 10.0);
                    if v.len() < off + p.len() {
                        v.resize(off + p.len(), 0.0);
                    }
                    for (k, x) in p.iter().enumerate() {
                        v[off + k] += x;
                    }
                }
                v = mix(&v, &burst(0.18, 0.8, 8.0, seed), 0.35);
            }
            v
        }
        // Wool: a soft, muffled pat.
        Mat::Wool => gain(burst(len * 0.8, 0.06, 9.0, seed), 0.9),
        // Leaves: a crisp rustle.
        Mat::Leaves => gain(grains(len * 1.2, 22, 0.008, 0.65, 3.0, seed), 0.8),
        // Snow: a cold, squeaky crunch.
        Mat::Snow => mix(&grains(len * 1.2, 12, 0.014, 0.30, 3.0, seed), &ring(len * 0.6, &[(760.0, 0.06)], 8.0), 1.0),
        // Metal: a ringing clang.
        Mat::Metal => mix(&ring(len * 1.5, &[(520.0, 0.35), (1310.0, 0.22), (2120.0, 0.14)], 5.0), &burst(len * 0.3, 0.6, 10.0, seed), 0.4),
    };
    gain(v, loud)
}

fn synth(s: Sfx) -> Vec<f32> {
    match s {
        Sfx::Break(m) => material(0, m),
        Sfx::Place(m) => material(1, m),
        Sfx::Step(m) => material(2, m),
        Sfx::Dig(m) => material(3, m),
        // "Oof": a short voiced drop with a breath of noise.
        Sfx::Hurt => mix(&ring(0.22, &[(240.0, 0.5), (480.0, 0.2), (720.0, 0.1)], 4.0).iter().enumerate().map(|(i, v)| v * (1.0 - i as f32 / (0.22 * RATE as f32) * 0.4)).collect::<Vec<_>>(), &burst(0.15, 0.4, 6.0, 51), 0.25),
        Sfx::Eat => {
            let mut v = grains(0.07, 6, 0.010, 0.5, 2.0, 61);
            v.extend(grains(0.07, 6, 0.010, 0.45, 2.0, 67));
            v.extend(grains(0.08, 7, 0.010, 0.4, 2.0, 71));
            v
        }
        Sfx::Splash => mix(&burst(0.40, 0.35, 4.0, 83), &grains(0.35, 20, 0.010, 0.7, 3.0, 89), 0.5),
        // A blow on a creature: a punchy thump.
        Sfx::Hit => mix(&ring(0.10, &[(140.0, 0.6), (95.0, 0.3)], 6.0), &burst(0.07, 0.4, 9.0, 97), 0.5),
        Sfx::Pop => sweep(0.07, 620.0, 1100.0, 6.0).into_iter().map(|v| v * 0.5).collect(),
        Sfx::Fuse => burst(0.7, 0.85, 1.5, 107).into_iter().map(|v| v * 0.45).collect(),
        Sfx::Launch => mix(&burst(0.30, 0.55, 5.0, 113), &sweep(0.25, 320.0, 110.0, 4.0), 0.6),
        // A blast: a sharp crack, a deep falling boom, a long rumble and falling debris.
        Sfx::Explosion => {
            let crack = burst(0.12, 0.85, 14.0, 127);
            let boom = sweep(1.1, 90.0, 26.0, 2.2);
            let rumble = burst(1.8, 0.05, 2.0, 131);
            let debris = grains(1.6, 40, 0.016, 0.45, 2.2, 137);
            let v = mix(&mix(&crack, &boom, 1.3), &rumble, 1.1);
            mix(&v, &debris, 0.35)
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
        _ => 0.45,
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
        let mut all = vec![Sfx::Hurt, Sfx::Eat, Sfx::Splash, Sfx::Hit, Sfx::Pop, Sfx::Fuse, Sfx::Launch, Sfx::Explosion];
        for m in Mat::ALL {
            all.extend([Sfx::Break(m), Sfx::Place(m), Sfx::Step(m), Sfx::Dig(m)]);
        }
        let bufs = all.into_iter().map(|s| (s, master(synth(s), room_for(s)))).collect();
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_material_sound_is_audible_and_does_not_clip() {
        for m in Mat::ALL {
            for s in [Sfx::Break(m), Sfx::Place(m), Sfx::Step(m), Sfx::Dig(m)] {
                let v = synth(s);
                let peak = v.iter().fold(0.0f32, |a, x| a.max(x.abs()));
                assert!(v.iter().all(|x| x.is_finite()), "{s:?}");
                assert!(peak > 0.02 && peak < 1.6, "{s:?} peak {peak}");
                assert!(v.len() < RATE as usize, "{s:?} short");
            }
        }
        assert_eq!(Mat::of(wolfcraft_engine::block::Block::Stone), Mat::Stone);
        assert_eq!(Mat::of(wolfcraft_engine::block::Block::OakPlanks), Mat::Wood);
        assert_eq!(Mat::of(wolfcraft_engine::block::Block::Sand), Mat::Sand);
        assert_eq!(Mat::of(wolfcraft_engine::block::Block::Grass), Mat::Grass);
        assert_eq!(Mat::of(wolfcraft_engine::block::Block::OakLeaves), Mat::Leaves);
    }
}
