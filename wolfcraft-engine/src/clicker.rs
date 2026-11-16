//! «Кликер-симулятор»: a WolfStudio mini-game template in the style of Roblox clicker sims.
//!
//! Tap to gather clicks (up to the backpack's size), sell them for money, spend money on
//! upgrades (click power, auto-click, a bigger backpack), on eggs (capsules that hatch pets —
//! the three best multiply every click) and on islands (each one multiplies clicks more).
//! A rebirth costs money, resets money, clicks and upgrades, and adds to every later income.

use crate::buttonsim::short;

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Rarity {
    Common,
    Rare,
    Epic,
    Legendary,
    Mythic,
}

impl Rarity {
    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "Обычный",
            Rarity::Rare => "Редкий",
            Rarity::Epic => "Эпический",
            Rarity::Legendary => "Легендарный",
            Rarity::Mythic => "Мифический",
        }
    }
    pub fn color(self) -> [u8; 3] {
        match self {
            Rarity::Common => [200, 205, 215],
            Rarity::Rare => [80, 160, 255],
            Rarity::Epic => [180, 100, 255],
            Rarity::Legendary => [255, 195, 50],
            Rarity::Mythic => [255, 80, 140],
        }
    }
}

/// A pet: name, rarity, multiplier, and its look (body and ear colours, ear shape 0 round /
/// 1 pointed / 2 long, a face pattern) — the pet atlas draws it from these.
pub struct Pet {
    pub name: &'static str,
    pub rarity: Rarity,
    pub mult: f64,
    pub body: [u8; 3],
    pub accent: [u8; 3],
    pub ears: u8,
    pub pattern: u8,
}

const fn pet(name: &'static str, rarity: Rarity, mult: f64, body: [u8; 3], accent: [u8; 3], ears: u8, pattern: u8) -> Pet {
    Pet { name, rarity, mult, body, accent, ears, pattern }
}

pub const PETS: [Pet; 15] = [
    pet("Котёнок", Rarity::Common, 1.2, [235, 170, 90], [250, 220, 180], 1, 0),
    pet("Щенок", Rarity::Common, 1.3, [180, 130, 90], [120, 80, 50], 2, 0),
    pet("Зайка", Rarity::Common, 1.4, [240, 240, 245], [250, 180, 200], 2, 1),
    pet("Мишка", Rarity::Rare, 1.8, [150, 100, 60], [220, 180, 130], 0, 0),
    pet("Лисёнок", Rarity::Rare, 2.0, [240, 120, 40], [255, 245, 235], 1, 1),
    pet("Панда", Rarity::Rare, 2.2, [245, 245, 245], [30, 30, 35], 0, 2),
    pet("Пингвин", Rarity::Epic, 3.0, [40, 45, 60], [250, 250, 250], 0, 1),
    pet("Волчонок", Rarity::Epic, 3.5, [140, 150, 170], [230, 235, 245], 1, 1),
    pet("Огненный дракон", Rarity::Epic, 4.0, [230, 70, 40], [255, 200, 60], 1, 2),
    pet("Ледяной феникс", Rarity::Legendary, 6.0, [120, 200, 255], [240, 250, 255], 1, 2),
    pet("Золотой кот", Rarity::Legendary, 7.0, [255, 205, 60], [255, 245, 180], 1, 0),
    pet("Тёмный единорог", Rarity::Legendary, 8.0, [70, 40, 110], [200, 120, 255], 2, 2),
    pet("Радужный дракон", Rarity::Mythic, 15.0, [255, 90, 160], [90, 220, 255], 1, 2),
    pet("Космический волк", Rarity::Mythic, 20.0, [30, 30, 80], [140, 200, 255], 1, 1),
    pet("Король кликов", Rarity::Mythic, 30.0, [255, 215, 0], [255, 60, 60], 0, 2),
];

/// An egg (capsule): price and chance weights for common, rare, epic, legendary, mythic.
pub struct Egg {
    pub name: &'static str,
    pub price: f64,
    pub weights: [u32; 5],
    /// Capsule colours (top, bottom).
    pub colors: [[u8; 3]; 2],
}

pub const EGGS: [Egg; 4] = [
    Egg { name: "Простая капсула", price: 250.0, weights: [75, 22, 3, 0, 0], colors: [[230, 230, 235], [140, 200, 120]] },
    Egg { name: "Лесная капсула", price: 5_000.0, weights: [30, 50, 17, 3, 0], colors: [[120, 210, 110], [60, 130, 60]] },
    Egg { name: "Ледяная капсула", price: 150_000.0, weights: [0, 40, 42, 16, 2], colors: [[170, 220, 255], [70, 140, 230]] },
    Egg { name: "Вулканическая капсула", price: 5_000_000.0, weights: [0, 0, 45, 42, 13], colors: [[255, 160, 60], [200, 50, 40]] },
];

/// An island: name, unlock price, click multiplier, its blocks (floor, accent).
pub struct Island {
    pub name: &'static str,
    pub price: f64,
    pub mult: f64,
    pub floor: crate::block::Block,
    pub accent: crate::block::Block,
}

pub const ISLANDS: [Island; 5] = [
    Island { name: "Стартовый остров", price: 0.0, mult: 1.0, floor: crate::block::Block::Grass, accent: crate::block::Block::OakLeaves },
    Island { name: "Лесной остров", price: 25_000.0, mult: 2.0, floor: crate::block::Block::MossBlock, accent: crate::block::Block::SpruceLeaves },
    Island { name: "Ледяной остров", price: 750_000.0, mult: 5.0, floor: crate::block::Block::Snow, accent: crate::block::Block::PackedIce },
    Island { name: "Вулкан", price: 25_000_000.0, mult: 12.0, floor: crate::block::Block::Basalt, accent: crate::block::Block::Magma },
    Island { name: "Небесный остров", price: 1_000_000_000.0, mult: 30.0, floor: crate::block::Block::Wool, accent: crate::block::Block::GoldBlock },
];

/// Upgrades bought with money: (name, what it does, base price, price growth).
pub const UPGRADES: [(&str, &str, f64, f64); 3] = [
    ("Сила клика", "+1 к каждому клику", 25.0, 1.55),
    ("Автокликер", "+1 клик в секунду сам", 120.0, 1.75),
    ("Рюкзак", "×2 места для кликов", 60.0, 2.4),
];

/// Pets that work at once.
pub const EQUIPPED: usize = 3;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Clicker {
    /// Clicks gathered, waiting to be sold.
    pub clicks: f64,
    pub money: f64,
    pub rebirths: u32,
    pub upgrades: [u32; 3],
    /// Pets owned (indices into `PETS`).
    pub pets: Vec<usize>,
    /// Islands unlocked (index of the furthest).
    pub unlocked: usize,
    /// The island the player is on.
    pub island: usize,
    /// Where each island was built (top centre), for teleports.
    pub island_pos: Vec<[f64; 3]>,
    /// The lobby was built here (the overlay shows).
    pub active: bool,
    /// Total clicks ever (for the stats).
    pub total: f64,
    auto_acc: f64,
}

impl Clicker {
    pub fn backpack(&self) -> f64 {
        50.0 * 2f64.powi(self.upgrades[2] as i32) * (1.0 + self.rebirths as f64)
    }

    pub fn rebirth_mult(&self) -> f64 {
        1.0 + self.rebirths as f64 * 0.5
    }

    /// The best pets (up to `EQUIPPED`), strongest first.
    pub fn best(&self) -> Vec<usize> {
        let mut v = self.pets.clone();
        v.sort_by(|a, b| PETS[*b].mult.partial_cmp(&PETS[*a].mult).unwrap_or(std::cmp::Ordering::Equal));
        v.truncate(EQUIPPED);
        v
    }

    pub fn pet_mult(&self) -> f64 {
        1.0 + self.best().iter().map(|p| PETS[*p].mult - 1.0).sum::<f64>()
    }

    /// Everything that multiplies a click.
    pub fn multiplier(&self) -> f64 {
        self.rebirth_mult() * self.pet_mult() * ISLANDS[self.island.min(ISLANDS.len() - 1)].mult
    }

    pub fn per_click(&self) -> f64 {
        (1.0 + self.upgrades[0] as f64) * self.multiplier()
    }

    pub fn per_second(&self) -> f64 {
        self.upgrades[1] as f64 * self.multiplier()
    }

    /// One tap: clicks gained (0 when the backpack is full).
    pub fn tap(&mut self) -> f64 {
        self.add(self.per_click())
    }

    fn add(&mut self, n: f64) -> f64 {
        let got = n.min((self.backpack() - self.clicks).max(0.0));
        self.clicks += got;
        self.total += got;
        got
    }

    /// The auto-clicker for `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if !self.active || self.upgrades[1] == 0 {
            return;
        }
        self.auto_acc += self.per_second() * dt as f64;
        if self.auto_acc >= 1.0 {
            let n = self.auto_acc.floor();
            self.auto_acc -= n;
            self.add(n);
        }
    }

    pub fn full(&self) -> bool {
        self.clicks >= self.backpack()
    }

    /// Sells every click: one coin each (more on later islands). Returns the money.
    pub fn sell(&mut self) -> f64 {
        let m = self.clicks * (1.0 + self.island as f64);
        self.money += m;
        self.clicks = 0.0;
        m
    }

    pub fn upgrade_price(&self, k: usize) -> f64 {
        let (_, _, base, growth) = UPGRADES[k];
        (base * growth.powi(self.upgrades[k] as i32)).round()
    }

    pub fn buy_upgrade(&mut self, k: usize) -> Result<(), String> {
        let price = self.upgrade_price(k);
        if self.money < price {
            return Err(format!("Нужно {} денег", short(price)));
        }
        self.money -= price;
        self.upgrades[k] += 1;
        Ok(())
    }

    /// The price of rebirth number `n` (0-based): each next one costs a bit more.
    fn one_price(n: u32) -> f64 {
        10_000.0 * (1.0 + n as f64 * 0.6)
    }

    pub fn rebirth_price(&self) -> f64 {
        Self::one_price(self.rebirths)
    }

    /// The rebirth shop: `n` rebirths at once cost the sum of their prices.
    pub fn rebirths_price(&self, n: u32) -> f64 {
        (0..n).map(|i| Self::one_price(self.rebirths + i)).sum()
    }

    /// Packs in the rebirth shop.
    pub const PACKS: [u32; 5] = [1, 5, 25, 100, 500];

    /// Rebirth: money, clicks and upgrades go, the multiplier and the backpack grow for good.
    pub fn rebirth(&mut self) -> Result<(), String> {
        self.buy_rebirths(1)
    }

    /// Buys `n` rebirths at once.
    pub fn buy_rebirths(&mut self, n: u32) -> Result<(), String> {
        let price = self.rebirths_price(n);
        if self.money < price {
            return Err(format!("{n} перерожд. стоят {} денег", short(price)));
        }
        self.money = 0.0;
        self.clicks = 0.0;
        self.upgrades = [0; 3];
        self.rebirths += n;
        Ok(())
    }

    /// Opens an egg: `r1`, `r2` are rolls in 0..1. Returns the pet (index into `PETS`).
    pub fn hatch(&mut self, egg: usize, r1: f64, r2: f64) -> Result<usize, String> {
        let e = &EGGS[egg.min(EGGS.len() - 1)];
        if self.money < e.price {
            return Err(format!("{}: нужно {} денег", e.name, short(e.price)));
        }
        if self.pets.len() >= 60 {
            return Err("Питомцев уже 60 — больше не поместится".into());
        }
        self.money -= e.price;
        let total: u32 = e.weights.iter().sum();
        let mut pick = (r1 * total as f64) as u32;
        let mut rarity = Rarity::Common;
        for (i, w) in e.weights.iter().enumerate() {
            if pick < *w {
                rarity = [Rarity::Common, Rarity::Rare, Rarity::Epic, Rarity::Legendary, Rarity::Mythic][i];
                break;
            }
            pick -= w;
        }
        let pool: Vec<usize> = (0..PETS.len()).filter(|i| PETS[*i].rarity == rarity).collect();
        let k = pool[((r2 * pool.len() as f64) as usize).min(pool.len() - 1)];
        self.pets.push(k);
        Ok(k)
    }

    /// Unlocks (once, for money) and moves to an island.
    pub fn go_to(&mut self, i: usize) -> Result<(), String> {
        let i = i.min(ISLANDS.len() - 1);
        if i > self.unlocked {
            if i > self.unlocked + 1 {
                return Err(format!("Сначала откройте «{}»", ISLANDS[self.unlocked + 1].name));
            }
            if self.money < ISLANDS[i].price {
                return Err(format!("{}: нужно {} денег", ISLANDS[i].name, short(ISLANDS[i].price)));
            }
            self.money -= ISLANDS[i].price;
            self.unlocked = i;
        }
        self.island = i;
        Ok(())
    }

    pub fn to_text(&self) -> String {
        let mut s = format!(
            "clicks {}\nmoney {}\nrebirths {}\nupgrades {} {} {}\nunlocked {}\nisland {}\ntotal {}\npets {}\n",
            self.clicks,
            self.money,
            self.rebirths,
            self.upgrades[0],
            self.upgrades[1],
            self.upgrades[2],
            self.unlocked,
            self.island,
            self.total,
            self.pets.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(" ")
        );
        for p in &self.island_pos {
            s.push_str(&format!("isle {} {} {}\n", p[0], p[1], p[2]));
        }
        s
    }

    pub fn from_text(t: &str) -> Self {
        let mut c = Clicker::default();
        for line in t.lines() {
            let mut it = line.split_whitespace();
            let key = it.next().unwrap_or("");
            let nums: Vec<f64> = it.filter_map(|x| x.parse().ok()).collect();
            let n = |k: usize| nums.get(k).copied().unwrap_or(0.0);
            match key {
                "clicks" => c.clicks = n(0),
                "money" => c.money = n(0),
                "rebirths" => c.rebirths = n(0) as u32,
                "upgrades" => c.upgrades = [n(0) as u32, n(1) as u32, n(2) as u32],
                "unlocked" => c.unlocked = (n(0) as usize).min(ISLANDS.len() - 1),
                "island" => c.island = n(0) as usize,
                "total" => c.total = n(0),
                "pets" => c.pets = nums.iter().map(|p| *p as usize).filter(|p| *p < PETS.len()).collect(),
                "isle" if nums.len() == 3 => c.island_pos.push([n(0), n(1), n(2)]),
                _ => {}
            }
        }
        c.island = c.island.min(c.unlocked);
        c.active = !t.trim().is_empty();
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicker_taps_sells_upgrades_rebirths_hatches_and_travels() {
        let mut c = Clicker { active: true, ..Default::default() };
        assert_eq!(c.tap(), 1.0);
        for _ in 0..100 {
            c.tap();
        }
        assert!(c.full() && c.clicks == 50.0, "the backpack holds 50");
        assert_eq!(c.sell(), 50.0);
        c.buy_upgrade(0).unwrap();
        assert_eq!(c.per_click(), 2.0);
        assert!(c.buy_upgrade(0).is_err(), "too poor now");
        c.money = 1e12;
        c.buy_upgrade(1).unwrap();
        c.tick(2.5);
        assert!(c.clicks >= 2.0);
        let pet = c.hatch(0, 0.0, 0.0).unwrap();
        assert_eq!(PETS[pet].rarity, Rarity::Common);
        assert!(c.pet_mult() > 1.0);
        assert!(c.go_to(2).is_err(), "islands open in order");
        c.go_to(1).unwrap();
        assert_eq!(c.multiplier(), c.rebirth_mult() * c.pet_mult() * 2.0);
        c.rebirth().unwrap();
        assert_eq!((c.money, c.upgrades, c.rebirths), (0.0, [0; 3], 1));
        assert!(c.rebirths_price(5) > c.rebirth_price() * 5.0);
        c.money = c.rebirths_price(5);
        c.buy_rebirths(5).unwrap();
        assert_eq!(c.rebirths, 6);
        c.rebirths = 1;
        let back = Clicker::from_text(&c.to_text());
        assert_eq!((back.rebirths, back.pets.clone(), back.unlocked, back.island), (1, c.pets.clone(), 1, 1));
    }
}
