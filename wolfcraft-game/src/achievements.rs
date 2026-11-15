//! Achievements in the spirit of Minecraft's, along WolfCraft's own path: from the first log
//! to WolfTech, the reactor, dragons, Hell and the guardians. Each is granted once per world
//! and pops a golden toast on the left of the screen.

use wolfcraft_engine::block::Block;
use wolfcraft_engine::items::{ItemId, ids};

pub struct Achievement {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    /// Item shown on the toast.
    pub icon: ItemId,
}

const fn b(x: Block) -> ItemId {
    x as ItemId
}

pub const ALL: &[Achievement] = &[
    Achievement { id: "wood", name: "Первое бревно", desc: "Срубить дерево", icon: b(Block::OakLog) },
    Achievement { id: "bench", name: "Мастерская", desc: "Сделать верстак", icon: b(Block::CraftingTable) },
    Achievement { id: "pick", name: "Время копать", desc: "Сделать кирку", icon: ids::tool(0, 0) },
    Achievement { id: "stone", name: "Каменный век", desc: "Сделать каменную кирку", icon: ids::tool(1, 0) },
    Achievement { id: "iron", name: "Железный век", desc: "Выплавить железный слиток", icon: ids::IRON_INGOT },
    Achievement { id: "diamond", name: "Алмазы!", desc: "Найти алмаз", icon: ids::DIAMOND },
    Achievement { id: "hunter", name: "Охотник", desc: "Победить существо", icon: ids::tool(0, 3) },
    Achievement { id: "builder", name: "Архитектор", desc: "Собрать Конструктор", icon: b(Block::BuilderTable) },
    Achievement { id: "circuit", name: "Инженер", desc: "Собрать микросхему WolfTech", icon: ids::CIRCUIT },
    Achievement { id: "power", name: "Да будет ток", desc: "Поставить генератор или солнечную панель", icon: b(Block::SolarPanel) },
    Achievement { id: "dust", name: "Дробь и пыль", desc: "Получить пыль из дробителя или обогатителя", icon: ids::IRON_DUST },
    Achievement { id: "overclock", name: "Разгон", desc: "Сделать ускоритель", icon: ids::OVERCLOCKER },
    Achievement { id: "jetpack", name: "Ракетчик", desc: "Взлететь на реактивном ранце", icon: ids::JETPACK },
    Achievement { id: "nano", name: "Нано-костюм", desc: "Надеть нано-нагрудник", icon: ids::NANO_BASE + 1 },
    Achievement { id: "plasma", name: "Плазма", desc: "Сделать плазменную винтовку или рельсотрон", icon: ids::PLASMA_RIFLE },
    Achievement { id: "atom", name: "Мирный атом", desc: "Поставить ядерный реактор", icon: b(Block::Reactor) },
    Achievement { id: "meltdown", name: "Это было ожидаемо", desc: "Перегреть реактор", icon: ids::URANIUM_ROD },
    Achievement { id: "dragon", name: "Повелитель драконов", desc: "Оседлать дракона", icon: ids::SADDLE },
    Achievement { id: "hell", name: "В пекло", desc: "Попасть в Ад", icon: b(Block::Obsidian) },
    Achievement { id: "boss", name: "Гроза стражей", desc: "Победить стража", icon: ids::RITUAL_SEAL },
];

pub fn find(id: &str) -> Option<(usize, &'static Achievement)> {
    ALL.iter().enumerate().find(|(_, a)| a.id == id)
}

/// Achievements that holding an item in the inventory grants.
pub fn for_item(id: ItemId) -> Option<&'static str> {
    Some(match id {
        x if x == b(Block::OakLog) || x == b(Block::BirchLog) || x == b(Block::SpruceLog) || x == b(Block::AcaciaLog) => "wood",
        x if x == b(Block::CraftingTable) => "bench",
        x if x == ids::tool(0, 0) => "pick",
        x if x == ids::tool(1, 0) => "stone",
        ids::IRON_INGOT => "iron",
        ids::DIAMOND => "diamond",
        x if x == b(Block::BuilderTable) => "builder",
        ids::CIRCUIT => "circuit",
        ids::IRON_DUST | ids::GOLD_DUST | ids::COPPER_DUST | ids::TIN_DUST | ids::SILVER_DUST => "dust",
        ids::OVERCLOCKER => "overclock",
        ids::PLASMA_RIFLE | ids::RAILGUN => "plasma",
        _ => return None,
    })
}

/// Achievements that placing a block grants.
pub fn for_placed(block: Block) -> Option<&'static str> {
    match wolfcraft_engine::wolftech::kind(block) {
        Some(wolfcraft_engine::wolftech::Kind::Reactor) => Some("atom"),
        Some(k) if wolfcraft_engine::wolftech::is_source(k) => Some("power"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_items_point_at_real_ones() {
        for (i, a) in ALL.iter().enumerate() {
            assert!(ALL.iter().skip(i + 1).all(|o| o.id != a.id), "{} twice", a.id);
        }
        for id in [ids::DIAMOND, ids::CIRCUIT, b(Block::OakLog)] {
            assert!(find(for_item(id).unwrap()).is_some());
        }
    }
}
