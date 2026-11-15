//! Weather like in Minecraft: clear spells, rain, thunderstorms; snow where it is cold, nothing
//! in deserts. Drops and flakes fall around the player, the sky darkens, lightning flashes.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sky {
    Clear,
    Rain,
    Thunder,
}

pub struct Weather {
    pub sky: Sky,
    /// How much it rains right now, 0..1 (eases in and out).
    pub rain: f32,
    /// Seconds until the weather changes.
    pub timer: f32,
    /// Seconds to the next lightning strike in a storm.
    pub bolt_in: f32,
    rng: u64,
}

impl Weather {
    pub fn new(seed: u64) -> Self {
        let mut w = Self { sky: Sky::Clear, rain: 0.0, timer: 0.0, bolt_in: 8.0, rng: seed | 1 };
        w.timer = 240.0 + w.rand() * 480.0;
        w
    }

    pub fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % 1_000_000) as f32 / 1_000_000.0
    }

    /// Sets the weather now (the /weather command) for `secs` seconds.
    pub fn set(&mut self, sky: Sky, secs: f32) {
        self.sky = sky;
        self.timer = secs;
        self.bolt_in = 3.0;
    }

    /// Advances the weather. Returns true when a new spell begins (for a notice).
    pub fn step(&mut self, dt: f32) -> bool {
        self.timer -= dt;
        let mut changed = false;
        if self.timer <= 0.0 {
            let r = self.rand();
            let (sky, secs) = if self.sky != Sky::Clear || r < 0.6 { (Sky::Clear, 360.0 + self.rand() * 600.0) } else if r < 0.88 { (Sky::Rain, 120.0 + self.rand() * 240.0) } else { (Sky::Thunder, 90.0 + self.rand() * 150.0) };
            changed = sky != self.sky;
            self.sky = sky;
            self.timer = secs;
        }
        let target = match self.sky {
            Sky::Clear => 0.0,
            Sky::Rain => 0.8,
            Sky::Thunder => 1.0,
        };
        // About 10 s to set in or to stop.
        self.rain += (target - self.rain).clamp(-dt * 0.1, dt * 0.1);
        changed
    }

    /// In a storm: whether a lightning strikes this frame.
    pub fn bolt(&mut self, dt: f32) -> bool {
        if self.sky != Sky::Thunder || self.rain < 0.6 {
            return false;
        }
        self.bolt_in -= dt;
        if self.bolt_in <= 0.0 {
            self.bolt_in = 5.0 + self.rand() * 15.0;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_comes_and_goes_and_storms_flash() {
        let mut w = Weather::new(7);
        let mut seen = [false; 3];
        let mut bolts = 0;
        let mut max_rain = 0.0f32;
        for _ in 0..(3600 * 20) {
            w.step(0.5);
            if w.bolt(0.5) {
                bolts += 1;
            }
            seen[w.sky as usize] = true;
            max_rain = max_rain.max(w.rain);
            assert!((0.0..=1.0).contains(&w.rain));
        }
        assert!(seen.iter().all(|s| *s), "clear, rain and thunder all happen: {seen:?}");
        assert!(max_rain > 0.75 && bolts > 0);
    }

    #[test]
    fn rain_eases_in() {
        let mut w = Weather::new(1);
        w.set(Sky::Rain, 100.0);
        w.step(1.0);
        assert!(w.rain > 0.0 && w.rain < 0.2);
        for _ in 0..20 {
            w.step(1.0);
        }
        assert!((w.rain - 0.8).abs() < 1e-3);
    }
}
