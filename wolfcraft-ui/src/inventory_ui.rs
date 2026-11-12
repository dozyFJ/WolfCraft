//! Inventory screen: header bar (title + close), category navbar, inventory slots of the
//! selected category, and on the right the creative palette or the survival crafting grid
//! (2×2 by hand, 3×3 at a crafting table).

use crate::gui_renderer::GuiIcons;
use crate::hud::draw_stack;
use crate::theme_manager::{ANIM_TIME, Palette, bold_font, palette};
use crate::ui_components::{section_label, text_input};
use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use wolfcraft_assets::gui as icon;
use wolfcraft_engine::inventory::{CraftGrid, HOTBAR, Inventory, SLOTS};
use wolfcraft_engine::items::{self, ItemId, ItemStack};

const CELL: f32 = 46.0;
const GAP: f32 = 5.0;
/// Drag ids: 0..SLOTS = inventory, OFFHAND = second hand, GRID.. = crafting cells.
const GRID: usize = 100;
const OFFHAND: usize = 99;
/// Worn armor: ARMOR..ARMOR+4 (helmet, chestplate, leggings, boots).
const ARMOR: usize = 95;

/// Creative tabs (icons along the top): item categories plus the backpack as the last tab.
const CREATIVE_TABS: [&str; 12] = ["Все", "Природа", "Дерево", "Строительство", "Руды", "Техника", "Растения", "Предметы", "Инструменты", "Вооружение", "Доспехи", "Инвентарь"];
const BACKPACK_TAB: usize = CREATIVE_TABS.len() - 1;


#[derive(Default)]
pub struct InventoryUiState {
    pub tab: usize,
    pub search: String,
    pub grid: CraftGrid,
    /// Slot being dragged (inventory or grid id).
    drag_slot: Option<usize>,
    /// Item being dragged from the creative palette.
    drag_item: Option<ItemId>,
    /// Last known pointer position: on touch screens the pointer is gone in the release frame.
    last_pos: Option<Pos2>,
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

/// Order of items in the creative grid.
fn sort_key(id: ItemId) -> (usize, u8, String) {
    let cat = CREATIVE_TABS.iter().position(|c| *c == items::category(id)).unwrap_or(99);
    let group = if id >= items::ITEM_BASE {
        if items::info(id).tool.is_some() || items::armor_slot(id).is_some() { 5 } else { 4 }
    } else {
        let b = wolfcraft_engine::block::Block::from_u8(id as u8);
        match b.info().render {
            wolfcraft_engine::block::RenderKind::Cross => 3,
            _ if !b.boxes().is_empty() => 2,
            _ => 1,
        }
    };
    (cat, group, items::info(id).name.to_lowercase())
}

fn matches_search(id: ItemId, search: &str) -> bool {
    search.trim().is_empty() || items::info(id).name.to_lowercase().contains(&search.trim().to_lowercase())
}

fn in_tab(id: ItemId, tab: usize) -> bool {
    tab == 0 || tab >= BACKPACK_TAB || items::category(id) == CREATIVE_TABS[tab]
}

fn get(inv: &Inventory, grid: &CraftGrid, slot: usize) -> Option<ItemStack> {
    if slot >= GRID {
        grid.cells[slot - GRID]
    } else if (ARMOR..ARMOR + 4).contains(&slot) {
        inv.armor[slot - ARMOR]
    } else if slot == OFFHAND {
        inv.offhand
    } else {
        inv.slots[slot]
    }
}

fn set(inv: &mut Inventory, grid: &mut CraftGrid, slot: usize, v: Option<ItemStack>) {
    if slot >= GRID {
        grid.cells[slot - GRID] = v
    } else if (ARMOR..ARMOR + 4).contains(&slot) {
        inv.armor[slot - ARMOR] = v
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
    // Armor slots take only the matching piece (in either direction of a swap).
    let fits = |slot: usize, s: Option<ItemStack>| !(ARMOR..ARMOR + 4).contains(&slot) || s.is_none_or(|s| items::armor_slot(s.id) == Some(slot - ARMOR));
    if !fits(b, get(inv, grid, a)) || !fits(a, get(inv, grid, b)) {
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
    // The workbench always opens its own 3×3 window, in creative too.
    let creative = creative && !near_table;
    if !creative {
        let want = if near_table { 3 } else { 2 };
        if state.grid.size != want {
            state.grid.set_size(want, inv);
        }
    }

    let left_w = 9.0 * (CELL + GAP) - GAP;
    if creative {
        // Cells as big as the screen allows: three rows of items plus the hotbar must fit the
        // height (with the tabs, title and the phone's gesture bar), ten cells the width.
        let (sw, h) = (ctx.screen_rect().width(), ctx.screen_rect().height());
        let top = crate::ui_components::SCREEN_BAR;
        let by_height = (h - 100.0 - top) / (4.0 + 4.0 * GAP / 40.0);
        let by_width = (sw - 60.0 - 8.0 * GAP - 14.0 - 10.0) / (10.0 + 0.92);
        let cell = by_height.min(by_width).clamp(40.0, 84.0).floor();
        let rows = (((h - 100.0 - top - cell) / (cell + GAP)).floor() as usize).clamp(2, 7);
        // Category sidebar + the grid (hotbar of 9 + gap + bin is as wide).
        let grid_w = 10.0 * cell + 8.0 * GAP + 14.0;
        let width = grid_w + (cell * 0.92).floor() + 16.0;
        let dismissed = crate::ui_components::game_screen(ctx, "inventory", "Творческий режим", width.min(ctx.screen_rect().width() - 40.0), |ui| {
            creative_layout(ui, icons, &p, state, inv, cell, rows, grid_w, &mut slot_rects, &mut trash_rect, &mut result);
        });
        if dismissed {
            result.close = true;
        }
    } else {
    let width = left_w + 8.0;
    let title = if near_table { "Верстак" } else { "Инвентарь" };
    let dismissed = crate::ui_components::game_screen(ctx, "inventory", title, width.min(ctx.screen_rect().width() - 40.0), |ui| {
        state.tab = 0;
        result.crafted = survival_layout(ui, icons, &p, state, inv, near_table, left_w, &mut slot_rects);
    });
    if dismissed {
        result.close = true;
    }
    }

    // ---- Drag & drop resolution ----
    if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
        state.last_pos = Some(pos);
    }
    let pointer = ctx.pointer_hover_pos().or(state.last_pos);
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
                // Armor: helmet, chestplate, leggings, boots.
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = GAP;
                    for a in 0..4 {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(CELL * 0.75), Sense::hover());
                        slot_rects.push((ARMOR + a, rect));
                        draw_slot(ui, icons, p, state, inv, ARMOR + a, rect);
                        if inv.armor[a].is_none() {
                            let names = ["Шлем", "Нагрудник", "Поножи", "Ботинки"];
                            ui.painter().text(rect.center(), Align2::CENTER_CENTER, names[a], FontId::proportional(8.5), p.text_muted.gamma_multiply(0.6));
                        }
                    }
                });
                ui.add_space(GAP);
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

/// Creative inventory laid out like the classic block game: a row of icon tabs on top,
/// the item grid below, then the hotbar with the bin slot on its right.
#[allow(clippy::too_many_arguments)]
fn creative_layout(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, cell: f32, rows: usize, grid_w: f32, slot_rects: &mut Vec<(usize, Rect)>, trash_rect: &mut Option<Rect>, result: &mut InventoryResult) {
    let palette_all = items::creative_palette();
    // ---- Categories: a vertical sidebar on the left (scrolls if the screen is short) ----
    let tab = (cell * 0.92).floor();
    let content_h = 30.0 + 6.0 + rows as f32 * (cell + GAP) - GAP + 10.0 + cell;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        // Top-down column (a plain child of the horizontal row would lay the tabs out in a row).
        ui.allocate_ui_with_layout(Vec2::new(tab + 6.0, content_h), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(tab + 6.0);
            egui::ScrollArea::vertical().id_salt("creative-tabs").max_height(content_h).auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.set_width(tab + 6.0);
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
                for (i, name) in CREATIVE_TABS.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(tab), Sense::click());
                    let sel = state.tab == i;
                    let fill = if sel { p.surface_hover } else if resp.hovered() { p.surface_hi.lerp_to_gamma(p.surface_hover, 0.5) } else { p.surface_hi };
                    ui.painter().rect_filled(rect, Rounding::ZERO, fill);
                    ui.painter().rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, if sel { p.accent } else { p.outline }));
                    if sel {
                        // Selected category: an accent bar on its outer edge.
                        ui.painter().rect_filled(Rect::from_min_size(rect.min, Vec2::new(4.0, rect.height())), Rounding::ZERO, p.accent);
                    }
                    let inner = rect.shrink(tab * 0.18);
                    match i {
                        0 => icons.paint(ui.painter(), inner, icon::STAR, if sel { p.accent } else { p.text }),
                        BACKPACK_TAB => icons.paint(ui.painter(), inner, icon::BACKPACK, if sel { p.accent } else { p.text }),
                        _ => {
                            if let Some(id) = palette_all.iter().copied().find(|id| items::category(*id) == *name) {
                                icons.paint_item(ui.painter(), inner, id);
                            }
                        }
                    }
                    if resp.clicked() {
                        state.tab = i;
                    }
                    resp.on_hover_text(*name);
                }
            });
        });
        ui.vertical(|ui| {
            creative_main(ui, icons, p, state, inv, cell, rows, grid_w, palette_all, slot_rects, trash_rect, result);
        });
    });
}

/// Right part of the creative inventory: category title and search, the item grid, the hotbar.
#[allow(clippy::too_many_arguments)]
fn creative_main(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, cell: f32, rows: usize, grid_w: f32, palette_all: Vec<ItemId>, slot_rects: &mut Vec<(usize, Rect)>, trash_rect: &mut Option<Rect>, result: &mut InventoryResult) {
    let _ = &result;
    // ---- Tab title + search ----
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(CREATIVE_TABS[state.tab]).font(bold_font(16.0)).color(p.text));
        if state.tab != BACKPACK_TAB {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                text_input(ui, icons, &mut state.search, "Поиск…", 220.0, Some(icon::SEARCH));
            });
        }
    });
    ui.add_space(6.0);
    // ---- Grid ----
    let grid_h = rows as f32 * (cell + GAP) - GAP;
    if state.tab == BACKPACK_TAB {
        // The survival inventory inside the creative one: armor, second hand, 2x2 crafting,
        // backpack and hotbar (scrolls on small screens).
        let (area, _) = ui.allocate_exact_size(Vec2::new(grid_w, grid_h + 10.0 + cell), Sense::hover());
        let mut ui2 = ui.new_child(egui::UiBuilder::new().max_rect(area));
        egui::ScrollArea::vertical().id_salt("creative-survival-inv").auto_shrink([false, false]).show(&mut ui2, |ui| {
            if let Some(msg) = survival_layout(ui, icons, p, state, inv, false, grid_w - 16.0, slot_rects) {
                result.crafted = Some(msg);
            }
        });
        return;
    } else {
        let mut list: Vec<ItemId> = palette_all.into_iter().filter(|id| in_tab(*id, state.tab) && matches_search(*id, &state.search)).collect();
        // Sorted: by category (in "Все"), then full blocks, shaped blocks (stairs, slabs,
        // doors…), plants, items and tools, alphabetically inside each group so related
        // things sit together (all oak, all diamond…).
        list.sort_by_cached_key(|id| sort_key(*id));
        let (area, _) = ui.allocate_exact_size(Vec2::new(grid_w, grid_h), Sense::hover());
        let mut ui2 = ui.new_child(egui::UiBuilder::new().max_rect(area));
        // Swipe scrolls the list (a slot only reacts to a tap, so the finger drag goes to the
        // scroll area); the bar stays visible so it is clear there is more below.
        egui::ScrollArea::vertical()
            .id_salt(("creative-scroll", state.tab))
            .auto_shrink([false, false])
            .drag_to_scroll(true)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(&mut ui2, |ui| {
            for chunk in list.chunks(10) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(GAP);
                    for id in chunk {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(cell), Sense::hover());
                        let stack = ItemStack::new(*id, 1);
                        let resp = slot_widget(ui, icons, p, rect, Some(&stack), false, Id::new(("pal", *id)), Sense::click());
                        if resp.clicked() {
                            // Tap: a full stack into the selected hotbar slot (Shift: into a free slot).
                            let full = ItemStack::new(*id, items::info(*id).max_stack);
                            if ui.input(|i| i.modifiers.shift) {
                                inv.add(full);
                            } else {
                                inv.slots[inv.selected] = Some(full);
                            }
                        }
                        if items::is_new(*id) {
                            let tag = Rect::from_min_size(rect.left_top() + Vec2::new(2.0, 2.0), Vec2::new(22.0, 10.0));
                            ui.painter().rect_filled(tag, Rounding::same(2.0), p.success);
                            ui.painter().text(tag.center(), egui::Align2::CENTER_CENTER, "NEW", egui::FontId::proportional(7.5), egui::Color32::BLACK);
                        }
                        resp.on_hover_text(items::info(*id).name);
                    }
                });
                ui.add_space(GAP - ui.spacing().item_spacing.y);
            }
            if list.is_empty() {
                ui.label(egui::RichText::new("Ничего не найдено").color(p.text_muted));
            }
        });
    }
    ui.add_space(10.0);
    // ---- Hotbar + bin ----
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(GAP);
        for slot in 0..HOTBAR {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(cell), Sense::hover());
            slot_rects.push((slot, rect));
            draw_slot(ui, icons, p, state, inv, slot, rect);
        }
        ui.add_space(14.0 - GAP);
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(cell), Sense::click());
        let hot = resp.hovered() || state.drag_slot.is_some();
        ui.painter().rect_filled(rect, Rounding::ZERO, p.danger.gamma_multiply(if hot { 0.35 } else { 0.15 }));
        ui.painter().rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, p.danger.gamma_multiply(0.7)));
        icons.paint(ui.painter(), rect.shrink(cell * 0.24), icon::TRASH, p.danger);
        // Tap: delete the item in the selected hotbar slot. Or drag any slot here.
        if resp.clicked() {
            inv.slots[inv.selected] = None;
        }
        resp.on_hover_text("Удалить: нажмите — предмет в выбранном слоте, или перетащите сюда");
        *trash_rect = Some(rect);
    });
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
