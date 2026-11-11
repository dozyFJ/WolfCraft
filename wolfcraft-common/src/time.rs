//! WolfCraft core: game loop timing, fixed timestep, frame limiting and a simple event queue.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Fixed simulation rate (ticks per second). Physics and survival logic run at this rate.
pub const TICKS_PER_SECOND: u32 = 60;
pub const TICK_DT: f32 = 1.0 / TICKS_PER_SECOND as f32;

/// Frame timing information and FPS statistics.
pub struct Time {
    start: Instant,
    last_frame: Instant,
    delta: f32,
    frame_count: u64,
    fps_window_start: Instant,
    fps_window_frames: u32,
    fps: f32,
    frame_ms_avg: f32,
    frame_ms_max: f32,
    window_max: f32,
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

impl Time {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last_frame: now,
            delta: 0.0,
            frame_count: 0,
            fps_window_start: now,
            fps_window_frames: 0,
            fps: 0.0,
            frame_ms_avg: 0.0,
            frame_ms_max: 0.0,
            window_max: 0.0,
        }
    }

    /// Call once at the start of every frame.
    pub fn tick(&mut self) {
        let now = Instant::now();
        // Clamp huge deltas (window drag, breakpoints) so simulation does not explode.
        self.delta = (now - self.last_frame).as_secs_f32().min(0.25);
        self.last_frame = now;
        self.frame_count += 1;
        self.fps_window_frames += 1;
        self.window_max = self.window_max.max(self.delta * 1000.0);
        let window = (now - self.fps_window_start).as_secs_f32();
        if window >= 0.5 {
            self.fps = self.fps_window_frames as f32 / window;
            self.frame_ms_avg = window * 1000.0 / self.fps_window_frames as f32;
            self.frame_ms_max = self.window_max;
            self.window_max = 0.0;
            self.fps_window_frames = 0;
            self.fps_window_start = now;
        }
    }

    pub fn delta(&self) -> f32 {
        self.delta
    }
    pub fn elapsed(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
    /// Measured frames per second over the last ~0.5 s.
    pub fn fps(&self) -> f32 {
        self.fps
    }
    pub fn frame_ms_avg(&self) -> f32 {
        self.frame_ms_avg
    }
    pub fn frame_ms_max(&self) -> f32 {
        self.frame_ms_max
    }
}

/// Fixed-timestep accumulator: converts variable frame time into a whole number of ticks.
#[derive(Default)]
pub struct FixedStep {
    accumulator: f32,
}

impl FixedStep {
    /// Adds frame time and returns how many fixed ticks should run (capped to avoid spiral of death).
    pub fn advance(&mut self, dt: f32) -> u32 {
        self.accumulator += dt;
        let mut ticks = 0;
        while self.accumulator >= TICK_DT && ticks < 8 {
            self.accumulator -= TICK_DT;
            ticks += 1;
        }
        if ticks == 8 {
            self.accumulator = 0.0;
        }
        ticks
    }

    /// Interpolation factor in [0,1) between the last and next tick.
    pub fn alpha(&self) -> f32 {
        self.accumulator / TICK_DT
    }
}

/// Software frame limiter. Returns the instant the next frame should start.
pub struct FrameLimiter {
    next_frame: Instant,
}

impl Default for FrameLimiter {
    fn default() -> Self {
        Self { next_frame: Instant::now() }
    }
}

impl FrameLimiter {
    /// `max_fps == 0` means unlimited.
    pub fn next_deadline(&mut self, max_fps: u32) -> Option<Instant> {
        if max_fps == 0 {
            return None;
        }
        let frame = Duration::from_secs_f64(1.0 / max_fps as f64);
        let now = Instant::now();
        self.next_frame += frame;
        // If we fell behind by more than a frame, resync instead of bursting.
        if self.next_frame + frame < now {
            self.next_frame = now;
        }
        Some(self.next_frame)
    }
}

/// Simple FIFO event queue used to decouple systems (UI -> game, game -> audio/storage, ...).
pub struct EventQueue<T> {
    events: VecDeque<T>,
}

impl<T> Default for EventQueue<T> {
    fn default() -> Self {
        Self { events: VecDeque::new() }
    }
}

impl<T> EventQueue<T> {
    pub fn push(&mut self, e: T) {
        self.events.push_back(e);
    }
    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.events.drain(..)
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Scoped CPU timer for lightweight profiling of frame stages.
#[derive(Default, Clone, Copy, Debug)]
pub struct CpuProfile {
    pub update_ms: f32,
    pub chunks_ms: f32,
    pub render_ms: f32,
    pub ui_ms: f32,
}

pub struct ScopeTimer(Instant);
impl ScopeTimer {
    pub fn start() -> Self {
        Self(Instant::now())
    }
    pub fn ms(&self) -> f32 {
        self.0.elapsed().as_secs_f32() * 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_step_counts_ticks() {
        let mut fs = FixedStep::default();
        assert_eq!(fs.advance(TICK_DT * 2.5), 2);
        assert!(fs.alpha() > 0.4 && fs.alpha() < 0.6);
        assert_eq!(fs.advance(TICK_DT * 0.6), 1);
    }

    #[test]
    fn fixed_step_caps_spiral() {
        let mut fs = FixedStep::default();
        assert_eq!(fs.advance(10.0), 8);
        assert_eq!(fs.alpha(), 0.0);
    }

    #[test]
    fn event_queue_fifo() {
        let mut q = EventQueue::default();
        q.push(1);
        q.push(2);
        assert_eq!(q.drain().collect::<Vec<_>>(), vec![1, 2]);
        assert!(q.is_empty());
    }
}
