//! «Питомник»: a pet-collecting mini-game.
//!
//! Coin blocks give coins when broken (by hand or by pets). Coins buy capsule eggs at the
//! incubators (three tiers). An egg hatches a random pet whose rarity depends on the tier. The
//! three strongest pets follow the player and dig the nearest coin blocks on their own.

use crate::cubesphere::Cell;
use crate::mobs::MobKind;
use crate::physics::up_at;
use glam::DVec3;

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Rarity {
    Common,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "Обычный",
            Rarity::Rare => "Редкий",
            Rarity::Epic => "Эпический",
            Rarity::Legendary => "Легендарный",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Rarity::Common => [0.85, 0.87, 0.90],
            Rarity::Rare => [0.35, 0.65, 1.0],
            Rarity::Epic => [0.75, 0.40, 1.0],
            Rarity::Legendary => [1.0, 0.78, 0.20],
        }
    }
}

pub struct PetKind {
    pub name: &'static str,
    /// Creature model used to draw the pet.
    pub model: MobKind,
    pub rarity: Rarity,
    /// Digging speed (coin-block hit points per second).
    pub power: f32,
}

const fn pet(name: &'static str, model: MobKind, rarity: Rarity, power: f32) -> PetKind {
    PetKind { name, model, rarity, power }
}

pub const PETS: [PetKind; 10] = [
    pet("Цыплёнок", MobKind::Chicken, Rarity::Common, 1.0),
    pet("Овечка", MobKind::Sheep, Rarity::Common, 1.3),
    pet("Кабанчик", MobKind::Boar, Rarity::Common, 1.6),
    pet("Слизнячок", MobKind::Slime, Rarity::Rare, 3.0),
    pet("Волчонок", MobKind::Wolf, Rarity::Rare, 3.6),
    pet("Бурёнка", MobKind::Cow, Rarity::Rare, 4.2),
    pet("Огненный цыплёнок", MobKind::Chicken, Rarity::Epic, 8.0),
    pet("Лунный волк", MobKind::Wolf, Rarity::Epic, 10.0),
    pet("Золотой слизень", MobKind::Slime, Rarity::Legendary, 22.0),
    pet("Призрачный волк", MobKind::Wolf, Rarity::Legendary, 28.0),
];

pub struct EggTier {
    pub name: &'static str,
    pub price: u64,
    /// Chance weights of common, rare, epic, legendary.
    pub weights: [u32; 4],
}

pub const EGGS: [EggTier; 3] = [
    EggTier { name: "Обычное яйцо", price: 100, weights: [80, 18, 2, 0] },
    EggTier { name: "Редкое яйцо", price: 600, weights: [30, 52, 15, 3] },
    EggTier { name: "Легендарное яйцо", price: 3000, weights: [0, 45, 40, 15] },
];

/// Hit points of a coin block and the coins it gives.
pub const COIN_HP: f32 = 8.0;
pub const COIN_REWARD: u64 = 15;
/// Pets that follow the player at once.
pub const ACTIVE: usize = 3;

/// Index into `PETS` for an egg of `tier`; `r1`, `r2` are uniform rolls in 0..1.
pub fn hatch(tier: usize, r1: f64, r2: f64) -> usize {
    let w = EGGS[tier.min(EGGS.len() - 1)].weights;
    let total: u32 = w.iter().sum();
    let mut pick = (r1 * total as f64) as u32;
    let mut rarity = Rarity::Common;
    for (i, wi) in w.iter().enumerate() {
        if pick < *wi {
            rarity = [Rarity::Common, Rarity::Rare, Rarity::Epic, Rarity::Legendary][i];
            break;
        }
        pick -= wi;
    }
    let of: Vec<usize> = (0..PETS.len()).filter(|i| PETS[*i].rarity == rarity).collect();
    of[((r2 * of.len() as f64) as usize).min(of.len() - 1)]
}

/// A pet in the world.
#[derive(Clone, Copy, Debug)]
pub struct Pet {
    pub kind: usize,
    pub pos: DVec3,
    pub heading: DVec3,
    pub anim: f32,
    pub moving: bool,
    pub target: Option<Cell>,
    pub dig: f32,
}

#[derive(Default)]
pub struct PetSim {
    pub coins: u64,
    /// Pets the player has (indices into `PETS`): the pet items in the inventory, refreshed by
    /// the game every frame. Old saves listed them here; they move into the inventory once.
    pub owned: Vec<usize>,
    /// The active pets in the world (the strongest owned ones).
    pub pets: Vec<Pet>,
    /// Hatch animation: (seconds since it started, pet index).
    pub hatching: Option<(f32, usize)>,
    /// Angle of the pets' circle round the player.
    pub orbit: f64,
}

impl PetSim {
    /// The strongest owned pets, at most `ACTIVE`.
    pub fn best(&self) -> Vec<usize> {
        let mut v = self.owned.clone();
        v.sort_by(|a, b| PETS[*b].power.total_cmp(&PETS[*a].power));
        v.truncate(ACTIVE);
        v
    }

    /// Makes the pets in the world match the strongest owned ones (after a hatch or a load).
    pub fn refresh(&mut self, player: DVec3, heading: DVec3) {
        let want = self.best();
        let mut keep: Vec<Pet> = Vec::new();
        for k in want {
            if let Some(i) = self.pets.iter().position(|p| p.kind == k) {
                keep.push(self.pets.remove(i));
            } else {
                keep.push(Pet { kind: k, pos: player + up_at(player) * 0.5, heading, anim: 0.0, moving: false, target: None, dig: 0.0 });
            }
        }
        self.pets = keep;
    }

    /// Moves the pets: they follow behind the player and fly to the coin blocks in `coins`
    /// (cell, centre). Returns the coin blocks dug out this frame.
    pub fn update(&mut self, player: DVec3, heading: DVec3, coins: &[(Cell, DVec3)], dt: f32) -> Vec<Cell> {
        let up = up_at(player);
        let right = heading.cross(up).normalize_or_zero();
        self.orbit = (self.orbit + dt as f64 * 0.7) % std::f64::consts::TAU;
        let mut done = Vec::new();
        let n = self.pets.len();
        for i in 0..n {
            let taken: Vec<Cell> = self.pets.iter().enumerate().filter(|(j, _)| *j != i).filter_map(|(_, p)| p.target).collect();
            let p = &mut self.pets[i];
            // Keep the current target while it exists; otherwise the nearest free coin block.
            if p.target.is_some_and(|t| !coins.iter().any(|(c, _)| *c == t)) {
                p.target = None;
                p.dig = 0.0;
            }
            if p.target.is_none() {
                p.target = coins
                    .iter()
                    .filter(|(c, _)| !taken.contains(c))
                    .filter(|(_, at)| (*at - player).length() < 14.0)
                    .min_by(|a, b| (a.1 - p.pos).length().total_cmp(&(b.1 - p.pos).length()))
                    .map(|(c, _)| *c);
            }
            let goal = match p.target.and_then(|t| coins.iter().find(|(c, _)| *c == t)) {
                Some((_, at)) => *at + up_at(*at) * 0.8,
                None => {
                    // Round the player: the pets circle slowly at waist height, evenly spaced.
                    let a = self.orbit + i as f64 / n.max(1) as f64 * std::f64::consts::TAU;
                    player + (heading * a.cos() + right * a.sin()) * 1.9 + up * 0.7
                }
            };
            let to = goal - p.pos;
            let dist = to.length();
            if (p.pos - player).length() > 30.0 {
                p.pos = goal;
            } else if dist > 0.15 {
                let speed = (dist * 3.0).clamp(1.5, 10.0);
                p.pos += to / dist * (speed * dt as f64).min(dist);
                let flat = to - up_at(p.pos) * to.dot(up_at(p.pos));
                if flat.length() > 0.05 {
                    p.heading = flat.normalize();
                }
            }
            p.moving = dist > 0.4;
            p.anim += dt * if p.moving { 9.0 } else { 2.0 };
            // Digging: close to the target block.
            if let Some(t) = p.target {
                if dist < 1.0 {
                    p.dig += PETS[p.kind].power * dt;
                    if p.dig >= COIN_HP {
                        done.push(t);
                        p.target = None;
                        p.dig = 0.0;
                    }
                }
            }
        }
        done
    }

    /// Small hop so idle pets look alive (added to the drawn position).
    pub fn bounce(p: &Pet) -> f64 {
        (p.anim.sin().abs() * if p.moving { 0.25 } else { 0.08 }) as f64
    }

    /// Only the coins: the pets live in the inventory (saved with the player).
    pub fn to_text(&self) -> String {
        format!("coins {}\n", self.coins)
    }

    pub fn from_text(t: &str) -> Self {
        let mut s = PetSim::default();
        for line in t.lines() {
            let mut it = line.split_whitespace();
            match it.next() {
                Some("coins") => s.coins = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
                Some("pets") => s.owned = it.filter_map(|v| v.parse::<usize>().ok()).filter(|k| *k < PETS.len()).collect(),
                _ => {}
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eggs_hatch_their_rarities() {
        assert_eq!(PETS[hatch(0, 0.0, 0.0)].rarity, Rarity::Common);
        assert_eq!(PETS[hatch(2, 0.99, 0.5)].rarity, Rarity::Legendary);
        for t in 0..3 {
            for i in 0..50 {
                let k = hatch(t, i as f64 / 50.0, 0.3);
                assert!(k < PETS.len());
            }
        }
    }

    #[test]
    fn save_round_trip_and_best_three() {
        let mut s = PetSim { coins: 42, owned: vec![0, 9, 3, 1], ..Default::default() };
        let back = PetSim::from_text(&s.to_text());
        assert_eq!(back.coins, 42);
        assert!(back.owned.is_empty(), "pets are saved in the inventory now");
        assert_eq!(PetSim::from_text("coins 1\npets 0 9").owned, vec![0, 9], "old saves still load");
        assert_eq!(s.best(), vec![9, 3, 1]);
        s.refresh(DVec3::new(0.0, 1000.0, 0.0), DVec3::X);
        assert_eq!(s.pets.len(), 3);
    }

    #[test]
    fn pets_dig_a_coin_block() {
        let player = DVec3::new(0.0, 1000.0, 0.0);
        let mut s = PetSim { owned: vec![9], ..Default::default() };
        s.refresh(player, DVec3::X);
        let cell = Cell::new(2, 1, 1, 1000);
        let at = player + DVec3::X * 3.0;
        let mut dug = Vec::new();
        for _ in 0..200 {
            dug.extend(s.update(player, DVec3::X, &[(cell, at)], 0.05));
            if !dug.is_empty() {
                break;
            }
        }
        assert_eq!(dug, vec![cell]);
    }
}
