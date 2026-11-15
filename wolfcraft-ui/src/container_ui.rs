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
    pub skin:u16,
    /// Slots per row for the container grid.
    pub columns: usize,
    /// Which container slots accept items from the player (e.g. only the furnace input).
    pub accepts: &'a dyn Fn(usize, &ItemStack) -> bool,
    /// Optional status line with progress 0..1 (furnace).
    pub status: Option<(&'a str, f32)>,
    /// How the block's own slots are laid out.
    pub layout: Layout,
    /// The furnace is burning (its flame is lit).
    pub working: bool,
    /// WolfTech: how full the machine's energy store is (0..1), drawn as a bar on the left.
    pub energy: Option<f32>,
    /// Reactor hull heat (0..1 of its limit), a bar on the right.
    pub heat: Option<f32>,
}

/// Layout of a block's slots.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Layout {
    /// A plain grid (chest, barrel, wardrobe).
    #[default]
    Grid,
    /// Slots 0 input, 1 fuel, 2 result: input over the flame over the fuel, an arrow that
    /// fills with the progress, the result.
    Furnace,
    /// Slots 0 item, 1 material or book, 2 result: item + material → result.
    Forge,
    /// Slots 0 item or book, 1 iskrit; the offers follow below.
    Enchant,
    /// WolfTech machine: slot 0 input → slot 2 result (no fuel: it runs on energy).
    Machine,
    /// WolfTech generator: only slot 1, the fuel.
    Generator,
    /// WolfTech energy cell: only slot 0, the gear to charge.
    Charger,
    /// WolfTech solar panel: no slots, just the energy bar.
    Panel,
    /// WolfTech reactor: a 6 × 3 core between the energy and the heat bars.
    Reactor,
}

/// Where each of the block's slots sits, in cells from the top-left of the slot area
/// (x, y), the caption under it, and the area's height in cells.
fn slot_places(layout: Layout, n: usize, cols: usize) -> (Vec<(f32, f32, &'static str)>, f32) {
    match layout {
        Layout::Furnace if n == 3 => (vec![(2.0, 0.0, "Сырьё"), (2.0, 1.7, "Топливо"), (6.0, 0.85, "Результат")], 3.1),
        Layout::Forge if n == 3 => (vec![(1.0, 0.0, "Предмет"), (3.0, 0.0, "Материал / книга"), (7.0, 0.0, "Результат")], 1.4),
        // Hidden slots sit at x < 0 (they exist but are not shown).
        Layout::Machine if n == 3 => (vec![(2.5, 0.3, "Сырьё"), (-9.0, 0.0, ""), (6.5, 0.3, "Результат")], 1.8),
        // With the IC2-style upgrade slots: a 2×2 block on the right.
        Layout::Machine if n == 7 => (
            vec![(1.6, 0.55, "Сырьё"), (-9.0, 0.0, ""), (4.6, 0.55, "Результат"), (6.85, 0.0, ""), (7.9, 0.0, ""), (6.85, 1.05, ""), (7.9, 1.05, "")],
            2.35,
        ),
        Layout::Generator if n == 3 => (vec![(-9.0, 0.0, ""), (4.0, 0.3, "Топливо"), (-9.0, 0.0, "")], 1.8),
        Layout::Charger if n == 3 => (vec![(4.0, 0.3, "Заряжается"), (-9.0, 0.0, ""), (-9.0, 0.0, "")], 1.8),
        Layout::Panel => ((0..n).map(|_| (-9.0, 0.0, "")).collect(), 1.8),
        Layout::Reactor => ((0..n).map(|i| if i < 18 { (1.5 + (i % 6) as f32 * 1.05, (i / 6) as f32 * 1.05, "") } else { (-9.0, 0.0, "") }).collect(), 3.15),
        Layout::Enchant if n == 2 => (vec![(3.0, 0.0, "Предмет / книга"), (5.0, 0.0, "Искрит")], 1.4),
        _ => {
            let rows = n.div_ceil(cols);
            let x0 = (9usize.max(cols) - cols) as f32 / 2.0;
            ((0..n).map(|i| (x0 + (i % cols) as f32, (i / cols) as f32, "")).collect(), rows as f32)
        }
    }
}

/// An arrow from `a` to `b` that fills with `f` (0..1) in the accent colour.
fn arrow(ui: &Ui, a: egui::Pos2, b: egui::Pos2, f: f32, p: &crate::theme_manager::Palette) {
    let h = 10.0;
    let body = Rect::from_min_max(egui::pos2(a.x, a.y - h / 2.0), egui::pos2(b.x - 12.0, a.y + h / 2.0));
    let head = vec![egui::pos2(b.x - 12.0, a.y - 11.0), egui::pos2(b.x, a.y), egui::pos2(b.x - 12.0, a.y + 11.0)];
    ui.painter().rect_filled(body, Rounding::same(2.0), p.outline);
    ui.painter().add(egui::Shape::convex_polygon(head.clone(), p.outline, Stroke::NONE));
    let f = f.clamp(0.0, 1.0);
    if f > 0.0 {
        let total = b.x - a.x;
        let fx = a.x + total * f;
        ui.painter().rect_filled(Rect::from_min_max(body.min, egui::pos2(fx.min(body.right()), body.bottom())), Rounding::same(2.0), p.accent);
        if fx > body.right() {
            ui.painter().with_clip_rect(Rect::from_min_max(egui::pos2(body.right(), a.y - 12.0), egui::pos2(fx, a.y + 12.0))).add(egui::Shape::convex_polygon(head, p.accent, Stroke::NONE));
        }
    }
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
    ui.painter().rect_filled(rect, Rounding::same(4.0), if hover { p.surface_hover } else { p.surface_hi });
    ui.painter().rect_stroke(rect, Rounding::same(4.0), Stroke::new(2.0_f32, if hover { p.accent } else { p.outline }));
    if let Some(s) = stack {
        icons.paint_stack(ui.painter(), rect.shrink(6.0), s);
        if s.count > 1 {
            ui.painter().text(rect.right_bottom() - Vec2::new(4.0, 2.0), Align2::RIGHT_BOTTOM, s.count.to_string(), FontId::monospace(13.0), Color32::WHITE);
        }
        if resp.hovered() {
            resp.clone().on_hover_text(s.display_name());
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
    show_with(ctx, icons, view, slots, inv, &mut |_, _, _| {})
}

/// `show` with extra controls under the container slots (the Чаровальня's offers): `extra`
/// gets the UI, the container slots and the player's inventory.
pub fn show_with(ctx: &egui::Context, icons: &GuiIcons, view: &ContainerView, slots: &mut [Option<ItemStack>], inv: &mut Inventory, extra: &mut dyn FnMut(&mut Ui, &mut [Option<ItemStack>], &mut Inventory)) -> (bool, Vec<ItemStack>) {
    let p = palette(ctx);
    let mut close = false;
    let drag_id = egui::Id::new("container-drag");
    let mut drag: Option<Src> = ctx.data(|d| d.get_temp(drag_id)).flatten();
    let mut rects: Vec<(Src, Rect)> = Vec::new();
    let mut thrown = Vec::new();
    let cols = view.columns.max(1);
    let width = cols.max(9) as f32 * (CELL + GAP) - GAP;
    let dismissed = crate::ui_components::block_screen(ctx, "container", width + 4.0, |ui| {
        // Atlas banner and recess belong to the interacted block.
        let (banner,_)=ui.allocate_exact_size(Vec2::new(width,30.),Sense::hover());
        icons.paint_panel(ui.painter(),banner,view.skin,Color32::WHITE);
        ui.painter().text(banner.center(),Align2::CENTER_CENTER,view.title,FontId::proportional(16.),Color32::WHITE);
        ui.add_space(8.);
        // The block's own slots, laid out by what the block does.
        let (places, rows_h) = slot_places(view.layout, slots.len(), cols);
        let step = CELL + GAP;
        let (area, _) = ui.allocate_exact_size(Vec2::new(width, rows_h * step - GAP), Sense::hover());
        icons.paint_panel(ui.painter(),area.expand(4.),view.skin,Color32::WHITE);
        let at = |x: f32, y: f32| Rect::from_min_size(egui::pos2(area.left() + x * step, area.top() + y * step), Vec2::splat(CELL));
        match view.layout {
            Layout::Furnace if slots.len() == 3 => {
                // Flame between the input and the fuel (lit while burning), arrow to the result.
                let (inp, fuel, out) = (at(places[0].0, places[0].1), at(places[1].0, places[1].1), at(places[2].0, places[2].1));
                let c = egui::pos2(inp.center().x, (inp.bottom() + fuel.top()) / 2.0 + 5.0);
                let t = ui.input(|i| i.time) as f32;
                let flick = if view.working { 1.0 + 0.12 * (t * 9.0).sin() } else { 1.0 };
                let (hot, core) = if view.working { (Color32::from_rgb(255, 120, 30), Color32::from_rgb(255, 220, 90)) } else { (p.outline, p.surface_hi) };
                ui.painter().add(egui::Shape::convex_polygon(vec![c + Vec2::new(-9.0, 9.0), c + Vec2::new(0.0, -13.0 * flick), c + Vec2::new(9.0, 9.0)], hot, Stroke::NONE));
                ui.painter().add(egui::Shape::convex_polygon(vec![c + Vec2::new(-4.5, 9.0), c + Vec2::new(0.0, -4.0 * flick), c + Vec2::new(4.5, 9.0)], core, Stroke::NONE));
                if view.working {
                    ui.ctx().request_repaint();
                }
                let f = view.status.map(|s| s.1).unwrap_or(0.0);
                arrow(ui, egui::pos2(inp.right() + 14.0, out.center().y), egui::pos2(out.left() - 10.0, out.center().y), f, &p);
            }
            Layout::Forge if slots.len() == 3 => {
                let (a, b, out) = (at(places[0].0, places[0].1), at(places[1].0, places[1].1), at(places[2].0, places[2].1));
                ui.painter().text(egui::pos2((a.right() + b.left()) / 2.0, a.center().y), Align2::CENTER_CENTER, "+", FontId::proportional(26.0), p.text);
                arrow(ui, egui::pos2(b.right() + 14.0, b.center().y), egui::pos2(out.left() - 10.0, b.center().y), if slots[2].is_some() { 1.0 } else { 0.0 }, &p);
            }
            Layout::Machine if slots.len() == 7 => {
                let (inp, out) = (at(places[0].0, places[0].1), at(places[2].0, places[2].1));
                let f = view.status.map(|s| s.1).unwrap_or(0.0);
                arrow(ui, egui::pos2(inp.right() + 14.0, out.center().y), egui::pos2(out.left() - 10.0, out.center().y), f, &p);
                // The upgrade bay: a darker recess with its caption above.
                let bay = Rect::from_min_max(at(places[3].0, places[3].1).min, at(places[6].0, places[6].1).max).expand(5.0);
                ui.painter().rect_filled(bay, Rounding::same(6.0), Color32::from_black_alpha(70));
                ui.painter().rect_stroke(bay, Rounding::same(6.0), Stroke::new(1.5_f32, p.accent.gamma_multiply(0.6)));
                ui.painter().text(egui::pos2(bay.center().x, bay.bottom() + 2.0), Align2::CENTER_TOP, "Улучшения", FontId::proportional(11.0), p.accent);
                // While it works: a glow pulsing along the arrow's track.
                if view.working {
                    let t = ui.input(|i| i.time) as f32;
                    let x = inp.right() + 14.0 + (out.left() - inp.right() - 24.0) * ((t * 1.5).fract());
                    ui.painter().circle_filled(egui::pos2(x, out.center().y), 4.0, Color32::from_rgb(120, 230, 255));
                    ui.ctx().request_repaint();
                }
            }
            Layout::Machine if slots.len() == 3 => {
                let (inp, out) = (at(places[0].0, places[0].1), at(places[2].0, places[2].1));
                let f = view.status.map(|s| s.1).unwrap_or(0.0);
                arrow(ui, egui::pos2(inp.right() + 14.0, out.center().y), egui::pos2(out.left() - 10.0, out.center().y), f, &p);
            }
            Layout::Enchant if slots.len() == 2 => {
                let (a, b) = (at(places[0].0, places[0].1), at(places[1].0, places[1].1));
                ui.painter().text(egui::pos2((a.right() + b.left()) / 2.0, a.center().y), Align2::CENTER_CENTER, "+", FontId::proportional(26.0), Color32::from_rgb(255, 110, 100));
            }
            _ => {}
        }
        // WolfTech: the energy bar down the left side, filling from the bottom.
        if let Some(e) = view.energy {
            let e = e.clamp(0.0, 1.0);
            let bar = Rect::from_min_size(area.left_top() + Vec2::new(8.0, 2.0), Vec2::new(26.0, area.height() - 4.0));
            ui.painter().rect_filled(bar, Rounding::same(4.0), Color32::from_rgb(18, 20, 26));
            let fill = Rect::from_min_max(egui::pos2(bar.left() + 3.0, bar.bottom() - 3.0 - (bar.height() - 6.0) * e), bar.max - Vec2::splat(3.0));
            let col = Color32::from_rgb((230.0 - 170.0 * e) as u8, (70.0 + 150.0 * e) as u8, (60.0 + 150.0 * e) as u8);
            if e > 0.0 {
                ui.painter().rect_filled(fill, Rounding::same(2.0), col);
            }
            ui.painter().rect_stroke(bar, Rounding::same(4.0), Stroke::new(2.0_f32, p.outline));
            // A lightning bolt over the bar.
            let c = bar.center();
            ui.painter().add(egui::Shape::convex_polygon(vec![c + Vec2::new(2.0, -11.0), c + Vec2::new(-6.0, 2.0), c + Vec2::new(0.0, 2.0)], Color32::WHITE, Stroke::NONE));
            ui.painter().add(egui::Shape::convex_polygon(vec![c + Vec2::new(0.0, -2.0), c + Vec2::new(6.0, -2.0), c + Vec2::new(-2.0, 11.0)], Color32::WHITE, Stroke::NONE));
            ui.painter().text(egui::pos2(bar.right() + 8.0, bar.bottom()), Align2::LEFT_BOTTOM, format!("{:.0} %", e * 100.0), FontId::proportional(12.0), p.text);
        }
        // Reactor: the hull heat bar on the right, green → orange → red, flashing when critical.
        if let Some(h) = view.heat {
            let h = h.clamp(0.0, 1.0);
            let bar = Rect::from_min_size(egui::pos2(area.right() - 34.0, area.top() + 2.0), Vec2::new(26.0, area.height() - 4.0));
            ui.painter().rect_filled(bar, Rounding::same(4.0), Color32::from_rgb(18, 20, 26));
            let col = if h < 0.5 { Color32::from_rgb(90, 200, 90) } else if h < 0.8 { Color32::from_rgb(240, 160, 40) } else { Color32::from_rgb(235, 60, 45) };
            let t = ui.input(|i| i.time) as f32;
            let col = if h > 0.85 && (t * 6.0).sin() > 0.0 { Color32::from_rgb(255, 230, 120) } else { col };
            if h > 0.0 {
                let fill = Rect::from_min_max(egui::pos2(bar.left() + 3.0, bar.bottom() - 3.0 - (bar.height() - 6.0) * h), bar.max - Vec2::splat(3.0));
                ui.painter().rect_filled(fill, Rounding::same(2.0), col);
            }
            ui.painter().rect_stroke(bar, Rounding::same(4.0), Stroke::new(2.0_f32, p.outline));
            ui.painter().text(egui::pos2(bar.center().x, bar.top() + 12.0), Align2::CENTER_CENTER, "🌡", FontId::proportional(13.0), Color32::WHITE);
            if h > 0.85 {
                ui.ctx().request_repaint();
            }
        }
        let mut to_player: Option<usize> = None;
        for i in 0..slots.len() {
            let (px, py, caption) = places[i];
            if px < 0.0 {
                continue;
            }
            let rect = at(px, py);
            if !caption.is_empty() {
                ui.painter().text(egui::pos2(rect.center().x, rect.bottom() + 3.0), Align2::CENTER_TOP, caption, FontId::proportional(11.0), p.text_muted);
            }
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
        // Slots are placed freely, so the layout cursor is wherever the last one ended:
        // put it back under the whole slot area (else the hint and the bag overlap it).
        ui.allocate_rect(Rect::from_min_max(egui::pos2(area.left(), area.bottom() - 1.0), area.right_bottom()), Sense::hover());
        if let Some((text, f)) = view.status {
            ui.add_space(8.0);
            if view.layout == Layout::Grid {
                progress_bar(ui, f, text, width);
            } else {
                // The arrow shows the progress: here only the hint, wrapped to the window.
                ui.allocate_ui(Vec2::new(width, 0.0), |ui| {
                    ui.set_width(width);
                    ui.label(egui::RichText::new(text).font(FontId::proportional(12.5)).color(p.text_muted));
                });
            }
        }
        extra(ui, slots, inv);
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
            layer.rect_filled(ghost, Rounding::same(4.0), p.surface.gamma_multiply(0.9));
            icons.paint_item(&layer, ghost.shrink(6.0), s.id);
        }
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(pos) = pointer.map(|q| crate::ui_components::to_body(ctx, "container", q)) {
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
    fn the_window_scales_to_fit_small_screens_by_width_and_height() {
        let ctx = egui::Context::default();
        crate::theme_manager::install(&ctx);
        crate::theme_manager::apply(&ctx, wolfcraft_engine::config::ThemeMode::Dark);
        let atlases = wolfcraft_assets::AtlasManager::new(None).unwrap();
        let icons = GuiIcons::new(&ctx, &atlases);
        let any = |_: usize, _: &ItemStack| true;
        let view = ContainerView { title: "Сундук", skin: wolfcraft_assets::gui::PANEL_WOOD, columns: 9, accepts: &any, status: Some(("Статус", 0.3)), layout: Layout::Grid, working: false, energy: None, heat: None };
        let mut inv = Inventory::default();
        for (w, h, scaled) in [(802.0f32, 360.0f32, true), (420.0, 800.0, true), (1400.0, 900.0, false)] {
            // A small window first: the big one after it must still be measured in full.
            let small = ContainerView { title: "Кузница", skin: wolfcraft_assets::gui::PANEL_WOOD, columns: 3, accepts: &any, status: None, layout: Layout::Forge, working: false, energy: None, heat: None };
            let mut three = vec![None; 3];
            for _ in 0..3 {
                let mut raw = egui::RawInput::default();
                raw.screen_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(w, h)));
                let _ = ctx.run(raw, |c| {
                    show(c, &icons, &small, &mut three, &mut inv);
                });
            }
            let mut slots = vec![None; 27];
            for _ in 0..3 {
                let mut raw = egui::RawInput::default();
                raw.screen_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(w, h)));
                let _ = ctx.run(raw, |c| {
                    show(c, &icons, &view, &mut slots, &mut inv);
                });
            }
            let (s, t) = ctx.data(|d| d.get_temp::<(f32, Vec2)>(egui::Id::new("container").with("fit"))).expect("fit stored");
            let body = ctx.memory(|m| m.area_rect(egui::Id::new("container").with("body"))).unwrap();
            assert_eq!(s < 0.999, scaled, "{w}x{h}: scale {s}");
            if h < 400.0 {
                // The whole window (title, 3 + 4 rows of slots) is on screen, not cut off.
                assert!(body.height() > 400.0 && s < 0.9, "{w}x{h}: the window is scaled, not clipped: body {} scale {s}", body.height());
            }
            let body = ctx.memory(|m| m.area_rect(egui::Id::new("container").with("body"))).unwrap();
            let on_screen = Rect::from_min_max((body.min.to_vec2() * s + t).to_pos2(), (body.max.to_vec2() * s + t).to_pos2());
            assert!(on_screen.left() >= 0.0 && on_screen.top() >= 0.0 && on_screen.right() <= w + 0.5 && on_screen.bottom() <= h + 0.5, "{w}x{h}: {on_screen:?} (scale {s})");
            // A finger on the window's centre maps back to the centre of the unscaled body.
            let back = crate::ui_components::to_body(&ctx, "container", on_screen.center());
            assert!((back - body.center()).length() < 1.0);
        }
    }

    #[test]
    fn every_block_layout_keeps_its_slots_apart_and_inside_the_window() {
        for (layout, n, cols) in [(Layout::Grid, 27, 9), (Layout::Grid, 3, 3), (Layout::Furnace, 3, 3), (Layout::Forge, 3, 3), (Layout::Enchant, 2, 2)] {
            let (places, rows) = slot_places(layout, n, cols);
            assert_eq!(places.len(), n, "{layout:?}");
            for (i, a) in places.iter().enumerate() {
                assert!(a.0 >= 0.0 && a.0 <= 8.0 && a.1 >= 0.0 && a.1 + 1.0 <= rows + 0.01, "{layout:?} slot {i} is outside: {a:?} (rows {rows})");
                for b in &places[i + 1..] {
                    assert!((a.0 - b.0).abs() >= 1.0 || (a.1 - b.1).abs() >= 1.0, "{layout:?}: slots overlap {a:?} {b:?}");
                }
            }
            // Special layouts name every slot.
            if layout != Layout::Grid {
                assert!(places.iter().all(|p| !p.2.is_empty()));
            }
        }
        // A furnace with a wrong slot count falls back to the grid instead of panicking.
        assert_eq!(slot_places(Layout::Furnace, 2, 2).0.len(), 2);
    }

    /// Writes the furnace, forge and Чаровальня windows as coloured triangles, for a look.
    #[test]
    #[ignore]
    fn dump_block_windows() {
        let ctx = egui::Context::default();
        crate::theme_manager::install(&ctx);
        crate::theme_manager::apply(&ctx, wolfcraft_engine::config::ThemeMode::Dark);
        let atlases = wolfcraft_assets::AtlasManager::new(None).unwrap();
        let icons = GuiIcons::new(&ctx, &atlases);
        let any = |_: usize, _: &ItemStack| true;
        let mut inv = Inventory::default();
        let mut out = String::new();
        for (k, (layout, n)) in [(Layout::Forge, 3), (Layout::Furnace, 3), (Layout::Enchant, 2)].into_iter().enumerate() {
            let view = ContainerView { title: "Блок", skin: wolfcraft_assets::gui::PANEL_STONE, columns: n, accepts: &any, status: Some(("Подсказка", 0.6)), layout, working: true, energy: None, heat: None };
            let mut slots = vec![None; n];
            let mut shapes = Vec::new();
            for _ in 0..8 {
                let mut raw = egui::RawInput::default();
                raw.screen_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(802.0, 360.0)));
                let o = ctx.run(raw, |c| {
                    show(c, &icons, &view, &mut slots, &mut inv);
                });
                shapes = ctx.tessellate(o.shapes, 1.0);
            }
            out += &format!("# {k}\n");
            for cp in shapes {
                if let egui::epaint::Primitive::Mesh(m) = cp.primitive {
                    for t in m.indices.chunks(3) {
                        let v: Vec<_> = t.iter().map(|i| m.vertices[*i as usize]).collect();
                        let c = v[0].color;
                        out += &format!("{} {} {} {} {} {} {} {} {}\n", v[0].pos.x, v[0].pos.y, v[1].pos.x, v[1].pos.y, v[2].pos.x, v[2].pos.y, c.r(), c.g(), c.b());
                    }
                }
            }
        }
        std::fs::write(std::env::var("WIN_OUT").unwrap(), out).unwrap();
    }

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

