//! Inventory screen: header bar (title + close), category navbar, inventory slots of the
//! selected category, and on the right the creative palette or the survival crafting grid
//! (2×2 by hand, 3×3 at a crafting table).

use crate::gui_renderer::GuiIcons;
use crate::hud::draw_stack;
use crate::theme_manager::{ANIM_TIME, Palette, bold_font, palette};
use crate::ui_components::text_input;
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
    catalog_sort: usize,
    pub build_form: usize,
    pub build_material: usize,
    pub build_open: bool,
    /// Opened from the «Конструктор» block: only the building-piece maker is shown.
    pub builder: bool,
    /// The builder in creative: pieces are taken for free.
    builder_free: bool,
    pub grid: CraftGrid,
    recipes_open: bool,
    confirm_clear: bool,
    recipe_search: String,
    picked_slot: Option<usize>,
    hold_transfer: Option<(usize,usize,f64,u32)>,
    hold_used: bool,
    hold_seen: bool,
    touch_source: Option<usize>,
    touch_origin: Option<Pos2>,
    touch_spreading: bool,
    spread_cells: Vec<usize>,
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
    state.picked_slot = None;
    state.hold_transfer = None;
    state.hold_used = false;
    state.touch_source=None;state.touch_origin=None;state.touch_spreading=false;state.spread_cells.clear();
    state.recipes_open = false;
    state.confirm_clear=false;
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
fn sort_key(id: ItemId) -> (usize,u16,u16,u16,String,ItemId) {
    use wolfcraft_engine::block::Block;
    let info=items::info(id);let name=info.name.to_lowercase().replace('ё',"е");
    let cat=CREATIVE_TABS.iter().position(|c|*c==items::category(id)).unwrap_or(99);
    if let Some((e,_))=wolfcraft_engine::construction::entry(id) {
        return (cat,1000+e.form as u16,0,e.material as u16,name,id);
    }
    if let Some((kind,tier))=items::tool_shape(id) {return (cat,400+kind as u16,tier as u16,0,name,id);}
    if let Some(slot)=items::armor_slot(id) {return (cat,500,items::armor_material(id).unwrap_or(99) as u16,slot as u16,name,id);}
    if let Some(b)=info.place {
        let natural=match b {
            Block::Stone=>Some((0,0)),Block::Cobblestone=>Some((0,1)),Block::MossyCobblestone=>Some((0,2)),
            Block::Granite=>Some((0,3)),Block::Marble=>Some((0,4)),Block::Slate=>Some((0,5)),Block::Limestone=>Some((0,6)),Block::Basalt=>Some((0,7)),Block::Obsidian=>Some((0,8)),
            Block::Grass=>Some((1,0)),Block::SnowyGrass=>Some((1,1)),Block::DryGrass=>Some((1,2)),Block::SwampGrass=>Some((1,3)),Block::Dirt=>Some((1,4)),Block::Mud=>Some((1,5)),Block::Podzol=>Some((1,6)),Block::Peat=>Some((1,7)),Block::Clay=>Some((1,8)),
            Block::Sand=>Some((2,0)),Block::RedSand=>Some((2,1)),Block::Gravel=>Some((2,2)),Block::Sandstone=>Some((2,3)),
            Block::Snow=>Some((3,0)),Block::Ice=>Some((3,1)),Block::PackedIce=>Some((3,2)),
            Block::Netherrack=>Some((4,0)),Block::Ash=>Some((4,1)),_=>None,
        };
        if items::category(id)=="Природа" {if let Some((family,variant))=natural{return(cat,family,variant,0,name,id);}}
        if items::category(id)=="Дерево" {
            let species=["дуб","берез","елов","акац","вишн","ксено"].iter().position(|s|name.contains(s)).unwrap_or(99) as u16;
            let kind=if name.contains("бревн"){0}else if name.contains("доск"){1}else if name.contains("листв")||name.contains("хво"){2}else{3};
            return(cat,20+species,kind,0,name,id);
        }
        if let Some(kind)=wolfcraft_engine::wolftech::kind(b) {
            let family=if wolfcraft_engine::wolftech::is_source(kind){100}else if wolfcraft_engine::wolftech::is_store(kind){101}else{103};
            return(cat,family,kind as u16,0,name,id);
        }
        if b==Block::Cable||wolfcraft_engine::pipes::is_pipe(b){return(cat,102,0,0,name,id);}
        let family=["каменн","булыж","гранит","мрамор","кирпич","песчан","стекл","шерст"].iter().position(|f|name.contains(f)).unwrap_or(99) as u16;
        let shape=if name.contains("плит"){1}else if name.contains("ступен"){2}else if name.contains("стен"){3}else if name.contains("панел"){4}else{0};
        let color=["бел","светло-сер","сер","черн","коричн","красн","оранж","желт","лайм","зелен","бирюз","голуб","син","фиолет","пурпур","роз"].iter().position(|c|name.contains(c)).unwrap_or(99) as u16;
        return(cat,200+family,shape,color,name,id);
    }
    (cat,900,0,0,name,id)
}

fn sort_catalog(list:&mut [ItemId],mode:usize) {
    match mode {
        1=>list.sort_by_cached_key(|id|(items::info(*id).name.to_lowercase().replace('ё',"е"),*id)),
        2=>list.sort_unstable(),
        _=>list.sort_by_cached_key(|id|sort_key(*id)),
    }
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

/// Transfers one item without swapping an incompatible destination stack.
fn move_one(inv:&mut Inventory,grid:&mut CraftGrid,src:usize,dst:usize)->bool {
    if src==dst {return false;}let Some(item)=get(inv,grid,src)else{return false;};
    let target=get(inv,grid,dst);
    if target.is_some_and(|t|!t.can_stack_with(&item)||t.count>=t.info().max_stack){return false;}
    set(inv,grid,dst,Some(ItemStack{count:target.map_or(1,|t|t.count+1),..item}));
    set(inv,grid,src,if item.count>1 {Some(ItemStack{count:item.count-1,..item})}else{None});true
}

/// Java-style left-drag distribution: equal shares, remainder stays in the source.
fn distribute_stack(inv:&mut Inventory,grid:&mut CraftGrid,source:usize,targets:&[usize])->bool {
    let Some(stack)=get(inv,grid,source)else{return false;};
    let targets:Vec<_>=targets.iter().copied().filter(|dst|*dst!=source&&*dst>=GRID).collect();
    if targets.is_empty(){return false;}
    let share=stack.count as usize/targets.len();if share==0{return false;}
    let mut used=0u8;
    for dst in targets {
        let old=get(inv,grid,dst);
        if old.is_some_and(|t|!t.can_stack_with(&stack)){continue;}
        let count=old.map_or(0,|t|t.count);let n=(share as u8).min(stack.info().max_stack.saturating_sub(count));
        if n>0 {set(inv,grid,dst,Some(ItemStack{count:count+n,..stack}));used+=n;}
    }
    if used==0{return false;}
    set(inv,grid,source,if used<stack.count {Some(ItemStack{count:stack.count-used,..stack})}else{None});true
}

#[derive(Clone,Copy)]
enum ControlIcon { Gui(u16), Item(ItemId) }

const INVENTORY_BODY_H:f32=432.0;
fn section_label(ui:&mut Ui,text:&str) {ui.label(egui::RichText::new(text).font(bold_font(12.0)).color(Color32::from_rgb(164,178,192)));}
fn inventory_surface(painter:&egui::Painter,rect:Rect,recessed:bool) {
    let p = Palette::dark();
    let radius = if recessed { 12.0 } else { 18.0 };
    painter.rect_filled(rect, Rounding::same(radius), if recessed { Color32::from_rgb(14, 23, 38) } else { Color32::from_rgb(22, 36, 60) });
    let shine = Rect::from_min_max(rect.min + Vec2::splat(1.0), Pos2::new(rect.right()-1.0, rect.top()+rect.height()*0.45));
    painter.add(crate::launcher::grad_shape(shine, radius-1.0, 0.0, crate::launcher::glass_gloss(&p), Color32::TRANSPARENT));
    painter.rect_stroke(rect, Rounding::same(radius), Stroke::new(1.0, crate::launcher::glass_rim(&p)));
}

fn inventory_plate(ui:&mut Ui,icons:&GuiIcons,p:&Palette,rect:Rect,id:Id,icon:ControlIcon,label:&str,on:bool)->egui::Response {
    let response=ui.interact(rect,id,Sense::click());let edge=if on {p.accent}else{Color32::from_rgb(78,93,110)};
    let hot=response.hovered()||response.is_pointer_button_down_on();
    ui.painter().rect_filled(rect,Rounding::same(12.0),if on||hot {Color32::from_rgb(38,70,102)}else{Color32::from_rgb(22,36,60)});
    let shine=Rect::from_min_max(rect.min+Vec2::splat(1.0),Pos2::new(rect.right()-1.0,rect.center().y));
    ui.painter().add(crate::launcher::grad_shape(shine,11.0,0.0,crate::launcher::glass_gloss(&Palette::dark()),Color32::TRANSPARENT));
    ui.painter().rect_stroke(rect,Rounding::same(12.0),Stroke::new(if on {1.5}else{1.0},edge));
    let ir=Rect::from_center_size(if label.is_empty(){rect.center()}else{Pos2::new(rect.left()+18.0,rect.center().y)},Vec2::splat(22.0));
    match icon {ControlIcon::Gui(ic)=>icons.paint(ui.painter(),ir,ic,if on {p.accent}else{p.text}),ControlIcon::Item(item)=>icons.paint_item(ui.painter(),ir,item)}
    if !label.is_empty(){let text=ui.painter().layout(label.to_string(),bold_font(12.0),p.text,(rect.width()-40.0).max(1.0));ui.painter().galley(Pos2::new(rect.left()+33.0,rect.center().y-text.size().y/2.0),text,p.text);}
    response
}

fn clear_inventory(state:&mut InventoryUiState,inv:&mut Inventory) {
    inv.slots.fill(None);inv.armor.fill(None);inv.offhand=None;state.grid.cells.fill(None);state.grid.prefer=None;
    state.drag_slot=None;state.drag_item=None;state.picked_slot=None;state.touch_source=None;state.touch_spreading=false;state.spread_cells.clear();state.hold_transfer=None;state.hold_used=false;state.caption=None;
}

/// Shows the inventory. `near_table` turns the crafting grid into 3×3.
pub fn show(ctx: &egui::Context, icons: &GuiIcons, state: &mut InventoryUiState, inv: &mut Inventory, creative: bool, near_table: bool, dt: f32) -> InventoryResult {
    let mut p = Palette::dark();
    p.text=Color32::from_rgb(226,232,239);p.text_muted=Color32::from_rgb(155,171,187);p.surface_hi=Color32::from_rgb(36,43,53);p.surface_hover=Color32::from_rgb(47,58,71);p.outline=Color32::from_rgb(65,79,95);
    state.hold_seen=false;
    if state.touch_source.is_none(){ctx.data_mut(|d|d.insert_temp(Id::new("inventory-touch-drag"),false));}
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

    let recipes_were_open = state.recipes_open;
    let confirmation_was_open=state.confirm_clear;
    if creative {
        // Dark Aero surfaces match the launcher; the existing layout scales to the screen.
        let cell = 46.0f32;
        let (rows, cols) = (7usize, 9usize);
        let grid_w = cols as f32 * (cell + GAP) - GAP + 14.0;
        let width = grid_w + 216.0;
        let esc = crate::ui_components::block_screen_material(ctx, "inventory", width, false, crate::ui_components::INVENTORY_BG, |ui| {
            if confirmation_was_open {ui.set_enabled(false);}
            creative_layout(ui, icons, &p, state, inv, cell, rows, cols, grid_w, &mut slot_rects, &mut trash_rect, &mut result);
        });
        let dismissed = (!confirmation_was_open && esc) || result.close;
        if dismissed {
            result.close = true;
        }
    } else if near_table && !state.builder {
        let width = 10.0 * CELL + 9.0 * GAP + 8.0;
        let esc = crate::ui_components::block_screen_material(ctx, "inventory", width, false, crate::gui_renderer::panel_color(wolfcraft_assets::gui::PANEL_WORKBENCH), |ui| {
            if recipes_were_open { ui.set_enabled(false); }
            let (banner, _) = ui.allocate_exact_size(Vec2::new(width, 76.0), Sense::hover());
            icons.paint_block_header(ui.painter(),banner,crate::gui_renderer::block_panel(wolfcraft_engine::block::Block::CraftingTable),"Верстак");
            ui.horizontal(|ui| {
                let (r,_)=ui.allocate_exact_size(Vec2::new(144.0,36.0),Sense::hover());
                if inventory_plate(ui,icons,&p,r,Id::new("workbench-recipes-open"),ControlIcon::Gui(icon::PLUS),"Рецепты",false).clicked(){state.recipes_open=true;state.drag_slot=None;state.picked_slot=None;}
                let (r,_)=ui.allocate_exact_size(Vec2::new(112.0,36.0),Sense::hover());
                if inventory_plate(ui,icons,&p,r,Id::new("workbench-close"),ControlIcon::Gui(icon::CLOSE),"Закрыть",false).clicked(){result.close=true;}
            });
            result.crafted=survival_layout(ui,icons,&p,state,inv,true,width,&mut slot_rects,false);
            ui.add_space(8.0);
            hotbar_row(ui,icons,&p,state,inv,CELL,&mut slot_rects,&mut trash_rect);
        });
        if !recipes_were_open && esc { result.close=true; }
    } else {
        let content_w=9.0*(CELL+GAP)-GAP+14.0;
        let width=content_w+84.0;
        let esc=crate::ui_components::block_screen_material(ctx,"inventory",width,false,crate::ui_components::INVENTORY_BG,|ui| {
            let (header,_)=ui.allocate_exact_size(Vec2::new(width,68.0),Sense::hover());
            inventory_surface(ui.painter(),header,false);
            ui.painter().text(Pos2::new(header.left()+14.0,header.center().y),Align2::LEFT_CENTER,if state.builder {"Конструктор"}else{"Инвентарь"},bold_font(18.0),p.text);
            let close=Rect::from_min_size(Pos2::new(header.right()-116.0,header.center().y-18.0),Vec2::new(104.0,36.0));
            if inventory_plate(ui,icons,&p,close,Id::new("survival-close"),ControlIcon::Gui(icon::CLOSE),"Закрыть",false).clicked(){result.close=true;}
            ui.add_space(6.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(Vec2::new(content_w,INVENTORY_BODY_H),egui::Layout::top_down(egui::Align::Min),|ui| {
                    let area=Rect::from_min_size(ui.cursor().min,Vec2::new(content_w,INVENTORY_BODY_H));
                    inventory_surface(ui.painter(),area,true);
                    egui::ScrollArea::vertical().id_salt("player-inventory-body").drag_to_scroll(state.touch_source.is_none()).max_height(INVENTORY_BODY_H).auto_shrink([false,false]).show(ui,|ui| {
                        ui.set_width(content_w-12.0);
                        if state.builder {let free=state.builder_free;result.crafted=construction_panel(ui,icons,state,inv,free);}
                        else {player_contents(ui,icons,&p,state,inv,&mut slot_rects,&mut result);}
                    });
                });
                right_hotbar(ui,icons,&p,state,inv,INVENTORY_BODY_H,&mut slot_rects,&mut trash_rect);
            });
        });
        if esc||result.close {result.close=true;}
    }

    if state.recipes_open && near_table {
        let mut dismiss=false;
        let esc=crate::ui_components::block_screen_ex(ctx,"workbench-recipes",480.0,false,|ui| {
            ui.horizontal(|ui| { ui.heading("Рецепты верстака"); if ui.button("Закрыть").clicked() {dismiss=true;} });
            text_input(ui,icons,&mut state.recipe_search,"Поиск рецепта…",450.0,Some(icon::SEARCH));
            let q=state.recipe_search.trim().to_lowercase();
            egui::ScrollArea::vertical().id_salt("workbench-recipe-list").max_height(300.0).show(ui,|ui| {
                for recipe in wolfcraft_engine::inventory::recipes().iter().filter(|r|q.is_empty()||r.name().to_lowercase().contains(&q)) {
                    let can=can_quick_craft(inv,&state.grid,recipe);
                    ui.group(|ui| { ui.set_width(450.0); ui.horizontal(|ui| {
                        let (r,_)=ui.allocate_exact_size(Vec2::splat(32.0),Sense::hover());
                        icons.paint_item(ui.painter(),r,recipe.output.0);
                        ui.vertical(|ui| {
                            ui.set_width(300.0);
                            ui.label(format!("{} ×{}",recipe.name(),recipe.output.1));
                            let needs=CraftGrid::needs(recipe).iter().map(|(id,n)|format!("{} ×{}",items::info(*id).name,n)).collect::<Vec<_>>().join(", ");
                            ui.add(egui::Label::new(egui::RichText::new(needs).small().color(p.text_muted)).wrap());
                        });
                        if ui.add_enabled(can,egui::Button::new("Создать")).clicked() && quick_craft(inv,&mut state.grid,recipe) {
                            let msg=format!("Создано: {} ×{}",recipe.name(),recipe.output.1);
                            state.toast=Some((msg.clone(),2.0));result.crafted=Some(msg);
                        }
                    }); });
                }
            });
        });
        if esc||dismiss {state.recipes_open=false;}
    }
    if state.confirm_clear {
        let mut answer=None;
        let esc=crate::ui_components::block_screen_material(ctx,"inventory-clear-confirm",390.0,false,crate::ui_components::INVENTORY_BG,|ui| {
            ui.heading("Очистить инвентарь?");
            ui.add(egui::Label::new("Удалить все предметы из рюкзака, быстрого доступа, экипировки и сетки крафта?").wrap());
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                for (yes,label,ic) in [(true,"Да",icon::TRASH),(false,"Нет",icon::ARROW_LEFT)] {
                    let (r,_)=ui.allocate_exact_size(Vec2::new(176.0,40.0),Sense::hover());
                    if inventory_plate(ui,icons,&p,r,Id::new(("inventory-clear-answer",yes)),ControlIcon::Gui(ic),label,false).clicked(){answer=Some(yes);}
                }
            });
        });
        if answer==Some(true){clear_inventory(state,inv);state.toast=Some(("Инвентарь очищен".into(),2.0));}
        if esc||answer.is_some(){state.confirm_clear=false;}
    }
    #[cfg(test)]
    ctx.data_mut(|d| d.insert_temp(Id::new("inventory-test-slots"),slot_rects.clone()));
    if ctx.input(|i|i.events.iter().any(|e|matches!(e,egui::Event::Touch{phase:egui::TouchPhase::Cancel,..}))) {
        state.touch_spreading=false;state.touch_source=None;state.touch_origin=None;state.spread_cells.clear();state.drag_slot=None;state.hold_used=true;
    }
    if state.touch_spreading && ctx.input(|i|i.pointer.any_released()) && !state.spread_cells.is_empty() {
        if let Some(src)=state.touch_source {distribute_stack(inv,&mut state.grid,src,&state.spread_cells);}
        state.drag_slot=None;state.picked_slot=None;state.hold_used=true;
    }
    if !state.hold_seen {state.hold_transfer=None;}
    if state.hold_used && ctx.input(|i|i.pointer.any_released()) {
        if let Some(src)=state.drag_slot.take(){state.picked_slot=get(inv,&state.grid,src).map(|_|src);}
        state.picked_slot=state.picked_slot.filter(|src|get(inv,&state.grid,*src).is_some());
    }
    // ---- Drag & drop resolution ----
    if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
        state.last_pos = Some(pos);
    }
    let pointer = ctx.pointer_hover_pos().or(state.last_pos);
    if !recipes_were_open && !state.recipes_open && (state.drag_slot.is_some() || state.drag_item.is_some()) {
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
            let local_pointer = pointer.map(|pos| crate::ui_components::to_body(ctx,"inventory",pos));
            let target = local_pointer.and_then(|pos| slot_rects.iter().find(|(_, r)| r.contains(pos)).map(|(s, _)| *s));
            if let Some(src) = state.drag_slot.take() {
                if let Some(dst) = target {
                    move_any(inv, &mut state.grid, src, dst);
                } else if let (Some(tr), Some(pos)) = (trash_rect, local_pointer) {
                    if tr.contains(pos) {
                        set(inv, &mut state.grid, src, None);
                    }
                } else if let Some(pos) = local_pointer {
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
    if ctx.input(|i|i.pointer.any_released()) {state.hold_transfer=None;state.hold_used=false;state.touch_source=None;state.touch_origin=None;state.touch_spreading=false;state.spread_cells.clear();ctx.data_mut(|d|d.insert_temp(Id::new("inventory-touch-drag"),false));}
    if result.close {
        ctx.data_mut(|d|d.insert_temp(Id::new("inventory-touch-drag"),false));
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
            let anchor=ctx.data(|d|d.get_temp::<(f32,Vec2)>(Id::new("inventory").with("fit"))).map_or(*rect,|(s,t)|Rect::from_min_max((rect.min.to_vec2()*s+t).to_pos2(),(rect.max.to_vec2()*s+t).to_pos2()));
            let mut pill = Rect::from_center_size(Pos2::new(anchor.center().x, anchor.top() - 6.0 - size.y * 0.5), size);
            pill = pill.translate(Vec2::new((sr.left() + 4.0 - pill.left()).max(0.0) + (sr.right() - 4.0 - pill.right()).min(0.0), (sr.top() + 4.0 - pill.top()).max(0.0)));
            let a = (*t * 4.0).clamp(0.0, 1.0);
            layer.rect_filled(pill, Rounding::same(6.0), p.surface.gamma_multiply(0.96 * a));
            layer.rect_stroke(pill, Rounding::same(6.0), Stroke::new(1.5_f32, p.accent.gamma_multiply(a)));
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
    // Read raw touch geometry: ScrollArea can own the drag and never emit drag_started.
    let touch_down=ui.input(|i|i.any_touches()&&i.pointer.primary_pressed());
    if touch_down && stack.is_some() && ui.rect_contains_pointer(rect) && !state.recipes_open && !state.confirm_clear {
        state.touch_source=Some(slot);state.touch_origin=ui.input(|i|i.pointer.interact_pos());state.touch_spreading=false;state.spread_cells.clear();
        ui.ctx().data_mut(|d|d.insert_temp(Id::new("inventory-touch-drag"),true));
    }
    if let Some(src)=state.touch_source {
        if ui.input(|i|i.pointer.primary_down()||i.pointer.any_released()) {
            let moved=ui.input(|i|i.pointer.interact_pos()).zip(state.touch_origin).is_some_and(|(a,b)|a.distance(b)>6.0);
            if moved {state.touch_spreading=true;state.drag_slot=Some(src);state.picked_slot=None;}
            if state.touch_spreading && slot>=GRID && slot!=src && ui.rect_contains_pointer(rect) {
                if let Some(item)=get(inv,&state.grid,src) {
                    let fits=stack.is_none_or(|t|t.can_stack_with(&item)&&t.count<t.info().max_stack);
                    if fits&&!state.spread_cells.contains(&slot)&&state.spread_cells.len()<item.count as usize {state.spread_cells.push(slot);}
                }
            }
        }
    }
    let shown = if state.drag_slot == Some(slot) { None } else { stack };
    let selected = state.picked_slot==Some(slot) || (slot == inv.selected && slot < HOTBAR);
    let resp = slot_widget(ui, icons, p, rect, shown.as_ref(), selected, Id::new(("inv-slot", slot)), Sense::click_and_drag());
    if resp.drag_started() && stack.is_some() && state.picked_slot.is_none_or(|src|src==slot) {
        state.picked_slot=None;
        state.drag_slot = Some(slot);
    }
    if slot>=GRID && state.touch_source.is_none() && !state.recipes_open && ui.input(|i|i.pointer.primary_down()) && ui.rect_contains_pointer(rect) {
        if let Some(src)=state.picked_slot.or(state.drag_slot).filter(|src|*src!=slot) {
            state.hold_seen=true;
            let now=ui.input(|i|i.time);
            let (start,done)=state.hold_transfer.filter(|(a,b,_,_)|*a==src&&*b==slot).map(|(_,_,start,done)|(start,done)).unwrap_or((now,0));
            let wanted=if now-start>=0.35 {1+((now-start-0.35)/0.12).floor() as u32}else{0};
            for _ in done..wanted.min(done+64) {if move_one(inv,&mut state.grid,src,slot){state.hold_used=true;}}
            state.hold_transfer=Some((src,slot,start,wanted));
            ui.ctx().request_repaint();
        }
    }
    if resp.clicked() && !state.confirm_clear && !state.recipes_open && !state.hold_used && !state.touch_spreading {
        if let Some(src)=state.picked_slot.take() {move_any(inv,&mut state.grid,src,slot);}
        else if stack.is_some() {state.picked_slot=Some(slot);}
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
    if state.touch_spreading && state.spread_cells.contains(&slot) {
        ui.painter().rect_stroke(rect,Rounding::same(8.0),Stroke::new(2.5,p.accent));
        if let Some(src)=state.touch_source.and_then(|src|get(inv,&state.grid,src)) {
            let share=src.count as usize/state.spread_cells.len().max(1);
            ui.painter().text(rect.center(),Align2::CENTER_CENTER,format!("+{share}"),bold_font(14.0),p.text);
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
    if !c::ENTRIES.iter().any(|e|e.form==c::FORMS[state.build_form].0&&c::available(e)) {state.build_form=0;}
    ui.horizontal_wrapped(|ui| {
        ui.label("Форма");
        egui::ComboBox::from_id_salt("build-form").selected_text(c::FORMS[state.build_form].1).width(220.0).show_ui(ui, |ui| {
            for (i,(form,name)) in c::FORMS.iter().enumerate().filter(|(_, (form,_))|c::ENTRIES.iter().any(|e|e.form==*form&&c::available(e))) {let _=form;ui.selectable_value(&mut state.build_form,i,*name);}
        });
        let form=c::FORMS[state.build_form].0;
        let valid:Vec<usize>=c::ENTRIES.iter().filter(|e|e.form==form&&c::available(e)).map(|e|e.material).collect();
        if !valid.contains(&state.build_material) { state.build_material=valid[0]; }
        ui.label("Материал");
        egui::ComboBox::from_id_salt("build-material").selected_text(c::MATERIALS[state.build_material].name).width(170.0).show_ui(ui, |ui| {
            for m in valid {ui.selectable_value(&mut state.build_material,m,c::MATERIALS[m].name);}
        });
    });
    let form=c::FORMS[state.build_form].0;
    let entry=c::ENTRIES.iter().find(|e|e.form==form&&e.material==state.build_material&&c::available(e))?;
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
        crafted = crafting_grid(ui, icons, p, state, inv, near_table, slot_rects);
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
/// Try on copies first: a failed craft must neither consume resources nor rearrange slots.
fn quick_craft(inv: &mut Inventory, grid: &mut CraftGrid, recipe: &wolfcraft_engine::inventory::Recipe) -> bool {
    let mut bag=inv.clone(); let mut cells=grid.clone();
    if !cells.fill_from(recipe,&mut bag) {return false;}
    let Some(output)=cells.craft_once() else {return false;};
    if !bag.can_hold(output) {return false;}
    bag.add(output); *inv=bag; *grid=cells; true
}

fn can_quick_craft(inv: &Inventory, grid: &CraftGrid, recipe: &wolfcraft_engine::inventory::Recipe) -> bool {
    quick_craft(&mut inv.clone(),&mut grid.clone(),recipe)
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
    let mode = usize::from(state.tab==BACKPACK_TAB);
    let (banner,_) = ui.allocate_exact_size(Vec2::new(full_w,68.0),Sense::hover());
    inventory_surface(ui.painter(),banner,false);
    let title=["Творчество","Инвентарь игрока"][mode];
    ui.painter().text(Pos2::new(banner.left()+14.0,banner.center().y),Align2::LEFT_CENTER,title,bold_font(18.0),p.text);
    let controls=Rect::from_min_max(Pos2::new(banner.right()-276.0,banner.top()+14.0),Pos2::new(banner.right()-6.0,banner.bottom()-14.0));
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(controls),|ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x=4.0;
            for (m,label,ic) in [(0,"Творчество",icon::STAR),(1,"Инвентарь",icon::BACKPACK)] {
                let (r,_)=ui.allocate_exact_size(Vec2::new(if m==0 {36.0}else{116.0},36.0),Sense::hover());
                let resp=inventory_plate(ui,icons,p,r,Id::new(("inventory-mode",m)),ControlIcon::Gui(ic),if m==0 {""}else{label},m==mode);
                if resp.on_hover_text(label).clicked() {
                    if m==0 {state.tab=state.last_tab.min(BACKPACK_TAB-1);} else {if state.tab!=BACKPACK_TAB {state.last_tab=state.tab;}state.tab=BACKPACK_TAB;state.inv_page=2;}
                    state.search_open=false;state.search.clear();
                }
                #[cfg(test)] ui.ctx().data_mut(|d|d.insert_temp(Id::new(("inventory-mode-test",m)),r));
            }
            let (r,_)=ui.allocate_exact_size(Vec2::new(104.0,36.0),Sense::hover());
            if inventory_plate(ui,icons,p,r,Id::new("inventory-close"),ControlIcon::Gui(icon::CLOSE),"Закрыть",false).clicked(){result.close=true;}
        });
    });
    let search_rect=Rect::from_center_size(banner.center(),Vec2::new(128.0,36.0));
    let search_response=inventory_plate(ui,icons,p,search_rect,Id::new("creative-header-search"),ControlIcon::Gui(icon::SEARCH),"Поиск",state.search_open);
    if search_response.clicked() {
        state.tab=state.last_tab.min(BACKPACK_TAB-1);state.search_open=!state.search_open;
        if state.search_open {ui.ctx().data_mut(|d|d.insert_temp(Id::new("inv-search-fresh"),true));}else{state.search.clear();}
    }
    ui.add_space(4.0);
    // ---- Category menu: pills in one row with arrows (Творчество only) ----
    // ---- The section: the item grid (or the page of Крафт / Персонаж / Рюкзак) ----
    let section_h = INVENTORY_BODY_H;
    ui.horizontal_top(|ui| {
        {
            ui.allocate_ui_with_layout(Vec2::new(120.0,section_h),egui::Layout::top_down(egui::Align::Min),|ui| {
                ui.set_width(120.0);
                egui::ScrollArea::vertical().id_salt("creative-category-sidebar").max_height(section_h-48.0).auto_shrink([false,false]).show(ui,|ui| {
                    for (i,name) in CREATIVE_TABS.iter().enumerate().take(BACKPACK_TAB) {
                        let on=state.tab==i&&!state.search_open;
                        let (r,_)=ui.allocate_exact_size(Vec2::new(112.0,44.0),Sense::hover());
                        let glyph=if i==0 {ControlIcon::Gui(icon::STAR)}else{palette_all.iter().copied().find(|id|items::category(*id)==*name).map(ControlIcon::Item).unwrap_or(ControlIcon::Gui(icon::CUBE))};
                        let resp=inventory_plate(ui,icons,p,r,Id::new(("inventory-category",i)),glyph,name,on);
                        if resp.clicked(){state.tab=i;state.last_tab=i;state.search_open=false;state.search.clear();}
                    }
                });
                let (r,_)=ui.allocate_exact_size(Vec2::new(112.0,40.0),Sense::hover());
                if inventory_plate(ui,icons,p,r,Id::new("inventory-clear"),ControlIcon::Gui(icon::TRASH),"Очистить",false).clicked(){state.confirm_clear=true;state.drag_slot=None;state.picked_slot=None;state.touch_source=None;state.touch_spreading=false;state.spread_cells.clear();}
            });
        }
        ui.allocate_ui_with_layout(Vec2::new(grid_w,section_h),egui::Layout::top_down(egui::Align::Min),|ui| {
            ui.set_width(grid_w);ui.set_height(section_h);
            creative_main(ui,icons,p,state,inv,cell,rows,cols,grid_w,palette_all,slot_rects,trash_rect,result);
        });
        right_hotbar(ui,icons,p,state,inv,section_h,slot_rects,trash_rect);
    });
    // Quick access is one vertical column on the right; no bottom bar.

}

fn right_hotbar(ui:&mut Ui,icons:&GuiIcons,p:&Palette,state:&mut InventoryUiState,inv:&mut Inventory,height:f32,slots:&mut Vec<(usize,Rect)>,trash:&mut Option<Rect>) {
    ui.allocate_ui_with_layout(Vec2::new(72.0,height),egui::Layout::top_down(egui::Align::Min),|ui| {
        ui.spacing_mut().item_spacing.y=4.0;
        let panel=Rect::from_min_size(ui.cursor().min,Vec2::new(72.0,height));
        inventory_surface(ui.painter(),panel,true);
        let (label,_)=ui.allocate_exact_size(Vec2::new(72.0,22.0),Sense::hover());ui.painter().text(label.center(),Align2::CENTER_CENTER,"1–9",bold_font(12.0),p.text);
        for slot in 0..HOTBAR {
            let (row,_)=ui.allocate_exact_size(Vec2::new(72.0,36.0),Sense::hover());let r=Rect::from_min_size(row.min+Vec2::new(25.0,0.0),Vec2::splat(36.0));
            ui.painter().text(Pos2::new(row.left()+13.0,row.center().y),Align2::CENTER_CENTER,(slot+1).to_string(),bold_font(11.0),p.text_muted);
            slots.push((slot,r));draw_slot(ui,icons,p,state,inv,slot,r);
        }
        let (row,_)=ui.allocate_exact_size(Vec2::new(72.0,36.0),Sense::hover());
        let r=Rect::from_min_size(row.min+Vec2::new(25.0,0.0),Vec2::splat(36.0));
        bin(ui,icons,p,state,inv,r,36.0,trash);
        #[cfg(test)] ui.ctx().data_mut(|d|d.insert_temp(Id::new("inventory-test-trash"),r));
    });
}

fn player_contents(ui:&mut Ui,icons:&GuiIcons,p:&Palette,state:&mut InventoryUiState,inv:&mut Inventory,slots:&mut Vec<(usize,Rect)>,result:&mut InventoryResult) {
    if state.grid.size!=2 && state.grid.cells.iter().all(Option::is_none){state.grid.set_size(2,inv);}
    ui.horizontal_top(|ui| {
        character_section(ui,icons,p,state,inv,slots);ui.add_space(12.0);
        ui.vertical(|ui| {if let Some(msg)=crafting_grid(ui,icons,p,state,inv,false,slots){result.crafted=Some(msg);}});
    });
    ui.add_space(12.0);section_label(ui,"Рюкзак");ui.add_space(4.0);
    let cell=((ui.available_width()-8.0*GAP)/9.0).min(CELL);
    backpack_section(ui,icons,p,state,inv,cell,slots);
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
    ui.painter().rect_filled(rect, Rounding::same(4.0), p.danger.gamma_multiply(if hot { 0.35 } else { 0.15 }));
    ui.painter().rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, p.danger.gamma_multiply(0.7)));
    icons.paint(ui.painter(), rect.shrink(s * 0.24), icon::TRASH, p.danger);
    // Tap: delete the item in the selected hotbar slot. Or drag any slot here.
    if resp.clicked() {
        let slot=state.picked_slot.unwrap_or(inv.selected);
        set(inv,&mut state.grid,slot,None);state.picked_slot=None;
    }
    resp.on_hover_text("Удалить выбранную стопку или перетащить предмет сюда");
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
    let player_page=state.tab==BACKPACK_TAB&&!state.search_open;
    let mut catalog:Vec<ItemId>=palette_all.into_iter().filter(|id|(state.search_open||in_tab(*id,state.tab)||!state.search.trim().is_empty())&&matches_search(*id,&state.search)).collect();
    let title=if player_page {"Инвентарь игрока"}else if state.search_open {"Результаты поиска"}else{CREATIVE_TABS[state.tab.min(BACKPACK_TAB)]};
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        ui.add(egui::Label::new(egui::RichText::new(if player_page {title.to_string()}else{format!("{title} · {}",catalog.len())}).font(bold_font(14.0)).color(p.text)).truncate());
        if !player_page {ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
            egui::ComboBox::from_id_salt("inventory-sort").width(116.0).selected_text(["По группам","По названию","По ID"][state.catalog_sort.min(2)]).show_ui(ui,|ui| {
                for (mode,label) in [(0,"По группам"),(1,"По названию"),(2,"По ID")] {ui.selectable_value(&mut state.catalog_sort,mode,label);}
            });
        });}
    });
    ui.add_space(4.0);
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
        inventory_surface(ui.painter(),area.expand(3.0),true);
        let area = area.shrink(4.0);
        let content = area;
        let mut ui2 = ui.new_child(egui::UiBuilder::new().max_rect(content));
        egui::ScrollArea::vertical().id_salt("creative-player-inventory").drag_to_scroll(state.touch_source.is_none()).auto_shrink([false, false]).show(&mut ui2, |ui| {
            ui.set_width(content.width() - 8.0);
            player_contents(ui,icons,p,state,inv,slot_rects,result);
        });
        return;
    } else {
        sort_catalog(&mut catalog,state.catalog_sort);
        let list=catalog;
        // Building pieces come in a dozen materials each: like on Bedrock, every form is one
        // cell with a "+" that unfolds its materials in place (a search shows them all).
        let cells = group_cells(&list, state.open_group, state.search.trim().is_empty());
        let list = cells;
        let (area, _) = ui.allocate_exact_size(Vec2::new(grid_w, grid_h), Sense::hover());
        // A recessed metal panel behind the slots, like in the block windows.
        inventory_surface(ui.painter(),area.expand(3.0),true);
        let area = area.shrink(4.0);
        let mut ui2 = ui.new_child(egui::UiBuilder::new().max_rect(area));
        ui2.spacing_mut().item_spacing.y=GAP;
        // Swipe scrolls the list (a slot only reacts to a tap, so the finger drag goes to the
        // scroll area); the bar stays visible so it is clear there is more below.
        egui::ScrollArea::vertical()
            .id_salt(("creative-scroll", state.tab, state.search.trim().to_lowercase(), state.catalog_sort))
            .auto_shrink([false, false])
            .drag_to_scroll(true)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show_rows(&mut ui2, cell, list.len().div_ceil(cols), |ui, visible| {
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
    let hint = if near_table { "Зажмите стопку и проведите по клеткам — распределить · ПКМ — 1" } else { "Верстак — сетка 3×3" };
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
        state.build_form=c::FORMS.iter().position(|(f,_)|*f==c::Form::Door).unwrap();
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
    fn recipe_craft_is_atomic_and_accepts_resources_from_the_grid() {
        let recipe=wolfcraft_engine::inventory::recipes().iter().find(|r|r.output.0==Block::OakPlanks as ItemId && CraftGrid::needs(r).iter().any(|(id,_)|*id==Block::OakLog as ItemId)).unwrap();
        let mut inv=Inventory::default();let mut grid=CraftGrid::default();grid.size=3;
        let before=grid.clone();
        assert!(!quick_craft(&mut inv,&mut grid,recipe));assert_eq!(grid,before);
        grid.cells[0]=Some(ItemStack::of_block(Block::OakLog,1));
        assert!(quick_craft(&mut inv,&mut grid,recipe));
        assert_eq!(inv.count(Block::OakPlanks as ItemId),recipe.output.1 as u32);
        assert!(grid.cells.iter().all(Option::is_none));
        // Full backpack: failure leaves both input and output counts unchanged.
        for slot in &mut inv.slots {*slot=Some(ItemStack::of_block(Block::Stone,64));}
        grid.cells[0]=Some(ItemStack::of_block(Block::OakLog,2));let before=grid.clone();
        assert!(!quick_craft(&mut inv,&mut grid,recipe));assert_eq!(grid,before);
        assert_eq!(inv.count(Block::Stone as ItemId),64*SLOTS as u32);
    }

    #[test]
    fn scaled_workbench_drag_moves_between_backpack_and_grid_slots() {
        for (w,h) in [(390.0,844.0),(900.0,420.0)] {
            let (ctx,icons)=ctx_with_icons();let mut st=InventoryUiState::default();let mut inv=Inventory::default();
            inv.slots[HOTBAR]=Some(ItemStack::of_block(Block::OakLog,3));
            let render=|st:&mut InventoryUiState,inv:&mut Inventory,events| {
                let _=ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,h))),events,..Default::default()},|ctx|{show(ctx,&icons,st,inv,false,true,0.016);});
            };
            for _ in 0..3 {render(&mut st,&mut inv,vec![]);}
            for (src,dst) in [(HOTBAR,GRID),(GRID,GRID+1),(GRID+1,HOTBAR+1)] {
                let rects:Vec<(usize,Rect)>=ctx.data(|d|d.get_temp(Id::new("inventory-test-slots"))).unwrap();
                let (scale,t)=ctx.data(|d|d.get_temp::<(f32,Vec2)>(Id::new("inventory").with("fit"))).unwrap();
                let pos=|slot| (rects.iter().find(|(s,_)|*s==slot).unwrap().1.center().to_vec2()*scale+t).to_pos2();
                let a=pos(src);let b=pos(dst);
                render(&mut st,&mut inv,vec![egui::Event::PointerMoved(a),egui::Event::PointerButton{pos:a,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
                render(&mut st,&mut inv,vec![egui::Event::PointerMoved(b)]);
                render(&mut st,&mut inv,vec![egui::Event::PointerButton{pos:b,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
                assert!(get(&inv,&st.grid,src).is_none(),"source {src} at {w}×{h}");
                assert_eq!(get(&inv,&st.grid,dst).unwrap().count,3);
            }
        }
    }

    #[test]
    fn creative_modes_and_close_share_the_main_header() {
        let (ctx,icons)=ctx_with_icons();let mut st=InventoryUiState::default();let mut inv=Inventory::default();
        let render=|st:&mut InventoryUiState,inv:&mut Inventory,events|ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(900.0,420.0))),events,..Default::default()},|ctx|{show(ctx,&icons,st,inv,true,false,0.016);});
        for _ in 0..3 {render(&mut st,&mut inv,vec![]);}
        let out=render(&mut st,&mut inv,vec![]);
        let text=|label:&str|out.shapes.iter().find_map(|s|match &s.shape {egui::epaint::Shape::Text(t) if t.galley.job.text==label=>Some(t.pos+t.galley.size()*0.5),_=>None}).expect(label);
        let close=crate::ui_components::to_body(&ctx,"inventory",text("Закрыть"));let title=crate::ui_components::to_body(&ctx,"inventory",text("Творчество"));
        let craft:Rect=ctx.data(|d|d.get_temp(Id::new(("inventory-mode-test",1usize)))).unwrap();
        assert!((close.y-craft.center().y).abs()<5.0&&craft.center().x>title.x);
        let craft=craft.center();
        let (scale,t)=ctx.data(|d|d.get_temp::<(f32,Vec2)>(Id::new("inventory").with("fit"))).unwrap();
        let at=(craft.to_vec2()*scale+t).to_pos2();
        render(&mut st,&mut inv,vec![egui::Event::PointerMoved(at),egui::Event::PointerButton{pos:at,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
        render(&mut st,&mut inv,vec![egui::Event::PointerButton{pos:at,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
        assert_eq!(st.tab,BACKPACK_TAB);assert_eq!(st.inv_page,2);
    }

    #[test]
    fn hold_in_crafting_slot_adds_items_gradually_and_release_does_not_dump_stack() {
        let (ctx,icons)=ctx_with_icons();let mut st=InventoryUiState::default();let mut inv=Inventory::default();
        inv.slots[HOTBAR]=Some(ItemStack::of_block(Block::OakPlanks,8));
        let render=|st:&mut InventoryUiState,inv:&mut Inventory,time,events|ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(900.0,540.0))),time:Some(time),events,..Default::default()},|ctx|{show(ctx,&icons,st,inv,false,true,0.016);});
        for time in [0.0,0.25,0.5] {render(&mut st,&mut inv,time,vec![]);}
        let slots:Vec<(usize,Rect)>=ctx.data(|d|d.get_temp(Id::new("inventory-test-slots"))).unwrap();
        let r=slots.iter().find(|(s,_)|*s==GRID).unwrap().1;
        let (scale,t)=ctx.data(|d|d.get_temp::<(f32,Vec2)>(Id::new("inventory").with("fit"))).unwrap();let at=(r.center().to_vec2()*scale+t).to_pos2();
        st.picked_slot=Some(HOTBAR);
        render(&mut st,&mut inv,1.0,vec![egui::Event::PointerMoved(at),egui::Event::PointerButton{pos:at,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
        render(&mut st,&mut inv,1.4,vec![]);assert_eq!(st.grid.cells[0].unwrap().count,1);
        render(&mut st,&mut inv,1.64,vec![]);assert_eq!(st.grid.cells[0].unwrap().count,3);
        render(&mut st,&mut inv,1.65,vec![egui::Event::PointerButton{pos:at,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
        assert_eq!(st.grid.cells[0].unwrap().count,3);assert_eq!(inv.slots[HOTBAR].unwrap().count,5);
    }

    #[test]
    fn creative_hotbar_is_one_right_column_and_search_is_centred_in_header() {
        let (ctx,icons)=ctx_with_icons();let mut st=InventoryUiState::default();let mut inv=Inventory::default();
        let mut out=None;
        for n in 0..3 {out=Some(ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(1000.0,620.0))),time:Some(n as f64),..Default::default()},|ctx|{show(ctx,&icons,&mut st,&mut inv,true,false,0.016);}));}
        let slots:Vec<(usize,Rect)>=ctx.data(|d|d.get_temp(Id::new("inventory-test-slots"))).unwrap();
        let hotbar:Vec<_>=slots.iter().filter(|(i,_)|*i<HOTBAR).collect();assert_eq!(hotbar.len(),HOTBAR);assert!(hotbar.last().unwrap().1.bottom()-hotbar.first().unwrap().1.top()<=358.0);
        for pair in hotbar.windows(2){assert!((pair[0].1.center().x-pair[1].1.center().x).abs()<1.0);assert!(pair[0].1.bottom()<pair[1].1.top());}
        let out=out.unwrap();let labels:Vec<_>=out.shapes.iter().filter_map(|s|match &s.shape {egui::epaint::Shape::Text(t) if t.galley.job.text=="Поиск"=>Some(t.pos+t.galley.size()*0.5),_=>None}).collect();
        assert_eq!(labels.len(),1);assert!((labels[0].x-500.0).abs()<50.0);
    }

    #[test]
    fn touch_drag_distributes_a_stack_evenly_across_crafting_cells() {
        for (w,h) in [(900.0,540.0),(390.0,844.0)] {
            let (ctx,icons)=ctx_with_icons();let mut st=InventoryUiState::default();let mut inv=Inventory::default();
            inv.slots[HOTBAR]=Some(ItemStack::of_block(Block::OakPlanks,13));
            let render=|st:&mut InventoryUiState,inv:&mut Inventory,time,events|ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,h))),time:Some(time),events,..Default::default()},|ctx|{show(ctx,&icons,st,inv,false,true,0.016);});
            for time in [0.0,0.3,0.6] {render(&mut st,&mut inv,time,vec![]);}
            let rects:Vec<(usize,Rect)>=ctx.data(|d|d.get_temp(Id::new("inventory-test-slots"))).unwrap();
            let (scale,t)=ctx.data(|d|d.get_temp::<(f32,Vec2)>(Id::new("inventory").with("fit"))).unwrap();
            let pos=|slot| (rects.iter().find(|(s,_)|*s==slot).unwrap().1.center().to_vec2()*scale+t).to_pos2();
            let finger=|phase,pos|egui::Event::Touch{device_id:egui::TouchDeviceId(1),id:egui::TouchId(1),phase,pos,force:None};
            let start=pos(HOTBAR);
            render(&mut st,&mut inv,1.0,vec![finger(egui::TouchPhase::Start,start),egui::Event::PointerMoved(start),egui::Event::PointerButton{pos:start,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
            for (n,slot) in [GRID,GRID+1,GRID+2].into_iter().enumerate() {
                let p=pos(slot);render(&mut st,&mut inv,1.1+n as f64*0.1,vec![finger(egui::TouchPhase::Move,p),egui::Event::PointerMoved(p)]);
            }
            assert_eq!(st.spread_cells.len(),3,"visited cells at {w}×{h}");assert_eq!(inv.slots[HOTBAR].unwrap().count,13,"preview does not consume resources");
            let end=pos(GRID+2);render(&mut st,&mut inv,1.5,vec![finger(egui::TouchPhase::End,end),egui::Event::PointerButton{pos:end,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
            for i in 0..3 {assert_eq!(st.grid.cells[i].unwrap().count,4);}
            assert_eq!(inv.slots[HOTBAR].unwrap().count,1);assert!(st.touch_source.is_none());
        }
    }

    #[test]
    fn distribution_keeps_incompatible_cells_and_remainders() {
        let mut inv=Inventory::default();let mut grid=CraftGrid::default();grid.size=3;
        inv.slots[0]=Some(ItemStack::of_block(Block::OakPlanks,13));grid.cells[1]=Some(ItemStack::of_block(Block::Stone,64));
        assert!(distribute_stack(&mut inv,&mut grid,0,&[GRID,GRID+2]));
        assert_eq!(grid.cells[0].unwrap().count,6);assert_eq!(grid.cells[2].unwrap().count,6);assert_eq!(inv.slots[0].unwrap().count,1);assert_eq!(grid.cells[1].unwrap().count,64);
    }

    #[test]
    fn clear_inventory_requires_yes_and_no_preserves_items() {
        let (ctx,icons)=ctx_with_icons();let mut st=InventoryUiState::default();let mut inv=Inventory::default();
        inv.slots[0]=Some(ItemStack::of_block(Block::OakPlanks,13));st.grid.cells[0]=Some(ItemStack::of_block(Block::OakLog,2));
        let render=|st:&mut InventoryUiState,inv:&mut Inventory,events|ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(1000.0,700.0))),events,..Default::default()},|ctx|{show(ctx,&icons,st,inv,true,false,0.016);});
        for (label,empty) in [("Нет",false),("Да",true)] {
            st.confirm_clear=true;render(&mut st,&mut inv,vec![]);render(&mut st,&mut inv,vec![]);let out=render(&mut st,&mut inv,vec![]);
            let at=out.shapes.iter().find_map(|s|match &s.shape {egui::epaint::Shape::Text(t) if t.galley.job.text==label=>Some(t.pos+t.galley.size()*0.5),_=>None}).unwrap();
            render(&mut st,&mut inv,vec![egui::Event::PointerMoved(at),egui::Event::PointerButton{pos:at,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
            render(&mut st,&mut inv,vec![egui::Event::PointerButton{pos:at,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
            assert!(!st.confirm_clear);
            assert_eq!(inv.slots.iter().all(Option::is_none),empty);assert_eq!(st.grid.cells.iter().all(Option::is_none),empty);
        }
    }

    #[test]
    fn survival_window_contains_the_whole_inventory_and_one_vertical_hotbar() {
        for (w,h) in [(900.0,540.0),(390.0,844.0)] {
            let (ctx,icons)=ctx_with_icons();let mut st=InventoryUiState::default();let mut inv=Inventory::default();
            for _ in 0..3 {let _=ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(w,h))),..Default::default()},|ctx|{show(ctx,&icons,&mut st,&mut inv,false,false,0.016);});}
            let slots:Vec<(usize,Rect)>=ctx.data(|d|d.get_temp(Id::new("inventory-test-slots"))).unwrap();
            for slot in 0..SLOTS {assert_eq!(slots.iter().filter(|(s,_)|*s==slot).count(),1,"slot {slot}");}
            for slot in [GRID,GRID+1,GRID+3,GRID+4,ARMOR,OFFHAND] {assert!(slots.iter().any(|(s,_)|*s==slot));}
            let hotbar:Vec<_>=slots.iter().filter(|(s,_)|*s<HOTBAR).collect();
            for p in hotbar.windows(2){assert_eq!(p[0].1.center().x,p[1].1.center().x);assert!(p[0].1.top()<p[1].1.top());}
        }
    }

    #[test]
    fn inventory_frame_stays_fixed_across_filters_and_player_page() {
        let (ctx,icons)=ctx_with_icons();let mut st=InventoryUiState::default();let mut inv=Inventory::default();
        let render=|st:&mut InventoryUiState,inv:&mut Inventory,time| {
            let _=ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(1100.0,720.0))),time:Some(time),..Default::default()},|ctx|{show(ctx,&icons,st,inv,true,false,0.016);});
            ctx.memory(|m|m.area_rect(Id::new("inventory").with("body"))).unwrap()
        };
        for time in [0.0,0.3,0.6] {render(&mut st,&mut inv,time);}
        let base=render(&mut st,&mut inv,1.0);
        for n in 0..4 {
            st.tab=if n==3 {BACKPACK_TAB}else{n};st.search_open=n==2;st.search=if n==2 {"алмаз".into()}else{String::new()};
            let rect=render(&mut st,&mut inv,2.0+n as f64);
            assert!((rect.width()-base.width()).abs()<1.0&&(rect.height()-base.height()).abs()<1.0,"layout changed: {base:?} -> {rect:?}");
        }
    }

    #[test]
    fn right_hotbar_bin_deletes_only_the_selected_stack() {
        let (ctx,icons)=ctx_with_icons();let mut st=InventoryUiState::default();let mut inv=Inventory::default();
        inv.slots[0]=Some(ItemStack::of_block(Block::OakPlanks,13));inv.slots[1]=Some(ItemStack::of_block(Block::Stone,7));
        let render=|st:&mut InventoryUiState,inv:&mut Inventory,time,events|ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(1000.0,700.0))),time:Some(time),events,..Default::default()},|ctx|{show(ctx,&icons,st,inv,true,false,0.016);});
        for time in [0.0,0.3,0.6] {render(&mut st,&mut inv,time,vec![]);}
        let trash:Rect=ctx.data(|d|d.get_temp(Id::new("inventory-test-trash"))).unwrap();
        let slots:Vec<(usize,Rect)>=ctx.data(|d|d.get_temp(Id::new("inventory-test-slots"))).unwrap();
        assert!(trash.top()>slots.iter().find(|(s,_)|*s==8).unwrap().1.bottom());
        let (scale,t)=ctx.data(|d|d.get_temp::<(f32,Vec2)>(Id::new("inventory").with("fit"))).unwrap();let at=(trash.center().to_vec2()*scale+t).to_pos2();
        render(&mut st,&mut inv,1.0,vec![egui::Event::PointerMoved(at),egui::Event::PointerButton{pos:at,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
        render(&mut st,&mut inv,1.1,vec![egui::Event::PointerButton{pos:at,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
        assert!(inv.slots[0].is_none());assert_eq!(inv.slots[1].unwrap().count,7);assert!(!st.confirm_clear);
    }

    #[test]
    fn catalogue_groups_nature_wood_and_tool_tiers() {
        let mut natural=vec![Block::Ice,Block::Mud,Block::Sand,Block::Granite,Block::Snow,Block::Dirt,Block::Stone].into_iter().map(|b|b as ItemId).collect::<Vec<_>>();
        sort_catalog(&mut natural,0);
        assert_eq!(natural,vec![Block::Stone,Block::Granite,Block::Dirt,Block::Mud,Block::Sand,Block::Snow,Block::Ice].into_iter().map(|b|b as ItemId).collect::<Vec<_>>());
        let mut wood=vec![Block::BirchLog as ItemId,Block::OakLeaves as ItemId,Block::OakPlanks as ItemId,Block::OakLog as ItemId];sort_catalog(&mut wood,0);
        assert_eq!(wood,vec![Block::OakLog as ItemId,Block::OakPlanks as ItemId,Block::OakLeaves as ItemId,Block::BirchLog as ItemId]);
        let mut tools=vec![items::ids::tool(0,1),items::ids::tool(3,0),items::ids::tool(0,0)];sort_catalog(&mut tools,0);
        assert_eq!(tools,vec![items::ids::tool(0,0),items::ids::tool(3,0),items::ids::tool(0,1)]);
        sort_catalog(&mut wood,2);assert!(wood.windows(2).all(|p|p[0]<p[1]));
        sort_catalog(&mut wood,1);assert!(wood.windows(2).all(|p|items::info(p[0]).name.to_lowercase()<=items::info(p[1]).name.to_lowercase()));
    }

    #[test]
    fn catalogue_header_names_the_active_category_and_search_results() {
        let (ctx,icons)=ctx_with_icons();let mut st=InventoryUiState::default();let mut inv=Inventory::default();st.tab=5;
        let render=|st:&mut InventoryUiState,inv:&mut Inventory|ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(1000.0,700.0))),..Default::default()},|ctx|{show(ctx,&icons,st,inv,true,false,0.016);});
        render(&mut st,&mut inv);let out=render(&mut st,&mut inv);
        let count=items::creative_palette().iter().filter(|id|in_tab(**id,5)).count();let expected=format!("Природа · {count}");
        assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text==expected)));
        st.search_open=true;st.search="точно_нет_такого_блока".into();render(&mut st,&mut inv);let out=render(&mut st,&mut inv);
        assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(t) if t.galley.job.text=="Результаты поиска · 0")));
    }

    #[test]
    fn search_filters_by_name() {
        assert!(matches_search(Block::Lamp as ItemId, "лам"));
        assert!(!matches_search(Block::Lamp as ItemId, "алмаз"));
        assert!(matches_search(Block::Lamp as ItemId, "  "));
    }
}

