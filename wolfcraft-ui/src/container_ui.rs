//! Container window (chest, electric furnace): block slots on top, the player's slots below.
//! Tap a slot to move its whole stack to the other side; drag a stack onto any slot to put it
//! exactly there (merging or swapping), or out of the window to throw it away.

use crate::gui_renderer::GuiIcons;
use crate::theme_manager::palette;
use crate::ui_components::progress_bar;
use egui::{Align2, Color32, FontId, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use wolfcraft_engine::inventory::Inventory;
use wolfcraft_engine::items::ItemStack;

const CELL: f32 = 44.0;
const GAP: f32 = 4.0;

/// What the window shows besides the slots.
pub struct ContainerView<'a> {
    pub title: &'a str,
    /// Slots per row for the container grid.
    pub columns: usize,
    /// Which container slots accept items from the player (e.g. only the furnace input).
    pub accepts: &'a dyn Fn(usize, &ItemStack) -> bool,
    /// Optional status line with progress 0..1 (furnace).
    pub status: Option<(&'a str, f32)>,
}

/// A slot of the window: the container's or the player's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Src {
    Box(usize),
    Bag(usize),
}

fn paint_slot(ui: &mut Ui, icons: &GuiIcons, rect: Rect, stack: Option<&ItemStack>, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let hover = resp.hovered() && enabled;
    ui.painter().rect_filled(rect, Rounding::ZERO, if hover { p.surface_hover } else { p.surface_hi });
    ui.painter().rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, if hover { p.accent } else { p.outline }));
    if let Some(s) = stack {
        icons.paint_item(ui.painter(), rect.shrink(6.0), s.id);
        if s.count > 1 {
            ui.painter().text(rect.right_bottom() - Vec2::new(4.0, 2.0), Align2::RIGHT_BOTTOM, s.count.to_string(), FontId::monospace(13.0), Color32::WHITE);
        }
        if resp.hovered() {
            resp.clone().on_hover_text(wolfcraft_engine::items::info(s.id).name);
        }
    }
    resp
}

fn move_into(slots: &mut [Option<ItemStack>], mut s: ItemStack, allowed: impl Fn(usize) -> bool) -> Option<ItemStack> {
    // Merge into matching stacks first, then into empty slots.
    for (i, slot) in slots.iter_mut().enumerate() {
        if !allowed(i) {
            continue;
        }
        if let Some(t) = slot {
            if t.can_stack_with(&s) {
                let room = t.info().max_stack.saturating_sub(t.count);
                let n = room.min(s.count);
                t.count += n;
                s.count -= n;
                if s.count == 0 {
                    return None;
                }
            }
        }
    }
    for (i, slot) in slots.iter_mut().enumerate() {
        if allowed(i) && slot.is_none() {
            *slot = Some(s);
            return None;
        }
    }
    Some(s)
}

/// Drops the dragged stack `from` onto `to`: merge into the same item, otherwise swap (only
/// into container slots that accept it).
fn drop_on(slots: &mut [Option<ItemStack>], inv: &mut Inventory, from: Src, to: Src, accepts: &dyn Fn(usize, &ItemStack) -> bool) {
    if from == to {
        return;
    }
    let get = |slots: &[Option<ItemStack>], inv: &Inventory, s: Src| match s {
        Src::Box(i) => slots.get(i).copied().flatten(),
        Src::Bag(i) => inv.slots.get(i).copied().flatten(),
    };
    let Some(a) = get(slots, inv, from) else { return };
    let b = get(slots, inv, to);
    if let Src::Box(i) = to {
        if !accepts(i, &a) {
            return;
        }
    }
    if let (Src::Box(i), Some(b)) = (from, b) {
        // A swap puts the other stack into the container too.
        if !accepts(i, &b) && !b.can_stack_with(&a) {
            return;
        }
    }
    let (na, nb) = match b {
        Some(b) if b.can_stack_with(&a) => {
            let n = (b.info().max_stack.saturating_sub(b.count)).min(a.count);
            let rest = a.count - n;
            ((rest > 0).then_some(ItemStack { count: rest, ..a }), Some(ItemStack { count: b.count + n, ..b }))
        }
        other => (other, Some(a)),
    };
    let mut put = |s: Src, v: Option<ItemStack>| match s {
        Src::Box(i) => slots[i] = v,
        Src::Bag(i) => inv.slots[i] = v,
    };
    put(from, na);
    put(to, nb);
}

/// Shows the window. Returns (close, stacks thrown out of the window).
pub fn show(ctx: &egui::Context, icons: &GuiIcons, view: &ContainerView, slots: &mut [Option<ItemStack>], inv: &mut Inventory) -> (bool, Vec<ItemStack>) {
    let p = palette(ctx);
    let mut close = false;
    let drag_id = egui::Id::new("container-drag");
    let mut drag: Option<Src> = ctx.data(|d| d.get_temp(drag_id)).flatten();
    let mut rects: Vec<(Src, Rect)> = Vec::new();
    let mut thrown = Vec::new();
    let cols = view.columns.max(1);
    let width = cols.max(9) as f32 * (CELL + GAP) - GAP;
    let dismissed = crate::ui_components::game_screen(ctx, "container", view.title, width + 4.0, |ui| {
        // Container slots.
        let rows = slots.len().div_ceil(cols);
        let (area, _) = ui.allocate_exact_size(Vec2::new(width, rows as f32 * (CELL + GAP) - GAP), Sense::hover());
        let grid_w = cols as f32 * (CELL + GAP) - GAP;
        let x0 = area.left() + (width - grid_w) / 2.0;
        let mut to_player: Option<usize> = None;
        for i in 0..slots.len() {
            let (r, c) = (i / cols, i % cols);
            let rect = Rect::from_min_size(egui::pos2(x0 + c as f32 * (CELL + GAP), area.top() + r as f32 * (CELL + GAP)), Vec2::splat(CELL));
            rects.push((Src::Box(i), rect));
            let shown = if drag == Some(Src::Box(i)) { None } else { slots[i].as_ref() };
            let resp = paint_slot(ui, icons, rect, shown, true);
            if resp.drag_started() && slots[i].is_some() {
                drag = Some(Src::Box(i));
            } else if resp.clicked() && slots[i].is_some() {
                to_player = Some(i);
            }
        }
        if let Some(i) = to_player {
            if let Some(s) = slots[i].take() {
                slots[i] = inv.add(s);
            }
        }
        if let Some((text, f)) = view.status {
            ui.add_space(8.0);
            progress_bar(ui, f, text, width);
        }
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Инвентарь").font(FontId::proportional(13.0)).color(p.text_muted));
        ui.add_space(4.0);
        // Player slots: main grid on top, hotbar below.
        let (area, _) = ui.allocate_exact_size(Vec2::new(width, 4.0 * (CELL + GAP) + 4.0), Sense::hover());
        let mut from_player: Option<usize> = None;
        for i in 0..36usize {
            // 0..9 is the hotbar: draw it on the bottom row.
            let (row, col) = if i < 9 { (3, i) } else { ((i - 9) / 9, (i - 9) % 9) };
            let y = area.top() + row as f32 * (CELL + GAP) + if row == 3 { 4.0 } else { 0.0 };
            let rect = Rect::from_min_size(egui::pos2(area.left() + col as f32 * (CELL + GAP), y), Vec2::splat(CELL));
            rects.push((Src::Bag(i), rect));
            let shown = if drag == Some(Src::Bag(i)) { None } else { inv.slots[i].as_ref() };
            let resp = paint_slot(ui, icons, rect, shown, true);
            if resp.drag_started() && inv.slots[i].is_some() {
                drag = Some(Src::Bag(i));
            } else if resp.clicked() && inv.slots[i].is_some() {
                from_player = Some(i);
            }
        }
        if let Some(i) = from_player {
            if let Some(s) = inv.slots[i].take() {
                let accepts = view.accepts;
                let probe = s;
                let rest = move_into(slots, s, |slot| accepts(slot, &probe));
                inv.slots[i] = rest;
            }
        }
    });
    // Drag & drop: a ghost of the stack follows the finger; on release it lands on the slot
    // under it, or is thrown away when released outside the window.
    let pointer = ctx.pointer_latest_pos();
    if let Some(src) = drag {
        let stack = match src {
            Src::Box(i) => slots.get(i).copied().flatten(),
            Src::Bag(i) => inv.slots.get(i).copied().flatten(),
        };
        if let (Some(s), Some(pos)) = (stack, pointer) {
            let layer = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("container-ghost")));
            let ghost = Rect::from_center_size(pos, Vec2::splat(CELL));
            layer.rect_filled(ghost, Rounding::ZERO, p.surface.gamma_multiply(0.9));
            icons.paint_item(&layer, ghost.shrink(6.0), s.id);
        }
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(pos) = pointer {
                if let Some((dst, _)) = rects.iter().find(|(_, r)| r.contains(pos)) {
                    drop_on(slots, inv, src, *dst, view.accepts);
                } else {
                    let area = rects.iter().fold(Rect::NOTHING, |a, (_, r)| a.union(*r)).expand(36.0);
                    if !area.contains(pos) {
                        let taken = match src {
                            Src::Box(i) => slots[i].take(),
                            Src::Bag(i) => inv.slots[i].take(),
                        };
                        thrown.extend(taken);
                    }
                }
            }
            drag = None;
        }
    }
    ctx.data_mut(|d| d.insert_temp(drag_id, drag));
    if close || dismissed {
        ctx.data_mut(|d| d.insert_temp::<Option<Src>>(drag_id, None));
    }
    (close || dismissed, thrown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_merges_swaps_and_respects_what_the_container_accepts() {
        let mut slots = vec![None; 3];
        let mut inv = Inventory::default();
        let dirt = ItemStack::new(2, 10);
        let sand = ItemStack::new(5, 4);
        inv.slots[0] = Some(dirt);
        slots[0] = Some(ItemStack::new(2, 60));
        let any = |_: usize, _: &ItemStack| true;
        // Merge up to the stack limit, the rest stays.
        drop_on(&mut slots, &mut inv, Src::Bag(0), Src::Box(0), &any);
        assert_eq!(slots[0].unwrap().count, 64);
        assert_eq!(inv.slots[0].unwrap().count, 6);
        // Swap different items.
        slots[1] = Some(sand);
        drop_on(&mut slots, &mut inv, Src::Box(1), Src::Bag(0), &any);
        assert_eq!(inv.slots[0].unwrap().id, 5);
        assert_eq!(slots[1].unwrap().id, 2);
        // A furnace slot that only takes sand refuses dirt.
        let only_sand = |_: usize, s: &ItemStack| s.id == 5;
        inv.slots[1] = Some(ItemStack::new(2, 1));
        drop_on(&mut slots, &mut inv, Src::Bag(1), Src::Box(2), &only_sand);
        assert!(slots[2].is_none() && inv.slots[1].is_some());
    }
}
