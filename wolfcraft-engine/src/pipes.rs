//! WolfTech item pipes (ideas from Thermal Dynamics and Ender IO conduits). A pipe network
//! joins inventories: machines push what they made into it, an extracting pipe also pulls
//! from the chest it touches; items go to machines that can work them, then to chests.

use crate::block::Block;
use crate::items::{ItemId, ItemStack};
use crate::wolftech::{self as wt, Kind};

pub fn is_pipe(b: Block) -> bool {
    matches!(b, Block::ItemPipe | Block::PipeExtractor)
}

/// What a pipe can be attached to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Port {
    /// A chest, barrel or storage furniture.
    Store,
    /// A WolfTech machine or fuel generator.
    Machine(Kind),
    /// The electric furnace.
    Furnace,
}

pub fn port(b: Block) -> Option<Port> {
    let b = b.base_kind();
    if let Some(k) = wt::kind(b) {
        return (wt::is_machine(k) || k == Kind::Generator || k == Kind::Magma).then_some(Port::Machine(k));
    }
    if b == Block::ElectricFurnace {
        return Some(Port::Furnace);
    }
    (matches!(b, Block::Chest | Block::Battery) || crate::construction::is_container(b)).then_some(Port::Store)
}

/// Pipes join pipes and anything with a port (the mesher draws an arm towards it).
pub fn joins(b: Block) -> bool {
    is_pipe(b) || port(b).is_some()
}

/// Slots items are taken from: machine and furnace outputs; every slot of a store, but only
/// through an extracting pipe.
pub fn outputs(p: Port, extract: bool, len: usize) -> std::ops::Range<usize> {
    match p {
        Port::Machine(k) if wt::is_machine(k) => 2..3,
        Port::Furnace => 2..3,
        Port::Store if extract => 0..len,
        _ => 0..0,
    }
}

/// Slots of `p` this item may go into, best first.
pub fn inputs(p: Port, id: ItemId, len: usize) -> Vec<usize> {
    match p {
        Port::Machine(k) if wt::is_machine(k) => {
            if wt::recipe(k, id).is_some() && !wt::keeps(k) { vec![0] } else { vec![] }
        }
        Port::Machine(k) => {
            if wt::fuel_for(k, id) > 0.0 { vec![1] } else { vec![] }
        }
        Port::Furnace => {
            if crate::interaction::smelt_result(id).is_some() {
                vec![0]
            } else if crate::interaction::fuel_time(id) > 0.0 {
                vec![1]
            } else {
                vec![]
            }
        }
        Port::Store => (0..len).collect(),
    }
}

/// Puts up to `n` of `st` into slot `i`, returns how many went in.
pub fn insert(slots: &mut [Option<ItemStack>], i: usize, st: ItemStack, n: u8) -> u8 {
    let max = st.info().max_stack.max(1);
    match slots.get_mut(i) {
        Some(s @ None) => {
            let k = n.min(max);
            *s = Some(ItemStack { count: k, ..st });
            k
        }
        Some(Some(s)) if s.id == st.id && s.damage == st.damage && s.count < max => {
            let k = n.min(max - s.count);
            s.count += k;
            k
        }
        _ => 0,
    }
}

/// Stores are filled in order: first a slot with the same item, then an empty one.
pub fn order(slots: &[Option<ItemStack>], cand: Vec<usize>, id: ItemId) -> Vec<usize> {
    let (mut same, mut empty): (Vec<usize>, Vec<usize>) = (vec![], vec![]);
    for i in cand {
        match slots.get(i) {
            Some(Some(s)) if s.id == id => same.push(i),
            Some(None) => empty.push(i),
            _ => {}
        }
    }
    same.extend(empty);
    same
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::ids;

    #[test]
    fn pipes_feed_ore_to_crushers_and_fuel_to_generators() {
        assert_eq!(port(Block::Crusher), Some(Port::Machine(Kind::Crusher)));
        assert_eq!(inputs(Port::Machine(Kind::Crusher), ids::RAW_IRON, 7), vec![0]);
        assert_eq!(inputs(Port::Machine(Kind::Generator), ids::COAL, 3), vec![1]);
        assert!(outputs(Port::Store, false, 27).is_empty() && outputs(Port::Store, true, 27).len() == 27);
        assert!(joins(Block::ItemPipe) && joins(Block::Chest) && !joins(Block::Stone));
        let mut v = vec![None; 3];
        assert_eq!(insert(&mut v, 0, ItemStack::new(ids::COAL, 1), 5), 5);
        assert_eq!(v[0].unwrap().count, 5);
    }
}
