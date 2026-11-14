//! Touch controls: left thumb = virtual joystick, right thumb = look (tap = place, hold = break),
//! round buttons for jump, sneak, inventory, pause and hotbar scrolling.

use std::time::Instant;
use winit::event::{Touch, TouchPhase};

/// Something the touch layer wants the game to do right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchAction {
    /// Creative flight on/off.
    Fly,
    Inventory,
    Pause,
    Debug,
    Camera,
    Chat,
    HotbarSet(usize),
    Place,
    Hotbar(i32),
    /// Tap on the minimap: open the world map.
    Map,
    /// Short tap on the microphone: the voice chat menu.
    Voice,
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Joystick,
    Look,
    Hotbar,
    /// A finger on a hotbar slot: held long it throws the item out.
    Slot,
    Button(Btn),
}

#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Jump,
    Sneak,
    Sprint,
    Place,
    Inventory,
    Pause,
    Chat,
    Camera,
    Tab,
    /// Voice chat: tap for the menu, hold to talk.
    Voice,
    Prev,
    Next,
    /// Creative: flight on/off (separate from jump).
    Fly,
}

struct Finger {
    id: u64,
    role: Role,
    start: (f32, f32),
    last: (f32, f32),
    t0: Instant,
    moved: f32,
    breaking: bool,
    /// Items thrown by this finger after the first (hotbar long press).
    throws: u32,
}

#[derive(Default)]
pub struct TouchState {
    fingers: Vec<Finger>,
    /// Set once any touch arrives: the on-screen controls are shown from then on.
    pub seen: bool,
    /// Joystick deflection, x = strafe right, y = forward (both -1..1).
    pub move_vec: (f32, f32),
    pub sneak: bool,
    /// Sprint toggle (on-screen button).
    pub sprint: bool,
    /// The "Поставить" button is held (draws a bow, raises a shield).
    pub place_held: bool,
    pub jump_held: bool,
    /// "Вниз" held while flying.
    pub down_held: bool,
    /// Set by the game every frame: creative mode (shows the flight button) and flying
    /// (jump and sneak become "up" and "down").
    pub creative: bool,
    pub flying: bool,
    /// Accumulated look drag in pixels since the last frame.
    pub look: (f32, f32),
    pub breaking: bool,
    /// Hotbar rectangle in physical pixels (set while drawing), for the buttons beside it.
    hotbar: Option<(f32, f32, f32, f32)>,
    /// Inventory tile and minimap (physical pixels), published by the HUD.
    inv: Option<(f32, f32, f32, f32)>,
    map: Option<(f32, f32, f32, f32)>,
    /// Studio mode: the top island is replaced by the studio ribbon.
    pub studio: bool,
    /// Screen areas covered by other panels (physical pixels): touches there are left to them.
    pub blocked: Vec<(f32, f32, f32, f32)>,
    /// Items to throw out of the selected hotbar slot (long press), taken by the game.
    pub throws: u32,
    /// Where the last short tap landed (physical pixels), for the frame that handles it.
    pub tap_point: Option<(f32, f32)>,
    /// The microphone button is held (push-to-talk).
    pub voice_held: bool,
    /// Microphone icon: 0 off (crossed out), 1 on, 2 sending now. Set by the game.
    pub voice_state: u8,
    /// A studio place in «Играть» (Roblox style): its GUI is the place's own (scripts); of
    /// the game's controls only the stick, «Прыжок» and the top bar (chat, voice, menu) stay.
    pub place_play: bool,
    /// Width covered on the left (the studio's tools sidebar), physical pixels.
    pub left_inset: f32,
}

/// Stick deflection from the fixed base to the thumb: a small dead zone, then smooth to full.
fn stick_vec(home: (f32, f32), p: (f32, f32), size: (u32, u32)) -> (f32, f32) {
    let rr = stick_r(size);
    let (dx, dy) = (p.0 - home.0, p.1 - home.1);
    let n = (dx.hypot(dy) / rr).min(1.0);
    let m = ((n - STICK_DEAD) / (1.0 - STICK_DEAD)).max(0.0);
    let len = dx.hypot(dy).max(1e-3);
    (dx / len * m, -dy / len * m)
}

/// Joystick throw radius (physical pixels): grows with the screen like the buttons.
fn stick_r(size: (u32, u32)) -> f32 {
    95.0 * unit(size)
}

/// Deflection under this fraction counts as none (a resting thumb does not drift).
const STICK_DEAD: f32 = 0.12;

/// Pixel size of the controls scales with the screen height.
fn unit(size: (u32, u32)) -> f32 {
    // Narrow views (the upright studio) shrink the controls so both thumbs' clusters fit.
    (size.1 as f32 / 720.0).min(size.0 as f32 / 1100.0).clamp(0.55, 2.2)
}

/// (centre x, centre y, radius) of a button, in physical pixels.
fn button_rect(b: Btn, size: (u32, u32), hotbar: Option<(f32, f32, f32, f32)>) -> (f32, f32, f32) {
    let (w, h) = (size.0 as f32, size.1 as f32);
    let u = unit(size);
    if let Some((l, t, r, bt)) = hotbar {
        // Arrows and the inventory button sit right next to the hotbar, centred on it.
        let rad = (bt - t) * 0.5;
        let cy = (t + bt) * 0.5;
        match b {
            Btn::Prev => return (l - rad * 1.2, cy, rad),
            Btn::Next => return (r + rad * 1.2, cy, rad),
            Btn::Inventory => return (r + rad * 3.6, cy, rad),
            _ => {}
        }
    }
    match b {
        // Right-hand cluster in two tidy columns: the outer one holds the movement pair
        // (jump/up over sneak/down), the inner one place, sprint and flight.
        Btn::Jump => (w - 120.0 * u, h - 255.0 * u, 56.0 * u),
        Btn::Sneak => (w - 120.0 * u, h - 110.0 * u, 50.0 * u),
        Btn::Place => (w - 255.0 * u, h - 320.0 * u, 42.0 * u),
        Btn::Sprint => (w - 255.0 * u, h - 200.0 * u, 40.0 * u),
        Btn::Fly => (w - 255.0 * u, h - 85.0 * u, 36.0 * u),
        Btn::Inventory => (w - 70.0 * u, h - 50.0 * u, 34.0 * u),
        Btn::Chat | Btn::Voice | Btn::Camera | Btn::Tab | Btn::Pause => {
            let i = ISLAND.iter().position(|x| *x == b).unwrap_or(0) as f32;
            let (x0, y, step) = island_layout(size);
            (x0 + step * (i + 0.5), y, 30.0 * u)
        }
        Btn::Prev => (w * 0.5 - 130.0 * u, h - 50.0 * u, 34.0 * u),
        Btn::Next => (w * 0.5 + 130.0 * u, h - 50.0 * u, 34.0 * u),
    }
}

/// Buttons of the top "island", left to right.
/// Chat, voice and the menu first: a place's «Играть» keeps only those three.
const ISLAND: [Btn; 5] = [Btn::Chat, Btn::Voice, Btn::Pause, Btn::Camera, Btn::Tab];

/// Island: (left x, centre y, slot width) in physical pixels, centred at the top.
fn island_layout(size: (u32, u32)) -> (f32, f32, f32) {
    let u = unit(size);
    let step = 78.0 * u;
    let w = step * ISLAND.len() as f32;
    let _ = w;
    // Top-left corner, clear of the screen's rounded corner.
    (40.0 * u, 48.0 * u, step)
}

/// Round on-screen buttons (the inventory is a tile next to the hotbar, drawn by the HUD).
const BUTTONS: [Btn; 10] = [Btn::Jump, Btn::Sneak, Btn::Sprint, Btn::Place, Btn::Fly, Btn::Pause, Btn::Chat, Btn::Voice, Btn::Camera, Btn::Tab];

fn hit(b: Btn, size: (u32, u32), hotbar: Option<(f32, f32, f32, f32)>, p: (f32, f32)) -> bool {
    let (cx, cy, r) = button_rect(b, size, hotbar);
    // A slightly generous hit area is easier on thumbs.
    (p.0 - cx).hypot(p.1 - cy) <= r * 1.25
}

impl TouchState {
    /// Top-bar buttons shown now.
    fn island(&self) -> &'static [Btn] {
        if self.place_play { &ISLAND[..3] } else { &ISLAND }
    }

    /// Whether a round button is shown (and can be pressed) now.
    fn shown(&self, b: Btn) -> bool {
        if ISLAND.contains(&b) {
            return !self.studio && self.island().contains(&b);
        }
        if self.place_play {
            return b == Btn::Jump;
        }
        b != Btn::Fly || self.creative
    }

    /// Handles a touch event while playing. Returns actions to apply.
    pub fn event(&mut self, t: &Touch, size: (u32, u32)) -> Vec<TouchAction> {
        self.seen = true;
        let p = (t.location.x as f32, t.location.y as f32);
        let mut out = Vec::new();
        match t.phase {
            TouchPhase::Started => {
                let (w, _h) = (size.0 as f32, size.1 as f32);
                let on_hotbar = self.hotbar.is_some_and(|(l, t, r, bt)| p.0 >= l - 6.0 && p.0 <= r + 6.0 && p.1 >= t - 6.0 && p.1 <= bt + 6.0);
                let inside = |r: Option<(f32, f32, f32, f32)>| r.is_some_and(|(l, t, rr, b)| p.0 >= l - 4.0 && p.0 <= rr + 4.0 && p.1 >= t - 4.0 && p.1 <= b + 4.0);
                let role = if self.blocked.iter().any(|r| inside(Some(*r))) {
                    // A studio panel is there: not a game touch (an inert role so it does nothing).
                    Role::Hotbar
                } else if inside(self.inv) {
                    out.push(TouchAction::Inventory);
                    Role::Button(Btn::Inventory)
                } else if inside(self.map) {
                    out.push(TouchAction::Map);
                    Role::Button(Btn::Tab)
                } else if on_hotbar {
                    // Tapping the hotbar picks that slot (it must not start the stick or the look).
                    let (l, _, r, _) = self.hotbar.unwrap_or((0.0, 0.0, 1.0, 1.0));
                    let slot = (((p.0 - l) / (r - l).max(1.0)) * 9.0).floor().clamp(0.0, 8.0) as usize;
                    out.push(TouchAction::HotbarSet(slot));
                    Role::Slot
                } else if let Some(b) = BUTTONS.iter().copied().filter(|b| self.shown(*b)).find(|b| hit(*b, size, self.hotbar, p)) {
                    match b {
                        Btn::Jump => {
                            // Held: jump on the ground, rise while flying. Flight itself has
                            // its own button, so no double-tap toggling here.
                            self.jump_held = true;
                        }
                        Btn::Fly => out.push(TouchAction::Fly),
                        Btn::Sneak if self.flying => self.down_held = true,
                        Btn::Sneak => {
                            self.sneak = !self.sneak;
                            if self.sneak {
                                self.sprint = false;
                            }
                        }
                        Btn::Sprint => {
                            self.sprint = !self.sprint;
                            if self.sprint {
                                self.sneak = false;
                            }
                        }
                        Btn::Place => {
                            self.place_held = true;
                            out.push(TouchAction::Place);
                        }
                        Btn::Inventory => out.push(TouchAction::Inventory),
                        Btn::Pause => out.push(TouchAction::Pause),
                        Btn::Chat => out.push(TouchAction::Chat),
                        Btn::Voice => self.voice_held = true,
                        Btn::Camera => out.push(TouchAction::Camera),
                        Btn::Tab => out.push(TouchAction::Debug),
                        Btn::Prev => out.push(TouchAction::Hotbar(1)),
                        Btn::Next => out.push(TouchAction::Hotbar(-1)),
                    }
                    Role::Button(b)
                } else if { let hm = self.stick_home(size); (p.0 - hm.0).hypot(p.1 - hm.1) < stick_r(size) * 2.2 } && p.0 < self.left_inset + w * 0.4 && !self.fingers.iter().any(|f| f.role == Role::Joystick) {
                    Role::Joystick
                } else {
                    Role::Look
                };
                // The joystick is fixed in its corner: its base never jumps to where the thumb
                // lands, the knob just goes to the thumb.
                let start = if role == Role::Joystick { self.stick_home(size) } else { p };
                if role == Role::Joystick {
                    self.stick_to(start, p, size);
                }
                self.fingers.push(Finger { id: t.id, role, start, last: p, t0: Instant::now(), moved: 0.0, breaking: false, throws: 0 });
            }
            TouchPhase::Moved => {
                if let Some(f) = self.fingers.iter_mut().find(|f| f.id == t.id) {
                    match f.role {
                        Role::Joystick => {
                            let home = f.start;
                            self.move_vec = stick_vec(home, p, size);
                        }
                        Role::Look => {
                            self.look.0 += p.0 - f.last.0;
                            self.look.1 += p.1 - f.last.1;
                            f.moved += (p.0 - f.last.0).abs() + (p.1 - f.last.1).abs();
                        }
                        Role::Slot => f.moved += (p.0 - f.last.0).abs() + (p.1 - f.last.1).abs(),
                        Role::Button(_) | Role::Hotbar => {}
                    }
                    f.last = p;
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if let Some(i) = self.fingers.iter().position(|f| f.id == t.id) {
                    let f = self.fingers.remove(i);
                    match f.role {
                        Role::Joystick => self.move_vec = (0.0, 0.0),
                        Role::Look => {
                            if f.breaking {
                                self.breaking = false;
                            } else if f.moved < 24.0 && f.t0.elapsed().as_millis() < 300 && t.phase == TouchPhase::Ended {
                                self.tap_point = Some(f.last);
                                out.push(TouchAction::Place);
                            }
                        }
                        Role::Button(Btn::Jump) => self.jump_held = false,
                        Role::Button(Btn::Sneak) => self.down_held = false,
                        Role::Button(Btn::Place) => self.place_held = false,
                        Role::Button(Btn::Voice) => {
                            self.voice_held = false;
                            // A short tap opens the menu; holding was talking.
                            if f.t0.elapsed().as_millis() < 350 && t.phase == TouchPhase::Ended {
                                out.push(TouchAction::Voice);
                            }
                        }
                        Role::Button(_) | Role::Hotbar | Role::Slot => {}
                    }
                }
            }
        }
        out
    }

    /// Call every frame: a look finger held still for a moment starts breaking.
    pub fn tick(&mut self) {
        for f in self.fingers.iter_mut() {
            // Long press on a hotbar slot: throw one item, then one more every 0.2 s.
            if f.role == Role::Slot && f.moved < 30.0 {
                let ms = f.t0.elapsed().as_millis() as f32;
                let due = if ms < 600.0 { 0 } else { 1 + ((ms - 600.0) / 200.0) as u32 };
                let done = f.breaking as u32 + f.throws;
                if due > done {
                    f.breaking = true;
                    f.throws = due - 1;
                    self.throws += 1;
                }
            }
            if f.role == Role::Look && !f.breaking && f.moved < 24.0 && f.t0.elapsed().as_millis() > 350 {
                f.breaking = true;
                self.breaking = true;
            }
        }
    }

    /// No HUD on screen (the studio's edit mode): no hotbar, inventory tile or minimap to tap.
    pub fn clear_hud(&mut self) {
        self.hotbar = None;
        self.inv = None;
        self.map = None;
    }

    /// Reads the hotbar rectangle the HUD published (points -> physical pixels).
    pub fn sync_hotbar(&mut self, ppp: f32) {
        let read = |l: &std::sync::RwLock<[f32; 4]>| l.read().ok().and_then(|h| (h[2] > h[0]).then(|| (h[0] * ppp, h[1] * ppp, h[2] * ppp, h[3] * ppp)));
        self.hotbar = read(&wolfcraft_ui::hud::HOTBAR_RECT);
        self.inv = read(&wolfcraft_ui::hud::INV_RECT);
        self.map = read(&wolfcraft_ui::hud::MINIMAP_RECT);
    }

    /// Tap targeting (as on Bedrock phones): the point under the finger that is on the world,
    /// or where the last tap landed. `None` when no finger is on the world.
    pub fn aim_point(&self) -> Option<(f32, f32)> {
        self.fingers.iter().rev().find(|f| f.role == Role::Look).map(|f| f.last).or(self.tap_point)
    }

    /// Takes the accumulated look drag.
    pub fn take_look(&mut self) -> (f32, f32) {
        std::mem::take(&mut self.look)
    }

    /// Drops every finger (screen change, focus loss).
    pub fn reset(&mut self) {
        self.fingers.clear();
        self.move_vec = (0.0, 0.0);
        self.jump_held = false;
        self.down_held = false;
        self.place_held = false;
        self.breaking = false;
        self.look = (0.0, 0.0);
    }

    /// Draws the controls with an egui painter; `ppp` = pixels per point.
    /// Where the fixed joystick sits (physical pixels).
    fn stick_home(&self, size: (u32, u32)) -> (f32, f32) {
        let u = unit(size);
        (self.left_inset + 150.0 * u, size.1 as f32 - 150.0 * u)
    }

    fn stick_to(&mut self, home: (f32, f32), p: (f32, f32), size: (u32, u32)) {
        self.move_vec = stick_vec(home, p, size);
    }

    pub fn draw(&self, painter: &egui::Painter, size: (u32, u32), ppp: f32) {
        use egui::{Align2, Color32, FontId, Pos2, Stroke};
        let pt = |x: f32, y: f32| Pos2::new(x / ppp, y / ppp);
        let fill = Color32::from_rgba_unmultiplied(20, 24, 32, 110);
        let line = Stroke::new(2.0, Color32::from_rgba_unmultiplied(220, 230, 240, 150));
        let u = unit(size);
        // Joystick: base where the thumb went down, otherwise a hint in the lower left.
        let rr = stick_r(size);
        let held = self.fingers.iter().find(|f| f.role == Role::Joystick);
        let (bx, by, kx, ky) = match held {
            // The knob sits under the thumb.
            Some(f) => {
                // The knob follows the thumb but stays inside the ring.
                let (dx, dy) = (f.last.0 - f.start.0, f.last.1 - f.start.1);
                let d = dx.hypot(dy).max(1e-3);
                let k = (rr / d).min(1.0);
                (f.start.0, f.start.1, f.start.0 + dx * k, f.start.1 + dy * k)
            }
            None => {
                let (x, y) = self.stick_home(size);
                (x, y, x, y)
            }
        };
        // Faint while waiting, clear while in use.
        let a = if held.is_some() { 1.0 } else { 0.55 };
        let fade = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * a) as u8);
        painter.circle(pt(bx, by), rr / ppp, fade(fill), Stroke::new(2.0, fade(line.color)));
        painter.circle(pt(kx, ky), rr * 0.42 / ppp, fade(Color32::from_rgba_unmultiplied(230, 240, 250, 160)), Stroke::new(2.0, fade(line.color)));
        // Top island: one rounded pill holding chat, camera, debug and pause.
        if !self.studio {
            let (x0, y, step) = island_layout(size);
            let hh = 34.0 * u;
            let rect = egui::Rect::from_min_max(pt(x0 - 8.0 * u, y - hh), pt(x0 + step * self.island().len() as f32 + 8.0 * u, y + hh));
            painter.rect(rect, hh / ppp, Color32::from_rgba_unmultiplied(12, 14, 20, 200), Stroke::new(1.5, Color32::from_rgba_unmultiplied(220, 230, 240, 70)));
            for i in 1..self.island().len() {
                let x = x0 + step * i as f32;
                painter.line_segment([pt(x, y - hh * 0.45), pt(x, y + hh * 0.45)], Stroke::new(1.0, Color32::from_rgba_unmultiplied(220, 230, 240, 50)));
            }
            let white = Color32::from_rgba_unmultiplied(240, 245, 250, 235);
            let st = Stroke::new(2.4, white);
            for &b in self.island() {
                let (cx, cy, r) = button_rect(b, size, self.hotbar);
                let c = pt(cx, cy);
                let k = r * 0.5 / ppp;
                match b {
                    Btn::Pause => {
                        for dx in [-0.45, 0.45] {
                            painter.rect_filled(egui::Rect::from_center_size(c + egui::vec2(dx * k, 0.0), egui::vec2(k * 0.42, k * 1.6)), 1.0, white);
                        }
                    }
                    Btn::Chat => {
                        // Speech bubble.
                        let bubble = egui::Rect::from_center_size(c - egui::vec2(0.0, k * 0.15), egui::vec2(k * 2.0, k * 1.4));
                        painter.rect_stroke(bubble, k * 0.35, st);
                        painter.add(egui::Shape::convex_polygon(vec![bubble.left_bottom() + egui::vec2(k * 0.35, 0.0), bubble.left_bottom() + egui::vec2(k * 0.85, 0.0), bubble.left_bottom() + egui::vec2(k * 0.2, k * 0.55)], white, Stroke::NONE));
                    }
                    Btn::Voice => {
                        // Microphone: capsule, cradle and stand; red while sending, crossed
                        // out when off.
                        let col = match self.voice_state {
                            2 => Color32::from_rgb(90, 230, 120),
                            1 => white,
                            _ => Color32::from_rgba_unmultiplied(240, 245, 250, 140),
                        };
                        let cap = egui::Rect::from_center_size(c - egui::vec2(0.0, k * 0.35), egui::vec2(k * 0.8, k * 1.2));
                        if self.voice_state == 2 {
                            painter.rect_filled(cap, k * 0.4, col);
                        } else {
                            painter.rect_stroke(cap, k * 0.4, Stroke::new(2.2, col));
                        }
                        let s2 = Stroke::new(2.2, col);
                        let pts: Vec<egui::Pos2> = (0..=10).map(|i| {
                            let a = std::f32::consts::PI * (i as f32 / 10.0);
                            c + egui::vec2(-a.cos() * k * 0.7, -k * 0.2 + a.sin() * k * 0.55)
                        }).collect();
                        painter.add(egui::Shape::line(pts, s2));
                        painter.line_segment([c + egui::vec2(0.0, k * 0.35), c + egui::vec2(0.0, k * 0.8)], s2);
                        painter.line_segment([c + egui::vec2(-k * 0.4, k * 0.8), c + egui::vec2(k * 0.4, k * 0.8)], s2);
                        if self.voice_state == 0 {
                            painter.line_segment([c + egui::vec2(-k * 0.9, -k * 0.9), c + egui::vec2(k * 0.9, k * 0.9)], Stroke::new(2.6, Color32::from_rgb(240, 90, 90)));
                        }
                    }
                    Btn::Camera => {
                        // Eye: switch between first and third person.
                        painter.circle_stroke(c, k * 0.95, st);
                        painter.circle_filled(c, k * 0.38, white);
                    }
                    _ => {
                        painter.text(c, Align2::CENTER_CENTER, "F3", FontId::proportional(k * 1.25), white);
                    }
                }
            }
        }
        for b in BUTTONS {
            if ISLAND.contains(&b) || !self.shown(b) {
                continue;
            }
            let (cx, cy, r) = button_rect(b, size, self.hotbar);
            let on = match b {
                Btn::Sneak if self.flying => self.down_held,
                Btn::Sneak => self.sneak,
                Btn::Sprint => self.sprint,
                Btn::Jump => self.jump_held,
                Btn::Fly => self.flying,
                _ => false,
            };
            let col = if on { Color32::from_rgba_unmultiplied(38, 198, 218, 170) } else { fill };
            painter.circle(pt(cx, cy), r / ppp, col, line);
            let white = Color32::from_rgba_unmultiplied(240, 245, 250, 235);
            let c = pt(cx, cy);
            let k = r * 0.42 / ppp;
            let st = Stroke::new(2.6, white);
            match b {
                Btn::Jump | Btn::Sneak => {
                    // The pixel font has no arrow glyphs: draw the triangle as a shape.
                    let sgn = if b == Btn::Jump { -1.0 } else { 1.0 };
                    painter.add(egui::Shape::convex_polygon(vec![c + egui::vec2(0.0, sgn * k), c + egui::vec2(-k, -sgn * k * 0.7), c + egui::vec2(k, -sgn * k * 0.7)], white, Stroke::NONE));
                }
                Btn::Sprint => {
                    // Double chevron: run.
                    for dx in [-0.45f32, 0.35] {
                        let o = c + egui::vec2(dx * k, 0.0);
                        painter.line_segment([o + egui::vec2(-k * 0.35, -k * 0.7), o + egui::vec2(k * 0.35, 0.0)], st);
                        painter.line_segment([o + egui::vec2(k * 0.35, 0.0), o + egui::vec2(-k * 0.35, k * 0.7)], st);
                    }
                }
                Btn::Fly => {
                    // Two wings.
                    for sgn in [-1.0f32, 1.0] {
                        painter.add(egui::Shape::convex_polygon(
                            vec![c + egui::vec2(sgn * k * 0.15, k * 0.5), c + egui::vec2(sgn * k * 1.15, -k * 0.75), c + egui::vec2(sgn * k * 0.95, k * 0.05), c + egui::vec2(sgn * k * 0.55, k * 0.45)],
                            white,
                            Stroke::NONE,
                        ));
                    }
                }
                Btn::Place => {
                    // A block with a plus.
                    painter.rect_stroke(egui::Rect::from_center_size(c, egui::vec2(k * 1.6, k * 1.6)), 0.0, st);
                    painter.line_segment([c - egui::vec2(k * 0.5, 0.0), c + egui::vec2(k * 0.5, 0.0)], st);
                    painter.line_segment([c - egui::vec2(0.0, k * 0.5), c + egui::vec2(0.0, k * 0.5)], st);
                }
                _ => {}
            }
            let label = match b {
                Btn::Jump if self.flying => "Вверх",
                Btn::Sneak if self.flying => "Вниз",
                Btn::Jump => "Прыжок",
                Btn::Sneak => "Присесть",
                Btn::Fly => "Полёт",
                Btn::Sprint => "Бег",
                Btn::Place => "Поставить",
                _ => "",
            };
            painter.text(pt(cx, cy + r + 4.0 * u), Align2::CENTER_TOP, label, FontId::proportional(11.0), Color32::from_rgba_unmultiplied(240, 245, 250, 200));
        }
    }
}
