//! Inventory screen: header bar (title + close), category navbar, inventory slots of the
//! selected category, and on the right the creative palette or the survival crafting grid
//! (2×2 by hand, 3×3 at a crafting table).

use crate::gui_renderer::GuiIcons;
use crate::hud::draw_stack;
use crate::theme_manager::{ANIM_TIME, Palette, RADIUS_MD, RADIUS_SM, bold_font, palette};
use crate::ui_components::{ButtonKind, divider, modal, pill_button, section_label, text_input};
use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use wolfcraft_assets::gui as icon;
use wolfcraft_engine::inventory::{CraftGrid, HOTBAR, Inventory, SLOTS};
use wolfcraft_engine::items::{self, ItemId, ItemStack};

const CELL: f32 = 46.0;
const GAP: f32 = 5.0;
/// Drag ids: 0..SLOTS = inventory, OFFHAND = second hand, GRID.. = crafting cells.
const GRID: usize = 100;
const OFFHAND: usize = 99;

const TABS: [&str; 8] = ["Все", "Природа", "Дерево", "Строительство", "Руды", "Растения", "Предметы", "Инструменты"];
/// Creative navbar: item categories plus the backpack as the last tab.
const CREATIVE_TABS: [&str; 9] = ["Все", "Природа", "Дерево", "Строительство", "Руды", "Растения", "Предметы", "Инструменты", "Рюкзак"];
const BACKPACK_TAB: usize = 8;
/// Width of the creative category column.
const SIDE_W: f32 = 170.0;


#[derive(Default)]
pub struct InventoryUiState {
    pub tab: usize,
    pub search: String,
    pub grid: CraftGrid,
    /// Slot being dragged (inventory or grid id).
    drag_slot: Option<usize>,
    /// Item being dragged from the creative palette.
    drag_item: Option<ItemId>,
    /// Message from the last craft ("Создано: ...").
    pub toast: Option<(String, f32)>,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct InventoryResult {
    pub close: bool,
    pub crafted: Option<String>,
}

/// Call when the inventory closes: items left in the crafting grid go back to the inventory.
pub fn close(state: &mut InventoryUiState, inv: &mut Inventory) {
    state.grid.return_all(inv);
    state.drag_slot = None;
    state.drag_item = None;
}

/// Draws one slot and returns its response. `rect` is where the slot sits.
fn slot_widget(ui: &mut Ui, icons: &GuiIcons, p: &Palette, rect: Rect, stack: Option<&ItemStack>, selected: bool, id: Id, sense: Sense) -> egui::Response {
    let resp = ui.interact(rect, id, sense);
    let hv = ui.ctx().animate_bool_with_time(id.with("hv"), resp.hovered(), ANIM_TIME);
    let fill = p.surface_hi.lerp_to_gamma(p.surface_hover, hv);
    ui.painter().rect_filled(rect, Rounding::ZERO, fill);
    ui.painter().rect_stroke(rect, Rounding::ZERO, Stroke::new(if selected { 2.0_f32 } else { 1.0 }, if selected { p.accent } else { p.outline.lerp_to_gamma(p.accent, hv * 0.8) }));
    if let Some(s) = stack {
        draw_stack(ui.painter(), icons, rect, s, p);
    }
    resp
}

fn matches_search(id: ItemId, search: &str) -> bool {
    search.trim().is_empty() || items::info(id).name.to_lowercase().contains(&search.trim().to_lowercase())
}

fn in_tab(id: ItemId, tab: usize) -> bool {
    tab == 0 || tab >= TABS.len() || items::category(id) == TABS[tab]
}

fn get(inv: &Inventory, grid: &CraftGrid, slot: usize) -> Option<ItemStack> {
    if slot >= GRID {
        grid.cells[slot - GRID]
    } else if slot == OFFHAND {
        inv.offhand
    } else {
        inv.slots[slot]
    }
}

fn set(inv: &mut Inventory, grid: &mut CraftGrid, slot: usize, v: Option<ItemStack>) {
    if slot >= GRID {
        grid.cells[slot - GRID] = v
    } else if slot == OFFHAND {
        inv.offhand = v
    } else {
        inv.slots[slot] = v
    }
}

/// Drag & drop between any two slots (inventory or grid): merge equal stacks, else swap.
fn move_any(inv: &mut Inventory, grid: &mut CraftGrid, a: usize, b: usize) {
    if a == b {
        return;
    }
    match (get(inv, grid, a), get(inv, grid, b)) {
        (Some(x), Some(y)) if x.can_stack_with(&y) => {
            let n = (y.info().max_stack - y.count).min(x.count);
            set(inv, grid, b, Some(ItemStack { count: y.count + n, ..y }));
            set(inv, grid, a, if x.count > n { Some(ItemStack { count: x.count - n, ..x }) } else { None });
        }
        (x, y) => {
            set(inv, grid, a, y);
            set(inv, grid, b, x);
        }
    }
}

/// Shows the inventory. `near_table` turns the crafting grid into 3×3.
pub fn show(ctx: &egui::Context, icons: &GuiIcons, state: &mut InventoryUiState, inv: &mut Inventory, creative: bool, near_table: bool, dt: f32) -> InventoryResult {
    let p = palette(ctx);
    let mut result = InventoryResult::default();
    let mut slot_rects: Vec<(usize, Rect)> = Vec::new();
    let mut trash_rect: Option<Rect> = None;
    if !creative {
        let want = if near_table { 3 } else { 2 };
        if state.grid.size != want {
            state.grid.set_size(want, inv);
        }
    }

    let left_w = 9.0 * (CELL + GAP) - GAP;
    let right_w = if creative { 9.0 * (CELL + GAP) - GAP } else { 3.0 * (CELL + GAP) + 140.0 };
    let _ = right_w;
    // Creative: category column on the left, palette (9 columns) on the right.
    let width = if creative { SIDE_W + 18.0 + left_w + 8.0 } else { left_w + 8.0 };
    let dismissed = modal(ctx, "inventory", "", width.min(ctx.screen_rect().width() - 40.0), |ui| {
        // ---- Header bar: title + close ----
        ui.horizontal(|ui| {
            let (ir, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
            icons.paint(ui.painter(), ir, icon::BACKPACK, p.accent);
            ui.add_space(6.0);
            ui.label(egui::RichText::new(if creative { "Творческий инвентарь" } else { "Инвентарь" }).font(bold_font(20.0)).color(p.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if pill_button(ui, icons, Some(icon::CLOSE), "Закрыть", ButtonKind::Secondary, 0.0, true).clicked() {
                    result.close = true;
                }
            });
        });
        ui.add_space(8.0);
        divider(ui);
        ui.add_space(6.0);
        // ---- Category navbar ----
        if !creative {
            state.tab = 0;
        }

        if !creative {
            result.crafted = survival_layout(ui, icons, &p, state, inv, near_table, left_w, &mut slot_rects);
            return;
        }
        ui.set_width(width);
        ui.horizontal_top(|ui| {
            // ---- Categories, top to bottom ----
            ui.vertical(|ui| {
                ui.set_width(SIDE_W);
                category_column(ui, &p, &mut state.tab);
            });
            ui.add_space(18.0);
            // ---- Palette or backpack, then the hotbar and the bin ----
            ui.vertical(|ui| {
                ui.set_width(left_w);
                if state.tab == BACKPACK_TAB {
                    section_label(ui, "Рюкзак");
                    ui.add_space(4.0);
                    let backpack: Vec<usize> = (HOTBAR..SLOTS).collect();
                    for row in backpack.chunks(9) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::splat(GAP);
                            for &slot in row {
                                let (rect, _) = ui.allocate_exact_size(Vec2::splat(CELL), Sense::hover());
                                slot_rects.push((slot, rect));
                                draw_slot(ui, icons, &p, state, inv, slot, rect);
                            }
                        });
                    }
                } else {
                    creative_panel(ui, icons, &p, state, inv);
                }
                ui.add_space(10.0);
                section_label(ui, "Панель быстрого доступа");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(GAP);
                    // Stretch the nine slots over the full panel width, like the bin below.
                    let cell = ((left_w - GAP * (HOTBAR as f32 - 1.0)) / HOTBAR as f32).max(CELL);
                    for slot in 0..HOTBAR {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(cell), Sense::hover());
                        slot_rects.push((slot, rect));
                        draw_slot(ui, icons, &p, state, inv, slot, rect);
                    }
                });
                ui.add_space(10.0);
                let (r, _) = ui.allocate_exact_size(Vec2::new(left_w, 40.0), Sense::hover());
                let hv = ctx.animate_bool_with_time(Id::new("trash-hv"), state.drag_slot.is_some(), ANIM_TIME);
                ui.painter().rect_filled(r, Rounding::ZERO, p.danger.gamma_multiply(0.10 + 0.2 * hv));
                ui.painter().rect_stroke(r, Rounding::ZERO, Stroke::new(2.0_f32, p.danger.gamma_multiply(0.6)));
                icons.paint(ui.painter(), Rect::from_center_size(Pos2::new(r.center().x - 120.0, r.center().y), Vec2::splat(20.0)), icon::TRASH, p.danger);
                ui.painter().text(Pos2::new(r.center().x - 102.0, r.center().y), Align2::LEFT_CENTER, "Перетащите сюда, чтобы удалить", FontId::proportional(13.5), p.danger);
                trash_rect = Some(r);
            });
        });
    });
    if dismissed {
        result.close = true;
    }

    // ---- Drag & drop resolution ----
    let pointer = ctx.pointer_hover_pos();
    if state.drag_slot.is_some() || state.drag_item.is_some() {
        if let Some(pos) = pointer {
            let layer = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("drag-ghost")));
            let ghost = Rect::from_center_size(pos, Vec2::splat(CELL));
            let id = state.drag_slot.and_then(|s| get(inv, &state.grid, s)).map(|s| s.id).or(state.drag_item);
            if let Some(id) = id {
                layer.rect_filled(ghost, Rounding::ZERO, p.surface.gamma_multiply(0.9));
                icons.paint_item(&layer, ghost.shrink(6.0), id);
            }
        }
        if ctx.input(|i| i.pointer.any_released()) {
            let target = pointer.and_then(|pos| slot_rects.iter().find(|(_, r)| r.contains(pos)).map(|(s, _)| *s));
            if let Some(src) = state.drag_slot.take() {
                if let Some(dst) = target {
                    move_any(inv, &mut state.grid, src, dst);
                } else if let (Some(tr), Some(pos)) = (trash_rect, pointer) {
                    if tr.contains(pos) {
                        set(inv, &mut state.grid, src, None);
                    }
                }
            }
            if let Some(item) = state.drag_item.take() {
                if let Some(dst) = target {
                    set(inv, &mut state.grid, dst, Some(ItemStack::new(item, items::info(item).max_stack)));
                }
            }
        }
    }
    if result.close {
        close(state, inv);
    }
    if let Some((msg, t)) = &mut state.toast {
        *t -= dt;
        let a = (*t * 2.0).clamp(0.0, 1.0);
        ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("inv-toast"))).text(
            Pos2::new(ctx.screen_rect().center().x, ctx.screen_rect().bottom() - 90.0),
            Align2::CENTER_CENTER,
            msg.as_str(),
            FontId::proportional(16.0),
            p.success.gamma_multiply(a),
        );
        if *t <= 0.0 {
            state.toast = None;
        }
    }
    result
}

/// Inventory or grid slot: drag to move, click a hotbar slot to select it,
/// right-click an inventory slot to put one item into the crafting grid.
fn draw_slot(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, slot: usize, rect: Rect) {
    let stack = get(inv, &state.grid, slot);
    let shown = if state.drag_slot == Some(slot) { None } else { stack };
    let selected = slot == inv.selected && slot < HOTBAR;
    let resp = slot_widget(ui, icons, p, rect, shown.as_ref(), selected, Id::new(("inv-slot", slot)), Sense::click_and_drag());
    if resp.drag_started() && stack.is_some() {
        state.drag_slot = Some(slot);
    }
    if resp.clicked() && slot < HOTBAR {
        inv.selected = slot;
    }
    if resp.secondary_clicked() && slot < SLOTS {
        if let Some(s) = stack {
            // One item into the first grid cell holding the same item, else the first empty one.
            let cells = state.grid.active();
            let same = cells.iter().copied().find(|&i| state.grid.cells[i].is_some_and(|c| c.can_stack_with(&s) && c.count < c.info().max_stack));
            let target = same.or_else(|| cells.iter().copied().find(|&i| state.grid.cells[i].is_none()));
            if let Some(i) = target {
                let c = state.grid.cells[i].map_or(1, |c| c.count + 1);
                state.grid.cells[i] = Some(ItemStack { count: c, ..s });
                inv.slots[slot] = if s.count > 1 { Some(ItemStack { count: s.count - 1, ..s }) } else { None };
            }
        }
    }
    if let Some(s) = stack {
        let dur = s.info().durability;
        let mut tip = s.info().name.to_string();
        if dur > 0 {
            tip += &format!("\nПрочность {}/{}", dur - s.damage, dur);
        }
        resp.on_hover_text(tip);
    }
}

/// Survival inventory: character (with the second-hand slot) and crafting on top,
/// the whole backpack below, the hotbar at the bottom.
#[allow(clippy::too_many_arguments)]
fn survival_layout(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, near_table: bool, width: f32, slot_rects: &mut Vec<(usize, Rect)>) -> Option<String> {
    let mut crafted = None;
    ui.set_width(width);
    ui.horizontal_top(|ui| {
        // ---- Character ----
        ui.vertical(|ui| {
            section_label(ui, "Персонаж");
            ui.add_space(4.0);
            ui.horizontal_top(|ui| {
                let h = 3.0 * (CELL + GAP) - GAP;
                let (r, _) = ui.allocate_exact_size(Vec2::new(h * 0.62, h), Sense::hover());
                ui.painter().rect_filled(r, Rounding::ZERO, p.surface_hi);
                ui.painter().rect_stroke(r, Rounding::ZERO, Stroke::new(2.0_f32, p.outline));
                paint_character(ui.painter(), r.shrink(10.0), inv.offhand.is_some_and(|s| s.id == items::ids::SHIELD));
                ui.add_space(GAP);
                ui.vertical(|ui| {
                    ui.add_space(h - CELL - 16.0);
                    ui.label(egui::RichText::new("2-я рука").size(11.5).color(p.text_muted));
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(CELL), Sense::hover());
                    slot_rects.push((OFFHAND, rect));
                    draw_slot(ui, icons, p, state, inv, OFFHAND, rect);
                });
            });
        });
        ui.add_space(24.0);
        // ---- Crafting ----
        ui.vertical(|ui| {
            crafted = crafting_grid(ui, icons, p, state, inv, near_table, slot_rects);
        });
    });
    ui.add_space(12.0);
    section_label(ui, "Рюкзак");
    ui.add_space(4.0);
    let backpack: Vec<usize> = (HOTBAR..SLOTS).collect();
    for row in backpack.chunks(9) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(GAP);
            for &slot in row {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(CELL), Sense::hover());
                slot_rects.push((slot, rect));
                draw_slot(ui, icons, p, state, inv, slot, rect);
            }
        });
    }
    ui.add_space(10.0);
    section_label(ui, "Панель быстрого доступа");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(GAP);
        for slot in 0..HOTBAR {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(CELL), Sense::hover());
            slot_rects.push((slot, rect));
            draw_slot(ui, icons, p, state, inv, slot, rect);
        }
    });
    crafted
}

/// Blocky front view of the player, in the skin's colours.
fn paint_character(painter: &egui::Painter, r: Rect, shield: bool) {
    let c = |r: f32, g: f32, b: f32| Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
    let (skin, hair, jacket, sleeve, belt, trousers, boots, eye) = (
        c(0.86, 0.68, 0.55),
        c(0.20, 0.13, 0.08),
        c(0.10, 0.50, 0.55),
        c(0.08, 0.42, 0.47),
        c(0.95, 0.60, 0.15),
        c(0.18, 0.20, 0.30),
        c(0.26, 0.17, 0.11),
        c(0.10, 0.12, 0.18),
    );
    // Figure is 16 units wide x 32 tall (head 8, body 12, legs 12), centred in `r`.
    let u = (r.width() / 16.0).min(r.height() / 32.0);
    let o = Pos2::new(r.center().x - 8.0 * u, r.center().y - 16.0 * u);
    let b = |x: f32, y: f32, w: f32, h: f32, col: Color32| painter.rect_filled(Rect::from_min_size(o + Vec2::new(x * u, y * u), Vec2::new(w * u, h * u)), 0.0, col);
    b(4.0, 0.0, 8.0, 8.0, skin);
    b(4.0, 0.0, 8.0, 2.5, hair);
    b(5.5, 4.0, 1.5, 1.0, eye);
    b(9.0, 4.0, 1.5, 1.0, eye);
    b(4.0, 8.0, 8.0, 12.0, jacket);
    b(4.0, 18.0, 8.0, 1.5, belt);
    b(0.0, 8.0, 4.0, 10.0, sleeve);
    b(12.0, 8.0, 4.0, 10.0, sleeve);
    b(0.0, 18.0, 4.0, 2.0, skin);
    b(12.0, 18.0, 4.0, 2.0, skin);
    b(4.0, 20.0, 4.0, 9.0, trousers);
    b(8.0, 20.0, 4.0, 9.0, trousers);
    b(4.0, 29.0, 4.0, 3.0, boots);
    b(8.0, 29.0, 4.0, 3.0, boots);
    if shield {
        // Shield on the character's left arm (viewer's right).
        b(11.0, 11.0, 5.5, 8.0, c(0.62, 0.64, 0.68));
        b(11.7, 11.7, 4.1, 6.6, c(0.62, 0.45, 0.25));
    }
}

/// Vertical list of creative categories (the backpack is the last entry).
fn category_column(ui: &mut Ui, p: &Palette, tab: &mut usize) {
    ui.spacing_mut().item_spacing.y = 2.0;
    for (i, name) in CREATIVE_TABS.iter().enumerate() {
        if i == BACKPACK_TAB {
            ui.add_space(8.0);
        }
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(SIDE_W, 34.0), Sense::click());
        let sel = *tab == i;
        let a = ui.ctx().animate_bool_with_time(Id::new(("cat", i)), sel, ANIM_TIME);
        let hv = ui.ctx().animate_bool_with_time(Id::new(("cat-h", i)), resp.hovered(), ANIM_TIME);
        let fill = Color32::TRANSPARENT.lerp_to_gamma(p.surface_hover, (hv * 0.8).max(a * 0.9));
        ui.painter().rect_filled(rect, Rounding::ZERO, fill);
        // Accent bar on the left of the selected entry.
        ui.painter().rect_filled(Rect::from_min_size(rect.min + Vec2::new(0.0, 7.0), Vec2::new(3.0 * a, 20.0)), Rounding::ZERO, p.accent);
        let col = p.text_muted.lerp_to_gamma(p.accent, a).lerp_to_gamma(p.text, hv * (1.0 - a));
        ui.painter().text(Pos2::new(rect.left() + 14.0, rect.center().y), Align2::LEFT_CENTER, *name, FontId::proportional(14.5), col);
        if resp.clicked() {
            *tab = i;
        }
        resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
}

fn creative_panel(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory) {
    section_label(ui, "Все предметы");
    ui.add_space(4.0);
    text_input(ui, icons, &mut state.search, "Поиск предметов…", 320.0, Some(icon::SEARCH));
    ui.add_space(8.0);
    let palette_items: Vec<ItemId> = items::creative_palette().into_iter().filter(|id| in_tab(*id, state.tab) && matches_search(*id, &state.search)).collect();
    let cols = 9;
    egui::ScrollArea::vertical().max_height(CELL * 5.0 + GAP * 5.0).auto_shrink([false, true]).show(ui, |ui| {
        for chunk in palette_items.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(GAP);
                for id in chunk {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(CELL), Sense::hover());
                    let stack = ItemStack::new(*id, 1);
                    let resp = slot_widget(ui, icons, p, rect, Some(&stack), false, Id::new(("pal", *id)), Sense::click_and_drag());
                    if resp.drag_started() {
                        state.drag_item = Some(*id);
                    }
                    if resp.clicked() {
                        let n = if ui.input(|i| i.modifiers.shift) { items::info(*id).max_stack } else { 1 };
                        inv.add(ItemStack::new(*id, n));
                    }
                    resp.on_hover_text(items::info(*id).name);
                }
            });
        }
        if palette_items.is_empty() {
            ui.label(egui::RichText::new("Ничего не найдено").color(p.text_muted));
        }
    });
    ui.label(egui::RichText::new("Клик — добавить 1, Shift+клик — стак, перетащите на слот").size(12.5).color(p.text_muted));
}

/// Survival crafting: grid → arrow → result. Click the result to craft, Shift+click for all.
fn crafting_grid(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, near_table: bool, slot_rects: &mut Vec<(usize, Rect)>) -> Option<String> {
    let mut crafted = None;
    let n = state.grid.size;
    section_label(ui, if n == 3 { "Крафт · верстак 3×3" } else { "Крафт 2×2" });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            for r in 0..n {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(GAP);
                    for c in 0..n {
                        let id = GRID + r * 3 + c;
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(CELL), Sense::hover());
                        slot_rects.push((id, rect));
                        draw_slot(ui, icons, p, state, inv, id, rect);
                    }
                });
            }
        });
        ui.add_space(12.0);
        let recipe = state.grid.matched();
        let (ar, _) = ui.allocate_exact_size(Vec2::new(28.0, n as f32 * (CELL + GAP)), Sense::hover());
        {
            let col = if recipe.is_some() { p.accent } else { p.text_muted };
            let st = Stroke::new(2.5_f32, col);
            let c = ar.center();
            ui.painter().line_segment([c - Vec2::new(10.0, 0.0), c + Vec2::new(9.0, 0.0)], st);
            ui.painter().line_segment([c + Vec2::new(10.0, 0.0), c + Vec2::new(3.0, -7.0)], st);
            ui.painter().line_segment([c + Vec2::new(10.0, 0.0), c + Vec2::new(3.0, 7.0)], st);
        }
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.add_space((n as f32 * (CELL + GAP) - CELL - 8.0) / 2.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(CELL + 8.0), Sense::hover());
            let out = recipe.map(|r| ItemStack::new(r.output.0, r.output.1));
            let resp = slot_widget(ui, icons, p, rect, out.as_ref(), recipe.is_some(), Id::new("craft-out"), Sense::click());
            if let Some(r) = recipe {
                let resp = resp.on_hover_text(format!("{}\nКлик — создать, Shift+клик — сколько хватит", r.name()));
                if resp.clicked() {
                    let all = ui.input(|i| i.modifiers.shift);
                    let mut done = 0u32;
                    while state.grid.matched().is_some_and(|m| inv.can_hold(ItemStack::new(m.output.0, m.output.1))) {
                        if let Some(s) = state.grid.craft_once() {
                            inv.add(s);
                            done += s.count as u32;
                        }
                        if !all {
                            break;
                        }
                    }
                    if done > 0 {
                        crafted = Some(format!("Создано: {} ×{}", r.name(), done));
                    }
                }
            }
        });
    });
    ui.add_space(8.0);
    let hint = if near_table { "ПКМ — положить 1" } else { "Верстак — сетка 3×3" };
    ui.label(egui::RichText::new(hint).size(12.5).color(p.text_muted));
    if let Some(msg) = &crafted {
        state.toast = Some((msg.clone(), 2.0));
    }
    crafted
}

#[allow(dead_code)]
fn _unused(_: Color32) {}

#[cfg(test)]
mod tests {
    use super::*;
    use wolfcraft_engine::block::Block;

    fn ctx_with_icons() -> (egui::Context, GuiIcons) {
        let ctx = egui::Context::default();
        crate::theme_manager::install(&ctx);
        crate::theme_manager::apply(&ctx, wolfcraft_engine::config::ThemeMode::Dark);
        let atlases = wolfcraft_assets::AtlasManager::new(None).unwrap();
        let icons = GuiIcons::new(&ctx, &atlases);
        (ctx, icons)
    }

    fn frame(ctx: &egui::Context, mut f: impl FnMut(&egui::Context)) {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 800.0)));
        let _ = ctx.run(raw, |c| f(c));
    }

    #[test]
    fn both_modes_render() {
        let (ctx, icons) = ctx_with_icons();
        let mut st = InventoryUiState::default();
        let mut inv = Inventory::default();
        inv.add(ItemStack::of_block(Block::OakPlanks, 20));
        for creative in [false, true] {
            for tab in 0..4 {
                st.tab = tab;
                frame(&ctx, |c| {
                    show(c, &icons, &mut st, &mut inv, creative, true, 0.016);
                });
            }
        }
    }

    #[test]
    fn closing_returns_grid_items_and_moves_work() {
        let mut st = InventoryUiState::default();
        let mut inv = Inventory::default();
        inv.slots[HOTBAR] = Some(ItemStack::of_block(Block::OakLog, 3));
        move_any(&mut inv, &mut st.grid, HOTBAR, GRID);
        assert!(inv.slots[HOTBAR].is_none());
        assert_eq!(st.grid.matched().unwrap().output.0, Block::OakPlanks as ItemId);
        close(&mut st, &mut inv);
        assert_eq!(inv.count(Block::OakLog as ItemId), 3);
    }

    #[test]
    fn search_filters_by_name() {
        assert!(matches_search(Block::Lamp as ItemId, "лам"));
        assert!(!matches_search(Block::Lamp as ItemId, "алмаз"));
        assert!(matches_search(Block::Lamp as ItemId, "  "));
    }
}
