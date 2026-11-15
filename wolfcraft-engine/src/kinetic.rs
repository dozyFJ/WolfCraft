//! «WolfCraft: Механика» — rotation power in the spirit of the Create mod. Water wheels and
//! hand cranks turn shafts; shafts and cogwheels carry the turning through the world (you can
//! see them spin); machines — the mechanical press and the millstone — work while they turn.
//! Every source gives so much stress capacity (SU), every machine takes its share: overload
//! the network and it stops.

use crate::block::Block;
use crate::items::{ItemId, ids};

/// A kinetic part. `axis`: 0 = i, 1 = j, 2 = k (up).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Shaft(u8),
    Cog(u8),
    Wheel(u8),
    Crank,
    Press,
    Millstone,
}

pub fn part(b: Block) -> Option<Part> {
    use Block::*;
    Some(match b {
        Shaft => Part::Shaft(0),
        ShaftJ => Part::Shaft(1),
        ShaftK => Part::Shaft(2),
        Cogwheel => Part::Cog(0),
        CogwheelJ => Part::Cog(1),
        CogwheelK => Part::Cog(2),
        WaterWheel => Part::Wheel(0),
        WaterWheelJ => Part::Wheel(1),
        HandCrank => Part::Crank,
        MechPress => Part::Press,
        Millstone => Part::Millstone,
        _ => return None,
    })
}

/// The block of a shaft / cogwheel / water wheel turned to `axis`.
pub fn with_axis(b: Block, axis: u8) -> Block {
    use Block::*;
    match (part(b), axis) {
        (Some(Part::Shaft(_)), 0) => Shaft,
        (Some(Part::Shaft(_)), 1) => ShaftJ,
        (Some(Part::Shaft(_)), _) => ShaftK,
        (Some(Part::Cog(_)), 0) => Cogwheel,
        (Some(Part::Cog(_)), 1) => CogwheelJ,
        (Some(Part::Cog(_)), _) => CogwheelK,
        (Some(Part::Wheel(_)), 1) => WaterWheelJ,
        (Some(Part::Wheel(_)), _) => WaterWheel,
        _ => b,
    }
}

/// The turning axis of a part (machines turn with whatever drives them).
pub fn axis(p: Part) -> Option<u8> {
    match p {
        Part::Shaft(a) | Part::Cog(a) | Part::Wheel(a) => Some(a),
        Part::Crank => Some(2),
        Part::Press | Part::Millstone => None,
    }
}

pub fn is_machine(p: Part) -> bool {
    matches!(p, Part::Press | Part::Millstone)
}

/// Drawn as a spinning model (not as a block mesh).
pub fn spins(b: Block) -> bool {
    part(b).is_some_and(|p| !is_machine(p))
}

/// Stress a machine takes while it turns (SU).
pub fn load(p: Part) -> f32 {
    match p {
        Part::Press => 128.0,
        Part::Millstone => 64.0,
        _ => 0.0,
    }
}

/// Do two neighbouring parts turn together? `dir` = (axis, sign) from `a` to `b`. Returns
/// Some(flip): `true` when the rotation reverses (cogs meshing side by side).
pub fn link(a: Part, b: Part, dir: (u8, i8)) -> Option<bool> {
    let along = |p: Part| match p {
        // A crank only drives what is under it.
        Part::Crank => None,
        _ => axis(p),
    };
    let (ax, sign) = dir;
    let a_axis = if a == Part::Crank { if ax == 2 && sign < 0 { Some(2) } else { None } } else { along(a) };
    let b_axis = if b == Part::Crank { if ax == 2 && sign > 0 { Some(2) } else { None } } else { along(b) };
    match (is_machine(a), is_machine(b)) {
        (true, true) => None,
        (true, false) => (b_axis == Some(ax)).then_some(false),
        (false, true) => (a_axis == Some(ax)).then_some(false),
        (false, false) => {
            if a_axis == Some(ax) && b_axis == Some(ax) {
                Some(false)
            } else if let (Part::Cog(x), Part::Cog(y)) = (a, b) {
                (x == y && x != ax).then_some(true)
            } else {
                None
            }
        }
    }
}

/// What a machine makes from `input` (output, how many).
pub fn recipe(p: Part, input: ItemId) -> Option<(ItemId, u8)> {
    let b = |x: Block| x as ItemId;
    match p {
        Part::Press => Some(match input {
            ids::IRON_INGOT => (ids::IRON_PLATE, 1),
            ids::COPPER_INGOT => (ids::COPPER_PLATE, 1),
            ids::GOLD_INGOT => (ids::GOLD_PLATE, 1),
            x if x == b(Block::Sand) => (b(Block::Sandstone), 1),
            _ => return None,
        }),
        Part::Millstone => Some(match input {
            x if x == b(Block::Cobblestone) => (b(Block::Gravel), 1),
            x if x == b(Block::Gravel) => (b(Block::Sand), 1),
            ids::BONE => (ids::BONE_MEAL, 3),
            x if x == b(Block::CoalOre) => (ids::COAL, 2),
            ids::WHEAT => (ids::WHEAT_SEEDS, 2),
            _ => return None,
        }),
        _ => None,
    }
}

pub fn name(p: Part) -> &'static str {
    match p {
        Part::Shaft(_) => "Вал",
        Part::Cog(_) => "Шестерня",
        Part::Wheel(_) => "Водяное колесо",
        Part::Crank => "Ручной кривошип",
        Part::Press => "Механический пресс",
        Part::Millstone => "Жернова",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shafts_join_along_their_axis_and_cogs_mesh_side_by_side() {
        assert_eq!(link(Part::Shaft(0), Part::Shaft(0), (0, 1)), Some(false));
        assert_eq!(link(Part::Shaft(0), Part::Shaft(0), (1, 1)), None, "side by side shafts do not touch");
        assert_eq!(link(Part::Cog(2), Part::Cog(2), (0, 1)), Some(true), "cogs mesh and turn the other way");
        assert_eq!(link(Part::Shaft(1), Part::Press, (1, -1)), Some(false));
        assert_eq!(link(Part::Crank, Part::Shaft(2), (2, -1)), Some(false), "a crank drives the shaft under it");
        assert_eq!(link(Part::Crank, Part::Shaft(2), (2, 1)), None);
        assert_eq!(recipe(Part::Press, ids::IRON_INGOT), Some((ids::IRON_PLATE, 1)));
    }
}
