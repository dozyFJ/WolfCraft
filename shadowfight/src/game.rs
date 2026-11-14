//! Screens, touch controls, HUD and the glue between the duel and the picture.

use crate::fight::{Duel, Fighter, Input, Move, Phase, State};
use crate::gfx::Scene;
use crate::story::{CHAPTERS, EPILOGUE, PROLOGUE, load_progress, save_progress};
use crate::world::{self, Spark};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, Vec2 as EV};
use glam::{Vec2, Vec3};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use winit::event::TouchPhase;
use winit::keyboard::KeyCode;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Screen {
    Title,
    Chapters,
    Prologue,
    Story { outro: bool },
    Fight,
    Paused,
    Lost,
    Ending,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Btn {
    Jab,
    Hook,
    Kick,
    Block,
    Dodge,
    Shadow,
}

#[derive(Clone, Copy)]
enum Role {
    Joy { origin: Pos2, cur: Pos2 },
    Btn(Btn),
}

pub const MOUSE: u64 = 1 << 40;

pub struct Game {
    data: PathBuf,
    open: usize,
    pub screen: Screen,
    chapter: usize,
    duel: Option<Duel>,
    pub scene: Scene,
    sparks: Vec<Spark>,
    shake: f32,
    hurt: f32,
    hitstop: f32,
    time: f32,
    dt: f32,
    touches: HashMap<u64, Role>,
    taps: HashSet<Btn>,
    keys: HashSet<KeyCode>,
    view: EV,
    seed: u32,
    preview: Fighter,
    banner_t: f32,
    pub quit: bool,
}

impl Game {
    pub fn new(data: PathBuf) -> Self {
        let open = load_progress(&data);
        Self {
            data,
            open,
            screen: Screen::Title,
            chapter: (open - 1).min(CHAPTERS.len() - 1),
            duel: None,
            scene: Scene::new(),
            sparks: Vec::new(),
            shake: 0.0,
            hurt: 0.0,
            hitstop: 0.0,
            time: 0.0,
            dt: 0.016,
            touches: HashMap::new(),
            taps: HashSet::new(),
            keys: HashSet::new(),
            view: EV::new(800.0, 400.0),
            seed: 12345,
            preview: Fighter::new(Vec2::ZERO, 100.0),
            banner_t: 0.0,
            quit: false,
        }
    }

    fn start_fight(&mut self) {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        self.duel = Some(Duel::new(CHAPTERS[self.chapter].ai, self.seed));
        self.sparks.clear();
        self.touches.clear();
        self.taps.clear();
        self.screen = Screen::Fight;
    }

    // ---------------------------------------------------------------- input

    pub fn key(&mut self, code: KeyCode, down: bool, repeat: bool) {
        if down {
            self.keys.insert(code);
        } else {
            self.keys.remove(&code);
        }
        if !down || repeat {
            return;
        }
        match code {
            KeyCode::KeyJ => {
                self.taps.insert(Btn::Jab);
            }
            KeyCode::KeyK => {
                self.taps.insert(Btn::Hook);
            }
            KeyCode::KeyL => {
                self.taps.insert(Btn::Kick);
            }
            KeyCode::KeyU => {
                self.taps.insert(Btn::Shadow);
            }
            KeyCode::Space => {
                self.taps.insert(Btn::Dodge);
            }
            KeyCode::Escape | KeyCode::BrowserBack => self.back(),
            _ => {}
        }
    }

    /// The phone's Back button / Esc.
    pub fn back(&mut self) {
        self.screen = match self.screen {
            Screen::Fight => Screen::Paused,
            Screen::Paused => Screen::Fight,
            Screen::Title => {
                self.quit = true;
                Screen::Title
            }
            _ => Screen::Title,
        };
    }

    fn layout(&self) -> (f32, Vec<(Btn, Pos2, f32, &'static str)>) {
        let (w, h) = (self.view.x, self.view.y);
        let u = (h.min(w) * 0.13).clamp(34.0, 70.0);
        let b = |x: f32, y: f32| Pos2::new(w - x * u, h - y * u);
        (
            u,
            vec![
                (Btn::Jab, b(1.5, 1.5), 0.95 * u, "Удар"),
                (Btn::Hook, b(3.6, 1.2), 0.75 * u, "Сильный"),
                (Btn::Kick, b(1.3, 3.7), 0.75 * u, "Нога"),
                (Btn::Block, b(3.4, 3.2), 0.75 * u, "Блок"),
                (Btn::Dodge, b(5.4, 1.0), 0.6 * u, "Уклон"),
                (Btn::Shadow, b(5.3, 3.0), 0.7 * u, "Тень"),
            ],
        )
    }

    fn pause_rect(&self) -> Rect {
        Rect::from_center_size(Pos2::new(self.view.x / 2.0, 26.0), EV::new(44.0, 34.0))
    }

    pub fn touch(&mut self, id: u64, phase: TouchPhase, px: (f64, f64), ppp: f32) {
        if self.screen != Screen::Fight {
            self.touches.clear();
            return;
        }
        let p = Pos2::new(px.0 as f32 / ppp, px.1 as f32 / ppp);
        match phase {
            TouchPhase::Started => {
                if self.pause_rect().expand(10.0).contains(p) {
                    self.screen = Screen::Paused;
                    return;
                }
                let (_, btns) = self.layout();
                let hit = btns.iter().filter(|b| b.1.distance(p) < b.2 * 1.15).min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p))).map(|b| b.0);
                if let Some(b) = hit {
                    self.touches.insert(id, Role::Btn(b));
                    self.taps.insert(b);
                } else if p.x < self.view.x * 0.5 {
                    self.touches.insert(id, Role::Joy { origin: p, cur: p });
                }
            }
            TouchPhase::Moved => {
                if let Some(Role::Joy { cur, .. }) = self.touches.get_mut(&id) {
                    *cur = p;
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.touches.remove(&id);
            }
        }
    }

    fn joystick(&self) -> Option<(Pos2, Pos2)> {
        self.touches.values().find_map(|r| match r {
            Role::Joy { origin, cur } => Some((*origin, *cur)),
            _ => None,
        })
    }

    fn input(&mut self) -> Input {
        let (u, _) = self.layout();
        let mut mv = Vec2::ZERO;
        if let Some((o, c)) = self.joystick() {
            let d = (c - o) / (u * 1.1);
            mv = Vec2::new(d.x, -d.y).clamp_length_max(1.0);
        }
        let k = |c: KeyCode| self.keys.contains(&c);
        if k(KeyCode::KeyW) {
            mv.y += 1.0;
        }
        if k(KeyCode::KeyS) {
            mv.y -= 1.0;
        }
        if k(KeyCode::KeyD) {
            mv.x += 1.0;
        }
        if k(KeyCode::KeyA) {
            mv.x -= 1.0;
        }
        let held = self.touches.values().any(|r| matches!(r, Role::Btn(Btn::Block))) || k(KeyCode::KeyI) || k(KeyCode::ShiftLeft);
        let t = std::mem::take(&mut self.taps);
        Input { mv: mv.clamp_length_max(1.0), jab: t.contains(&Btn::Jab), hook: t.contains(&Btn::Hook), kick: t.contains(&Btn::Kick), shadow: t.contains(&Btn::Shadow), block: held, dodge: t.contains(&Btn::Dodge) }
    }

    // ---------------------------------------------------------------- update

    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        self.dt = dt;
        self.shake = (self.shake - dt * 2.5).max(0.0);
        self.hurt = (self.hurt - dt * 2.0).max(0.0);
        self.banner_t += dt;
        if self.screen != Screen::Fight {
            return;
        }
        let input = self.input();
        let mut sim = dt;
        if self.hitstop > 0.0 {
            self.hitstop -= dt;
            sim *= 0.08;
        }
        let Some(d) = self.duel.as_mut() else { return };
        let before = d.phase;
        d.update(sim, input);
        if std::mem::discriminant(&before) != std::mem::discriminant(&d.phase) {
            self.banner_t = 0.0;
        }
        let aura = Vec3::from_array(CHAPTERS[self.chapter].theme.aura);
        let cam = world::camera(d, 0.0, self.time);
        for h in d.hits.clone() {
            let heavy = matches!(h.m, Move::Hook | Move::Kick | Move::Shadow);
            let (at, color) = if h.on_player {
                (cam.eye + cam.fwd * 0.45 - Vec3::Y * 0.05, if h.blocked { Vec3::new(0.7, 0.8, 1.0) } else { aura })
            } else {
                let e = d.enemy.pos;
                let toward = (d.player.pos - e).normalize_or_zero();
                (Vec3::new(e.x + toward.x * 0.25, 1.42, e.y + toward.y * 0.25), if h.blocked { Vec3::new(0.7, 0.8, 1.0) } else if h.m == Move::Shadow { Vec3::new(0.6, 0.35, 1.0) } else { Vec3::new(1.0, 0.7, 0.3) })
            };
            let n = if h.blocked { 8 } else if heavy { 22 } else { 12 };
            for i in 0..n {
                self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let r = |s: u32, k: u32| ((s >> k) % 1000) as f32 / 500.0 - 1.0;
                let v = Vec3::new(r(self.seed, 0), r(self.seed, 10).abs() * 0.8 + 0.2, r(self.seed, 20)) * (2.0 + (i % 3) as f32);
                self.sparks.push(Spark { pos: at, vel: v, life: 0.35 + (i % 4) as f32 * 0.08, color });
            }
            self.shake = self.shake.max(if heavy { 0.9 } else { 0.45 } * if h.on_player { 1.0 } else { 0.6 });
            self.hitstop = if h.blocked { 0.03 } else if heavy { 0.1 } else { 0.05 };
            if h.on_player && !h.blocked {
                self.hurt = 1.0;
            }
        }
        if let Phase::Over { player_won } = d.phase {
            if self.banner_t > 1.5 {
                if player_won {
                    if self.chapter + 2 > self.open {
                        self.open = self.chapter + 2;
                        save_progress(&self.data, self.open);
                    }
                    self.screen = if self.chapter + 1 >= CHAPTERS.len() { Screen::Ending } else { Screen::Story { outro: true } };
                } else {
                    self.screen = Screen::Lost;
                }
            }
        }
    }

    pub fn build_scene(&mut self) {
        let ch = &CHAPTERS[self.chapter];
        let s = &mut self.scene;
        s.clear();
        world::apply_theme(s, &ch.theme);
        s.time = self.time;
        s.hurt = self.hurt;
        world::arena(s, &ch.theme, self.time);
        let aura = Vec3::from_array(ch.theme.aura);
        let in_fight = matches!(self.screen, Screen::Fight | Screen::Paused | Screen::Lost);
        match (&self.duel, in_fight) {
            (Some(d), true) => {
                let cam = world::camera(d, self.shake, self.time);
                s.eye = cam.eye;
                s.target = cam.eye + cam.fwd;
                world::shadow(s, &d.enemy, d.player.pos, aura, self.time);
                world::arms(s, d, &cam, self.time);
            }
            _ => {
                // Menus: the camera circles the shadow standing in the middle.
                let a = self.time * 0.12;
                let eye = Vec3::new(a.sin() * 4.6, 1.75, a.cos() * 4.6);
                s.eye = eye;
                s.target = Vec3::new(0.0, 1.25, 0.0);
                self.preview.moving = 0.0;
                world::shadow(s, &self.preview, Vec2::new(eye.x, eye.z), aura, self.time);
            }
        }
        world::sparks(s, &mut self.sparks, self.dt);
    }

    // ---------------------------------------------------------------- UI

    pub fn ui(&mut self, ctx: &egui::Context) {
        self.view = ctx.screen_rect().size();
        match self.screen {
            Screen::Title => self.title(ctx),
            Screen::Chapters => self.chapters(ctx),
            Screen::Prologue => self.text_screen(ctx, "Пролог", "Врата Теней", PROLOGUE, "Дальше", Screen::Story { outro: false }),
            Screen::Story { outro } => self.story(ctx, outro),
            Screen::Fight => self.hud(ctx),
            Screen::Paused => {
                self.hud(ctx);
                self.pause(ctx);
            }
            Screen::Lost => self.lost(ctx),
            Screen::Ending => self.text_screen(ctx, "Эпилог", "Тень вернулась", EPILOGUE, "В меню", Screen::Title),
        }
    }

    fn title(&mut self, ctx: &egui::Context) {
        let t = self.time;
        dim(ctx, 0.25);
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let r = ui.max_rect();
            let p = ui.painter();
            let c = Pos2::new(r.left() + r.width() * 0.3, r.center().y - 30.0);
            let glow = 0.6 + 0.4 * (t * 1.6).sin();
            for k in 1..4 {
                p.text(c + EV::new(0.0, 0.0), Align2::CENTER_CENTER, "БОЙ С ТЕНЬЮ", FontId::proportional(50.0 + k as f32 * 1.5), Color32::from_rgba_unmultiplied(255, 120, 60, (14.0 * glow) as u8));
            }
            shadow_text(p, c, "БОЙ С ТЕНЬЮ", 50.0, Color32::from_rgb(245, 235, 225));
            shadow_text(p, c + EV::new(0.0, 44.0), "Хроники Врат", 20.0, Color32::from_rgb(255, 160, 110));
            let col = Rect::from_center_size(Pos2::new(r.left() + r.width() * 0.76, r.center().y), EV::new(280.0, 260.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(col), |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    let label = if self.open > 1 { "Продолжить" } else { "Начать" };
                    if big(ui, label) {
                        if self.open > 1 {
                            self.chapter = (self.open - 1).min(CHAPTERS.len() - 1);
                            self.screen = Screen::Story { outro: false };
                        } else {
                            self.chapter = 0;
                            self.screen = Screen::Prologue;
                        }
                    }
                    if big(ui, "Главы") {
                        self.screen = Screen::Chapters;
                    }
                    if big(ui, "Выход") {
                        self.quit = true;
                    }
                });
            });
            p.text(r.right_bottom() + EV::new(-10.0, -8.0), Align2::RIGHT_BOTTOM, "Движение — левая половина экрана · Удар, Сильный, Нога, Блок, Уклон — справа", FontId::proportional(11.0), Color32::from_white_alpha(130));
        });
    }

    fn chapters(&mut self, ctx: &egui::Context) {
        dim(ctx, 0.55);
        egui::CentralPanel::default().frame(egui::Frame::none().inner_margin(16.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("‹ Назад").size(18.0)).min_size(EV::new(100.0, 40.0))).clicked() {
                    self.screen = Screen::Title;
                }
                ui.add_space(12.0);
                ui.label(RichText::new("Главы").size(26.0).strong().color(Color32::WHITE));
            });
            ui.add_space(8.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, ch) in CHAPTERS.iter().enumerate() {
                    let open = i < self.open;
                    let done = i + 1 < self.open;
                    let aura = ch.theme.aura;
                    let accent = Color32::from_rgb((aura[0] * 255.0) as u8, (aura[1] * 255.0) as u8, (aura[2] * 255.0) as u8);
                    let frame = egui::Frame::none().fill(Color32::from_black_alpha(if open { 170 } else { 110 })).rounding(14.0).stroke(Stroke::new(1.5, if open { accent } else { Color32::from_white_alpha(30) })).inner_margin(12.0);
                    let resp = frame
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(ch.title).size(19.0).strong().color(if open { Color32::WHITE } else { Color32::from_white_alpha(90) }));
                                    ui.label(RichText::new(if open { ch.place } else { "Закрыто — победи в прошлой главе" }).size(14.0).color(Color32::from_white_alpha(170)));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if done {
                                        ui.label(RichText::new("Пройдено").size(14.0).color(accent));
                                    } else if open {
                                        ui.label(RichText::new(format!("Соперник: {}", ch.foe)).size(14.0).color(accent));
                                    }
                                });
                            });
                        })
                        .response;
                    if open && resp.interact(egui::Sense::click()).clicked() {
                        self.chapter = i;
                        self.screen = if i == 0 { Screen::Prologue } else { Screen::Story { outro: false } };
                    }
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn text_screen(&mut self, ctx: &egui::Context, kicker: &str, title: &str, text: &str, button: &str, next: Screen) {
        dim(ctx, 0.6);
        let reveal = (self.banner_t * 60.0) as usize;
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let r = ui.max_rect();
            let card = Rect::from_center_size(r.center(), EV::new(r.width().min(640.0) - 32.0, r.height() - 32.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(card), |ui| {
                egui::Frame::none().fill(Color32::from_black_alpha(175)).rounding(18.0).inner_margin(18.0).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(kicker).size(14.0).color(Color32::from_rgb(255, 160, 110)));
                    ui.label(RichText::new(title).size(26.0).strong().color(Color32::WHITE));
                    ui.add_space(6.0);
                    let shown: String = text.chars().take(reveal).collect();
                    egui::ScrollArea::vertical().max_height(card.height() - 150.0).show(ui, |ui| {
                        ui.label(RichText::new(shown).size(16.0).color(Color32::from_white_alpha(225)));
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if reveal < text.chars().count() {
                            if ui.add(egui::Button::new(RichText::new("Показать всё").size(16.0)).min_size(EV::new(150.0, 40.0))).clicked() {
                                self.banner_t = 1000.0;
                            }
                        } else if ui.add(egui::Button::new(RichText::new(button).size(18.0).strong()).min_size(EV::new(170.0, 44.0)).fill(Color32::from_rgb(170, 60, 40))).clicked() {
                            self.banner_t = 0.0;
                            self.screen = next;
                        }
                    });
                });
            });
        });
    }

    fn story(&mut self, ctx: &egui::Context, outro: bool) {
        let ch = &CHAPTERS[self.chapter];
        if outro {
            let next = Screen::Story { outro: false };
            let text = ch.outro;
            self.text_screen(ctx, "Победа", ch.title, text, "Следующая глава", next);
            if self.screen == next {
                self.chapter = (self.chapter + 1).min(CHAPTERS.len() - 1);
            }
        } else {
            let before = self.screen;
            self.text_screen(ctx, ch.place, ch.title, ch.intro, "В бой", Screen::Fight);
            if self.screen == Screen::Fight && before != Screen::Fight {
                self.start_fight();
            }
        }
    }

    fn lost(&mut self, ctx: &egui::Context) {
        dim(ctx, 0.5);
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let r = ui.max_rect();
            shadow_text(ui.painter(), Pos2::new(r.center().x, r.top() + r.height() * 0.3), "ПОРАЖЕНИЕ", 46.0, Color32::from_rgb(230, 70, 60));
            shadow_text(ui.painter(), Pos2::new(r.center().x, r.top() + r.height() * 0.3 + 40.0), "Тень оказалась сильнее. Пока.", 17.0, Color32::from_white_alpha(200));
            let col = Rect::from_center_size(Pos2::new(r.center().x, r.top() + r.height() * 0.66), EV::new(280.0, 140.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(col), |ui| {
                ui.vertical_centered(|ui| {
                    if big(ui, "Ещё раз") {
                        self.start_fight();
                    }
                    if big(ui, "В меню") {
                        self.screen = Screen::Title;
                    }
                });
            });
        });
    }

    fn pause(&mut self, ctx: &egui::Context) {
        dim(ctx, 0.55);
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let r = ui.max_rect();
            shadow_text(ui.painter(), Pos2::new(r.center().x, r.top() + r.height() * 0.22), "ПАУЗА", 40.0, Color32::WHITE);
            let col = Rect::from_center_size(Pos2::new(r.center().x, r.top() + r.height() * 0.62), EV::new(280.0, 200.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(col), |ui| {
                ui.vertical_centered(|ui| {
                    if big(ui, "Продолжить") {
                        self.screen = Screen::Fight;
                    }
                    if big(ui, "Начать бой заново") {
                        self.start_fight();
                    }
                    if big(ui, "В меню") {
                        self.screen = Screen::Title;
                    }
                });
            });
        });
    }

    fn hud(&mut self, ctx: &egui::Context) {
        let Some(d) = &self.duel else { return };
        let ch = &CHAPTERS[self.chapter];
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("hud")));
        let (w, h) = (self.view.x, self.view.y);
        let aura = ch.theme.aura;
        let accent = Color32::from_rgb((aura[0] * 255.0) as u8, (aura[1] * 255.0) as u8, (aura[2] * 255.0) as u8);
        let bw = (w * 0.34).min(340.0);
        // Health bars.
        bar(&p, Rect::from_min_size(Pos2::new(16.0, 14.0), EV::new(bw, 14.0)), d.player.hp / d.player.max_hp, Color32::from_rgb(90, 200, 120), false);
        bar(&p, Rect::from_min_size(Pos2::new(w - 16.0 - bw, 14.0), EV::new(bw, 14.0)), d.enemy.hp / d.enemy.max_hp, accent, true);
        p.text(Pos2::new(16.0, 32.0), Align2::LEFT_TOP, "Арен", FontId::proportional(14.0), Color32::WHITE);
        p.text(Pos2::new(w - 16.0, 32.0), Align2::RIGHT_TOP, ch.foe, FontId::proportional(14.0), accent);
        // Round wins.
        for k in 0..2u8 {
            let on_p = d.wins[0] > k;
            let on_e = d.wins[1] > k;
            p.circle(Pos2::new(16.0 + bw - 8.0 - k as f32 * 16.0, 38.0), 5.0, if on_p { Color32::from_rgb(255, 210, 90) } else { Color32::from_black_alpha(120) }, Stroke::new(1.0, Color32::from_white_alpha(120)));
            p.circle(Pos2::new(w - 16.0 - bw + 8.0 + k as f32 * 16.0, 38.0), 5.0, if on_e { Color32::from_rgb(255, 210, 90) } else { Color32::from_black_alpha(120) }, Stroke::new(1.0, Color32::from_white_alpha(120)));
        }
        // Shadow power.
        let full = d.player.power >= 1.0;
        let pw = Rect::from_min_size(Pos2::new(16.0, 52.0), EV::new(bw * 0.6, 7.0));
        bar(&p, pw, d.player.power, if full { Color32::from_rgb(170, 110, 255) } else { Color32::from_rgb(110, 80, 180) }, false);
        if full {
            let g = 0.5 + 0.5 * (self.time * 6.0).sin();
            p.rect_stroke(pw.expand(2.0), 4.0, Stroke::new(1.5, Color32::from_rgba_unmultiplied(200, 160, 255, (200.0 * g) as u8)));
        }
        // Pause + timer.
        let pr = self.pause_rect();
        p.rect_filled(pr, 10.0, Color32::from_black_alpha(120));
        for dx in [-5.0, 5.0] {
            p.rect_filled(Rect::from_center_size(pr.center() + EV::new(dx, 0.0), EV::new(4.0, 14.0)), 1.0, Color32::WHITE);
        }
        shadow_text(&p, Pos2::new(w / 2.0, 58.0), &format!("{:.0}", d.timer.ceil()), 22.0, Color32::WHITE);
        // Combo.
        if d.player.combo >= 2 {
            shadow_text(&p, Pos2::new(w * 0.5, h * 0.3), &format!("×{} комбо", d.player.combo), 24.0, Color32::from_rgb(255, 200, 90));
        }
        // Banners.
        let bt = self.banner_t;
        match d.phase {
            Phase::Intro { .. } => {
                let text = if bt < 1.3 { format!("РАУНД {}", d.round) } else { "БОЙ!".to_string() };
                let s = 1.0 + (1.0 - (bt.fract() * 3.0).min(1.0)) * 0.3;
                shadow_text(&p, Pos2::new(w / 2.0, h * 0.42), &text, 54.0 * s, Color32::WHITE);
            }
            Phase::RoundEnd { player_won, .. } => {
                let ko = !d.player.alive() || !d.enemy.alive();
                if ko {
                    shadow_text(&p, Pos2::new(w / 2.0, h * 0.36), "НОКАУТ", 58.0, Color32::from_rgb(255, 90, 60));
                }
                shadow_text(&p, Pos2::new(w / 2.0, h * 0.36 + if ko { 50.0 } else { 0.0 }), if player_won { "Раунд за тобой" } else { "Раунд проигран" }, 26.0, Color32::WHITE);
            }
            Phase::Over { player_won } => {
                shadow_text(&p, Pos2::new(w / 2.0, h * 0.4), if player_won { "ПОБЕДА" } else { "ПОРАЖЕНИЕ" }, 60.0, if player_won { Color32::from_rgb(255, 215, 100) } else { Color32::from_rgb(230, 70, 60) });
            }
            Phase::Fight => {}
        }
        if self.screen != Screen::Fight {
            return;
        }
        // Joystick.
        let (u, btns) = self.layout();
        let (o, c) = self.joystick().unwrap_or((Pos2::new(2.2 * u, h - 2.0 * u), Pos2::new(2.2 * u, h - 2.0 * u)));
        p.circle(o, u * 1.1, Color32::from_black_alpha(70), Stroke::new(1.5, Color32::from_white_alpha(70)));
        let d2 = { let v = c - o; let l = v.length(); if l > u * 1.1 { v * (u * 1.1 / l) } else { v } };
        p.circle_filled(o + d2, u * 0.45, Color32::from_white_alpha(110));
        // Buttons.
        let held: HashSet<Btn> = self.touches.values().filter_map(|r| if let Role::Btn(b) = r { Some(*b) } else { None }).collect();
        for (b, pos, r, label) in btns {
            let ready = b != Btn::Shadow || full;
            let on = held.contains(&b);
            let fill = match (b, on) {
                (Btn::Shadow, _) if full => Color32::from_rgba_unmultiplied(130, 80, 230, if on { 230 } else { 160 }),
                (Btn::Block, true) => Color32::from_rgba_unmultiplied(80, 140, 230, 200),
                (_, true) => Color32::from_white_alpha(110),
                _ => Color32::from_black_alpha(110),
            };
            p.circle(pos, r, fill, Stroke::new(2.0, Color32::from_white_alpha(if ready { 150 } else { 50 })));
            p.text(pos, Align2::CENTER_CENTER, label, FontId::proportional((r * 0.32).clamp(11.0, 18.0)), Color32::from_white_alpha(if ready { 235 } else { 90 }));
        }
        let _ = State::Idle;
    }
}

fn dim(ctx: &egui::Context, k: f32) {
    let p = ctx.layer_painter(egui::LayerId::background());
    p.rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha((k * 255.0) as u8));
}

fn big(ui: &mut egui::Ui, text: &str) -> bool {
    let b = egui::Button::new(RichText::new(text).size(21.0).strong().color(Color32::WHITE)).min_size(EV::new(260.0, 50.0)).rounding(14.0).fill(Color32::from_rgba_unmultiplied(20, 14, 18, 200)).stroke(Stroke::new(1.5, Color32::from_rgb(200, 90, 60)));
    let r = ui.add(b).clicked();
    ui.add_space(8.0);
    r
}

fn shadow_text(p: &egui::Painter, at: Pos2, text: &str, size: f32, color: Color32) {
    p.text(at + EV::new(2.0, 3.0), Align2::CENTER_CENTER, text, FontId::proportional(size), Color32::from_black_alpha(170));
    p.text(at, Align2::CENTER_CENTER, text, FontId::proportional(size), color);
}

fn bar(p: &egui::Painter, r: Rect, k: f32, color: Color32, from_right: bool) {
    p.rect_filled(r.expand(2.0), 5.0, Color32::from_black_alpha(160));
    let w = r.width() * k.clamp(0.0, 1.0);
    let fill = if from_right { Rect::from_min_max(Pos2::new(r.right() - w, r.top()), r.max) } else { Rect::from_min_size(r.min, EV::new(w, r.height())) };
    p.rect_filled(fill, 4.0, color);
    p.rect_filled(Rect::from_min_size(fill.min, EV::new(fill.width(), r.height() * 0.4)), 4.0, Color32::from_white_alpha(40));
}
