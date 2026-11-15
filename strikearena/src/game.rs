//! Screens, touch controls and the HUD of Strike Arena.

use crate::gfx::Scene;
use crate::render::{self, Spark, Tracer};
use crate::sim::{Event, GUNS, Gun, Input, Item, Match, SHOP, gun};
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
    /// The buy menu; the match goes on behind it.
    Shop,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Btn {
    Fire,
    /// The second trigger on the left, over the joystick (like Standoff).
    FireLeft,
    Aim,
    Reload,
    Shop,
    Pause,
}

#[derive(Clone, Copy)]
enum Role {
    Joy { cur: Pos2 },
    /// Dragging turns the view. A touch that stays put fires (tap — one shot, hold — burst),
    /// until it has moved enough to count as a drag.
    Look { last: Pos2, moved: f32 },
    Btn { b: Btn, last: Pos2 },
}

pub const MOUSE: u64 = 1 << 40;

/// How far (points) a finger may slide on the screen and still be a shot rather than a turn.
const TAP_SLOP: f32 = 14.0;

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
    pending_switch: Option<usize>,
    shop_msg: (String, f32),
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
            pending_switch: None,
            shop_msg: (String::new(), 0.0),
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
            KeyCode::Tab => self.next_gun(),
            KeyCode::KeyB => self.toggle_shop(),
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
            Screen::Shop => Screen::Playing,
        };
    }

    fn toggle_shop(&mut self) {
        self.screen = match self.screen {
            Screen::Playing => Screen::Shop,
            Screen::Shop => Screen::Playing,
            s => s,
        };
        self.touches.clear();
    }

    fn next_gun(&mut self) {
        if let Some(m) = &self.m {
            let s = &m.men[0];
            let next = (1..=4).map(|d| (s.gun + d) % 4).find(|&k| s.owned[k]);
            self.pending_switch = next;
        }
    }

    /// The weapon hotbar at the bottom (WolfCraft style): one tile per gun.
    fn hotbar(&self) -> Vec<(usize, Rect)> {
        let t = 54.0;
        let gap = 6.0;
        let total = 4.0 * t + 3.0 * gap;
        let x0 = self.view.x / 2.0 - total / 2.0;
        let y = self.view.y - t - 10.0;
        (0..4).map(|k| (k, Rect::from_min_size(Pos2::new(x0 + k as f32 * (t + gap), y), EV::splat(t)))).collect()
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
            (Btn::FireLeft, Pos2::new(1.3 * u, h - 4.9 * u), 0.62 * u, "Огонь"),
            (Btn::Aim, Pos2::new(w - 3.7 * u, h - 1.2 * u), 0.62 * u, "Прицел"),
            (Btn::Reload, Pos2::new(w - 1.1 * u, h - 4.0 * u), 0.55 * u, "Перезар."),
            (Btn::Shop, Pos2::new(w - 3.3 * u, h - 3.3 * u), 0.55 * u, "Магазин"),
            (Btn::Pause, Pos2::new(26.0, 26.0), 0.36 * u, "II"),
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
                if let Some((k, _)) = self.hotbar().into_iter().find(|(_, r)| r.expand(3.0).contains(p)) {
                    self.pending_switch = Some(k);
                    return;
                }
                let hit = self.buttons().into_iter().filter(|b| b.1.distance(p) < b.2 * 1.15).min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p))).map(|b| b.0);
                if let Some(b) = hit {
                    match b {
                        Btn::Pause => {
                            self.screen = Screen::Paused;
                            return;
                        }
                        Btn::Aim => self.aim_toggle = !self.aim_toggle,
                        Btn::Reload => {
                            self.taps.insert(b);
                        }
                        Btn::Shop => {
                            self.toggle_shop();
                            return;
                        }
                        Btn::Fire | Btn::FireLeft => {}
                    }
                    self.touches.insert(id, Role::Btn { b, last: p });
                } else if p.distance(self.joy_home()) < self.unit() * 2.4 && !self.touches.values().any(|r| matches!(r, Role::Joy { .. })) {
                    self.touches.insert(id, Role::Joy { cur: p });
                } else if p.x > self.view.x * 0.35 {
                    self.touches.insert(id, Role::Look { last: p, moved: 0.0 });
                }
            }
            TouchPhase::Moved => {
                let mut d = None;
                match self.touches.get_mut(&id) {
                    Some(Role::Joy { cur }) => *cur = p,
                    // Firing and looking with the same thumb.
                    Some(Role::Look { last, moved }) => {
                        d = Some(p - *last);
                        *moved += (p - *last).length();
                        *last = p;
                    }
                    Some(Role::Btn { b: Btn::Fire | Btn::FireLeft, last }) => {
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
        let fire = self.touches.values().any(|r| match r {
            Role::Btn { b: Btn::Fire | Btn::FireLeft, .. } => true,
            Role::Look { moved, .. } => *moved < TAP_SLOP,
            _ => false,
        }) || k(KeyCode::Space);
        let taps = std::mem::take(&mut self.taps);
        let me = self.m.as_ref().map(|m| &m.men[0]);
        let zoom = me.map(|s| if s.aiming { s.spec().zoom } else { 1.0 }).unwrap_or(1.0);
        let k_look = 0.0055 * self.sens / zoom;
        let _ = &me;
        let mut switch = self.pending_switch.take();
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
        self.shop_msg.1 -= dt;
        if !matches!(self.screen, Screen::Playing | Screen::Shop) {
            return;
        }
        let input = if self.screen == Screen::Playing { self.input() } else { Input::default() };
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
        let playing = self.m.is_some() && matches!(self.screen, Screen::Playing | Screen::Paused | Screen::Over | Screen::Shop);
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
            let spect = (!me.alive() && me.dead_t > 1.2).then_some(me.last_by).filter(|&k| k != 0 && m.men[k].alive());
            if let Some(k) = spect {
                // Watching the killer from behind his shoulder until the respawn.
                let o = &m.men[k];
                let f = o.forward();
                let flat = Vec3::new(f.x, 0.0, f.z).normalize_or_zero();
                let right = flat.cross(Vec3::Y);
                let want = o.eye() - flat * 2.6 + Vec3::Y * 0.7 + right * 0.6;
                // Do not look through walls: pull in to the first hit.
                let d = want - o.eye();
                let len = d.length();
                let t = m.map.ray(o.eye(), d / len.max(1e-3), len).map(|t| (t - 0.3).max(0.3)).unwrap_or(len);
                s.eye = o.eye() + d / len.max(1e-3) * t;
                s.target = o.eye() + f * 6.0;
                s.fov_y = 72f32.to_radians();
                render::everyone(s, m, 0, self.time);
            } else {
                let cam = render::camera(me, self.shake, self.time);
                s.eye = cam.eye;
                s.target = cam.eye + cam.fwd;
                s.fov_y = fov;
                render::everyone(s, m, 0, self.time);
                render::viewmodel(s, me, &cam, self.time);
            }
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
            Screen::Shop => {
                self.hud(ctx);
                self.shop(ctx);
            }
        }
    }

    fn shop(&mut self, ctx: &egui::Context) {
        dim(ctx, 0.45);
        let Some(m) = &self.m else { return };
        let me = &m.men[0];
        let (money, owned, gun_now) = (me.money, me.owned, me.gun);
        let mut buy = None;
        let mut close = false;
        egui::CentralPanel::default().frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(24.0, 14.0))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("МАГАЗИН").size(26.0).strong().color(Color32::WHITE));
                ui.add_space(16.0);
                ui.label(RichText::new(format!("$ {money}")).size(24.0).strong().color(Color32::from_rgb(120, 220, 110)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new("Закрыть").size(18.0)).min_size(EV::new(120.0, 40.0))).clicked() {
                        close = true;
                    }
                    if self.shop_msg.1 > 0.0 {
                        ui.label(RichText::new(&self.shop_msg.0).size(16.0).color(Color32::from_rgb(255, 150, 120)));
                    }
                });
            });
            ui.add_space(8.0);
            let cols = 4;
            let gap = 10.0;
            let card_w = ((ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32).floor();
            let card_h = ((ui.available_height() - gap) / 2.0).clamp(90.0, 150.0);
            for row in SHOP.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (item, name, price) in row {
                        let (rect, resp) = ui.allocate_exact_size(EV::new(card_w, card_h), egui::Sense::click());
                        let have = matches!(item, Item::Gun(k) if owned[*k]);
                        let afford = money >= *price;
                        let p = ui.painter();
                        let hot = resp.hovered() || resp.is_pointer_button_down_on();
                        p.rect_filled(rect, 12.0, Color32::from_black_alpha(if hot { 200 } else { 160 }));
                        let border = if matches!(item, Item::Gun(k) if *k == gun_now) { Color32::from_rgb(90, 150, 255) } else if have { Color32::from_white_alpha(60) } else if afford { Color32::from_white_alpha(130) } else { Color32::from_rgb(140, 60, 50) };
                        p.rect_stroke(rect, 12.0, Stroke::new(2.0_f32, border));
                        let icon = Rect::from_center_size(rect.center() - EV::new(0.0, card_h * 0.12), EV::new(card_w * 0.7, card_h * 0.42));
                        item_icon(p, icon, *item, if have || afford { Color32::from_white_alpha(230) } else { Color32::from_white_alpha(90) });
                        p.text(rect.left_bottom() + EV::new(10.0, -10.0), Align2::LEFT_BOTTOM, *name, FontId::proportional(15.0), Color32::WHITE);
                        let tag = if have { "Есть".to_string() } else { format!("${price}") };
                        p.text(rect.right_bottom() + EV::new(-10.0, -10.0), Align2::RIGHT_BOTTOM, tag, FontId::proportional(15.0), if have { Color32::from_white_alpha(140) } else if afford { Color32::from_rgb(120, 220, 110) } else { Color32::from_rgb(230, 90, 80) });
                        if resp.clicked() {
                            if have {
                                if let Item::Gun(k) = item {
                                    self.pending_switch = Some(*k);
                                }
                            } else {
                                buy = Some(*item);
                            }
                        }
                    }
                });
                ui.add_space(gap);
            }
        });
        if let Some(item) = buy {
            if let Some(m) = self.m.as_mut() {
                match m.buy(0, item) {
                    Ok(()) => self.shop_msg = (String::new(), 0.0),
                    Err(e) => self.shop_msg = (e.to_string(), 2.0),
                }
            }
        }
        if let Some(k) = self.pending_switch.take() {
            if let Some(m) = self.m.as_mut() {
                if m.men[0].owned[k] {
                    m.men[0].gun = k;
                }
            }
        }
        if close {
            self.screen = Screen::Playing;
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
        // CS-style top bar: score boxes around the timer, a figure for every soldier (dim = dead).
        let top = Pos2::new(w / 2.0, 30.0);
        let blue = Color32::from_rgb(90, 150, 255);
        let red = Color32::from_rgb(240, 95, 80);
        p.rect_filled(Rect::from_center_size(top, EV::new(76.0, 34.0)), 6.0, Color32::from_black_alpha(170));
        let secs = m.time.ceil() as u32;
        let tcol = if m.time < 30.0 { red } else { Color32::WHITE };
        p.text(top, Align2::CENTER_CENTER, format!("{}:{:02}", secs / 60, secs % 60), FontId::proportional(18.0), tcol);
        for team in 0..2u8 {
            let sgn = if team == 0 { -1.0 } else { 1.0 };
            let col = if team == 0 { blue } else { red };
            let sc = Rect::from_center_size(top + EV::new(sgn * 66.0, 0.0), EV::new(52.0, 34.0));
            p.rect_filled(sc, 6.0, Color32::from_rgba_unmultiplied(col.r() / 2, col.g() / 2, col.b() / 2, 200));
            p.text(sc.center(), Align2::CENTER_CENTER, m.score[team as usize].to_string(), FontId::proportional(20.0), Color32::WHITE);
            for (i, o) in m.men.iter().filter(|o| o.team == team).enumerate() {
                let cx = top.x + sgn * (106.0 + i as f32 * 22.0);
                let cell = Rect::from_center_size(Pos2::new(cx, top.y), EV::new(20.0, 34.0));
                p.rect_filled(cell, 4.0, Color32::from_black_alpha(140));
                let c = if o.alive() { col } else { Color32::from_white_alpha(45) };
                p.circle_filled(Pos2::new(cx, top.y - 7.0), 4.5, c);
                p.rect_filled(Rect::from_center_size(Pos2::new(cx, top.y + 6.0), EV::new(10.0, 13.0)), 3.0, c);
                if o.bot.is_none() {
                    p.rect_stroke(cell, 4.0, Stroke::new(1.5_f32, Color32::from_rgb(255, 215, 90)));
                }
            }
        }
        // Money, health, armour (top left, next to pause).
        p.text(Pos2::new(54.0, 12.0), Align2::LEFT_TOP, format!("$ {}", me.money), FontId::proportional(18.0), Color32::from_rgb(120, 220, 110));
        let hk = (me.hp / 100.0).clamp(0.0, 1.0);
        let hc = if hk > 0.5 { Color32::WHITE } else if hk > 0.25 { Color32::from_rgb(240, 190, 60) } else { Color32::from_rgb(235, 70, 60) };
        let hp_at = Pos2::new(54.0, 40.0);
        p.rect_filled(Rect::from_center_size(hp_at + EV::new(6.0, 0.0), EV::new(12.0, 4.0)), 1.0, hc);
        p.rect_filled(Rect::from_center_size(hp_at + EV::new(6.0, 0.0), EV::new(4.0, 12.0)), 1.0, hc);
        p.text(hp_at + EV::new(16.0, 0.0), Align2::LEFT_CENTER, format!("{:.0}", me.hp.max(0.0)), FontId::proportional(18.0), hc);
        let ar_at = hp_at + EV::new(66.0, 0.0);
        let shield = vec![ar_at + EV::new(0.0, -6.0), ar_at + EV::new(10.0, -6.0), ar_at + EV::new(10.0, 1.0), ar_at + EV::new(5.0, 7.0), ar_at + EV::new(0.0, 1.0)];
        p.add(egui::Shape::convex_polygon(shield, if me.armor > 0.0 { Color32::from_rgb(150, 190, 230) } else { Color32::from_white_alpha(50) }, Stroke::NONE));
        p.text(ar_at + EV::new(15.0, 0.0), Align2::LEFT_CENTER, format!("{:.0}", me.armor), FontId::proportional(18.0), Color32::from_rgb(150, 190, 230));
        // Kill feed, under the top bar on the right.
        for (i, (text, col, life)) in self.feed.iter().enumerate() {
            let a = (life.min(1.0) * 255.0) as u8;
            let pos = Pos2::new(w - 14.0, 60.0 + i as f32 * 20.0);
            p.text(pos + EV::new(1.0, 1.0), Align2::RIGHT_TOP, text, FontId::proportional(13.0), Color32::from_black_alpha(a / 2 + 40));
            p.text(pos, Align2::RIGHT_TOP, text, FontId::proportional(13.0), Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), a));
        }
        // Weapon hotbar (WolfCraft style).
        if me.alive() {
            for (k, r) in self.hotbar() {
                let sel = k == me.gun;
                let has = me.owned[k];
                p.rect_filled(r, 10.0, Color32::from_black_alpha(if sel { 170 } else { 120 }));
                p.rect_stroke(r, 10.0, if sel { Stroke::new(2.5_f32, Color32::from_rgb(90, 150, 255)) } else { Stroke::new(1.0_f32, Color32::from_white_alpha(90)) });
                p.text(r.left_top() + EV::new(5.0, 3.0), Align2::LEFT_TOP, (k + 1).to_string(), FontId::proportional(11.0), Color32::from_white_alpha(170));
                item_icon(&p, r.shrink2(EV::new(7.0, 14.0)), Item::Gun(k), if has { Color32::from_white_alpha(235) } else { Color32::from_white_alpha(45) });
                if has {
                    p.text(r.right_bottom() + EV::new(-5.0, -3.0), Align2::RIGHT_BOTTOM, me.mag[k].to_string(), FontId::proportional(12.0), Color32::WHITE);
                }
            }
        }
        if !me.alive() {
            // A compact panel at the bottom: who killed you, whom you watch, the respawn bar.
            let killer = (me.last_by != 0).then(|| &m.men[me.last_by]);
            let panel = Rect::from_center_size(Pos2::new(w / 2.0, h - 70.0), EV::new(340.0, 92.0));
            p.rect_filled(panel, 14.0, Color32::from_black_alpha(170));
            p.rect_stroke(panel, 14.0, Stroke::new(1.5_f32, Color32::from_rgb(235, 70, 60)));
            p.text(panel.center_top() + EV::new(0.0, 18.0), Align2::CENTER_CENTER, "ТЫ УБИТ", FontId::proportional(24.0), Color32::from_rgb(240, 80, 70));
            if let Some(k) = killer {
                let watching = me.dead_t > 1.2 && k.alive();
                let line = format!("{}{} · {}", if watching { "Наблюдение: " } else { "Убил: " }, k.name, gun(GUNS[k.gun]).name);
                p.text(panel.center_top() + EV::new(0.0, 44.0), Align2::CENTER_CENTER, line, FontId::proportional(15.0), Color32::WHITE);
            }
            let bar = Rect::from_min_size(Pos2::new(panel.left() + 20.0, panel.bottom() - 22.0), EV::new(panel.width() - 40.0, 8.0));
            let k = (1.0 - me.respawn / 3.0).clamp(0.0, 1.0);
            p.rect_filled(bar, 4.0, Color32::from_white_alpha(40));
            p.rect_filled(Rect::from_min_size(bar.min, EV::new(bar.width() * k, bar.height())), 4.0, Color32::from_rgb(240, 200, 80));
            p.text(bar.right_top() - EV::new(0.0, 4.0), Align2::RIGHT_BOTTOM, format!("Возрождение через {:.0}", me.respawn.max(0.0).ceil()), FontId::proportional(12.0), Color32::from_white_alpha(210));
            return;
        }
        // Before the start: the teams line up, a big countdown.
        if m.warmup > 0.0 {
            let n = m.warmup.ceil() as u32;
            let pop = 1.0 + (1.0 - m.warmup.fract()).powi(3) * 0.25;
            p.rect_filled(Rect::from_center_size(Pos2::new(w / 2.0, h * 0.34), EV::new(w.min(520.0), 92.0)), 16.0, Color32::from_black_alpha(150));
            p.text(Pos2::new(w / 2.0, h * 0.34 - 22.0), Align2::CENTER_CENTER, "МАТЧ НАЧНЁТСЯ ЧЕРЕЗ", FontId::proportional(18.0), Color32::from_white_alpha(220));
            p.text(Pos2::new(w / 2.0, h * 0.34 + 16.0), Align2::CENTER_CENTER, n.to_string(), FontId::proportional(44.0 * pop), Color32::from_rgb(255, 205, 80));
            for team in 0..2u8 {
                let x = if team == 0 { 16.0 } else { w - 16.0 };
                let al = if team == 0 { Align2::LEFT_TOP } else { Align2::RIGHT_TOP };
                let col = if team == 0 { Color32::from_rgb(120, 170, 255) } else { Color32::from_rgb(255, 120, 110) };
                let top = h * 0.2;
                let card = if team == 0 { Rect::from_min_size(Pos2::new(8.0, top - 8.0), EV::new(170.0, 140.0)) } else { Rect::from_min_size(Pos2::new(w - 178.0, top - 8.0), EV::new(170.0, 140.0)) };
                p.rect_filled(card, 12.0, Color32::from_black_alpha(130));
                p.text(Pos2::new(x, top), al, if team == 0 { "СИНИЕ" } else { "КРАСНЫЕ" }, FontId::proportional(16.0), col);
                for (i, o) in m.men.iter().filter(|o| o.team == team).enumerate() {
                    let c = if o.bot.is_none() { Color32::from_rgb(255, 215, 90) } else { Color32::WHITE };
                    p.text(Pos2::new(x, top + 22.0 + i as f32 * 20.0), al, format!("{} · {}", o.name, gun(GUNS[o.gun]).name), FontId::proportional(13.0), c);
                }
            }
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
                (Btn::Fire | Btn::FireLeft, true) => Color32::from_rgba_unmultiplied(230, 80, 50, 210),
                (Btn::Fire | Btn::FireLeft, false) => Color32::from_rgba_unmultiplied(160, 40, 30, 150),
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

/// Simple flat pictures of the shop items and the guns.
fn item_icon(p: &egui::Painter, r: Rect, item: Item, c: Color32) {
    let at = |x: f32, y: f32| Pos2::new(r.left() + r.width() * x, r.top() + r.height() * y);
    let bar = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
    match item {
        Item::Gun(0) => {
            p.rect_filled(bar(0.22, 0.35, 0.78, 0.55), 2.0, c);
            p.rect_filled(bar(0.78, 0.42, 1.0, 0.5), 1.0, c);
            p.rect_filled(bar(0.0, 0.38, 0.22, 0.62), 2.0, c);
            p.rect_filled(bar(0.48, 0.55, 0.58, 0.9), 1.0, c);
            p.rect_filled(bar(0.32, 0.55, 0.4, 0.8), 1.0, c);
        }
        Item::Gun(1) => {
            p.rect_filled(bar(0.25, 0.36, 1.0, 0.5), 2.0, c);
            p.rect_filled(bar(0.45, 0.5, 0.75, 0.6), 2.0, c);
            p.rect_filled(bar(0.0, 0.4, 0.25, 0.66), 3.0, c);
        }
        Item::Gun(2) => {
            p.rect_filled(bar(0.18, 0.42, 0.7, 0.56), 2.0, c);
            p.rect_filled(bar(0.7, 0.46, 1.0, 0.52), 1.0, c);
            p.rect_filled(bar(0.3, 0.26, 0.6, 0.38), 3.0, c);
            p.rect_filled(bar(0.0, 0.44, 0.18, 0.66), 2.0, c);
            p.rect_filled(bar(0.38, 0.56, 0.45, 0.78), 1.0, c);
        }
        Item::Gun(_) => {
            p.rect_filled(bar(0.25, 0.3, 0.8, 0.46), 2.0, c);
            p.rect_filled(bar(0.28, 0.46, 0.45, 0.85), 2.0, c);
        }
        Item::Armor => {
            let pts = vec![at(0.3, 0.1), at(0.7, 0.1), at(0.82, 0.3), at(0.75, 0.9), at(0.25, 0.9), at(0.18, 0.3)];
            p.add(egui::Shape::convex_polygon(pts, c, Stroke::NONE));
        }
        Item::Medkit => {
            p.rect_filled(bar(0.2, 0.15, 0.8, 0.85), 6.0, c);
            let red = Color32::from_rgb(220, 60, 50);
            p.rect_filled(bar(0.44, 0.27, 0.56, 0.73), 1.0, red);
            p.rect_filled(bar(0.32, 0.44, 0.68, 0.56), 1.0, red);
        }
        Item::Ammo => {
            for k in 0..4 {
                let x = 0.18 + k as f32 * 0.17;
                p.rect_filled(bar(x, 0.35, x + 0.11, 0.85), 2.0, c);
                p.circle_filled(at(x + 0.055, 0.33), r.width() * 0.055, c);
            }
        }
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
    p.text(at + EV::new(1.0, 1.5), Align2::CENTER_CENTER, text, FontId::proportional(size), Color32::from_black_alpha(120));
    p.text(at, Align2::CENTER_CENTER, text, FontId::proportional(size), color);
}
