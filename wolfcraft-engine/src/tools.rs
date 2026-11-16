//! What each tool can and cannot do. One place for the rules, used by mining, combat and the
//! item tooltips (so what the game does and what it tells the player always agree).
//!
//! * **Кирка** — камень, руды, кирпич, металл, лёд, обсидиан. Its material decides which ores
//!   drop anything: wood — coal; stone — iron, copper, tin; iron — gold, silver, cobalt,
//!   diamond, iskrit, uranium; diamond — obsidian too.
//! * **Топор** — logs, planks and everything wooden, pumpkins and melons. A strong blow.
//! * **Лопата** — dirt, grass, sand, gravel, clay, snow.
//! * **Меч** — the weapon: the strongest blow, wears 1 per hit. Cuts leaves, plants, wool and
//!   cobweb-like soft blocks quickly. It does not break other blocks at all (no digging
//!   holes in a fight), and in creative mode none.
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
    /// WolfTech drill: stone, ores and earth, very fast.
    Drill,
    /// WolfTech chainsaw: wood, leaves and plants, very fast.
    Chainsaw,
    /// WolfTech omni-tool: everything a pickaxe, axe, shovel and sword take, fastest.
    Omni,
    Bow,
    Other,
}

pub fn role(id: Option<ItemId>) -> Role {
    let Some(id) = id else { return Role::Other };
    if id == ids::BOW {
        return Role::Bow;
    }
    if id == ids::DRILL {
        return Role::Drill;
    }
    if id == ids::CHAINSAW {
        return Role::Chainsaw;
    }
    if id == ids::OMNI_TOOL || id == ids::DRACONIC_PICKAXE {
        return Role::Omni;
    }
    // Tinkers' big tools dig and chop like the diamond ones.
    match id {
        ids::TINKER_HAMMER => return Role::Pickaxe(3),
        ids::TINKER_EXCAVATOR => return Role::Shovel(3),
        ids::LUMBER_AXE => return Role::Axe(3),
        _ => {}
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
        || matches!(b, OakLeaves | BirchLeaves | SpruceLeaves | AcaciaLeaves | CherryLeaves | Wool | RedWool | BlueWool | YellowWool | Pumpkin | Melon)
}

/// Blocks a hoe harvests.
fn hoe_cuts(b: Block) -> bool {
    use Block::*;
    b.crop().is_some() || matches!(b, OakLeaves | BirchLeaves | SpruceLeaves | AcaciaLeaves | CherryLeaves | MossBlock | Farmland)
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
        Role::Drill => matches!(tool, ToolKind::Pickaxe | ToolKind::Shovel),
        Role::Chainsaw => tool == ToolKind::Axe || sword_cuts(b),
        Role::Omni => matches!(tool, ToolKind::Pickaxe | ToolKind::Shovel | ToolKind::Axe) || sword_cuts(b),
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
        Role::Drill | Role::Chainsaw => 12.0,
        Role::Omni => 16.0,
        _ => 1.0,
    }
}

/// Can the held item break `b` at all? A sword is a weapon: it only cuts soft blocks
/// (leaves, plants, wool) and never breaks anything in creative.
pub fn may_break(id: Option<ItemId>, b: Block, creative: bool) -> bool {
    match role(id) {
        Role::Sword(_) => !creative && sword_cuts(b),
        _ => true,
    }
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
        Role::Pickaxe(_) | Role::Axe(_) | Role::Shovel(_) | Role::Hoe(_) | Role::Drill | Role::Chainsaw | Role::Omni => 2,
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
        Role::Sword(_) => "Оружие: самый сильный удар · режет листву, растения, шерсть · другие блоки не ломает".into(),
        Role::Hoe(_) => "Вспахивает землю и траву · быстро собирает урожай, листву, мох".into(),
        Role::Bow => "Только стреляет: держите, чтобы натянуть".into(),
        Role::Drill => "WolfTech: камень, руды и землю — очень быстро, все руды · режимы 1×1 … 13×13 (кнопка справа или R) · тратит заряд".into(),
        Role::Chainsaw => "WolfTech: дерево, листву и растения — очень быстро · тратит заряд".into(),
        Role::Omni => "WolfTech: всё — камень, руды, землю, дерево, листву — быстрее всех · тратит заряд".into(),
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
        assert!(!may_break(sword, Block::Stone, false) && !may_break(sword, Block::Dirt, false), "a sword digs nothing");
        assert!(may_break(sword, Block::OakLeaves, false), "but cuts leaves");
        assert_eq!(wear_for_block(sword, Block::OakLeaves), 1);
        assert!(!may_break(sword, Block::OakLeaves, true), "no block breaking with a sword in creative");
        assert!(may_break(pick(0), Block::Stone, true) && may_break(None, Block::Dirt, false));
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

/// Electric drill digging modes: the side of the square dug at once (1×1, 3×3 … 13×13).
pub const DRILL_AREAS: [i32; 6] = [1, 3, 5, 7, 9, 13];

/// The drill's area side for a mode index.
pub fn drill_side(mode: u8) -> i32 {
    DRILL_AREAS[mode as usize % DRILL_AREAS.len()]
}

/// Tools with switchable digging squares (the drill and the draconic pickaxe).
pub fn has_area_modes(id: ItemId) -> bool {
    matches!(id, ids::DRILL | ids::DRACONIC_PICKAXE)
}

/// The square a held tool digs at once: the drill and the draconic pickaxe by their mode,
/// the Tinkers' hammer and excavator 3×3, everything else one block.
pub fn area_side(id: Option<ItemId>, mode: u8) -> i32 {
    match id {
        Some(i) if has_area_modes(i) => drill_side(mode),
        Some(ids::TINKER_HAMMER | ids::TINKER_EXCAVATOR) => 3,
        _ => 1,
    }
}

/// Which blocks the wide dig takes for this tool.
pub fn area_takes(id: ItemId, b: Block) -> bool {
    let tool = b.info().tool;
    match id {
        ids::TINKER_HAMMER => tool == ToolKind::Pickaxe,
        ids::TINKER_EXCAVATOR => tool == ToolKind::Shovel,
        ids::DRACONIC_PICKAXE => matches!(tool, ToolKind::Pickaxe | ToolKind::Shovel | ToolKind::Axe),
        _ => matches!(tool, ToolKind::Pickaxe | ToolKind::Shovel),
    }
}

/// The next drill mode (taps cycle through them).
pub fn next_drill_mode(mode: u8) -> u8 {
    ((mode as usize + 1) % DRILL_AREAS.len()) as u8
}
