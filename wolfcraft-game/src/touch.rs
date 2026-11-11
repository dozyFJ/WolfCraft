//! Touch controls: left thumb = virtual joystick, right thumb = look (tap = place, hold = break),
//! round buttons for jump, sneak, inventory, pause and hotbar scrolling.

use std::time::Instant;
use winit::event::{Touch, TouchPhase};

/// Something the touch layer wants the game to do right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchAction {
    Jump,
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
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Joystick,
    Look,
    Hotbar,
    Button(Btn),
}

#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Jump,
    Sneak,
    Inventory,
    Pause,
    Chat,
    Camera,
    Tab,
    Prev,
    Next,
}

struct Finger {
    id: u64,
    role: Role,
    start: (f32, f32),
    last: (f32, f32),
    t0: Instant,
    moved: f32,
    breaking: bool,
}

#[derive(Default)]
pub struct TouchState {
    fingers: Vec<Finger>,
    /// Set once any touch arrives: the on-screen controls are shown from then on.
    pub seen: bool,
    /// Joystick deflection, x = strafe right, y = forward (both -1..1).
    pub move_vec: (f32, f32),
    pub sneak: bool,
    pub jump_held: bool,
    /// Accumulated look drag in pixels since the last frame.
    pub look: (f32, f32),
    pub breaking: bool,
    /// Hotbar rectangle in physical pixels (set while drawing), for the buttons beside it.
    hotbar: Option<(f32, f32, f32, f32)>,
    /// Inventory tile and minimap (physical pixels), published by the HUD.
    inv: Option<(f32, f32, f32, f32)>,
    map: Option<(f32, f32, f32, f32)>,
}

pub const STICK_R: f32 = 110.0;

/// Pixel size of the controls scales with the screen height.
fn unit(size: (u32, u32)) -> f32 {
    (size.1 as f32 / 720.0).clamp(0.8, 2.2)
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
        Btn::Jump => (w - 120.0 * u, h - 150.0 * u, 62.0 * u),
        Btn::Sneak => (w - 120.0 * u, h - 290.0 * u, 44.0 * u),
        Btn::Inventory => (w - 70.0 * u, h - 50.0 * u, 34.0 * u),
        Btn::Chat | Btn::Camera | Btn::Tab | Btn::Pause => {
            let i = ISLAND.iter().position(|x| *x == b).unwrap_or(0) as f32;
            let (x0, y, step) = island_layout(size);
            (x0 + step * (i + 0.5), y, 30.0 * u)
        }
        Btn::Prev => (w * 0.5 - 130.0 * u, h - 50.0 * u, 34.0 * u),
        Btn::Next => (w * 0.5 + 130.0 * u, h - 50.0 * u, 34.0 * u),
    }
}

/// Buttons of the top "island", left to right.
const ISLAND: [Btn; 4] = [Btn::Chat, Btn::Camera, Btn::Tab, Btn::Pause];

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
const BUTTONS: [Btn; 6] = [Btn::Jump, Btn::Sneak, Btn::Pause, Btn::Chat, Btn::Camera, Btn::Tab];

fn hit(b: Btn, size: (u32, u32), hotbar: Option<(f32, f32, f32, f32)>, p: (f32, f32)) -> bool {
    let (cx, cy, r) = button_rect(b, size, hotbar);
    // A slightly generous hit area is easier on thumbs.
    (p.0 - cx).hypot(p.1 - cy) <= r * 1.25
}

impl TouchState {
    /// Handles a touch event while playing. Returns actions to apply.
    pub fn event(&mut self, t: &Touch, size: (u32, u32)) -> Vec<TouchAction> {
        self.seen = true;
        let p = (t.location.x as f32, t.location.y as f32);
        let mut out = Vec::new();
        match t.phase {
            TouchPhase::Started => {
                let (w, h) = (size.0 as f32, size.1 as f32);
                let on_hotbar = self.hotbar.is_some_and(|(l, t, r, bt)| p.0 >= l - 6.0 && p.0 <= r + 6.0 && p.1 >= t - 6.0 && p.1 <= bt + 6.0);
                let inside = |r: Option<(f32, f32, f32, f32)>| r.is_some_and(|(l, t, rr, b)| p.0 >= l - 4.0 && p.0 <= rr + 4.0 && p.1 >= t - 4.0 && p.1 <= b + 4.0);
                let role = if inside(self.inv) {
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
                    Role::Hotbar
                } else if let Some(b) = BUTTONS.iter().copied().find(|b| hit(*b, size, self.hotbar, p)) {
                    match b {
                        Btn::Jump => {
                            self.jump_held = true;
                            out.push(TouchAction::Jump);
                        }
                        Btn::Sneak => self.sneak = !self.sneak,
                        Btn::Inventory => out.push(TouchAction::Inventory),
                        Btn::Pause => out.push(TouchAction::Pause),
                        Btn::Chat => out.push(TouchAction::Chat),
                        Btn::Camera => out.push(TouchAction::Camera),
                        Btn::Tab => out.push(TouchAction::Debug),
                        Btn::Prev => out.push(TouchAction::Hotbar(1)),
                        Btn::Next => out.push(TouchAction::Hotbar(-1)),
                    }
                    Role::Button(b)
                } else if p.0 < w * 0.4 && p.1 > h * 0.3 && !self.fingers.iter().any(|f| f.role == Role::Joystick) {
                    Role::Joystick
                } else {
                    Role::Look
                };
                self.fingers.push(Finger { id: t.id, role, start: p, last: p, t0: Instant::now(), moved: 0.0, breaking: false });
            }
            TouchPhase::Moved => {
                if let Some(f) = self.fingers.iter_mut().find(|f| f.id == t.id) {
                    match f.role {
                        Role::Joystick => {
                            let (dx, dy) = ((p.0 - f.start.0) / STICK_R, (p.1 - f.start.1) / STICK_R);
                            let len = dx.hypot(dy).max(1.0);
                            self.move_vec = (dx / len, -dy / len);
                        }
                        Role::Look => {
                            self.look.0 += p.0 - f.last.0;
                            self.look.1 += p.1 - f.last.1;
                            f.moved += (p.0 - f.last.0).abs() + (p.1 - f.last.1).abs();
                        }
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
                                out.push(TouchAction::Place);
                            }
                        }
                        Role::Button(Btn::Jump) => self.jump_held = false,
                        Role::Button(_) | Role::Hotbar => {}
                    }
                }
            }
        }
        out
    }

    /// Call every frame: a look finger held still for a moment starts breaking.
    pub fn tick(&mut self) {
        for f in self.fingers.iter_mut() {
            if f.role == Role::Look && !f.breaking && f.moved < 24.0 && f.t0.elapsed().as_millis() > 350 {
                f.breaking = true;
                self.breaking = true;
            }
        }
    }

    /// Reads the hotbar rectangle the HUD published (points -> physical pixels).
    pub fn sync_hotbar(&mut self, ppp: f32) {
        let read = |l: &std::sync::RwLock<[f32; 4]>| l.read().ok().and_then(|h| (h[2] > h[0]).then(|| (h[0] * ppp, h[1] * ppp, h[2] * ppp, h[3] * ppp)));
        self.hotbar = read(&wolfcraft_ui::hud::HOTBAR_RECT);
        self.inv = read(&wolfcraft_ui::hud::INV_RECT);
        self.map = read(&wolfcraft_ui::hud::MINIMAP_RECT);
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
        self.breaking = false;
        self.look = (0.0, 0.0);
    }

    /// Draws the controls with an egui painter; `ppp` = pixels per point.
    pub fn draw(&self, painter: &egui::Painter, size: (u32, u32), ppp: f32) {
        use egui::{Align2, Color32, FontId, Pos2, Stroke};
        let pt = |x: f32, y: f32| Pos2::new(x / ppp, y / ppp);
        let fill = Color32::from_rgba_unmultiplied(20, 24, 32, 110);
        let line = Stroke::new(2.0, Color32::from_rgba_unmultiplied(220, 230, 240, 150));
        let u = unit(size);
        // Joystick: base where the thumb went down, otherwise a hint in the lower left.
        let (bx, by, kx, ky) = match self.fingers.iter().find(|f| f.role == Role::Joystick) {
            Some(f) => (f.start.0, f.start.1, f.start.0 + self.move_vec.0 * STICK_R, f.start.1 - self.move_vec.1 * STICK_R),
            None => {
                let (x, y) = (170.0 * u, size.1 as f32 - 170.0 * u);
                (x, y, x, y)
            }
        };
        painter.circle(pt(bx, by), STICK_R * 0.9 / ppp, fill, line);
        painter.circle(pt(kx, ky), 40.0 * u / ppp, Color32::from_rgba_unmultiplied(230, 240, 250, 150), line);
        // Top island: one rounded pill holding chat, camera, debug and pause.
        {
            let (x0, y, step) = island_layout(size);
            let hh = 34.0 * u;
            let rect = egui::Rect::from_min_max(pt(x0 - 8.0 * u, y - hh), pt(x0 + step * ISLAND.len() as f32 + 8.0 * u, y + hh));
            painter.rect(rect, hh / ppp, Color32::from_rgba_unmultiplied(12, 14, 20, 200), Stroke::new(1.5, Color32::from_rgba_unmultiplied(220, 230, 240, 70)));
            for i in 1..ISLAND.len() {
                let x = x0 + step * i as f32;
                painter.line_segment([pt(x, y - hh * 0.45), pt(x, y + hh * 0.45)], Stroke::new(1.0, Color32::from_rgba_unmultiplied(220, 230, 240, 50)));
            }
            let white = Color32::from_rgba_unmultiplied(240, 245, 250, 235);
            let st = Stroke::new(2.4, white);
            for b in ISLAND {
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
            if ISLAND.contains(&b) {
                continue;
            }
            let (cx, cy, r) = button_rect(b, size, self.hotbar);
            let on = match b {
                Btn::Sneak => self.sneak,
                Btn::Jump => self.jump_held,
                _ => false,
            };
            let col = if on { Color32::from_rgba_unmultiplied(60, 160, 200, 170) } else { fill };
            painter.circle(pt(cx, cy), r / ppp, col, line);
            let white = Color32::from_rgba_unmultiplied(240, 245, 250, 230);
            if matches!(b, Btn::Jump | Btn::Sneak) {
                // The pixel font has no arrow glyphs: draw the triangle as a shape.
                let k = r * 0.42 / ppp;
                let c = pt(cx, cy);
                let s = if b == Btn::Jump { -1.0 } else { 1.0 };
                painter.add(egui::Shape::convex_polygon(vec![c + egui::vec2(0.0, s * k), c + egui::vec2(-k, -s * k * 0.7), c + egui::vec2(k, -s * k * 0.7)], white, Stroke::NONE));
                continue;
            }
            let label = match b {
                Btn::Jump => "",
                Btn::Sneak => "",
                Btn::Inventory => "E",
                Btn::Pause => "II",
                Btn::Chat => "T",
                Btn::Camera => "V",
                Btn::Tab => "TAB",
                Btn::Prev => "◀",
                Btn::Next => "▶",
            };
            painter.text(pt(cx, cy), Align2::CENTER_CENTER, label, FontId::proportional(r * 0.9 / ppp), Color32::from_rgba_unmultiplied(240, 245, 250, 220));
        }
    }
}
