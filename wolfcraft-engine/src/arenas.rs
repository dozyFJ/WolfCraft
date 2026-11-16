//! PvP mini-game templates for WolfStudio: Бедварс, Скайварс, Эггварс, Мардер мистери, Прятки.
//!
//! Each builds its map in front of the player (see the session) and runs its rules here:
//! resource generators and a shop (Бед/Эггварс), a bed or an egg that lets you come back,
//! chests with loot (Скайварс), roles and a round timer (Мардер, Прятки).

use crate::items::{ItemId, ids};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    BedWars,
    SkyWars,
    EggWars,
    Murder,
    HideSeek,
}

impl Mode {
    pub const ALL: [Mode; 5] = [Mode::BedWars, Mode::SkyWars, Mode::EggWars, Mode::Murder, Mode::HideSeek];

    pub fn name(self) -> &'static str {
        match self {
            Mode::BedWars => "Бедварс",
            Mode::SkyWars => "Скайварс",
            Mode::EggWars => "Эггварс",
            Mode::Murder => "Мардер мистери",
            Mode::HideSeek => "Прятки",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Mode::BedWars => "bedwars",
            Mode::SkyWars => "skywars",
            Mode::EggWars => "eggwars",
            Mode::Murder => "murder",
            Mode::HideSeek => "hide",
        }
    }

    pub fn from_key(k: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.key() == k)
    }

    /// Bed or egg: you come back while it stands.
    pub fn has_heart(self) -> bool {
        matches!(self, Mode::BedWars | Mode::EggWars)
    }

    pub fn has_shop(self) -> bool {
        self.has_heart()
    }

    /// Round length in seconds (0 = no timer).
    pub fn round(self) -> f32 {
        match self {
            Mode::Murder => 240.0,
            Mode::HideSeek => 180.0,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    None,
    Murderer,
    Detective,
    Innocent,
    Seeker,
    Hider,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::None => "",
            Role::Murderer => "Убийца",
            Role::Detective => "Детектив",
            Role::Innocent => "Мирный",
            Role::Seeker => "Ищущий",
            Role::Hider => "Прячущийся",
        }
    }

    pub fn goal(self) -> &'static str {
        match self {
            Role::None => "",
            Role::Murderer => "Устраните всех, пока не кончилось время. Не попадитесь детективу!",
            Role::Detective => "Найдите убийцу и выстрелите в него из лука",
            Role::Innocent => "Выживите. Соберите 10 золота — получите лук",
            Role::Seeker => "Досчитайте до 20 и найдите всех прячущихся",
            Role::Hider => "Спрячьтесь, пока ищущий считает до 20, и продержитесь до конца",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Role::None => "none",
            Role::Murderer => "murderer",
            Role::Detective => "detective",
            Role::Innocent => "innocent",
            Role::Seeker => "seeker",
            Role::Hider => "hider",
        }
    }

    fn from_key(k: &str) -> Role {
        [Role::Murderer, Role::Detective, Role::Innocent, Role::Seeker, Role::Hider].into_iter().find(|r| r.key() == k).unwrap_or(Role::None)
    }

    /// A role by a roll in 0..1.
    pub fn roll(mode: Mode, r: f64) -> Role {
        match mode {
            Mode::Murder if r < 0.2 => Role::Murderer,
            Mode::Murder if r < 0.4 => Role::Detective,
            Mode::Murder => Role::Innocent,
            Mode::HideSeek if r < 0.3 => Role::Seeker,
            Mode::HideSeek => Role::Hider,
            _ => Role::None,
        }
    }

    /// What the role starts with.
    pub fn kit(self) -> Vec<(ItemId, u8)> {
        match self {
            Role::Murderer => vec![(ids::tool(2, 3), 1)],
            Role::Detective => vec![(ids::BOW, 1), (ids::ARROW, 8)],
            Role::Seeker => vec![(ids::tool(0, 3), 1)],
            _ => vec![],
        }
    }
}

/// The shop of Бед/Эггварс: (name, what you get, what it costs).
pub const SHOP: [(&str, (ItemId, u8), (ItemId, u8)); 12] = [
    ("Шерсть ×16", (crate::block::Block::Wool as ItemId, 16), (ids::IRON_INGOT, 4)),
    ("Доски ×16", (crate::block::Block::OakPlanks as ItemId, 16), (ids::GOLD_INGOT, 4)),
    ("Обсидиан ×4", (crate::block::Block::Obsidian as ItemId, 4), (ids::DIAMOND, 4)),
    ("Каменный меч", (ids::tool(1, 3), 1), (ids::IRON_INGOT, 10)),
    ("Железный меч", (ids::tool(2, 3), 1), (ids::GOLD_INGOT, 7)),
    ("Алмазный меч", (ids::tool(3, 3), 1), (ids::DIAMOND, 4)),
    ("Железный нагрудник", (ids::ARMOR_BASE + 9, 1), (ids::GOLD_INGOT, 12)),
    ("Алмазный нагрудник", (ids::ARMOR_BASE + 13, 1), (ids::DIAMOND, 6)),
    ("Лук", (ids::BOW, 1), (ids::GOLD_INGOT, 12)),
    ("Стрелы ×8", (ids::ARROW, 8), (ids::GOLD_INGOT, 2)),
    ("Кирка", (ids::tool(2, 0), 1), (ids::IRON_INGOT, 10)),
    ("Хлеб ×4", (ids::BREAD, 4), (ids::IRON_INGOT, 4)),
];

/// A resource generator: drops `item` at `pos` every `every` seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Gen {
    pub pos: [f64; 3],
    pub item: ItemId,
    pub every: f32,
    pub t: f32,
}

/// Loot for a Скайварс chest: a roll per slot picks from this list (centre chests are better).
pub fn chest_loot(centre: bool, rolls: &[f64]) -> Vec<(ItemId, u8)> {
    let common: [(ItemId, u8); 10] = [
        (ids::tool(1, 3), 1),
        (ids::tool(2, 0), 1),
        (crate::block::Block::OakPlanks as ItemId, 24),
        (crate::block::Block::Stone as ItemId, 24),
        (ids::ARMOR_BASE + 8, 1),
        (ids::ARMOR_BASE + 11, 1),
        (ids::BREAD, 4),
        (ids::ARROW, 8),
        (ids::BOW, 1),
        (ids::COOKED_MEAT, 3),
    ];
    let rich: [(ItemId, u8); 6] = [(ids::tool(3, 3), 1), (ids::ARMOR_BASE + 13, 1), (ids::ARMOR_BASE + 14, 1), (ids::tool(2, 3), 1), (ids::ARROW, 16), (crate::block::Block::Obsidian as ItemId, 8)];
    rolls
        .iter()
        .map(|r| if centre && *r > 0.5 { rich[((r - 0.5) * 2.0 * rich.len() as f64) as usize % rich.len()] } else { common[(r * common.len() as f64) as usize % common.len()] })
        .collect()
}

/// The running game of one player.
#[derive(Clone, Debug, PartialEq)]
pub struct Arena {
    pub mode: Mode,
    pub role: Role,
    /// Team colour name (Бед/Эггварс).
    pub team: &'static str,
    /// Where you come back.
    pub base: [f64; 3],
    /// The bed / egg cell (f, i, j, k).
    pub heart: Option<(u8, i32, i32, i32)>,
    pub heart_alive: bool,
    pub eliminated: bool,
    pub gens: Vec<Gen>,
    /// Seconds left in the round (`round()` > 0).
    pub timer: f32,
    /// Seconds since the start.
    pub age: f32,
    pub over: Option<String>,
}

impl Arena {
    pub fn new(mode: Mode, role: Role) -> Arena {
        Arena { mode, role, team: if mode.has_heart() { "Красные" } else { "" }, base: [0.0; 3], heart: None, heart_alive: true, eliminated: false, gens: vec![], timer: mode.round(), age: 0.0, over: None }
    }

    /// The seeker counts to 20 with eyes closed.
    pub fn counting(&self) -> bool {
        self.role == Role::Seeker && self.age < 20.0
    }

    /// Time passes: generators drop their items (returned), the round ends on time.
    pub fn tick(&mut self, dt: f32) -> Vec<(ItemId, [f64; 3])> {
        self.age += dt;
        let mut out = Vec::new();
        if self.over.is_none() {
            for g in &mut self.gens {
                g.t += dt;
                if g.t >= g.every {
                    g.t = 0.0;
                    out.push((g.item, g.pos));
                }
            }
        }
        if self.mode.round() > 0.0 && self.over.is_none() {
            self.timer = (self.timer - dt).max(0.0);
            if self.timer <= 0.0 {
                self.over = Some(match self.role {
                    Role::Murderer => "Время вышло — убийца не успел. Мирные победили!".into(),
                    Role::Seeker => "Время вышло — прячущиеся победили!".into(),
                    Role::Hider => "Вы продержались — победа прячущихся!".into(),
                    _ => "Время вышло — мирные победили!".into(),
                });
            }
        }
        out
    }

    /// The player died: with the bed / egg standing you come back, without it you are out.
    pub fn on_death(&mut self) {
        if self.mode.has_heart() && !self.heart_alive {
            self.eliminated = true;
        }
        if matches!(self.mode, Mode::SkyWars | Mode::Murder | Mode::HideSeek) {
            self.eliminated = true;
        }
    }

    /// Buys shop item `k` with what the player has (`count`, `take` on the inventory).
    pub fn price(k: usize) -> (ItemId, u8) {
        SHOP[k.min(SHOP.len() - 1)].2
    }

    pub fn to_text(&self) -> String {
        let mut s = format!("mode {}\nrole {}\nbase {} {} {}\n", self.mode.key(), self.role.key(), self.base[0], self.base[1], self.base[2]);
        if let Some((f, i, j, k)) = self.heart {
            s.push_str(&format!("heart {f} {i} {j} {k} {}\n", self.heart_alive as u8));
        }
        s.push_str(&format!("state {} {} {}\n", self.timer, self.age, self.eliminated as u8));
        for g in &self.gens {
            s.push_str(&format!("gen {} {} {} {} {}\n", g.pos[0], g.pos[1], g.pos[2], g.item, g.every));
        }
        s
    }

    pub fn from_text(t: &str) -> Option<Arena> {
        let mut a: Option<Arena> = None;
        for line in t.lines() {
            let mut it = line.split_whitespace();
            let key = it.next().unwrap_or("");
            let rest: Vec<&str> = it.collect();
            let f = |k: usize| rest.get(k).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);
            match key {
                "mode" => a = Mode::from_key(rest.first().copied().unwrap_or("")).map(|m| Arena::new(m, Role::None)),
                "role" => {
                    if let Some(a) = &mut a {
                        a.role = Role::from_key(rest.first().copied().unwrap_or(""));
                    }
                }
                "base" => {
                    if let Some(a) = &mut a {
                        a.base = [f(0), f(1), f(2)];
                    }
                }
                "heart" => {
                    if let Some(a) = &mut a {
                        a.heart = Some((f(0) as u8, f(1) as i32, f(2) as i32, f(3) as i32));
                        a.heart_alive = f(4) > 0.5;
                    }
                }
                "state" => {
                    if let Some(a) = &mut a {
                        a.timer = f(0) as f32;
                        a.age = f(1) as f32;
                        a.eliminated = f(2) > 0.5;
                    }
                }
                "gen" => {
                    if let Some(a) = &mut a {
                        a.gens.push(Gen { pos: [f(0), f(1), f(2)], item: f(3) as ItemId, every: f(4) as f32, t: 0.0 });
                    }
                }
                _ => {}
            }
        }
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arenas_generators_hearts_roles_timers_and_saves() {
        let mut a = Arena::new(Mode::BedWars, Role::None);
        a.gens.push(Gen { pos: [1.0, 2.0, 3.0], item: ids::IRON_INGOT, every: 1.5, t: 0.0 });
        assert!(a.tick(1.0).is_empty());
        assert_eq!(a.tick(0.6), vec![(ids::IRON_INGOT, [1.0, 2.0, 3.0])]);
        a.on_death();
        assert!(!a.eliminated, "the bed still stands");
        a.heart_alive = false;
        a.on_death();
        assert!(a.eliminated);
        let mut m = Arena::new(Mode::Murder, Role::roll(Mode::Murder, 0.1));
        assert_eq!(m.role, Role::Murderer);
        assert!(!m.role.kit().is_empty());
        m.tick(241.0);
        assert!(m.over.is_some());
        let h = Arena::new(Mode::HideSeek, Role::roll(Mode::HideSeek, 0.0));
        assert!(h.counting());
        let back = Arena::from_text(&a.to_text()).unwrap();
        assert_eq!((back.mode, back.eliminated, back.gens.len()), (Mode::BedWars, true, 1));
        assert_eq!(chest_loot(true, &[0.9, 0.1]).len(), 2);
        assert_eq!(Mode::from_key("eggwars"), Some(Mode::EggWars));
    }
}
