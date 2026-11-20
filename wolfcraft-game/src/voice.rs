//! Voice chat on servers,.
//!
//! The microphone is read through cpal (the library rodio plays sound with), mixed down to
//! mono 16 kHz and cut into 40 ms frames. A frame is sent while the player talks: in
//! «Активация голосом» when it is louder than the threshold (with a short hang so words are
//! not chopped), in «Нажми и говори» while the microphone button (or V) is held. Frames are
//! packed with IMA ADPCM (4 bits a sample, 8 KB/s) and base64, and the server relays them to
//! the other players. Each speaker plays through its own queue; by default voices fade with
//! distance (48 blocks), like talking in the world.

use glam::DVec3;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub const RATE: u32 = 16_000;
/// Samples in one frame (40 ms).
pub const FRAME: usize = 640;
/// Voices are heard up to this far (blocks) when «Только рядом» is on.
pub const RANGE: f64 = 48.0;
/// Keep sending this long after the voice drops under the threshold (seconds).
const HANG: f32 = 0.4;

// ---------------------------------------------------------------------------------------------
// IMA ADPCM

const STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408,
    449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];
const INDEX: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

fn adpcm_step(code: u8, pred: &mut i32, index: &mut i32) -> i16 {
    let step = STEPS[*index as usize];
    let mut diff = step >> 3;
    if code & 4 != 0 {
        diff += step;
    }
    if code & 2 != 0 {
        diff += step >> 1;
    }
    if code & 1 != 0 {
        diff += step >> 2;
    }
    *pred = if code & 8 != 0 { *pred - diff } else { *pred + diff }.clamp(-32768, 32767);
    *index = (*index + INDEX[code as usize]).clamp(0, 88);
    *pred as i16
}

/// Packs samples (-1..1): 3 header bytes (start value, step index), then 4 bits a sample.
pub fn encode(samples: &[f32]) -> Vec<u8> {
    let first = (samples.first().copied().unwrap_or(0.0).clamp(-1.0, 1.0) * 32767.0) as i32;
    let (mut pred, mut index) = (first, 20i32);
    let mut out = Vec::with_capacity(3 + samples.len() / 2 + 1);
    out.extend_from_slice(&(pred as i16).to_le_bytes());
    out.push(index as u8);
    let mut half: Option<u8> = None;
    for s in samples {
        let target = (s.clamp(-1.0, 1.0) * 32767.0) as i32;
        let step = STEPS[index as usize];
        let mut diff = target - pred;
        let mut code = 0u8;
        if diff < 0 {
            code = 8;
            diff = -diff;
        }
        let mut s2 = step;
        for bit in [4u8, 2, 1] {
            if diff >= s2 {
                code |= bit;
                diff -= s2;
            }
            s2 >>= 1;
        }
        adpcm_step(code, &mut pred, &mut index);
        match half.take() {
            None => half = Some(code),
            Some(lo) => out.push(lo | (code << 4)),
        }
    }
    if let Some(lo) = half {
        out.push(lo);
    }
    out
}

pub fn decode(data: &[u8], samples: usize) -> Vec<f32> {
    if data.len() < 3 {
        return Vec::new();
    }
    let mut pred = i16::from_le_bytes([data[0], data[1]]) as i32;
    let mut index = (data[2] as i32).clamp(0, 88);
    let mut out = Vec::with_capacity(samples);
    for b in &data[3..] {
        for code in [b & 15, b >> 4] {
            if out.len() >= samples {
                break;
            }
            out.push(adpcm_step(code, &mut pred, &mut index) as f32 / 32768.0);
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// base64 (the protocol is JSON lines)

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for b in s.bytes() {
        if b == b'=' {
            break;
        }
        let v = B64.iter().position(|x| *x == b)? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------------------------
// Microphone

/// Mono 16 kHz samples from the microphone, filled by cpal's thread.
struct Capture {
    _stream: rodio::cpal::Stream,
    buf: Arc<Mutex<Vec<f32>>>,
}

fn open_mic() -> Result<Capture, String> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use rodio::cpal::{SampleFormat, StreamConfig};
    let host = rodio::cpal::default_host();
    let dev = host.default_input_device().ok_or("нет микрофона")?;
    let cfg = dev.default_input_config().map_err(|e| e.to_string())?;
    let channels = cfg.channels().max(1) as usize;
    let rate = cfg.sample_rate().0.max(1) as f64;
    let format = cfg.sample_format();
    let config: StreamConfig = cfg.into();
    let buf = Arc::new(Mutex::new(Vec::<f32>::new()));
    // Resampler state: position between input samples.
    let state = Arc::new(Mutex::new((0.0f64, 0.0f32)));
    let step = rate / RATE as f64;
    let push = {
        let buf = buf.clone();
        move |mono: &mut dyn Iterator<Item = f32>| {
            let Ok(mut st) = state.lock() else { return };
            let Ok(mut b) = buf.lock() else { return };
            for x in mono {
                // Linear resampling: emit output samples that fall between prev and x.
                while st.0 < 1.0 {
                    let t = st.0 as f32;
                    b.push(st.1 + (x - st.1) * t);
                    st.0 += step;
                }
                st.0 -= 1.0;
                st.1 = x;
            }
            // Never let it grow without bound (the game paused, say).
            let len = b.len();
            if len > RATE as usize * 2 {
                b.drain(..len - RATE as usize / 2);
            }
        }
    };
    let err = |e| log::warn!("микрофон: {e}");
    let stream = match format {
        SampleFormat::F32 => {
            let mut push = push;
            dev.build_input_stream(&config, move |d: &[f32], _| push(&mut d.chunks(channels).map(|c| c.iter().sum::<f32>() / c.len() as f32)), err, None)
        }
        SampleFormat::I16 => {
            let mut push = push;
            dev.build_input_stream(&config, move |d: &[i16], _| push(&mut d.chunks(channels).map(|c| c.iter().map(|v| *v as f32 / 32768.0).sum::<f32>() / c.len() as f32)), err, None)
        }
        SampleFormat::U16 => {
            let mut push = push;
            dev.build_input_stream(&config, move |d: &[u16], _| push(&mut d.chunks(channels).map(|c| c.iter().map(|v| (*v as f32 - 32768.0) / 32768.0).sum::<f32>() / c.len() as f32)), err, None)
        }
        other => return Err(format!("формат микрофона {other:?} не поддерживается")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Capture { _stream: stream, buf })
}

// ---------------------------------------------------------------------------------------------

struct Speaker {
    sink: rodio::Sink,
    /// Last frame time (seconds of the game clock), for the "talking" marks.
    last: f64,
}

/// Voice chat state: settings, the microphone, and one playback queue per speaker.
pub struct Voice {
    /// Microphone on (the player chose to talk).
    pub enabled: bool,
    /// «Нажми и говори» instead of «Активация голосом».
    pub push_to_talk: bool,
    /// Activation threshold (RMS level 0..1).
    pub threshold: f32,
    /// Loudness of other players.
    pub volume: f32,
    /// Voices fade with distance (off = everyone on the server is heard at full volume).
    pub proximity: bool,
    /// Players muted by this player (ids).
    pub muted: HashSet<u32>,
    /// The push-to-talk key or button is held.
    pub ptt_held: bool,
    /// Microphone level right now (0..1), for the meter.
    pub level: f32,
    /// Sending right now.
    pub talking: bool,
    /// Why the microphone does not work, if it does not.
    pub error: Option<String>,
    capture: Option<Capture>,
    mic_retry: f32,
    pending: Vec<f32>,
    hang: f32,
    speakers: HashMap<u32, Speaker>,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            enabled: false,
            push_to_talk: false,
            threshold: 0.02,
            volume: 1.0,
            proximity: true,
            muted: HashSet::new(),
            ptt_held: false,
            level: 0.0,
            talking: false,
            error: None,
            capture: None,
            mic_retry: 0.0,
            pending: Vec::new(),
            hang: 0.0,
            speakers: HashMap::new(),
        }
    }
}

impl Voice {
    /// Frames to send this frame (already packed). `allowed`: the microphone may be used
    /// (on a voice server, permission granted).
    pub fn capture(&mut self, allowed: bool, dt: f32) -> Vec<String> {
        if !self.enabled || !allowed {
            self.capture = None;
            self.talking = false;
            self.level = 0.0;
            return Vec::new();
        }
        if self.capture.is_none() {
            self.mic_retry -= dt;
            if self.mic_retry > 0.0 {
                return Vec::new();
            }
            match open_mic() {
                Ok(c) => {
                    self.capture = Some(c);
                    self.error = None;
                }
                Err(e) => {
                    self.error = Some(e);
                    self.mic_retry = 3.0;
                    return Vec::new();
                }
            }
        }
        let Some(cap) = &self.capture else { return Vec::new() };
        let fresh = std::mem::take(&mut *cap.buf.lock().unwrap());
        self.pending.extend(fresh);
        let mut out = Vec::new();
        while self.pending.len() >= FRAME {
            let frame: Vec<f32> = self.pending.drain(..FRAME).collect();
            let rms = (frame.iter().map(|x| x * x).sum::<f32>() / FRAME as f32).sqrt();
            self.level = self.level * 0.6 + rms.min(1.0) * 0.4;
            let open = if self.push_to_talk {
                self.ptt_held
            } else {
                if rms >= self.threshold {
                    self.hang = HANG;
                } else {
                    self.hang -= FRAME as f32 / RATE as f32;
                }
                self.hang > 0.0
            };
            self.talking = open;
            if open {
                out.push(b64_encode(&encode(&frame)));
            }
        }
        out
    }

    /// A frame from player `id` arrived.
    pub fn receive(&mut self, handle: Option<&rodio::OutputStreamHandle>, id: u32, data: &str, now: f64) {
        if self.muted.contains(&id) {
            return;
        }
        let Some(handle) = handle else { return };
        let Some(bytes) = b64_decode(data) else { return };
        let samples = decode(&bytes, FRAME);
        if samples.is_empty() {
            return;
        }
        if !self.speakers.contains_key(&id) {
            let Ok(sink) = rodio::Sink::try_new(handle) else { return };
            self.speakers.insert(id, Speaker { sink, last: now });
        }
        let sp = self.speakers.get_mut(&id).unwrap();
        sp.last = now;
        // Too far behind (a hiccup): drop this frame instead of lagging more and more.
        if sp.sink.len() > 12 {
            return;
        }
        sp.sink.append(rodio::buffer::SamplesBuffer::new(1, RATE, samples));
    }

    /// Volume by distance; forgets speakers that left. `me` is the listener's position.
    pub fn update(&mut self, me: DVec3, players: &[wolfcraft_net::PlayerState], now: f64) {
        self.speakers.retain(|id, sp| {
            let Some(p) = players.iter().find(|p| p.id == *id) else { return false };
            let d = (DVec3::from_array(p.pos) - me).length();
            let k = if self.proximity { (1.0 - d / RANGE).clamp(0.0, 1.0).powf(1.3) } else { 1.0 };
            sp.sink.set_volume((k as f32 * self.volume).clamp(0.0, 2.0));
            now - sp.last < 60.0 || !sp.sink.empty()
        });
    }

    /// Players heard speaking in the last moment.
    pub fn speaking(&self, now: f64) -> Vec<u32> {
        self.speakers.iter().filter(|(_, s)| now - s.last < 0.3).map(|(id, _)| *id).collect()
    }

    /// Leaving the server: close the microphone and the queues.
    pub fn reset(&mut self) {
        self.capture = None;
        self.speakers.clear();
        self.pending.clear();
        self.talking = false;
        self.level = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adpcm_keeps_speech_shape() {
        let wave: Vec<f32> = (0..FRAME).map(|i| (i as f32 * 0.07).sin() * 0.5 + (i as f32 * 0.31).sin() * 0.1).collect();
        let packed = encode(&wave);
        assert_eq!(packed.len(), 3 + FRAME / 2);
        let back = decode(&packed, FRAME);
        assert_eq!(back.len(), FRAME);
        let err = wave.iter().zip(&back).map(|(a, b)| (a - b).abs()).sum::<f32>() / FRAME as f32;
        assert!(err < 0.03, "mean error {err}");
    }

    #[test]
    fn base64_round_trip() {
        for n in 0..10 {
            let data: Vec<u8> = (0..n * 7 + 1).map(|i| (i * 37 + n) as u8).collect();
            assert_eq!(b64_decode(&b64_encode(&data)).unwrap(), data);
        }
        assert_eq!(b64_encode(b"Man"), "TWFu");
    }
}
