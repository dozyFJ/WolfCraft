//! What each tool can and cannot do. One place for the rules, used by mining, combat and the
//! item tooltips (so what the game does and what it tells the player always agree).
//!
//! * **Кирка** — камень, руды, кирпич, металл, лёд, обсидиан. Its material decides which ores
//!   drop anything: wood — coal; stone — iron, copper, tin; iron — gold, silver, cobalt,
//!   diamond, iskrit, uranium; diamond — obsidian too.
//! * **Топор** — logs, planks and everything wooden, pumpkins and melons. A strong blow.
//! * **Лопата** — dirt, grass, sand, gravel, clay, snow.
//! * **Меч** — the weapon: the strongest blow, wears 1 per hit. Cuts leaves, plants, wool and
//!   cobweb-like soft blocks quickly. Not for mining: on other blocks it is as slow as a hand
//!   and wears 2. In creative mode a sword does not break blocks at all (no accidents in a
//!   fight).
//! * **Мотыга** — tills dirt and grass into farmland; harvests leaves, crops and moss quickly.
//! * **Лук** — only shoots; as a club it is a bare hand and does not wear.
//! * Any tool on a block that is not its own works like a bare hand and wears 2.
//! * Blocks that break instantly (plants, torches) never wear a tool.
//! * A tool of too low a material digs 3.3 times slower and the block drops nothing.

use crate::block::{Block, RenderKind, ToolKind};
use crate::items::{self, ItemId, ids};

/// What a held item is, for the rules.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Pickaxe(u8),
    Axe(u8),
    Shovel(u8),
    Sword(u8),
    Hoe(u8),
    Bow,
    Other,
}

pub fn role(id: Option<ItemId>) -> Role {
    let Some(id) = id else { return Role::Other };
    if id == ids::BOW {
        return Role::Bow;
    }
    match items::tool_shape(id) {
        Some((0, m)) => Role::Pickaxe(m),
        Some((1, m)) => Role::Axe(m),
        Some((2, m)) => Role::Shovel(m),
        Some((3, m)) => Role::Sword(m),
        Some((4, m)) => Role::Hoe(m),
        _ => Role::Other,
    }
}

/// Soft blocks a sword cuts through.
fn sword_cuts(b: Block) -> bool {
    use Block::*;
    b.info().render == RenderKind::Cross
        || matches!(b, OakLeaves | BirchLeaves | SpruceLeaves | AcaciaLeaves | Wool | RedWool | BlueWool | YellowWool | Pumpkin | Melon)
}

/// Blocks a hoe harvests.
fn hoe_cuts(b: Block) -> bool {
    use Block::*;
    b.crop().is_some() || matches!(b, OakLeaves | BirchLeaves | SpruceLeaves | AcaciaLeaves | MossBlock | Farmland)
}

/// Is `b` the tool's own kind of block (fast, no extra wear)?
pub fn fits(role: Role, b: Block) -> bool {
    let tool = b.info().tool;
    match role {
        Role::Pickaxe(_) => tool == ToolKind::Pickaxe,
        Role::Axe(_) => tool == ToolKind::Axe,
        Role::Shovel(_) => tool == ToolKind::Shovel,
        Role::Sword(_) => sword_cuts(b),
        Role::Hoe(_) => hoe_cuts(b),
        Role::Bow | Role::Other => false,
    }
}

/// Mining speed multiplier of the held item on `b` (1 = bare hand).
pub fn speed(id: Option<ItemId>, b: Block) -> f32 {
    let tier = |m: u8| [2.0f32, 4.0, 6.0, 8.0][m as usize & 3];
    let r = role(id);
    if !fits(r, b) {
        return 1.0;
    }
    match r {
        Role::Pickaxe(m) | Role::Axe(m) | Role::Shovel(m) => tier(m),
        // Soft blocks: a sword or hoe cuts them about as fast as a stone tool, whatever its
        // material (they are soft, not hard).
        Role::Sword(_) => 4.0,
        Role::Hoe(m) => tier(m).max(3.0),
        _ => 1.0,
    }
}

/// Can the held item break `b` at all? (Creative swords do not break blocks.)
pub fn may_break(id: Option<ItemId>, creative: bool) -> bool {
    !(creative && matches!(role(id), Role::Sword(_)))
}

/// Wear for breaking `b`: 0 for instant blocks and non-tools, 1 on the tool's own blocks, 2
/// on others.
pub fn wear_for_block(id: Option<ItemId>, b: Block) -> u16 {
    let r = role(id);
    if b.info().hardness <= 0.0 || matches!(r, Role::Bow | Role::Other) {
        return 0;
    }
    if fits(r, b) { 1 } else { 2 }
}

/// Wear for a blow on a creature: swords 1, other tools 2, the bow and plain items 0.
pub fn wear_for_hit(id: Option<ItemId>) -> u16 {
    match role(id) {
        Role::Sword(_) => 1,
        Role::Pickaxe(_) | Role::Axe(_) | Role::Shovel(_) | Role::Hoe(_) => 2,
        Role::Bow | Role::Other => 0,
    }
}

/// The rules of an item, in a line for its tooltip (`None` for non-tools).
pub fn describe(id: ItemId) -> Option<String> {
    const MAT: [&str; 4] = ["дерево", "камень", "железо", "алмаз"];
    let ores = |m: u8| match m {
        0 => "уголь",
        1 => "уголь, железо, медь, олово",
        2 => "все руды, кроме обсидиана",
        _ => "все руды и обсидиан",
    };
    let _ = MAT;
    Some(match role(Some(id)) {
        Role::Pickaxe(m) => format!("Копает: камень, руды, кирпич, металл · добывает: {}", ores(m)),
        Role::Axe(_) => "Рубит: брёвна, доски, деревянное, тыквы · сильный удар".into(),
        Role::Shovel(_) => "Копает: землю, траву, песок, гравий, глину, снег".into(),
        Role::Sword(_) => "Оружие: самый сильный удар · режет листву, растения, шерсть · блоки не копает (износ ×2)".into(),
        Role::Hoe(_) => "Вспахивает землю и траву · быстро собирает урожай, листву, мох".into(),
        Role::Bow => "Только стреляет: держите, чтобы натянуть".into(),
        Role::Other => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_follows_its_rules() {
        let pick = |m| Some(ids::tool(m, 0));
        let axe = Some(ids::tool(0, 1));
        let shovel = Some(ids::tool(0, 2));
        let sword = Some(ids::tool(3, 3));
        let hoe = Some(ids::HOE_BASE);
        // Own blocks are fast, others are bare-hand speed.
        assert!(speed(pick(0), Block::Stone) > 1.0);
        assert_eq!(speed(pick(3), Block::Dirt), 1.0);
        assert!(speed(axe, Block::OakLog) > 1.0);
        assert_eq!(speed(axe, Block::Stone), 1.0);
        assert!(speed(shovel, Block::Sand) > 1.0);
        assert_eq!(speed(shovel, Block::OakLog), 1.0);
        // A sword is a weapon: fast on leaves and plants, a hand on stone, wears double there.
        assert!(speed(sword, Block::OakLeaves) > 1.0);
        assert_eq!(speed(sword, Block::Stone), 1.0);
        assert_eq!(wear_for_block(sword, Block::Stone), 2);
        assert_eq!(wear_for_block(sword, Block::OakLeaves), 1);
        assert!(!may_break(sword, true), "no block breaking with a sword in creative");
        assert!(may_break(sword, false) && may_break(pick(0), true));
        // A hoe harvests crops and leaves fast.
        assert!(speed(hoe, Block::Wheat) > 1.0 && speed(hoe, Block::OakLeaves) > 1.0);
        assert_eq!(speed(hoe, Block::Stone), 1.0);
        // Wrong tool wears double, the right one once, instant blocks never.
        assert_eq!(wear_for_block(pick(0), Block::Stone), 1);
        assert_eq!(wear_for_block(pick(0), Block::Dirt), 2);
        assert_eq!(wear_for_block(pick(0), Block::Torch), 0);
        assert_eq!(wear_for_block(Some(ids::BOW), Block::Stone), 0);
        // Hits: a sword wears once, other tools twice, the bow never.
        assert_eq!(wear_for_hit(sword), 1);
        assert_eq!(wear_for_hit(axe), 2);
        assert_eq!(wear_for_hit(Some(ids::BOW)), 0);
        // Ore tiers: wood takes coal, not iron; stone takes iron, not diamond; iron takes
        // diamond, not obsidian; diamond takes obsidian.
        let harvest = |id: Option<ItemId>, b: Block| items::can_harvest(id.map(items::info), b);
        assert!(harvest(pick(0), Block::CoalOre) && !harvest(pick(0), Block::IronOre));
        assert!(harvest(pick(1), Block::IronOre) && !harvest(pick(1), Block::DiamondOre));
        assert!(harvest(pick(2), Block::DiamondOre) && !harvest(pick(2), Block::Obsidian));
        assert!(harvest(pick(3), Block::Obsidian));
        assert!(!harvest(sword, Block::CoalOre), "a sword mines no ore");
        // Every tool and the bow explain themselves.
        for id in (ids::TOOLS_BASE..=ids::TOOLS_LAST).chain(ids::HOE_BASE..=ids::HOE_LAST).chain([ids::BOW]) {
            assert!(describe(id).is_some_and(|d| !d.is_empty()), "{id}");
        }
        assert!(describe(ids::APPLE).is_none());
    }
}
