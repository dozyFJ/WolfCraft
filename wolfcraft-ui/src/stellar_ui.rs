//! «Звёздный пульт»: the skill menu (pick a skill, power / damage / speed sliders, cut-scenes
//! on or off), the floating button that opens it while the remote is in hand, and the
//! cinema bars and screen flashes of a cast.

use crate::gui_renderer::GuiIcons;
use crate::theme_manager::{bold_font, palette};
use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Rounding, Sense, Stroke, Vec2};

/// The remote's skills: name, what it does, colour.
pub const SKILLS: [(&str, &str, [u8; 3]); 9] = [
    ("Рельсотрон", "Красный лазер из космоса метит цель, потом белый луч стирает столб мира — от неба до глубины.", [235, 60, 50]),
    ("Игла", "Чёрная игла падает беззвучно: ударная волна плавит кратер, грохот приходит позже. Игла остаётся торчать.", [245, 140, 40]),
    ("Семь звёзд", "Ковш Большой Медведицы: семь звёзд падают одна за другой, затем линии созвездия выгорают траншеями.", [140, 190, 255]),
    ("Тессеракт", "Радужная клетка режет землю на ячейки, они улетают в небо. Остаётся ступенчатая яма и гиперкуб.", [190, 110, 255]),
    ("Чёрная дыра", "Втягивает блоки, мобов и всё вокруг по спирали, а потом схлопывается вспышкой.", [120, 70, 200]),
    ("Метеоритный дождь", "Шесть секунд горящие камни сыплются с неба на всю область, дымя и взрываясь.", [255, 120, 40]),
    ("Солнечное копьё", "Золотая линия метит путь, луч с неба проходит по нему и прорезает горящую траншею.", [255, 210, 80]),
    ("Ледяная комета", "Голубая комета падает и замораживает всё вокруг: снег, лёд, ледяные шипы, мобы замедлены.", [120, 220, 255]),
    ("Сверхновая", "Маленькое солнце раздувается и пульсирует… и сносит огромный шар мира. Самое мощное.", [255, 250, 200]),
];

#[derive(Clone, Debug)]
pub struct StellarMenu {
    pub open: bool,
    /// Index in `SKILLS`.
    pub mode: u8,
    /// Size of the strike (×).
    pub power: f32,
    /// Damage (×).
    pub damage: f32,
    /// Speed of the whole cast and its film (×).
    pub speed: f32,
    /// Fly the camera along with each strike.
    pub cutscenes: bool,
}

impl Default for StellarMenu {
    fn default() -> Self {
        Self { open: false, mode: 2, power: 1.0, damage: 1.0, speed: 1.0, cutscenes: true }
    }
}

/// What the overlay did this frame.
#[derive(Default)]
pub struct StellarOut {
    /// Screen areas (points) taken by the menu or button: no touch controls there.
    pub cover: Vec<Rect>,
    /// «Пропустить»: end the cut-scene.
    pub skip: bool,
}

/// `holding`: the remote is in hand. `flash`: colour and strength of a screen flash.
/// `cinema`: 0..1, how far the cinema bars are in.
/// `title`: the cast's name and motto and how long they have been up (s).
pub fn draw(ctx: &egui::Context, _icons: &GuiIcons, m: &mut StellarMenu, holding: bool, flash: Option<([f32; 3], f32)>, cinema: f32, title: Option<(&str, &str, f32)>) -> StellarOut {
    let mut out = StellarOut::default();
    let p = palette(ctx);
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("stellar-fx")));
    if let Some((c, a)) = flash {
        let a = (a.clamp(0.0, 1.0) * 255.0) as u8;
        painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, a));
    }
    // The cast's title: big letters spreading apart, fading in and out.
    if let Some((name, motto, age)) = title {
        let a = (age / 0.5).min(1.0).min(((4.0 - age) / 0.8).max(0.0));
        let alpha = (a * 255.0) as u8;
        let y = screen.top() + screen.height() * 0.30;
        let size = 30.0 + age * 2.0;
        let spaced: String = name.chars().flat_map(|c| [c, ' ']).collect();
        painter.text(Pos2::new(screen.center().x + 2.0, y + 2.0), Align2::CENTER_CENTER, &spaced, bold_font(size), Color32::from_black_alpha(alpha / 2));
        painter.text(Pos2::new(screen.center().x, y), Align2::CENTER_CENTER, &spaced, bold_font(size), Color32::from_rgba_unmultiplied(255, 240, 220, alpha));
        let lw = (screen.width() * 0.25 * a).max(1.0);
        painter.line_segment([Pos2::new(screen.center().x - lw, y + size * 0.75), Pos2::new(screen.center().x + lw, y + size * 0.75)], Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(255, 200, 120, alpha)));
        painter.text(Pos2::new(screen.center().x, y + size * 0.75 + 16.0), Align2::CENTER_CENTER, motto, FontId::proportional(15.0), Color32::from_rgba_unmultiplied(220, 225, 240, alpha));
    }
    if cinema > 0.0 {
        let h = screen.height() * 0.11 * cinema;
        painter.rect_filled(Rect::from_min_size(screen.min, Vec2::new(screen.width(), h)), 0.0, Color32::BLACK);
        painter.rect_filled(Rect::from_min_max(Pos2::new(screen.left(), screen.bottom() - h), screen.max), 0.0, Color32::BLACK);
        // «Пропустить» in the lower bar.
        let r = Rect::from_center_size(Pos2::new(screen.right() - 80.0, screen.bottom() - h * 0.5), Vec2::new(130.0, (h - 8.0).max(20.0).min(34.0)));
        out.cover.push(r);
        egui::Area::new(Id::new("stellar-skip")).order(egui::Order::Foreground).fixed_pos(r.min).show(ctx, |ui| {
            let (rr, resp) = ui.allocate_exact_size(r.size(), Sense::click());
            ui.painter().rect_stroke(rr, Rounding::same(rr.height() / 2.0), Stroke::new(1.5_f32, Color32::from_white_alpha(170)));
            ui.painter().text(rr.center(), Align2::CENTER_CENTER, "Пропустить ▸", FontId::proportional(13.0), Color32::WHITE);
            if resp.clicked() {
                out.skip = true;
            }
        });
    }
    if !holding {
        m.open = false;
        return out;
    }
    if !m.open {
        // A round button on the left: the remote's red button under its cover.
        let r = Rect::from_center_size(Pos2::new(screen.left() + 46.0, screen.center().y - 40.0), Vec2::splat(58.0));
        out.cover.push(r);
        egui::Area::new(Id::new("stellar-open")).order(egui::Order::Foreground).fixed_pos(r.min).show(ctx, |ui| {
            let (rr, resp) = ui.allocate_exact_size(r.size(), Sense::click());
            let c = SKILLS[m.mode as usize % SKILLS.len()].2;
            ui.painter().circle_filled(rr.center(), 29.0, Color32::from_rgba_unmultiplied(15, 18, 30, 210));
            ui.painter().circle_stroke(rr.center(), 29.0, Stroke::new(2.0_f32, Color32::from_rgb(c[0], c[1], c[2])));
            ui.painter().circle_filled(rr.center() - Vec2::new(0.0, 6.0), 9.0, Color32::from_rgb(230, 40, 35));
            ui.painter().text(rr.center() + Vec2::new(0.0, 15.0), Align2::CENTER_CENTER, "Пульт", FontId::proportional(10.5), Color32::WHITE);
            if resp.clicked() {
                m.open = true;
            }
        });
        return out;
    }
    // The skill menu.
    out.cover.push(screen);
    let w = (screen.width() - 24.0).min(560.0);
    let h = (screen.height() - 24.0).min(520.0);
    let panel = Rect::from_center_size(screen.center(), Vec2::new(w, h));
    painter.rect_filled(screen, 0.0, Color32::from_black_alpha(140));
    egui::Area::new(Id::new("stellar-menu")).order(egui::Order::Foreground).fixed_pos(panel.min).show(ctx, |ui| {
        egui::Frame::none().fill(Color32::from_rgb(12, 14, 26)).rounding(Rounding::same(18.0)).stroke(Stroke::new(1.5_f32, Color32::from_rgb(70, 80, 140))).inner_margin(egui::Margin::same(14.0)).show(ui, |ui| {
            ui.set_width(w - 28.0);
            ui.set_max_height(h - 28.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("☄ Звёздный пульт").font(bold_font(20.0)).color(Color32::WHITE));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("Готово").color(Color32::WHITE)).fill(p.accent).min_size(Vec2::new(90.0, 30.0))).clicked() {
                        m.open = false;
                    }
                });
            });
            ui.add_space(6.0);
            egui::ScrollArea::vertical().id_salt("stellar-menu-scroll").show(ui, |ui| {
                // Skill cards.
                for (i, (name, text, c)) in SKILLS.iter().enumerate() {
                    let on = m.mode as usize == i;
                    let col = Color32::from_rgb(c[0], c[1], c[2]);
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 62.0), Sense::click());
                    ui.painter().rect_filled(r, Rounding::same(12.0), if on { col.gamma_multiply(0.30) } else { Color32::from_rgb(22, 25, 40) });
                    if on {
                        ui.painter().rect_stroke(r, Rounding::same(12.0), Stroke::new(2.0_f32, col));
                    }
                    ui.painter().circle_filled(Pos2::new(r.left() + 24.0, r.center().y), 12.0, col);
                    ui.painter().text(Pos2::new(r.left() + 24.0, r.center().y), Align2::CENTER_CENTER, format!("{}", i + 1), FontId::proportional(12.0), Color32::BLACK);
                    ui.painter().text(Pos2::new(r.left() + 46.0, r.top() + 15.0), Align2::LEFT_CENTER, *name, bold_font(15.0), Color32::WHITE);
                    let g = ui.painter().layout(text.to_string(), FontId::proportional(11.5), Color32::from_rgb(180, 186, 210), r.width() - 56.0);
                    ui.painter().galley(Pos2::new(r.left() + 46.0, r.top() + 24.0), g, Color32::WHITE);
                    if resp.clicked() {
                        m.mode = i as u8;
                    }
                    ui.add_space(4.0);
                }
                ui.add_space(6.0);
                let row = |ui: &mut egui::Ui, label: &str, v: &mut f32, lo: f32, hi: f32| {
                    ui.horizontal(|ui| {
                        ui.add_sized(Vec2::new(90.0, 22.0), egui::Label::new(egui::RichText::new(label).color(Color32::WHITE)));
                        ui.add(egui::Slider::new(v, lo..=hi).step_by(0.1).suffix("×"));
                    });
                };
                row(ui, "Мощность", &mut m.power, 0.5, 2.5);
                row(ui, "Урон", &mut m.damage, 0.0, 5.0);
                row(ui, "Скорость", &mut m.speed, 0.3, 3.0);
                ui.checkbox(&mut m.cutscenes, egui::RichText::new("Катсцены (камера летит за ударом)").color(Color32::WHITE));
                ui.label(egui::RichText::new("Тап с пультом — удар по точке прицела. Присесть + тап — это меню. Удары навсегда меняют мир!").size(11.5).color(Color32::from_rgb(160, 166, 190)));
            });
        });
    });
    out
}
