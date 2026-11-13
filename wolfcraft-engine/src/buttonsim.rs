//! «Кнопки»: a button-simulator mini-game.
//!
//! Money comes in every second: `multiplier × (1 + rebirths)`. Standing on a floor button buys
//! it again and again while you can afford it: green-to-red buttons add multiplier for money,
//! the purple one is a rebirth — it costs multiplier, resets money and multiplier, and every
//! rebirth makes all later income bigger.

use crate::block::Block;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cost {
    Money(f64),
    Multiplier(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub block: Block,
    pub cost: Cost,
    /// Multiplier added, or rebirths gained (rebirth button).
    pub gain: f64,
    pub rebirth: bool,
}

pub const PADS: [Pad; 5] = [
    Pad { block: Block::ButtonMult1, cost: Cost::Money(10.0), gain: 1.0, rebirth: false },
    Pad { block: Block::ButtonMult2, cost: Cost::Money(250.0), gain: 10.0, rebirth: false },
    Pad { block: Block::ButtonMult3, cost: Cost::Money(6_000.0), gain: 120.0, rebirth: false },
    Pad { block: Block::ButtonMult4, cost: Cost::Money(150_000.0), gain: 1_500.0, rebirth: false },
    Pad { block: Block::ButtonRebirth, cost: Cost::Multiplier(1_000.0), gain: 1.0, rebirth: true },
];

pub fn pad(b: Block) -> Option<&'static Pad> {
    PADS.iter().find(|p| p.block == b)
}

/// Short form of a big number: 1.2K, 3.4M, 5.6B…
pub fn short(v: f64) -> String {
    let (n, s) = if v >= 1e12 { (v / 1e12, "T") } else if v >= 1e9 { (v / 1e9, "B") } else if v >= 1e6 { (v / 1e6, "M") } else if v >= 1e3 { (v / 1e3, "K") } else { (v, "") };
    if s.is_empty() { format!("{:.0}", n) } else if n < 10.0 { format!("{:.2}{s}", n) } else if n < 100.0 { format!("{:.1}{s}", n) } else { format!("{:.0}{s}", n) }
}

/// What a button does, for the hint while standing on it.
pub fn describe(p: &Pad) -> String {
    match p.cost {
        Cost::Money(c) => format!("+{} множитель за {} денег", short(p.gain), short(c)),
        Cost::Multiplier(c) => format!("Перерождение за {} множителя: деньги и множитель обнулятся, весь доход вырастет", short(c)),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ButtonSim {
    pub money: f64,
    pub multiplier: f64,
    pub rebirths: u32,
    /// Seconds to the next purchase while standing on a button.
    cooldown: f32,
    /// Ever played (shows the counters).
    pub active: bool,
}

impl Default for ButtonSim {
    fn default() -> Self {
        Self { money: 0.0, multiplier: 1.0, rebirths: 0, cooldown: 0.0, active: false }
    }
}

impl ButtonSim {
    pub fn income(&self) -> f64 {
        self.multiplier * (1.0 + self.rebirths as f64)
    }

    /// Money for `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if self.active {
            self.money += self.income() * dt as f64;
        }
        self.cooldown = (self.cooldown - dt).max(0.0);
    }

    /// Standing on `b` (the block under the feet): buys it when the cooldown allows and it is
    /// affordable. Returns a message for the screen when something was bought.
    pub fn stand_on(&mut self, b: Block) -> Option<String> {
        let p = pad(b)?;
        self.active = true;
        if self.cooldown > 0.0 {
            return None;
        }
        match p.cost {
            Cost::Money(c) if self.money >= c => {
                self.money -= c;
                self.multiplier += p.gain;
                self.cooldown = 0.12;
                Some(format!("+{} множитель · всего ×{}", short(p.gain), short(self.multiplier)))
            }
            Cost::Multiplier(c) if self.multiplier >= c => {
                self.rebirths += p.gain as u32;
                self.money = 0.0;
                self.multiplier = 1.0;
                self.cooldown = 0.6;
                Some(format!("Перерождение! Всего {} — доход ×{}", self.rebirths, 1 + self.rebirths))
            }
            _ => None,
        }
    }

    pub fn to_text(&self) -> String {
        format!("money {}\nmultiplier {}\nrebirths {}\n", self.money, self.multiplier, self.rebirths)
    }

    pub fn from_text(t: &str) -> Self {
        let mut s = ButtonSim::default();
        for line in t.lines() {
            let mut it = line.split_whitespace();
            match (it.next(), it.next()) {
                (Some("money"), Some(v)) => s.money = v.parse().unwrap_or(0.0),
                (Some("multiplier"), Some(v)) => s.multiplier = v.parse::<f64>().unwrap_or(1.0).max(1.0),
                (Some("rebirths"), Some(v)) => s.rebirths = v.parse().unwrap_or(0),
                _ => {}
            }
        }
        s.active = !t.trim().is_empty();
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buying_multiplier_and_rebirth() {
        let mut s = ButtonSim { active: true, ..Default::default() };
        s.tick(9.0);
        assert!(s.stand_on(Block::ButtonMult1).is_none(), "9 money is not enough");
        s.tick(1.0);
        assert!(s.stand_on(Block::ButtonMult1).is_some());
        assert_eq!(s.multiplier, 2.0);
        assert!(s.stand_on(Block::ButtonMult1).is_none(), "cooldown");
        s.multiplier = 1000.0;
        s.money = 5.0;
        s.tick(1.0);
        assert!(s.stand_on(Block::ButtonRebirth).is_some());
        assert_eq!((s.rebirths, s.multiplier, s.money), (1, 1.0, 0.0));
        assert_eq!(s.income(), 2.0);
        let back = ButtonSim::from_text(&s.to_text());
        assert_eq!(back.rebirths, 1);
    }

    #[test]
    fn short_numbers() {
        assert_eq!(short(950.0), "950");
        assert_eq!(short(1500.0), "1.50K");
        assert_eq!(short(25_300_000.0), "25.3M");
    }
}
