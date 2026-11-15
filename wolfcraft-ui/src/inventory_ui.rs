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
// "Техника" is put away for now (its items are still under "Все" and in the search).
const CREATIVE_TABS: [&str; 19] = ["Все", "Мебель", "Стекло", "Двери", "Декор", "Природа", "Дерево", "Строительство", "Руды", "Растения", "Хранилища", "Предметы", "Инструменты", "Вооружение", "Доспехи", "Мобы", "WolfTech", "Механика", "Инвентарь"];
const BACKPACK_TAB: usize = CREATIVE_TABS.len() - 1;


#[derive(Default)]
pub struct InventoryUiState {
    pub tab: usize,
    pub search: String,
    pub build_form: usize,
    pub build_material: usize,
    pub build_open: bool,
    /// Opened from the «Конструктор» block: only the building-piece maker is shown.
    pub builder: bool,
    /// The builder in creative: pieces are taken for free.
    builder_free: bool,
    pub grid: CraftGrid,
    /// Slot being dragged (inventory or grid id).
    drag_slot: Option<usize>,
    /// Item being dragged from the creative palette.
    drag_item: Option<ItemId>,
    /// Last known pointer position: on touch screens the pointer is gone in the release frame.
    last_pos: Option<Pos2>,
    /// Message from the last craft ("Создано: ...").
    pub toast: Option<(String, f32)>,
    /// Name of the item the finger is on (phones have no hover): text, slot, seconds left.
    caption: Option<(String, Rect, f32)>,
    /// Creative: the search line is open (the magnifier in the sidebar).
    search_open: bool,
    /// Creative: the building-piece group (a form: slab, arc, chair…) that is expanded.
    open_group: Option<usize>,
    /// Creative «Инвентарь» tab: which page the right sidebar shows (0 персонаж, 1 крафт,
    /// 2 рюкзак with the hotbar).
    inv_page: u8,
    /// Creative: the category to come back to from Крафт / Персонаж / Рюкзак.
    last_tab: usize,
    /// Creative: horizontal scroll of the category pills (arrows move it).
    cat_scroll: f32,
    cat_scroll_set: bool,
}

/// Shows the item's name over its slot while it is pressed (and a moment after), as on
/// Bedrock phones.
fn note(state: &mut InventoryUiState, resp: &egui::Response, rect: Rect, text: impl Into<String>) {
    if resp.is_pointer_button_down_on() || resp.clicked() || resp.drag_started() || resp.hovered() {
        let fresh = resp.is_pointer_button_down_on() || resp.clicked() || resp.drag_started();
        let keep = state.caption.as_ref().map(|c| c.2).unwrap_or(0.0);
        state.caption = Some((text.into(), rect, if fresh { 1.6 } else { keep.max(0.05) }));
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct InventoryResult {
    pub close: bool,
    pub crafted: Option<String>,
    /// Stacks dragged out of the window: the game throws them into the world (Bedrock style).
    pub thrown: Vec<ItemStack>,
}

/// Call when the inventory closes: items left in the crafting grid go back to the inventory.
pub fn close(state: &mut InventoryUiState, inv: &mut Inventory) {
    state.grid.return_all(inv);
    state.drag_slot = None;
    state.drag_item = None;
}

/// Slot tile of the HUD hotbar (and its hovered/pressed shade).
const HUD_TILE: Color32 = Color32::from_rgba_premultiplied(12, 14, 19, 200);
const HUD_TILE_HOT: Color32 = Color32::from_rgba_premultiplied(30, 36, 50, 220);

/// A sidebar button in the style of the top bar's islands: a round-cornered dark glass pill.
fn hud_button(ui: &Ui, rect: Rect, on: bool, hot: bool, p: &Palette) {
    let fill = if on { p.accent.gamma_multiply(0.30) } else if hot { HUD_TILE_HOT } else { HUD_TILE };
    ui.painter().rect_filled(rect, Rounding::same(rect.height() * 0.28), fill);
    ui.painter().rect_stroke(rect, Rounding::same(rect.height() * 0.28), Stroke::new(if on { 2.5_f32 } else { 1.5 }, if on { p.accent } else { Color32::from_white_alpha(45) }));
}

/// Draws one slot and returns its response. `rect` is where the slot sits.
fn slot_widget(ui: &mut Ui, icons: &GuiIcons, p: &Palette, rect: Rect, stack: Option<&ItemStack>, selected: bool, id: Id, sense: Sense) -> egui::Response {
    let resp = ui.interact(rect, id, sense);
    let hv = ui.ctx().animate_bool_with_time(id.with("hv"), resp.hovered() || resp.is_pointer_button_down_on(), ANIM_TIME);
    // Like the hotbar: a dark see-through tile, a faint light edge, the accent when chosen.
    // Like the slots of the block windows (machines, chests): a solid tile with a firm edge.
    let fill = p.surface_hi.lerp_to_gamma(p.surface_hover, hv);
    ui.painter().rect_filled(rect, Rounding::same(4.0), fill);
    let edge = if selected { Stroke::new(2.5_f32, p.accent) } else { Stroke::new(2.0_f32, p.outline.lerp_to_gamma(p.accent, hv * 0.7)) };
    ui.painter().rect_stroke(rect, Rounding::same(4.0), edge);
    if let Some(s) = stack {
        draw_stack(ui.painter(), icons, rect, s, p);
    }
    resp
}

/// Order of items in the creative grid.
fn sort_key(id: ItemId) -> (usize, u8, String) {
    let cat = CREATIVE_TABS.iter().position(|c| *c == items::category(id)).unwrap_or(99);
    let group = if (items::ITEM_BASE..1024).contains(&id) {
        if items::info(id).tool.is_some() || items::armor_slot(id).is_some() { 5 } else { 4 }
    } else {
        let b = wolfcraft_engine::block::Block::from_id(id);
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
    // The workbench and the «Конструктор» always open their own window, in creative too.
    state.builder_free = creative && state.builder;
    let creative = creative && !near_table && !state.builder;
    if !creative {
        let want = if near_table { 3 } else { 2 };
        if state.grid.size != want {
            state.grid.set_size(want, inv);
        }
    }

    if creative {
        // A block window like the machines' (WolfTech): a metal banner with the title, the
        // categories and the item grid in recessed panels, the hotbar under them; the window
        // is scaled to fit the screen.
        let cell = 46.0f32;
        let (rows, cols) = (5usize, 9usize);
        let grid_w = cols as f32 * (cell + GAP) - GAP + 14.0;
        let width = grid_w + 10.0 * (cell + GAP) - 9.0 * (cell + GAP) + 24.0;
        let width = width.max(grid_w + 8.0);
        let esc = crate::ui_components::block_screen_ex(ctx, "inventory", width, false, |ui| {
            creative_layout(ui, icons, &p, state, inv, cell, rows, cols, grid_w, &mut slot_rects, &mut trash_rect, &mut result);
        });
        let dismissed = esc || result.close;
        if dismissed {
            result.close = true;
        }
    } else {
        // Full screen like the creative one: back on the left, the inventory in the middle
        // (scrolls on short screens), the hotbar and the bin along the bottom.
        let ins = crate::gui_renderer::SAFE_INSETS.read().map(|i| *i).unwrap_or([0.0; 4]);
        let h = ctx.screen_rect().height() - ins[1] - ins[3] - 32.0;
        let sw = ctx.screen_rect().width() - ins[0] - ins[2] - 32.0;
        let s = ((sw - 80.0 - 70.0) / 10.0).min((h - 40.0) / 5.0).clamp(36.0, 64.0).floor();
        let tab = 52.0f32;
        let title = if state.builder { "Конструктор" } else if near_table { "Верстак" } else { "Инвентарь" };
        let esc = crate::ui_components::full_screen(ctx, "inventory", |ui| {
            let content_h = ui.available_height();
            state.tab = 0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.allocate_ui_with_layout(Vec2::new(tab, content_h), egui::Layout::top_down(egui::Align::Min), |ui| {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(tab), Sense::click());
                    hud_button(ui, rect, false, resp.hovered() || resp.is_pointer_button_down_on(), &p);
                    icons.paint(ui.painter(), rect.shrink(tab * 0.24), icon::ARROW_LEFT, Color32::from_white_alpha(225));
                    if resp.clicked() {
                        result.close = true;
                    }
                });
                let mid_w = ui.available_width().max(CELL * 4.0);
                ui.allocate_ui_with_layout(Vec2::new(mid_w, content_h), egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.set_width(mid_w);
                    ui.label(egui::RichText::new(title).font(bold_font(16.0)).color(p.text));
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().id_salt("survival-inv").auto_shrink([false, false]).max_height(content_h - 30.0 - s - 12.0).show(ui, |ui| {
                        result.crafted = survival_layout(ui, icons, &p, state, inv, near_table, mid_w - 16.0, &mut slot_rects, false);
                    });
                    ui.add_space(6.0);
                    hotbar_row(ui, icons, &p, state, inv, s, &mut slot_rects, &mut trash_rect);
                });
            });
        });
        if esc || result.close {
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
                layer.rect_filled(ghost, Rounding::same(10.0), p.surface.gamma_multiply(0.9));
                icons.paint_item(&layer, ghost.shrink(6.0), id);
            }
        }
        if ctx.input(|i| i.pointer.any_released()) {
            // Slots are recorded in the window's own coordinates (it may be scaled to fit).
            // Both inventories are full screen: slots are in screen coordinates.
            let target = pointer.and_then(|pos| slot_rects.iter().find(|(_, r)| r.contains(pos)).map(|(s, _)| *s));
            if let Some(src) = state.drag_slot.take() {
                if let Some(dst) = target {
                    move_any(inv, &mut state.grid, src, dst);
                } else if let (Some(tr), Some(pos)) = (trash_rect, pointer) {
                    if tr.contains(pos) {
                        set(inv, &mut state.grid, src, None);
                    }
                } else if let Some(pos) = pointer {
                    // Released outside the window (well away from every slot): throw it out.
                    let area = slot_rects.iter().fold(Rect::NOTHING, |a, (_, r)| a.union(*r)).expand(36.0);
                    if !area.contains(pos) {
                        if let Some(s) = get(inv, &state.grid, src) {
                            set(inv, &mut state.grid, src, None);
                            result.thrown.push(s);
                        }
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
    // Item name over the slot under the finger / mouse.
    if let Some((text, rect, t)) = &mut state.caption {
        *t -= dt;
        if *t <= 0.0 {
            state.caption = None;
        } else {
            let layer = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("inv-caption")));
            let font = FontId::proportional(14.0);
            let galley = ctx.fonts(|f| f.layout_no_wrap(text.clone(), font, p.text));
            let size = galley.size() + Vec2::new(16.0, 8.0);
            let sr = ctx.screen_rect();
            let mut pill = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() - 6.0 - size.y * 0.5), size);
            pill = pill.translate(Vec2::new((sr.left() + 4.0 - pill.left()).max(0.0) + (sr.right() - 4.0 - pill.right()).min(0.0), (sr.top() + 4.0 - pill.top()).max(0.0)));
            let a = (*t * 4.0).clamp(0.0, 1.0);
            layer.rect_filled(pill, Rounding::same(6.0), p.surface.gamma_multiply(0.96 * a));
            layer.rect_stroke(pill, Rounding::same(6.0), Stroke::new(1.5, p.accent.gamma_multiply(a)));
            layer.galley(pill.min + Vec2::new(8.0, 4.0), galley, p.text);
        }
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
        let mut tip = s.display_name();
        if let Some(rules) = wolfcraft_engine::tools::describe(s.id) {
            tip += &format!("\n{rules}");
        }
        if dur > 0 {
            tip += &format!("\nПрочность {}/{}", dur - s.damage, dur);
        }
        note(state, &resp, rect, tip.replace('\n', " · "));
    }
}

/// A form/material selector shared by the creative palette and the survival workbench.
fn construction_panel(ui: &mut Ui, icons: &GuiIcons, state: &mut InventoryUiState, inv: &mut Inventory, creative: bool) -> Option<String> {
    use wolfcraft_engine::construction as c;
    use crate::ui_components::{pill_button, ButtonKind};
    state.build_form=state.build_form.min(c::FORMS.len()-1);
    ui.horizontal_wrapped(|ui| {
        ui.label("Форма");
        egui::ComboBox::from_id_salt("build-form").selected_text(c::FORMS[state.build_form].1).width(220.0).show_ui(ui, |ui| {
            for (i,(_,name)) in c::FORMS.iter().enumerate() {ui.selectable_value(&mut state.build_form,i,*name);}
        });
        let form=c::FORMS[state.build_form].0;
        let valid:Vec<usize>=c::ENTRIES.iter().filter(|e|e.form==form).map(|e|e.material).collect();
        if !valid.contains(&state.build_material) { state.build_material=valid[0]; }
        ui.label("Материал");
        egui::ComboBox::from_id_salt("build-material").selected_text(c::MATERIALS[state.build_material].name).width(170.0).show_ui(ui, |ui| {
            for m in valid {ui.selectable_value(&mut state.build_material,m,c::MATERIALS[m].name);}
        });
    });
    let form=c::FORMS[state.build_form].0;
    let entry=c::ENTRIES.iter().find(|e|e.form==form&&e.material==state.build_material)?;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let (rect,_)=ui.allocate_exact_size(Vec2::splat(110.0),Sense::hover());
        icons.paint_item(ui.painter(),rect.shrink(6.0),entry.id);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(entry.name).font(bold_font(16.0)));
            ui.label("Поворот при установке — по направлению взгляда");
            if matches!(form,c::Form::Sofa|c::Form::Counter|c::Form::KitchenBase) { ui.label("Ставьте секции рядом, чтобы собрать мебель нужной длины"); }
        });
    });
    let recipe=c::recipe(entry.id)?;
    if !creative {
        ui.label("Нужно:");
        for (id,count) in &recipe.ingredients {ui.label(format!("{} ×{}  (есть {})",items::info(*id).name,count,inv.count(*id)));}
    }
    let can=creative||inv.can_craft(&recipe,true);
    if pill_button(ui,icons,Some(icon::PLUS),if creative{"Взять в руку"}else{"Изготовить"},ButtonKind::Primary,180.0,can).clicked() {
        if creative {inv.slots[inv.selected]=Some(ItemStack::new(entry.id,64));}
        else if !inv.craft(&recipe,true) {return None;}
        return Some(format!("{} готов",entry.name));
    }
    None
}

/// Survival inventory: character (with the second-hand slot) and crafting on top,
/// the whole backpack below, the hotbar at the bottom.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn survival_layout(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, near_table: bool, width: f32, slot_rects: &mut Vec<(usize, Rect)>, hotbar: bool) -> Option<String> {
    let mut crafted = None;
    ui.set_width(width);
    if state.builder {
        // The «Конструктор» block: form, material, the piece and what it takes.
        let free = state.builder_free;
        crafted = construction_panel(ui, icons, state, inv, free);
    } else if near_table {
        {
        // The crafting table has its own screen: a recipe book on the left (tap a recipe to
        // lay it out), the big 3×3 grid on the right; no character panel.
        ui.horizontal_top(|ui| {
            let grid_w = 3.0 * (CELL + GAP) + 28.0 + 24.0 + CELL + 8.0 + 8.0;
            let book_w = (width - grid_w - 24.0).max(CELL * 2.0);
            ui.vertical(|ui| {
                ui.set_width(book_w);
                recipe_book(ui, icons, p, state, inv, book_w);
            });
            ui.add_space(24.0);
            ui.vertical(|ui| {
                crafted = crafting_grid(ui, icons, p, state, inv, near_table, slot_rects);
            });
        });
        }
    } else {
    ui.horizontal_top(|ui| {
        // ---- Character ----
        character_section(ui, icons, p, state, inv, slot_rects);
        ui.add_space(24.0);
        // ---- Crafting ----
        ui.vertical(|ui| {
            crafted = crafting_grid(ui, icons, p, state, inv, near_table, slot_rects);
        });
    });
    }
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
    if !hotbar {
        return crafted;
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

/// Armor slots, the figure and the second hand.
fn character_section(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, slot_rects: &mut Vec<(usize, Rect)>) {
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
                ui.painter().rect_filled(r, Rounding::same(10.0), p.surface_hi);
                ui.painter().rect_stroke(r, Rounding::same(10.0), Stroke::new(1.0_f32, p.outline));
                let armor = inv.armor.map(|s| s.and_then(|s| items::armor_material(s.id)));
                paint_character(ui.painter(), r.shrink(10.0), inv.offhand.is_some_and(|s| s.id == items::ids::SHIELD), armor);
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
}

/// The backpack rows (slots after the hotbar).
fn backpack_section(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, cell: f32, slot_rects: &mut Vec<(usize, Rect)>) {
    let backpack: Vec<usize> = (HOTBAR..SLOTS).collect();
    for row in backpack.chunks(9) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(GAP);
            for &slot in row {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(cell), Sense::hover());
                slot_rects.push((slot, rect));
                draw_slot(ui, icons, p, state, inv, slot, rect);
            }
        });
    }
}

/// Blocky front view of the player, in the skin's colours.
/// Recipe book of the crafting table: every recipe as a tile, the ones the backpack can make
/// first and bright, the rest dimmed. A tap lays the recipe out in the grid.
fn recipe_book(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, width: f32) {
    use wolfcraft_engine::inventory::recipes;
    section_label(ui, "Рецепты");
    ui.add_space(4.0);
    let cell = CELL * 0.85;
    let per_row = ((width + GAP) / (cell + GAP)).floor().max(1.0) as usize;
    let mut list: Vec<(&'static wolfcraft_engine::inventory::Recipe, bool)> = recipes().iter().map(|r| (r, state.grid.can_fill(r, inv))).collect();
    // Craftable first, then by category; stable within.
    list.sort_by_key(|(r, ok)| (!ok, r.category));
    let can = list.iter().filter(|x| x.1).count();
    ui.label(egui::RichText::new(format!("Можно сделать: {can} из {}", list.len())).size(12.0).color(p.text_muted));
    ui.add_space(4.0);
    egui::ScrollArea::vertical().id_salt("recipe-book").max_height(3.0 * (CELL + GAP) + 6.0).auto_shrink([false, true]).show(ui, |ui| {
        for (row_i, row) in list.chunks(per_row).enumerate() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(GAP);
                for (i, (r, ok)) in row.iter().enumerate() {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(cell), Sense::hover());
                    let out = ItemStack::new(r.output.0, r.output.1);
                    let picked = state.grid.prefer == Some(r.output) && state.grid.matched().is_some_and(|m| m.output == r.output);
                    let resp = slot_widget(ui, icons, p, rect, Some(&out), picked, Id::new(("recipe", row_i, i)), Sense::click());
                    if !ok {
                        ui.painter().rect_filled(rect, Rounding::same(10.0), Color32::from_black_alpha(140));
                    }
                    let need: Vec<String> = CraftGrid::needs(r).iter().map(|(id, n)| format!("{} ×{n}", ItemStack::new(*id, 1).info().name)).collect();
                    let tip = format!("{}{}\n{}", r.name(), if r.output.1 > 1 { format!(" ×{}", r.output.1) } else { String::new() }, need.join(", "));
                    let full = if *ok { tip } else { format!("{tip}\nНе хватает предметов") };
                    note(state, &resp, rect, full.replace('\n', " · "));
                    if resp.clicked() && *ok {
                        state.grid.fill_from(r, inv);
                    }
                }
            });
        }
    });
}

fn paint_character(painter: &egui::Painter, r: Rect, shield: bool, armor: [Option<u8>; 4]) {
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
    // Worn armour over the clothes, in the material's colour with a lighter rim on top.
    let ac = |m: u8| {
        let v = wolfcraft_engine::renderer::model::armor_color(m);
        c(v[0].min(1.0), v[1].min(1.0), v[2].min(1.0))
    };
    let lit = |col: Color32| Color32::from_rgb(col.r().saturating_add(40), col.g().saturating_add(40), col.b().saturating_add(40));
    let dark = |col: Color32| Color32::from_rgb((col.r() as f32 * 0.7) as u8, (col.g() as f32 * 0.7) as u8, (col.b() as f32 * 0.7) as u8);
    if let Some(m) = armor[0] {
        let a = ac(m);
        b(3.5, -0.5, 9.0, 3.5, a);
        b(3.5, -0.5, 1.5, 7.0, a);
        b(11.0, -0.5, 1.5, 7.0, a);
        b(3.5, -0.5, 9.0, 1.0, lit(a));
    }
    if let Some(m) = armor[1] {
        let a = ac(m);
        b(3.5, 7.5, 9.0, 11.0, a);
        b(-0.5, 7.5, 4.5, 5.0, a);
        b(12.0, 7.5, 4.5, 5.0, a);
        b(3.5, 7.5, 9.0, 1.0, lit(a));
        b(7.5, 9.0, 1.0, 9.0, dark(a));
    }
    if let Some(m) = armor[2] {
        let a = ac(m);
        b(3.5, 18.0, 9.0, 2.5, a);
        b(3.5, 20.0, 4.3, 7.0, a);
        b(8.2, 20.0, 4.3, 7.0, a);
        b(3.5, 18.0, 9.0, 0.8, lit(a));
    }
    if let Some(m) = armor[3] {
        let a = ac(m);
        b(3.5, 28.0, 4.6, 4.2, a);
        b(7.9, 28.0, 4.6, 4.2, a);
        b(3.5, 28.0, 9.0, 0.8, lit(a));
    }
    if shield {
        // Shield on the character's left arm (viewer's right).
        b(11.0, 11.0, 5.5, 8.0, c(0.62, 0.64, 0.68));
        b(11.7, 11.7, 4.1, 6.6, c(0.62, 0.45, 0.25));
    }
}

/// Creative inventory laid out like the classic block game: a row of icon tabs on top,
/// the item grid below, then the hotbar with the bin slot on its right.
#[allow(clippy::too_many_arguments)]
fn creative_layout(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, cell: f32, rows: usize, cols: usize, grid_w: f32, slot_rects: &mut Vec<(usize, Rect)>, trash_rect: &mut Option<Rect>, result: &mut InventoryResult) {
    let palette_all = items::creative_palette();
    let full_w = ui.available_width();
    // Mode: 0 Творчество (the item grid), 1 Крафт, 2 Персонаж, 3 Рюкзак.
    let mode = if state.tab == BACKPACK_TAB { match state.inv_page { 1 => 1, 0 => 2, _ => 3 } } else { 0 };
    // ---- Header: icon + title on the left, modes and «Закрыть» on the right ----
    ui.horizontal(|ui| {
        ui.set_width(full_w);
        ui.set_height(38.0);
        let (ir, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
        match mode {
            0 => icons.paint(ui.painter(), ir, icon::STAR, p.accent),
            1 => icons.paint_item(ui.painter(), ir, wolfcraft_engine::block::Block::CraftingTable as ItemId),
            2 => paint_character(ui.painter(), ir, false, inv.armor.map(|s| s.and_then(|s| items::armor_material(s.id)))),
            _ => icons.paint(ui.painter(), ir, icon::BACKPACK, p.accent),
        }
        let title = ["Творчество", "Крафт", "Персонаж", "Рюкзак"][mode];
        ui.label(egui::RichText::new(title).font(bold_font(19.0)).color(p.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            // «Закрыть»: a red pill, always visible.
            let (cr, cresp) = ui.allocate_exact_size(Vec2::new(104.0, 34.0), Sense::click());
            let hot = cresp.hovered() || cresp.is_pointer_button_down_on();
            ui.painter().rect_filled(cr, Rounding::same(17.0), if hot { Color32::from_rgb(210, 70, 60) } else { Color32::from_rgb(180, 55, 50) });
            ui.painter().rect_stroke(cr, Rounding::same(17.0), Stroke::new(1.0_f32, Color32::from_white_alpha(70)));
            icons.paint(ui.painter(), Rect::from_center_size(Pos2::new(cr.left() + 20.0, cr.center().y), Vec2::splat(14.0)), icon::CLOSE, Color32::WHITE);
            ui.painter().text(Pos2::new(cr.left() + 32.0, cr.center().y), Align2::LEFT_CENTER, "Закрыть", bold_font(14.0), Color32::WHITE);
            if cresp.clicked() {
                result.close = true;
            }
            ui.add_space(6.0);
            // Modes, right to left: Рюкзак, Персонаж, Крафт, Творчество.
            for m in [3usize, 2, 1, 0] {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(40.0, 34.0), Sense::click());
                let on = m == mode;
                let hot = resp.hovered() || resp.is_pointer_button_down_on();
                ui.painter().rect_filled(r, Rounding::same(17.0), if on { p.accent } else if hot { p.surface_hover } else { p.surface_hi });
                ui.painter().rect_stroke(r, Rounding::same(17.0), Stroke::new(1.0_f32, if on { p.accent } else { p.outline }));
                let inner = Rect::from_center_size(r.center(), Vec2::splat(22.0));
                match m {
                    0 => icons.paint(ui.painter(), inner, icon::STAR, if on { Color32::WHITE } else { p.text }),
                    1 => icons.paint_item(ui.painter(), inner, wolfcraft_engine::block::Block::CraftingTable as ItemId),
                    2 => paint_character(ui.painter(), inner.shrink(1.0), false, inv.armor.map(|s| s.and_then(|s| items::armor_material(s.id)))),
                    _ => icons.paint(ui.painter(), inner, icon::BACKPACK, if on { Color32::WHITE } else { p.text }),
                }
                if resp.clicked() && !on {
                    if m == 0 {
                        state.tab = state.last_tab.min(BACKPACK_TAB - 1);
                    } else {
                        if state.tab != BACKPACK_TAB {
                            state.last_tab = state.tab;
                        }
                        state.tab = BACKPACK_TAB;
                        state.inv_page = match m {
                            1 => 1,
                            2 => 0,
                            _ => 2,
                        };
                        state.search_open = false;
                        state.search.clear();
                    }
                }
                resp.on_hover_text(["Творчество", "Крафт", "Персонаж", "Рюкзак"][m]);
            }
        });
    });
    ui.add_space(4.0);
    let grid_h = rows as f32 * (cell + GAP) - GAP + 12.0;
    // ---- Category menu: pills in one row with arrows (Творчество only) ----
    if mode == 0 {
        ui.horizontal(|ui| {
            ui.set_width(full_w);
            ui.spacing_mut().item_spacing.x = 6.0;
            let arrow = |ui: &mut Ui, left: bool| -> bool {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(30.0, 34.0), Sense::click());
                let hot = resp.hovered() || resp.is_pointer_button_down_on();
                ui.painter().rect_filled(r, Rounding::same(15.0), if hot { p.surface_hover } else { p.surface_hi });
                let c = r.center();
                let d = if left { -1.0 } else { 1.0 };
                ui.painter().line_segment([c + Vec2::new(-4.0 * d, -7.0), c + Vec2::new(4.0 * d, 0.0)], Stroke::new(2.5_f32, p.text));
                ui.painter().line_segment([c + Vec2::new(4.0 * d, 0.0), c + Vec2::new(-4.0 * d, 7.0)], Stroke::new(2.5_f32, p.text));
                resp.clicked()
            };
            if arrow(ui, true) {
                state.cat_scroll = (state.cat_scroll - 220.0).max(0.0);
                state.cat_scroll_set = true;
            }
            let strip_w = ui.available_width() - 36.0;
            let mut sa = egui::ScrollArea::horizontal().id_salt("creative-cat-pills").max_width(strip_w).auto_shrink([false, true]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
            if state.cat_scroll_set {
                sa = sa.horizontal_scroll_offset(state.cat_scroll);
                state.cat_scroll_set = false;
            }
            let out = sa.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    // Search first: icon + «Поиск».
                    let pill = |ui: &mut Ui, on: bool, label: &str, paint: &dyn Fn(&egui::Painter, Rect)| -> bool {
                        let galley = ui.painter().layout_no_wrap(label.to_string(), bold_font(13.0), if on { Color32::WHITE } else { p.text });
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 44.0, 34.0), Sense::click());
                        let hot = resp.hovered() || resp.is_pointer_button_down_on();
                        ui.painter().rect_filled(r, Rounding::same(17.0), if on { p.accent } else if hot { p.surface_hover } else { p.surface_hi });
                        ui.painter().rect_stroke(r, Rounding::same(17.0), Stroke::new(1.0_f32, if on { p.accent } else { p.outline }));
                        paint(ui.painter(), Rect::from_center_size(Pos2::new(r.left() + 20.0, r.center().y), Vec2::splat(20.0)));
                        ui.painter().galley(Pos2::new(r.left() + 34.0, r.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
                        resp.clicked()
                    };
                    let so = state.search_open;
                    if pill(ui, so, "Поиск", &|pt, r| icons.paint(pt, r, icon::SEARCH, if so { Color32::WHITE } else { p.text })) {
                        state.search_open = !state.search_open;
                        if state.search_open {
                            ui.ctx().data_mut(|d| d.insert_temp(Id::new("inv-search-fresh"), true));
                        } else {
                            state.search.clear();
                        }
                    }
                    for (i, name) in CREATIVE_TABS.iter().enumerate().take(BACKPACK_TAB) {
                        let sel = state.tab == i && !state.search_open;
                        let first = palette_all.iter().copied().find(|id| items::category(*id) == *name);
                        let clicked = pill(ui, sel, name, &|pt, r| match (i, first) {
                            (0, _) => icons.paint(pt, r, icon::STAR, if sel { Color32::WHITE } else { p.accent }),
                            (_, Some(id)) => icons.paint_item(pt, r, id),
                            _ => icons.paint(pt, r, icon::CUBE, p.text),
                        });
                        if clicked {
                            state.tab = i;
                            state.search_open = false;
                            state.search.clear();
                        }
                    }
                });
            });
            state.cat_scroll = out.state.offset.x;
            if arrow(ui, false) {
                state.cat_scroll += 220.0;
                state.cat_scroll_set = true;
            }
        });
        ui.add_space(6.0);
    }
    // ---- The section: the item grid (or the page of Крафт / Персонаж / Рюкзак) ----
    let section_h = grid_h + if mode == 0 { 0.0 } else { 40.0 + 6.0 };
    ui.allocate_ui_with_layout(Vec2::new(full_w, section_h), egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.set_width(full_w);
        ui.set_height(section_h);
        ui.allocate_ui_with_layout(Vec2::new(grid_w, section_h), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(grid_w);
            creative_main(ui, icons, p, state, inv, cell, rows, cols, grid_w, palette_all, slot_rects, trash_rect, result);
        });
    });
    // ---- Hotbar and the bin (double tap: clear everything) ----
    ui.add_space(6.0);
    ui.vertical_centered(|ui| {
        hotbar_row(ui, icons, p, state, inv, cell, slot_rects, trash_rect);
    });
}

/// The hotbar as one row along the bottom (slots 1–9) with the bin at the end, centred.
#[allow(clippy::too_many_arguments)]
fn hotbar_row(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, s: f32, slot_rects: &mut Vec<(usize, Rect)>, trash_rect: &mut Option<Rect>) {
    let (area, _) = ui.allocate_exact_size(Vec2::new(10.0 * s + 9.0 * GAP + 8.0, s), Sense::hover());
    let at = |i: usize| Rect::from_min_size(area.min + Vec2::new(i as f32 * (s + GAP) + if i == 9 { 8.0 } else { 0.0 }, 0.0), Vec2::splat(s));
    for slot in 0..HOTBAR {
        let rect = at(slot);
        slot_rects.push((slot, rect));
        draw_slot(ui, icons, p, state, inv, slot, rect);
    }
    bin(ui, icons, p, state, inv, at(9), s, trash_rect);
}

fn bin(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, rect: Rect, s: f32, trash_rect: &mut Option<Rect>) {
    let resp = ui.interact(rect, Id::new("inv-bin"), Sense::click());
    let hot = resp.hovered() || state.drag_slot.is_some();
    ui.painter().rect_filled(rect, Rounding::same(10.0), p.danger.gamma_multiply(if hot { 0.35 } else { 0.15 }));
    ui.painter().rect_stroke(rect, Rounding::same(10.0), Stroke::new(2.0_f32, p.danger.gamma_multiply(0.7)));
    icons.paint(ui.painter(), rect.shrink(s * 0.24), icon::TRASH, p.danger);
    // Tap: delete the item in the selected hotbar slot. Or drag any slot here.
    if resp.double_clicked() {
        // Double tap: everything goes (hotbar and backpack).
        for s in inv.slots.iter_mut() {
            *s = None;
        }
        state.toast = Some(("Инвентарь очищен".to_string(), 1.6));
    } else if resp.clicked() {
        inv.slots[inv.selected] = None;
    }
    resp.on_hover_text("Удалить: нажмите — предмет в выбранном слоте; двойное нажатие — всё; или перетащите сюда");
    *trash_rect = Some(rect);
}

/// One cell of the creative grid.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cell {
    Item(ItemId),
    /// A folded or unfolded group of building pieces: form, its first piece, how many
    /// materials, unfolded.
    Group(usize, ItemId, usize, bool),
    /// A piece of the unfolded group.
    Member(ItemId),
}

/// Folds the building pieces of `list` into one cell per form (`grouped`), unfolding `open`.
fn group_cells(list: &[ItemId], open: Option<usize>, grouped: bool) -> Vec<Cell> {
    if !grouped {
        return list.iter().copied().map(Cell::Item).collect();
    }
    let form_of = |id: ItemId| wolfcraft_engine::construction::entry(id).map(|(e, _)| e.form as usize);
    let mut cells = Vec::with_capacity(list.len());
    let mut seen: Vec<usize> = Vec::new();
    for id in list {
        match form_of(*id) {
            None => cells.push(Cell::Item(*id)),
            Some(f) if !seen.contains(&f) => {
                seen.push(f);
                let members: Vec<ItemId> = list.iter().copied().filter(|m| form_of(*m) == Some(f)).collect();
                let is_open = open == Some(f);
                cells.push(Cell::Group(f, *id, members.len(), is_open));
                if is_open {
                    cells.extend(members.into_iter().map(Cell::Member));
                }
            }
            Some(_) => {}
        }
    }
    cells
}

/// Right part of the creative inventory: category title and search, the item grid, the hotbar.
#[allow(clippy::too_many_arguments)]
fn creative_main(ui: &mut Ui, icons: &GuiIcons, p: &Palette, state: &mut InventoryUiState, inv: &mut Inventory, cell: f32, rows: usize, cols: usize, grid_w: f32, palette_all: Vec<ItemId>, slot_rects: &mut Vec<(usize, Rect)>, trash_rect: &mut Option<Rect>, result: &mut InventoryResult) {
    let _ = &result;
    // ---- Tab title + search ----
    ui.horizontal(|ui| {
        if state.search_open {
            // The magnifier is a tab of its own: a search over every item and block.
            let r = text_input(ui, icons, &mut state.search, "Поиск по всем предметам…", grid_w - 14.0, Some(icon::SEARCH));
            let fresh = ui.ctx().data_mut(|d| std::mem::take(d.get_temp_mut_or_default::<bool>(Id::new("inv-search-fresh"))));
            if fresh {
                r.request_focus();
            }
        }
    });
    if state.search_open {
        ui.add_space(6.0);
    }
    // ---- Grid ----
    // The grid takes all the height left above the hotbar (a cut-off last row shows there
    // is more to scroll), so no empty band stays under the hotbar.
    let _ = rows;
    let grid_h = (ui.available_height() - 4.0).max(cell);
    if state.tab == BACKPACK_TAB && !state.search_open {
        // The survival inventory inside the creative one: armor, second hand, 2x2 crafting,
        // backpack and hotbar (scrolls on small screens).
        let (area, _) = ui.allocate_exact_size(Vec2::new(grid_w, grid_h), Sense::hover());
        // A recessed metal panel behind the slots, like in the block windows.
        icons.paint_panel(ui.painter(), area.expand(3.0), wolfcraft_assets::gui::PANEL_METAL, Color32::WHITE);
        let area = area.shrink(4.0);
        let content = area;
        let mut ui2 = ui.new_child(egui::UiBuilder::new().max_rect(content));
        egui::ScrollArea::vertical().id_salt(("creative-survival-inv", state.inv_page)).auto_shrink([false, false]).show(&mut ui2, |ui| {
            ui.set_width(content.width() - 8.0);
            match state.inv_page {
                0 => character_section(ui, icons, p, state, inv, slot_rects),
                1 => {
                    if let Some(msg) = crafting_grid(ui, icons, p, state, inv, false, slot_rects) {
                        result.crafted = Some(msg);
                    }
                }
                _ => {
                    let c = ((content.width() - 16.0 - 8.0 * GAP) / 9.0).min(cell);
                    backpack_section(ui, icons, p, state, inv, c, slot_rects);
                }
            }
        });
        return;
    } else {
        let mut list: Vec<ItemId> = palette_all.into_iter().filter(|id| (state.search_open || in_tab(*id, state.tab) || !state.search.trim().is_empty()) && matches_search(*id, &state.search)).collect();
        // Sorted: by category (in "Все"), then full blocks, shaped blocks (stairs, slabs,
        // doors…), plants, items and tools, alphabetically inside each group so related
        // things sit together (all oak, all diamond…).
        list.sort_by_cached_key(|id| sort_key(*id));
        // Building pieces come in a dozen materials each: like on Bedrock, every form is one
        // cell with a "+" that unfolds its materials in place (a search shows them all).
        let cells = group_cells(&list, state.open_group, state.search.trim().is_empty());
        let list = cells;
        let (area, _) = ui.allocate_exact_size(Vec2::new(grid_w, grid_h), Sense::hover());
        // A recessed metal panel behind the slots, like in the block windows.
        icons.paint_panel(ui.painter(), area.expand(3.0), wolfcraft_assets::gui::PANEL_METAL, Color32::WHITE);
        let area = area.shrink(4.0);
        let mut ui2 = ui.new_child(egui::UiBuilder::new().max_rect(area));
        // Swipe scrolls the list (a slot only reacts to a tap, so the finger drag goes to the
        // scroll area); the bar stays visible so it is clear there is more below.
        egui::ScrollArea::vertical()
            .id_salt(("creative-scroll", state.tab))
            .auto_shrink([false, false])
            .drag_to_scroll(true)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show_rows(&mut ui2, cell + GAP, list.len().div_ceil(cols), |ui, visible| {
            for chunk in list.chunks(cols).skip(visible.start).take(visible.end-visible.start) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(GAP);
                    for entry in chunk {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(cell), Sense::hover());
                        let id = match entry {
                            Cell::Group(form, head, count, open) => {
                                // The group's cell: its first piece, a "+" (or "−") badge and
                                // how many materials are inside. A tap folds or unfolds it.
                                let stack = ItemStack::new(*head, 1);
                                let resp = slot_widget(ui, icons, p, rect, Some(&stack), *open, Id::new(("pal-group", *form)), Sense::click());
                                let badge = Rect::from_min_size(rect.right_top() + Vec2::new(-20.0, 2.0), Vec2::splat(18.0));
                                ui.painter().circle_filled(badge.center(), 9.0, p.accent.gamma_multiply(0.92));
                                ui.painter().circle_stroke(badge.center(), 9.0, Stroke::new(1.0_f32, Color32::from_white_alpha(90)));
                                ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, if *open { "−" } else { "+" }, bold_font(15.0), p.on_accent);
                                ui.painter().text(rect.left_bottom() + Vec2::new(5.0, -3.0), egui::Align2::LEFT_BOTTOM, count.to_string(), egui::FontId::proportional(10.5), Color32::from_white_alpha(130));
                                if resp.clicked() {
                                    state.open_group = if *open { None } else { Some(*form) };
                                }
                                let name = items::info(*head).name;
                                note(state, &resp, rect, name.split_once(" · ").map(|n| n.1).unwrap_or(name));
                                continue;
                            }
                            Cell::Member(id) => {
                                // Pieces of the open group sit on a tinted strip.
                                ui.painter().rect_filled(rect.expand(GAP / 2.0), Rounding::same(10.0), p.accent.gamma_multiply(0.18));
                                id
                            }
                            Cell::Item(id) => id,
                        };
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
                        if items::is_new(*id) && *id < 1024 {
                            let tag = Rect::from_min_size(rect.left_top() + Vec2::new(2.0, 2.0), Vec2::new(22.0, 10.0));
                            ui.painter().rect_filled(tag, Rounding::same(2.0), p.success);
                            ui.painter().text(tag.center(), egui::Align2::CENTER_CENTER, "NEW", egui::FontId::proportional(7.5), egui::Color32::BLACK);
                        }
                        let name = items::info(*id).name;
                        note(state, &resp, rect, match wolfcraft_engine::tools::describe(*id) { Some(r) => format!("{name} · {r}"), None => name.to_string() });
                    }
                });
                ui.add_space(GAP - ui.spacing().item_spacing.y);
            }
            if list.is_empty() {
                ui.label(egui::RichText::new("Ничего не найдено").color(p.text_muted));
            }
        });
    }
    let _ = (trash_rect, slot_rects.len());
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
    fn building_pieces_fold_into_one_cell_per_form() {
        use wolfcraft_engine::construction as c;
        let mut list = items::creative_palette();
        list.sort_by_cached_key(|id| sort_key(*id));
        let pieces = list.iter().filter(|id| c::entry(**id).is_some()).count();
        assert!(pieces > 200, "the catalogue is in the palette: {pieces}");
        let folded = group_cells(&list, None, true);
        let groups: Vec<_> = folded.iter().filter_map(|x| if let Cell::Group(f, _, n, open) = x { Some((*f, *n, *open)) } else { None }).collect();
        assert!(!folded.iter().any(|x| matches!(x, Cell::Member(_))), "nothing unfolded");
        assert!(groups.len() >= 40 && groups.len() <= c::FORMS.len(), "one cell per form: {}", groups.len());
        assert_eq!(groups.iter().map(|g| g.1).sum::<usize>(), pieces, "every piece is in a group");
        assert_eq!(folded.len(), list.len() - pieces + groups.len());
        // No form twice.
        let mut forms: Vec<usize> = groups.iter().map(|g| g.0).collect();
        forms.sort();
        forms.dedup();
        assert_eq!(forms.len(), groups.len());
        // Unfolding one group shows exactly its pieces right after its cell.
        let (form, n, _) = groups[3];
        let open = group_cells(&list, Some(form), true);
        let at = open.iter().position(|x| matches!(x, Cell::Group(f, ..) if *f == form)).unwrap();
        assert!(matches!(open[at], Cell::Group(_, _, _, true)));
        assert!(open[at + 1..=at + n].iter().all(|x| matches!(x, Cell::Member(id) if c::entry(*id).is_some_and(|(e, _)| e.form as usize == form))));
        assert_eq!(open.len(), folded.len() + n);
        // A search lists pieces one by one.
        assert_eq!(group_cells(&list, None, false).len(), list.len());
    }

    #[test]
    fn every_item_has_a_tab_and_every_tab_has_items() {
        let all = items::creative_palette();
        for (i, name) in CREATIVE_TABS.iter().enumerate().skip(1).take(BACKPACK_TAB - 1) {
            if *name == "Конструктор" {
                continue;
            }
            assert!(all.iter().any(|id| in_tab(*id, i)), "tab «{name}» is empty");
        }
        // Items of put-away categories are still reachable under "Все" and by search.
        let orphans: Vec<&str> = all.iter().filter(|id| !(1..BACKPACK_TAB).any(|t| in_tab(**id, t))).map(|id| items::category(*id)).collect();
        assert!(orphans.iter().all(|c| *c == "Техника"), "items without a tab: {orphans:?}");
        // Names are unique enough to search: no empty or placeholder names.
        assert!(all.iter().all(|id| !items::info(*id).name.is_empty() && items::info(*id).name != "???"));
    }

    #[test]
    fn creative_inventory_fits_phone_screens_on_every_tab() {
        let (ctx, icons) = ctx_with_icons();
        let mut inv = Inventory::default();
        for (w, h) in [(802.0f32, 360.0f32), (640.0, 360.0), (960.0, 432.0), (1400.0, 800.0)] {
            let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h));
            for tab in 0..CREATIVE_TABS.len() {
                let mut st = InventoryUiState { tab, ..Default::default() };
                for pass in 0..3 {
                    if pass == 2 {
                        st.open_group = Some(wolfcraft_engine::construction::Form::Chair as usize);
                    }
                    let mut raw = egui::RawInput::default();
                    raw.screen_rect = Some(screen);
                    let mut slots = 0;
                    let _ = ctx.run(raw, |c| {
                        show(c, &icons, &mut st, &mut inv, true, false, 0.016);
                        slots += 1;
                    });
                    assert!(slots > 0);
                }
                let used = ctx.used_rect();
                assert!(used.right() <= w + 1.0 && used.bottom() <= h + 1.0 && used.left() >= -1.0 && used.top() >= -1.0, "tab {} at {w}x{h} spills: {used:?}", CREATIVE_TABS[tab]);
            }
        }
    }

    #[test]
    fn construction_selector_can_give_any_visible_material_shape() {
        use wolfcraft_engine::construction as c;
        let ctx=egui::Context::default();crate::theme_manager::install(&ctx);crate::theme_manager::apply(&ctx,wolfcraft_engine::config::ThemeMode::Dark);
        let atlas=wolfcraft_assets::AtlasManager::new(None).unwrap();let icons=GuiIcons::new(&ctx,&atlas);
        let mut inv=Inventory::default();let mut state=InventoryUiState::default();
        state.build_form=c::FORMS.iter().position(|(f,_)|*f==c::Form::Arc45Large).unwrap();
        state.build_material=8;
        let raw=egui::RawInput {screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(800.,540.))),..Default::default()};
        let _=ctx.run(raw,|ctx|{egui::CentralPanel::default().show(ctx,|ui|{construction_panel(ui,&icons,&mut state,&mut inv,true);});});
        assert_eq!(state.build_material,8);
        assert!(c::block(c::FORMS[state.build_form].0,state.build_material,0).is_some());
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

