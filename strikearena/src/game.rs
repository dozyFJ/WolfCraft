//! Screens, touch controls and the HUD of Strike Arena.

use crate::gfx::Scene;
use crate::render::{self, Spark, Tracer};
use crate::sim::{Event, GUNS, Gun, Input, Match, gun};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, Vec2 as EV};
use glam::{Vec2, Vec3};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use winit::event::TouchPhase;
use winit::keyboard::KeyCode;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Screen {
    Title,
    Playing,
    Paused,
    Over,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Btn {
    Fire,
    Aim,
    Reload,
    Swap,
    Pause,
}

#[derive(Clone, Copy)]
enum Role {
    Joy { cur: Pos2 },
    Look { last: Pos2 },
    Btn { b: Btn, last: Pos2 },
}

pub const MOUSE: u64 = 1 << 40;

const SKILLS: [(&str, f32); 3] = [("Лёгкие", 0.25), ("Обычные", 0.55), ("Сложные", 0.85)];

pub struct Game {
    data: PathBuf,
    pub screen: Screen,
    pub m: Option<Match>,
    pub scene: Scene,
    tracers: Vec<Tracer>,
    sparks: Vec<Spark>,
    feed: Vec<(String, Color32, f32)>,
    hitmark: f32,
    killmark: f32,
    shake: f32,
    time: f32,
    dt: f32,
    touches: HashMap<u64, Role>,
    look: Vec2,
    aim_toggle: bool,
    taps: HashSet<Btn>,
    keys: HashSet<KeyCode>,
    view: EV,
    seed: u32,
    skill: usize,
    sens: f32,
    pub quit: bool,
}

impl Game {
    pub fn new(data: PathBuf) -> Self {
        let cfg = std::fs::read_to_string(data.join("settings.txt")).unwrap_or_default();
        let mut it = cfg.split_whitespace();
        let skill = it.next().and_then(|v| v.parse().ok()).unwrap_or(1usize).min(2);
        let sens = it.next().and_then(|v| v.parse().ok()).unwrap_or(1.0f32).clamp(0.3, 3.0);
        Self {
            data,
            screen: Screen::Title,
            m: None,
            scene: Scene::new(),
            tracers: Vec::new(),
            sparks: Vec::new(),
            feed: Vec::new(),
            hitmark: 0.0,
            killmark: 0.0,
            shake: 0.0,
            time: 0.0,
            dt: 0.016,
            touches: HashMap::new(),
            look: Vec2::ZERO,
            aim_toggle: false,
            taps: HashSet::new(),
            keys: HashSet::new(),
            view: EV::new(800.0, 400.0),
            seed: 777,
            skill,
            sens,
            quit: false,
        }
    }

    fn save(&self) {
        let _ = std::fs::create_dir_all(&self.data);
        let _ = std::fs::write(self.data.join("settings.txt"), format!("{} {}", self.skill, self.sens));
    }

    fn start(&mut self) {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        self.m = Some(Match::new(self.seed, SKILLS[self.skill].1));
        self.tracers.clear();
        self.sparks.clear();
        self.feed.clear();
        self.touches.clear();
        self.aim_toggle = false;
        self.screen = Screen::Playing;
    }

    // ------------------------------------------------------------ input

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
            KeyCode::KeyQ => self.aim_toggle = !self.aim_toggle,
            KeyCode::KeyR => {
                self.taps.insert(Btn::Reload);
            }
            KeyCode::Tab => {
                self.taps.insert(Btn::Swap);
            }
            KeyCode::Escape | KeyCode::BrowserBack => self.back(),
            _ => {}
        }
    }

    pub fn back(&mut self) {
        self.screen = match self.screen {
            Screen::Playing => Screen::Paused,
            Screen::Paused => Screen::Playing,
            Screen::Title => {
                self.quit = true;
                Screen::Title
            }
            Screen::Over => Screen::Title,
        };
    }

    fn unit(&self) -> f32 {
        (self.view.y.min(self.view.x) * 0.13).clamp(34.0, 70.0)
    }

    fn joy_home(&self) -> Pos2 {
        let u = self.unit();
        Pos2::new(2.3 * u, self.view.y - 2.1 * u)
    }

    fn buttons(&self) -> Vec<(Btn, Pos2, f32, &'static str)> {
        let (w, h, u) = (self.view.x, self.view.y, self.unit());
        vec![
            (Btn::Fire, Pos2::new(w - 1.6 * u, h - 1.9 * u), 1.0 * u, "Огонь"),
            (Btn::Aim, Pos2::new(w - 3.7 * u, h - 1.2 * u), 0.62 * u, "Прицел"),
            (Btn::Reload, Pos2::new(w - 1.1 * u, h - 4.0 * u), 0.55 * u, "Перезар."),
            (Btn::Swap, Pos2::new(w - 3.3 * u, h - 3.3 * u), 0.55 * u, "Оружие"),
            (Btn::Pause, Pos2::new(w / 2.0, 26.0), 0.42 * u, "II"),
        ]
    }

    pub fn touch(&mut self, id: u64, phase: TouchPhase, px: (f64, f64), ppp: f32) {
        if self.screen != Screen::Playing {
            self.touches.clear();
            return;
        }
        let p = Pos2::new(px.0 as f32 / ppp, px.1 as f32 / ppp);
        match phase {
            TouchPhase::Started => {
                let hit = self.buttons().into_iter().filter(|b| b.1.distance(p) < b.2 * 1.15).min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p))).map(|b| b.0);
                if let Some(b) = hit {
                    match b {
                        Btn::Pause => {
                            self.screen = Screen::Paused;
                            return;
                        }
                        Btn::Aim => self.aim_toggle = !self.aim_toggle,
                        Btn::Reload | Btn::Swap => {
                            self.taps.insert(b);
                        }
                        Btn::Fire => {}
                    }
                    self.touches.insert(id, Role::Btn { b, last: p });
                } else if p.distance(self.joy_home()) < self.unit() * 2.4 && !self.touches.values().any(|r| matches!(r, Role::Joy { .. })) {
                    self.touches.insert(id, Role::Joy { cur: p });
                } else if p.x > self.view.x * 0.35 {
                    self.touches.insert(id, Role::Look { last: p });
                }
            }
            TouchPhase::Moved => {
                let mut d = None;
                match self.touches.get_mut(&id) {
                    Some(Role::Joy { cur }) => *cur = p,
                    // Firing and looking with the same thumb.
                    Some(Role::Look { last }) | Some(Role::Btn { b: Btn::Fire, last }) => {
                        d = Some(p - *last);
                        *last = p;
                    }
                    _ => {}
                }
                if let Some(d) = d {
                    self.look += Vec2::new(d.x, d.y);
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.touches.remove(&id);
            }
        }
    }

    fn input(&mut self) -> Input {
        let mut mv = Vec2::ZERO;
        let u = self.unit();
        if let Some(c) = self.touches.values().find_map(|r| if let Role::Joy { cur } = r { Some(*cur) } else { None }) {
            let d = (c - self.joy_home()) / (u * 1.1);
            mv = Vec2::new(d.x, -d.y).clamp_length_max(1.0);
            if mv.length() < 0.12 {
                mv = Vec2::ZERO;
            }
        }
        let k = |c: KeyCode| self.keys.contains(&c);
        mv += Vec2::new((k(KeyCode::KeyD) as i32 - k(KeyCode::KeyA) as i32) as f32, (k(KeyCode::KeyW) as i32 - k(KeyCode::KeyS) as i32) as f32);
        let mut look = std::mem::take(&mut self.look);
        look += Vec2::new((k(KeyCode::ArrowRight) as i32 - k(KeyCode::ArrowLeft) as i32) as f32, (k(KeyCode::ArrowDown) as i32 - k(KeyCode::ArrowUp) as i32) as f32) * 6.0;
        let fire = self.touches.values().any(|r| matches!(r, Role::Btn { b: Btn::Fire, .. })) || k(KeyCode::Space);
        let taps = std::mem::take(&mut self.taps);
        let me = self.m.as_ref().map(|m| &m.men[0]);
        let zoom = me.map(|s| if s.aiming { s.spec().zoom } else { 1.0 }).unwrap_or(1.0);
        let k_look = 0.0055 * self.sens / zoom;
        let mut switch = None;
        if taps.contains(&Btn::Swap) {
            if let Some(s) = me {
                switch = Some((s.gun + 1) % GUNS.len());
            }
        }
        for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4].into_iter().enumerate() {
            if k(key) {
                switch = Some(i);
            }
        }
        if switch.is_some() {
            self.aim_toggle = false;
        }
        Input { mv: mv.clamp_length_max(1.0), look: Vec2::new(look.x, -look.y) * k_look, fire, aim: self.aim_toggle, reload: taps.contains(&Btn::Reload), switch }
    }

    // ------------------------------------------------------------ update

    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        self.dt = dt;
        self.hitmark = (self.hitmark - dt).max(0.0);
        self.killmark = (self.killmark - dt).max(0.0);
        self.shake = (self.shake - dt * 3.0).max(0.0);
        for f in &mut self.feed {
            f.2 -= dt;
        }
        self.feed.retain(|f| f.2 > 0.0);
        if self.screen != Screen::Playing {
            return;
        }
        let input = self.input();
        let Some(m) = self.m.as_mut() else { return };
        m.update(dt, input);
        let me = &m.men[0];
        let cam = render::camera(me, 0.0, self.time);
        for e in m.events.clone() {
            match e {
                Event::Shot { shooter, from, to } => {
                    // From the gun, not from the eyes.
                    let a = if shooter == 0 { cam.eye + cam.right * 0.17 - cam.up * 0.15 + cam.fwd * 0.9 } else { from - Vec3::Y * 0.3 + (to - from).normalize_or_zero() * 0.7 };
                    if self.tracers.len() < 60 {
                        self.tracers.push(Tracer { a, b: to, life: 0.05 });
                    }
                    if shooter == 0 {
                        self.shake = self.shake.max(gun(GUNS[m.men[0].gun]).kick * 6.0);
                    }
                }
                Event::Impact { at } => burst(&mut self.sparks, &mut self.seed, at, Vec3::new(0.9, 0.85, 0.7), 5),
                Event::Hit { victim, by, at, head } => {
                    burst(&mut self.sparks, &mut self.seed, at, Vec3::new(0.85, 0.1, 0.1), if head { 10 } else { 6 });
                    if by == 0 {
                        self.hitmark = 0.15;
                    }
                    if victim == 0 {
                        self.shake = self.shake.max(0.6);
                    }
                }
                Event::Kill { killer, victim, gun: g, head } => {
                    let kn = &m.men[killer];
                    let vn = &m.men[victim];
                    let col = if killer == 0 { Color32::from_rgb(255, 215, 90) } else if kn.team == 0 { Color32::from_rgb(120, 170, 255) } else { Color32::from_rgb(255, 120, 110) };
                    self.feed.push((format!("{} [{}{}] {}", kn.name, gun(g).name, if head { " · в голову" } else { "" }, vn.name), col, 5.0));
                    if self.feed.len() > 5 {
                        self.feed.remove(0);
                    }
                    if killer == 0 {
                        self.killmark = 0.5;
                    }
                }
            }
        }
        if m.over.is_some() {
            self.screen = Screen::Over;
        }
    }

    pub fn fov(&self) -> f32 {
        let base = 72f32.to_radians();
        match &self.m {
            Some(m) if self.screen != Screen::Title && m.men[0].aiming => base / m.men[0].spec().zoom,
            _ => base,
        }
    }

    pub fn build_scene(&mut self) {
        let fov = self.fov();
        let playing = self.m.is_some() && matches!(self.screen, Screen::Playing | Screen::Paused | Screen::Over);
        if self.m.is_none() {
            // Title: a calm match on the warehouse floor to fly over.
            self.m = Some(Match::new(1, 0.5));
        }
        let s = &mut self.scene;
        s.clear();
        render::sky(s);
        s.time = self.time;
        let m = self.m.as_ref().expect("match");
        render::map(s, &m.map, self.time);
        if playing {
            let me = &m.men[0];
            s.hurt = me.hurt * 0.8;
            let cam = render::camera(me, self.shake, self.time);
            s.eye = cam.eye;
            s.target = cam.eye + cam.fwd;
            s.fov_y = fov;
            render::everyone(s, m, 0, self.time);
            render::viewmodel(s, me, &cam, self.time);
        } else {
            s.hurt = 0.0;
            let a = self.time * 0.06;
            s.eye = Vec3::new(a.sin() * 18.0, 9.0, a.cos() * 18.0);
            s.target = Vec3::new(0.0, 1.0, 0.0);
            s.fov_y = 60f32.to_radians();
            for o in &m.men {
                render::soldier(s, o, self.time);
            }
        }
        render::effects(s, &mut self.tracers, &mut self.sparks, self.dt);
    }

    // ------------------------------------------------------------ UI

    pub fn ui(&mut self, ctx: &egui::Context) {
        self.view = ctx.screen_rect().size();
        match self.screen {
            Screen::Title => self.title(ctx),
            Screen::Playing => self.hud(ctx),
            Screen::Paused => {
                self.hud(ctx);
                self.pause(ctx);
            }
            Screen::Over => self.over(ctx),
        }
    }

    fn title(&mut self, ctx: &egui::Context) {
        dim(ctx, 0.35);
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let r = ui.max_rect();
            let p = ui.painter().clone();
            let c = Pos2::new(r.left() + r.width() * 0.3, r.center().y - 30.0);
            shadow_text(&p, c, "STRIKE ARENA", 52.0, Color32::WHITE);
            shadow_text(&p, c + EV::new(0.0, 44.0), "Склад · 5 на 5 · до 30 убийств", 18.0, Color32::from_rgb(255, 190, 80));
            let col = Rect::from_center_size(Pos2::new(r.left() + r.width() * 0.76, r.center().y), EV::new(300.0, 300.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(col), |ui| {
                ui.vertical_centered(|ui| {
                    if big(ui, "Играть") {
                        self.start();
                    }
                    if big(ui, &format!("Боты: {}", SKILLS[self.skill].0)) {
                        self.skill = (self.skill + 1) % SKILLS.len();
                        self.save();
                    }
                    ui.label(RichText::new("Чувствительность").size(15.0).color(Color32::WHITE));
                    let before = self.sens;
                    ui.add(egui::Slider::new(&mut self.sens, 0.3..=3.0).show_value(true));
                    if (before - self.sens).abs() > 1e-4 {
                        self.save();
                    }
                    ui.add_space(8.0);
                    if big(ui, "Выход") {
                        self.quit = true;
                    }
                });
            });
            p.text(r.right_bottom() + EV::new(-10.0, -8.0), Align2::RIGHT_BOTTOM, "Слева — ходьба, справа — обзор; «Огонь» стреляет и поворачивает", FontId::proportional(11.0), Color32::from_white_alpha(150));
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
                        self.screen = Screen::Playing;
                    }
                    if big(ui, "Заново") {
                        self.start();
                    }
                    if big(ui, "В меню") {
                        self.m = None;
                        self.screen = Screen::Title;
                    }
                });
            });
        });
    }

    fn over(&mut self, ctx: &egui::Context) {
        dim(ctx, 0.6);
        let Some(m) = &self.m else { return };
        let won = m.over;
        let score = m.score;
        let mut rows: Vec<(String, u8, u32, u32, bool)> = m.men.iter().enumerate().map(|(k, s)| (s.name.clone(), s.team, s.kills, s.deaths, k == 0)).collect();
        rows.sort_by(|a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2)));
        let mut again = false;
        let mut menu = false;
        egui::CentralPanel::default().frame(egui::Frame::none().inner_margin(16.0)).show(ctx, |ui| {
            let (text, col) = match won {
                Some(0) => ("ПОБЕДА", Color32::from_rgb(255, 215, 90)),
                Some(1) => ("ПОРАЖЕНИЕ", Color32::from_rgb(230, 80, 70)),
                _ => ("НИЧЬЯ", Color32::WHITE),
            };
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(text).size(36.0).strong().color(col));
                ui.label(RichText::new(format!("Синие {} : {} Красные", score[0], score[1])).size(20.0).color(Color32::WHITE));
            });
            ui.add_space(6.0);
            ui.columns(2, |cols| {
                for (t, c) in cols.iter_mut().enumerate() {
                    let tc = if t == 0 { Color32::from_rgb(120, 170, 255) } else { Color32::from_rgb(255, 120, 110) };
                    egui::Frame::none().fill(Color32::from_black_alpha(160)).rounding(12.0).inner_margin(10.0).show(c, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(if t == 0 { "Синие" } else { "Красные" }).size(17.0).strong().color(tc));
                        egui::Grid::new(("score", t)).num_columns(3).spacing(EV::new(18.0, 4.0)).show(ui, |ui| {
                            ui.label(RichText::new("Боец").color(Color32::from_white_alpha(150)));
                            ui.label(RichText::new("Убийства").color(Color32::from_white_alpha(150)));
                            ui.label(RichText::new("Смерти").color(Color32::from_white_alpha(150)));
                            ui.end_row();
                            for r in rows.iter().filter(|r| r.1 as usize == t) {
                                let c = if r.4 { Color32::from_rgb(255, 215, 90) } else { Color32::WHITE };
                                ui.label(RichText::new(&r.0).color(c));
                                ui.label(RichText::new(r.2.to_string()).color(c));
                                ui.label(RichText::new(r.3.to_string()).color(c));
                                ui.end_row();
                            }
                        });
                    });
                }
            });
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 540.0).max(0.0) / 2.0);
                    again = big(ui, "Ещё бой");
                    menu = big(ui, "В меню");
                });
            });
        });
        if again {
            self.start();
        } else if menu {
            self.m = None;
            self.screen = Screen::Title;
        }
    }

    fn hud(&mut self, ctx: &egui::Context) {
        let Some(m) = &self.m else { return };
        let me = &m.men[0];
        let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("hud")));
        let (w, h) = (self.view.x, self.view.y);
        let c = Pos2::new(w / 2.0, h / 2.0);
        let spec = me.spec();
        let sniper_scope = me.aiming && GUNS[me.gun] == Gun::Sniper && me.alive();
        if sniper_scope {
            // Scope: black around a round lens with a thin cross.
            let r = h * 0.46;
            let lens = Rect::from_center_size(c, EV::splat(r * 2.0));
            let black = Color32::from_black_alpha(250);
            p.rect_filled(Rect::from_min_max(Pos2::ZERO, Pos2::new(lens.left(), h)), 0.0, black);
            p.rect_filled(Rect::from_min_max(Pos2::new(lens.right(), 0.0), Pos2::new(w, h)), 0.0, black);
            p.circle_stroke(c, r + 60.0, Stroke::new(120.0_f32, black));
            p.line_segment([Pos2::new(c.x - r, c.y), Pos2::new(c.x + r, c.y)], Stroke::new(1.2_f32, Color32::BLACK));
            p.line_segment([Pos2::new(c.x, c.y - r), Pos2::new(c.x, c.y + r)], Stroke::new(1.2_f32, Color32::BLACK));
        } else if me.alive() {
            // Crosshair opening with the spread.
            let spread = (if me.aiming { spec.aim_spread } else { spec.spread }) * (1.0 + me.moving * 0.8) + me.recoil * 0.6;
            let gap = 4.0 + spread * h * 0.9;
            let col = Color32::from_white_alpha(230);
            for (dx, dy) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let a = c + EV::new(dx, dy) * gap;
                p.line_segment([a + EV::new(1.0, 1.0), a + EV::new(dx, dy) * 9.0 + EV::new(1.0, 1.0)], Stroke::new(2.5_f32, Color32::from_black_alpha(140)));
                p.line_segment([a, a + EV::new(dx, dy) * 9.0], Stroke::new(2.0_f32, col));
            }
        }
        if self.hitmark > 0.0 || self.killmark > 0.0 {
            let col = if self.killmark > 0.0 { Color32::from_rgb(255, 70, 60) } else { Color32::WHITE };
            for (dx, dy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                p.line_segment([c + EV::new(dx, dy) * 7.0, c + EV::new(dx, dy) * 14.0], Stroke::new(2.5_f32, col));
            }
        }
        // Where the hit came from.
        if me.hurt > 0.05 {
            let to = me.hurt_from - me.pos;
            let ang = to.x.atan2(to.y);
            let rel = me.yaw - ang;
            let dir = EV::new(rel.sin(), -rel.cos());
            let a = (me.hurt * 200.0) as u8;
            p.circle_filled(c + dir * h * 0.3, 10.0, Color32::from_rgba_unmultiplied(230, 40, 30, a));
        }
        // Score and time.
        let top = Pos2::new(w / 2.0, 62.0);
        p.rect_filled(Rect::from_center_size(top, EV::new(190.0, 30.0)), 8.0, Color32::from_black_alpha(140));
        p.text(top - EV::new(40.0, 0.0), Align2::RIGHT_CENTER, m.score[0].to_string(), FontId::proportional(20.0), Color32::from_rgb(120, 170, 255));
        p.text(top + EV::new(40.0, 0.0), Align2::LEFT_CENTER, m.score[1].to_string(), FontId::proportional(20.0), Color32::from_rgb(255, 120, 110));
        let secs = m.time.ceil() as u32;
        p.text(top, Align2::CENTER_CENTER, format!("{}:{:02}", secs / 60, secs % 60), FontId::proportional(16.0), Color32::WHITE);
        // Health.
        let hb = Rect::from_min_size(Pos2::new(16.0, 14.0), EV::new((w * 0.26).min(240.0), 12.0));
        p.rect_filled(hb.expand(2.0), 5.0, Color32::from_black_alpha(150));
        let k = (me.hp / 100.0).clamp(0.0, 1.0);
        let hc = if k > 0.5 { Color32::from_rgb(90, 210, 120) } else if k > 0.25 { Color32::from_rgb(240, 190, 60) } else { Color32::from_rgb(235, 70, 60) };
        p.rect_filled(Rect::from_min_size(hb.min, EV::new(hb.width() * k, hb.height())), 4.0, hc);
        p.text(Pos2::new(16.0, 30.0), Align2::LEFT_TOP, format!("{:.0} · {}", me.hp, spec.name), FontId::proportional(14.0), Color32::WHITE);
        // Kill feed.
        for (i, (text, col, life)) in self.feed.iter().enumerate() {
            let a = (life.min(1.0) * 255.0) as u8;
            let pos = Pos2::new(w - 14.0, 14.0 + i as f32 * 20.0);
            p.text(pos + EV::new(1.0, 1.0), Align2::RIGHT_TOP, text, FontId::proportional(13.0), Color32::from_black_alpha(a / 2 + 40));
            p.text(pos, Align2::RIGHT_TOP, text, FontId::proportional(13.0), Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), a));
        }
        if !me.alive() {
            let killer = if me.last_by != 0 { m.men[me.last_by].name.clone() } else { String::new() };
            shadow_text(&p, Pos2::new(w / 2.0, h * 0.35), "ТЫ УБИТ", 40.0, Color32::from_rgb(235, 70, 60));
            if !killer.is_empty() {
                shadow_text(&p, Pos2::new(w / 2.0, h * 0.35 + 36.0), &format!("Убил: {killer}"), 18.0, Color32::WHITE);
            }
            shadow_text(&p, Pos2::new(w / 2.0, h * 0.35 + 64.0), &format!("Возрождение через {:.0}", me.respawn.max(0.0).ceil()), 16.0, Color32::from_white_alpha(200));
        }
        if self.screen != Screen::Playing {
            return;
        }
        // Controls.
        let u = self.unit();
        let home = self.joy_home();
        let knob = self.touches.values().find_map(|r| if let Role::Joy { cur } = r { Some(*cur) } else { None }).map(|c| {
            let d = c - home;
            let l = d.length();
            if l > u * 1.1 { home + d * (u * 1.1 / l) } else { c }
        });
        p.circle(home, u * 1.1, Color32::from_black_alpha(70), Stroke::new(1.5_f32, Color32::from_white_alpha(80)));
        p.circle_filled(knob.unwrap_or(home), u * 0.45, Color32::from_white_alpha(if knob.is_some() { 150 } else { 90 }));
        let held: HashSet<Btn> = self.touches.values().filter_map(|r| if let Role::Btn { b, .. } = r { Some(*b) } else { None }).collect();
        for (b, pos, r, label) in self.buttons() {
            let on = held.contains(&b) || (b == Btn::Aim && self.aim_toggle);
            let fill = match (b, on) {
                (Btn::Fire, true) => Color32::from_rgba_unmultiplied(230, 80, 50, 210),
                (Btn::Fire, false) => Color32::from_rgba_unmultiplied(160, 40, 30, 150),
                (_, true) => Color32::from_white_alpha(120),
                _ => Color32::from_black_alpha(110),
            };
            p.circle(pos, r, fill, Stroke::new(2.0_f32, Color32::from_white_alpha(150)));
            p.text(pos, Align2::CENTER_CENTER, label, FontId::proportional((r * 0.34).clamp(11.0, 18.0)), Color32::WHITE);
        }
        // Ammo by the fire button.
        let (_, fire_at, fr, _) = self.buttons()[0];
        let ammo = if me.reload > 0.0 { "перезарядка…".to_string() } else { format!("{} / {}", me.mag[me.gun], me.reserve[me.gun]) };
        shadow_text(&p, fire_at - EV::new(0.0, fr + 16.0), &ammo, 18.0, if me.mag[me.gun] == 0 { Color32::from_rgb(255, 90, 70) } else { Color32::WHITE });
    }
}

fn burst(sparks: &mut Vec<Spark>, seed: &mut u32, at: Vec3, color: Vec3, n: u32) {
    for i in 0..n {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let r = |s: u32, k: u32| ((s >> k) % 1000) as f32 / 500.0 - 1.0;
        let v = Vec3::new(r(*seed, 0), r(*seed, 10).abs(), r(*seed, 20)) * (1.5 + (i % 3) as f32);
        if sparks.len() < 300 {
            sparks.push(Spark { pos: at, vel: v, life: 0.3 + (i % 3) as f32 * 0.08, color });
        }
    }
}

fn dim(ctx: &egui::Context, k: f32) {
    let p = ctx.layer_painter(egui::LayerId::background());
    p.rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha((k * 255.0) as u8));
}

fn big(ui: &mut egui::Ui, text: &str) -> bool {
    let b = egui::Button::new(RichText::new(text).size(21.0).strong().color(Color32::WHITE)).min_size(EV::new(260.0, 50.0)).rounding(12.0).fill(Color32::from_rgba_unmultiplied(18, 22, 30, 210)).stroke(Stroke::new(1.5_f32, Color32::from_rgb(255, 170, 60)));
    let r = ui.add(b).clicked();
    ui.add_space(8.0);
    r
}

fn shadow_text(p: &egui::Painter, at: Pos2, text: &str, size: f32, color: Color32) {
    p.text(at + EV::new(2.0, 3.0), Align2::CENTER_CENTER, text, FontId::proportional(size), Color32::from_black_alpha(170));
    p.text(at, Align2::CENTER_CENTER, text, FontId::proportional(size), color);
}
