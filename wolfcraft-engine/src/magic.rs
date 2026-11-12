//! Magic: enchantments ("чары") on tools, weapons, armour and the bow, and effects from
//! potions and tipped arrows.
//!
//! Enchantments are our own set (not a copy of the classic list): they are cast at the
//! Чаровальня with experience levels and iskrit dust, or carried in a «Книга чар» and put on
//! an item at the forge. An enchanted item glows red.
//!
//! An `ItemStack` keeps up to four enchantments packed in its `ench` word, one per byte:
//! the high 5 bits are the enchantment id, the low 3 bits the level. Potions and tipped
//! arrows keep their effect in the same word (id in the low byte).

use crate::items::{self, ItemId, ids};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Ench {
    /// Weapons: more damage.
    Sharpness = 1,
    /// Tools: faster mining.
    Efficiency = 2,
    /// Tools, weapons, the bow: wears out slower.
    Unbreaking = 3,
    /// Armour: less damage taken.
    Protection = 4,
    /// Weapons: a blow that lands heals the player a little (our own).
    Lifesteal = 5,
    /// Pickaxe: ores sometimes give more.
    Fortune = 6,
    /// Bow: arrows hit harder.
    Power = 7,
    /// Boots: softer falls.
    Feather = 8,
    /// Tools: dropped items fly to you from further away (our own).
    Magnet = 9,
}

pub const ALL_ENCH: [Ench; 9] = [Ench::Sharpness, Ench::Efficiency, Ench::Unbreaking, Ench::Protection, Ench::Lifesteal, Ench::Fortune, Ench::Power, Ench::Feather, Ench::Magnet];

/// What an item is, for which enchantments fit it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Sword,
    Pickaxe,
    Tool,
    Bow,
    Armor,
    Boots,
    Book,
    Other,
}

pub fn kind_of(id: ItemId) -> Kind {
    if id == ids::BOOK {
        return Kind::Book;
    }
    if id == ids::BOW {
        return Kind::Bow;
    }
    if let Some(slot) = items::armor_slot(id) {
        return if slot == 3 { Kind::Boots } else { Kind::Armor };
    }
    match items::tool_shape(id) {
        Some((3, _)) => Kind::Sword,
        Some((0, _)) => Kind::Pickaxe,
        Some(_) => Kind::Tool,
        None => Kind::Other,
    }
}

impl Ench {
    pub fn from_id(id: u8) -> Option<Ench> {
        ALL_ENCH.iter().copied().find(|e| *e as u8 == id)
    }
    pub fn name(self) -> &'static str {
        match self {
            Ench::Sharpness => "Острота",
            Ench::Efficiency => "Сноровка",
            Ench::Unbreaking => "Крепость",
            Ench::Protection => "Защита",
            Ench::Lifesteal => "Жажда жизни",
            Ench::Fortune => "Удача",
            Ench::Power => "Меткость",
            Ench::Feather => "Пёрышко",
            Ench::Magnet => "Магнит",
        }
    }
    pub fn max_level(self) -> u8 {
        match self {
            Ench::Lifesteal | Ench::Magnet => 2,
            _ => 3,
        }
    }
    /// Can this enchantment go on an item of this kind (books take any)?
    pub fn fits(self, k: Kind) -> bool {
        use Kind::*;
        match self {
            Ench::Sharpness | Ench::Lifesteal => matches!(k, Sword | Book),
            Ench::Efficiency | Ench::Magnet => matches!(k, Pickaxe | Tool | Book),
            Ench::Fortune => matches!(k, Pickaxe | Book),
            Ench::Unbreaking => matches!(k, Sword | Pickaxe | Tool | Bow | Book),
            Ench::Protection => matches!(k, Armor | Boots | Book),
            Ench::Feather => matches!(k, Boots | Book),
            Ench::Power => matches!(k, Bow | Book),
        }
    }
}

/// Roman numerals for levels.
pub fn roman(l: u8) -> &'static str {
    ["", "I", "II", "III", "IV", "V", "VI", "VII"][(l as usize).min(7)]
}

/// The enchantments packed in `ench`.
pub fn list(ench: u32) -> Vec<(Ench, u8)> {
    (0..4).filter_map(|i| {
        let b = (ench >> (i * 8)) as u8;
        let l = b & 7;
        Ench::from_id(b >> 3).filter(|_| l > 0).map(|e| (e, l))
    })
    .collect()
}

pub fn level(ench: u32, e: Ench) -> u8 {
    list(ench).into_iter().find(|x| x.0 == e).map(|x| x.1).unwrap_or(0)
}

/// `ench` with `e` at `lvl` (raised if already there, never lowered; at most four kinds).
pub fn with(ench: u32, e: Ench, lvl: u8) -> u32 {
    let mut v = list(ench);
    if let Some(x) = v.iter_mut().find(|x| x.0 == e) {
        x.1 = x.1.max(lvl).min(e.max_level());
    } else if v.len() < 4 {
        v.push((e, lvl.min(e.max_level())));
    }
    pack(&v)
}

fn pack(v: &[(Ench, u8)]) -> u32 {
    v.iter().take(4).enumerate().fold(0u32, |acc, (i, (e, l))| acc | ((((*e as u32) << 3) | (*l as u32 & 7)) << (i * 8)))
}

/// Puts the enchantments of `from` on an item of kind `k` (book → item at the forge):
/// the ones that fit; equal levels merge one step up. Returns the new word and whether
/// anything changed.
pub fn combine(onto: u32, from: u32, k: Kind) -> (u32, bool) {
    let mut out = onto;
    for (e, l) in list(from) {
        if !e.fits(k) {
            continue;
        }
        let have = level(out, e);
        let new = if have == l { l + 1 } else { have.max(l) };
        out = with(out, e, new);
    }
    (out, out != onto)
}

/// Three offers at the Чаровальня for an item, from weak to strong: (enchantment, level,
/// cost in levels and in iskrit dust). `seed` changes after every enchanting, so the offers
/// are fresh each time but do not change while you look.
pub fn offers(id: ItemId, seed: u32) -> Vec<(Ench, u8, u32)> {
    let k = kind_of(id);
    let fit: Vec<Ench> = ALL_ENCH.iter().copied().filter(|e| e.fits(k)).collect();
    if fit.is_empty() || k == Kind::Other {
        return Vec::new();
    }
    let mut h = seed ^ (id as u32).wrapping_mul(0x9E37_79B9);
    let mut next = || {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        h
    };
    (1..=3u8)
        .map(|tier| {
            let e = fit[(next() as usize) % fit.len()];
            let lvl = tier.min(e.max_level());
            (e, lvl, tier as u32 * 2 - 1 + lvl as u32 / 3)
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// Effects

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Effect {
    /// Instant: +4 health per level.
    Healing = 1,
    /// Health comes back over time.
    Regeneration = 2,
    /// Walk and run faster.
    Speed = 3,
    /// Blows hit harder.
    Strength = 4,
    /// Less damage taken.
    Resistance = 5,
    /// Lava and fire do not hurt.
    FireResist = 6,
    /// Damage over time (never kills).
    Poison = 7,
    /// Moves slower.
    Slowness = 8,
    /// Instant damage.
    Harm = 9,
}

pub const ALL_EFFECTS: [Effect; 9] = [Effect::Healing, Effect::Regeneration, Effect::Speed, Effect::Strength, Effect::Resistance, Effect::FireResist, Effect::Poison, Effect::Slowness, Effect::Harm];

impl Effect {
    pub fn from_id(id: u8) -> Option<Effect> {
        ALL_EFFECTS.iter().copied().find(|e| *e as u8 == id)
    }
    pub fn name(self) -> &'static str {
        match self {
            Effect::Healing => "Лечение",
            Effect::Regeneration => "Регенерация",
            Effect::Speed => "Скорость",
            Effect::Strength => "Сила",
            Effect::Resistance => "Стойкость",
            Effect::FireResist => "Огнестойкость",
            Effect::Poison => "Яд",
            Effect::Slowness => "Медлительность",
            Effect::Harm => "Вред",
        }
    }
    /// Colour of the potion, the arrow tip and the effect's particles.
    pub fn color(self) -> [f32; 3] {
        match self {
            Effect::Healing => [0.97, 0.25, 0.35],
            Effect::Regeneration => [0.95, 0.45, 0.75],
            Effect::Speed => [0.45, 0.85, 1.0],
            Effect::Strength => [0.85, 0.30, 0.15],
            Effect::Resistance => [0.55, 0.55, 0.70],
            Effect::FireResist => [1.0, 0.62, 0.15],
            Effect::Poison => [0.35, 0.75, 0.20],
            Effect::Slowness => [0.40, 0.45, 0.60],
            Effect::Harm => [0.40, 0.08, 0.10],
        }
    }
    /// Seconds a potion of it lasts (0 = instant).
    pub fn duration(self) -> f32 {
        match self {
            Effect::Healing | Effect::Harm => 0.0,
            Effect::Regeneration | Effect::Poison => 30.0,
            _ => 120.0,
        }
    }
    /// Good for the one who drinks it (bad ones are for arrows).
    pub fn good(self) -> bool {
        !matches!(self, Effect::Poison | Effect::Slowness | Effect::Harm)
    }
}

/// An effect running on the player.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Active {
    pub effect: Effect,
    pub left: f32,
    pub level: u8,
}

/// Effect carried by a potion or tipped arrow stack.
pub fn effect_of(ench: u32) -> Option<Effect> {
    Effect::from_id(ench as u8)
}

/// The potion (and its tipped arrows) recipe ingredient for each drinkable/arrow effect.
pub fn potion_ingredient(e: Effect) -> ItemId {
    match e {
        Effect::Healing => ids::APPLE,
        Effect::Regeneration => ids::SLIME,
        Effect::Speed => crate::block::Block::SugarCane as ItemId,
        Effect::Strength => ids::COOKED_MEAT,
        Effect::Resistance => ids::IRON_INGOT,
        Effect::FireResist => ids::COAL,
        Effect::Poison => ids::ROTTEN_FLESH,
        Effect::Slowness => ids::STRING,
        Effect::Harm => ids::BONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_merging_and_offers() {
        let e = with(0, Ench::Sharpness, 2);
        let e = with(e, Ench::Unbreaking, 1);
        assert_eq!(list(e), vec![(Ench::Sharpness, 2), (Ench::Unbreaking, 1)]);
        assert_eq!(level(e, Ench::Sharpness), 2);
        // A book with Sharpness II on a sword with Sharpness II gives III.
        let book = with(0, Ench::Sharpness, 2);
        let (r, changed) = combine(e, book, Kind::Sword);
        assert!(changed);
        assert_eq!(level(r, Ench::Sharpness), 3);
        // Power does not go on a sword.
        let (r2, changed) = combine(e, with(0, Ench::Power, 1), Kind::Sword);
        assert!(!changed);
        assert_eq!(r2, e);
        // Offers fit the item and cost more for stronger ones.
        let sword = ids::tool(3, 3);
        let o = offers(sword, 7);
        assert_eq!(o.len(), 3);
        assert!(o.iter().all(|(e, _, _)| e.fits(Kind::Sword)));
        assert!(o[0].2 < o[2].2);
        assert!(offers(ids::APPLE, 7).is_empty());
    }

    #[test]
    fn enchantments_survive_saving_books_work_at_the_forge_and_potions_drink() {
        use crate::inventory::Inventory;
        use crate::items::ItemStack;
        // Saved and loaded with the inventory.
        let mut inv = Inventory::default();
        let mut sword = ItemStack::new(ids::tool(3, 3), 1);
        sword.ench = with(0, Ench::Sharpness, 3);
        inv.slots[4] = Some(sword);
        let mut bytes = Vec::new();
        inv.to_bytes(&mut bytes);
        let (back, _) = Inventory::from_bytes(&bytes).unwrap();
        assert_eq!(back.slots[4].unwrap().ench_level(Ench::Sharpness), 3);
        assert!(back.slots[4].unwrap().enchanted());
        assert!(back.slots[4].unwrap().display_name().contains("Острота III"));
        // A book at the forge.
        let pick = ItemStack::new(ids::tool(2, 0), 1);
        let mut book = ItemStack::new(ids::ENCH_BOOK, 1);
        book.ench = with(0, Ench::Efficiency, 2);
        let (res, used) = crate::interaction::forge_result(pick, book).expect("book applies");
        assert_eq!(used, 1);
        assert_eq!(res.ench_level(Ench::Efficiency), 2);
        // A sword book does nothing on a pickaxe.
        let mut sharp = ItemStack::new(ids::ENCH_BOOK, 1);
        sharp.ench = with(0, Ench::Sharpness, 1);
        assert!(crate::interaction::forge_result(pick, sharp).is_none());
        // Different enchantments do not stack together.
        assert!(!ItemStack { ench: 5, ..ItemStack::new(ids::ARROW, 1) }.can_stack_with(&ItemStack::new(ids::ARROW, 1)));
        // Effects: speed runs out, healing is instant, poison never kills.
        let mut p = crate::player::Player::new(glam::DVec3::new(0.0, 1100.0, 0.0), crate::config::GameMode::Survival);
        p.apply_effect(Effect::Speed, 1, 2.0);
        assert_eq!(p.effect_level(Effect::Speed), 1);
        p.survival.health = 10.0;
        p.apply_effect(Effect::Healing, 1, 0.0);
        assert_eq!(p.survival.health, 14.0);
        p.survival.health = 2.0;
        p.apply_effect(Effect::Poison, 1, 30.0);
        for _ in 0..40 {
            p.tick_effects(0.5);
        }
        assert_eq!(p.survival.health, 1.0, "poison stops at one");
        assert_eq!(p.effect_level(Effect::Speed), 0, "speed wore off");
    }
}
