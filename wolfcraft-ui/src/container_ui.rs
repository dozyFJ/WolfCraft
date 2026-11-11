//! Container window (chest, electric furnace): block slots on top, the player's slots below.
//! Click a slot to move its whole stack to the other side.

use crate::gui_renderer::GuiIcons;
use crate::theme_manager::{bold_font, palette};
use crate::ui_components::{ButtonKind, icon_button, progress_bar};
use egui::{Align2, Color32, FontId, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use wolfcraft_assets::gui as icon;
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

fn paint_slot(ui: &mut Ui, icons: &GuiIcons, rect: Rect, stack: Option<&ItemStack>, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let resp = ui.allocate_rect(rect, Sense::click());
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

/// Shows the window. Returns true when it should close.
pub fn show(ctx: &egui::Context, icons: &GuiIcons, view: &ContainerView, slots: &mut [Option<ItemStack>], inv: &mut Inventory) -> bool {
    let p = palette(ctx);
    let mut close = false;
    let cols = view.columns.max(1);
    let width = cols.max(9) as f32 * (CELL + GAP) - GAP;
    let dismissed = crate::ui_components::game_window(ctx, "container", width + 4.0, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(view.title).font(bold_font(20.0)).color(p.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, icons, icon::CLOSE, 30.0, ButtonKind::Ghost).clicked() {
                    close = true;
                }
            });
        });
        ui.add_space(8.0);
        // Container slots.
        let rows = slots.len().div_ceil(cols);
        let (area, _) = ui.allocate_exact_size(Vec2::new(width, rows as f32 * (CELL + GAP) - GAP), Sense::hover());
        let grid_w = cols as f32 * (CELL + GAP) - GAP;
        let x0 = area.left() + (width - grid_w) / 2.0;
        let mut to_player: Option<usize> = None;
        for i in 0..slots.len() {
            let (r, c) = (i / cols, i % cols);
            let rect = Rect::from_min_size(egui::pos2(x0 + c as f32 * (CELL + GAP), area.top() + r as f32 * (CELL + GAP)), Vec2::splat(CELL));
            if paint_slot(ui, icons, rect, slots[i].as_ref(), true).clicked() && slots[i].is_some() {
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
            if paint_slot(ui, icons, rect, inv.slots[i].as_ref(), true).clicked() && inv.slots[i].is_some() {
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
    close || dismissed
}
