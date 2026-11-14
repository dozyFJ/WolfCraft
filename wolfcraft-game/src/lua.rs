//! World scripts in Luau (the Lua of Roblox): the studio's script, run when the world starts.
//!
//! API (names in the spirit of Roblox):
//! * `print(...)`, `chat(text)`, `command("/time set день")`
//! * `player.position()` → x, y, z · `player.health()` / `player.set_health(v)` ·
//!   `player.give(name, count)` · `player.teleport(x, y, z)` · `player.creative()`
//! * `world.set(dx, dy, dz, name)` / `world.get(dx, dy, dz)`: blocks around the player
//!   (right, up, forward in cells) · `world.time()` / `world.set_time(hours)`
//! * events: `on("start" | "tick" | "chat" | "block_broken" | "block_placed", fn)`
//! * timers: `task.delay(seconds, fn)` (also `wait(seconds, fn)`)
//! * block events (like VoxelCore's block scripts): `block.on_placed(name, fn)`,
//!   `block.on_broken(name, fn)`, `block.on_interact(name, fn)` (a tap on the block: the
//!   usual action — placing, opening — is skipped unless the handler returns `false`),
//!   `block.on_step(name, fn)` (a player steps onto it).
//!   `name` is the block's name ("Дубовые доски") or `"*"` for any block; the handler gets
//!   `(pos, nick)`. `pos` has `x, y, z` (world) and the cell; `block.get(pos)`,
//!   `block.set(pos, name)` (`"air"` clears), `block.offset(pos, di, up, dj)` (along the
//!   cell grid), `block.above(pos, n)`.
//! * GUI like ScreenGui: `gui.frame{}`, `gui.label{}`, `gui.button{ on_click = fn }` with `id`,
//!   `text`, `x`, `y` (centre, 0..1 of the screen), `w`, `h` (0..1), `color = {r, g, b}`
//!   (0..255), `text_color`, `size` (font px); `gui.text(id, s)`, `gui.show(id, on)`,
//!   `gui.remove(id)`, `gui.clear()`.
//!
//! The game runs the script inside a scope that lends it the session, so the API calls act
//! on the world directly. A runaway script (endless loop) is stopped after a short time.

use crate::session::GameSession;
use mlua::{Lua, MultiValue, Table, VmState};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wolfcraft_ui::chat_ui::ChatKind;

/// Longest a single call into the script may run.
const BUDGET: Duration = Duration::from_millis(60);

#[derive(Clone, Debug, PartialEq)]
pub enum GuiKind {
    Frame,
    Label,
    Button,
    Image,
}

/// One element of the script's GUI.
#[derive(Clone, Debug)]
pub struct GuiElement {
    pub id: String,
    pub kind: GuiKind,
    pub text: String,
    pub sprite: Option<u16>,
    pub opacity:f32,
    /// Centre and size as shares of the screen.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Option<[u8; 3]>,
    pub text_color: [u8; 3],
    pub size: f32,
    pub visible: bool,
    /// Creation order (later ones are drawn on top).
    pub order: u32,
}

#[derive(Default)]
struct Shared {
    gui: Vec<GuiElement>,
    out: Vec<(ChatKind, String)>,
    next: u32,
}

/// Runtime part of the script written in Luau (handlers, timers, GUI callbacks).
const PRELUDE: &str = r#"
local handlers = {}
local timers = {}
local clicks = {}
function on(event, fn)
    handlers[event] = handlers[event] or {}
    table.insert(handlers[event], fn)
end
task = {}
function task.delay(seconds, fn)
    table.insert(timers, { t = seconds, fn = fn })
end
function wait(seconds, fn)
    task.delay(seconds, fn)
end
local function run(fn, ...)
    local ok, err = pcall(fn, ...)
    if not ok then __error(tostring(err)) end
end
function __fire(event, ...)
    local list = handlers[event]
    if list then
        for _, fn in ipairs(list) do run(fn, ...) end
    end
end
function __tick(dt)
    local due = {}
    for i = #timers, 1, -1 do
        local t = timers[i]
        t.t -= dt
        if t.t <= 0 then
            table.remove(timers, i)
            table.insert(due, t.fn)
        end
    end
    for _, fn in ipairs(due) do run(fn) end
    __fire("tick", dt)
end
function __click(id)
    local fn = clicks[id]
    if fn then run(fn) end
end
function __has_tick()
    return (handlers.tick ~= nil) or (#timers > 0)
end
block = {}
local block_handlers = { placed = {}, broken = {}, interact = {}, step = {} }
local function on_block(kind, name, fn)
    local list = block_handlers[kind]
    name = __lower(name)
    list[name] = list[name] or {}
    table.insert(list[name], fn)
end
function block.on_placed(name, fn) on_block("placed", name, fn) end
function block.on_broken(name, fn) on_block("broken", name, fn) end
function block.on_interact(name, fn) on_block("interact", name, fn) end
function block.on_step(name, fn) on_block("step", name, fn) end
function block.get(p) return __bget(p.f, p.i, p.j, p.k) end
function block.set(p, name) return __bset(p.f, p.i, p.j, p.k, name) end
function block.above(p, n) return __boff(p.f, p.i, p.j, p.k, 0, 0, n or 1) end
function block.offset(p, di, dk, dj) return __boff(p.f, p.i, p.j, p.k, di or 0, dj or 0, dk or 0) end
function __block_has(kind, name)
    local list = block_handlers[kind]
    return list[name] ~= nil or list["*"] ~= nil
end
function __block_fire(kind, name, pos, nick)
    local stop = false
    for _, key in ipairs({ name, "*" }) do
        for _, fn in ipairs(block_handlers[kind][key] or {}) do
            local ok, res = pcall(fn, pos, nick)
            if not ok then
                __error(tostring(res))
            elseif res == true or (kind == "interact" and res ~= false) then
                stop = true
            end
        end
    end
    return stop
end
gui = {}
function gui.frame(t) __gui_add("frame", t) end
function gui.label(t) __gui_add("label", t) end
function gui.image(t) __gui_add("image", t) end
function gui.button(t)
    clicks[t.id] = t.on_click
    __gui_add("button", t)
end
"#;

pub struct LuaHost {
    lua: Lua,
    shared: Arc<Mutex<Shared>>,
    deadline: Arc<Mutex<Instant>>,
    /// The script asked for ticks (has a tick handler or pending timers).
    wants_tick: bool,
}

fn chat_err(e: impl std::fmt::Display) -> (ChatKind, String) {
    (ChatKind::Error, format!("Lua: {e}"))
}

impl LuaHost {
    /// A fresh Luau state with the API installed.
    pub fn new() -> Result<Self, String> {
        let lua = Lua::new();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let deadline = Arc::new(Mutex::new(Instant::now() + BUDGET));
        {
            let d = deadline.clone();
            lua.set_interrupt(move |_| {
                if Instant::now() > *d.lock().unwrap() {
                    return Err(mlua::Error::RuntimeError("скрипт работает слишком долго (бесконечный цикл?) и остановлен".into()));
                }
                Ok(VmState::Continue)
            });
        }
        let g = lua.globals();
        let set = |name: &str, f: mlua::Function| g.set(name, f).map_err(|e| e.to_string());
        // print / errors / GUI: they only touch the shared state.
        let sh = shared.clone();
        set("print", lua.create_function(move |_, args: MultiValue| {
            let text: Vec<String> = args.iter().map(|v| v.to_string().unwrap_or_else(|_| format!("{v:?}"))).collect();
            sh.lock().unwrap().out.push((ChatKind::System, text.join(" ")));
            Ok(())
        }).map_err(|e| e.to_string())?)?;
        // Block names in scripts ignore case (Cyrillic too: Lua's own lower() is ASCII only).
        set("__lower", lua.create_function(|_, s: String| Ok(s.to_lowercase())).map_err(|e| e.to_string())?)?;
        let sh = shared.clone();
        set("__error", lua.create_function(move |_, msg: String| {
            sh.lock().unwrap().out.push(chat_err(msg));
            Ok(())
        }).map_err(|e| e.to_string())?)?;
        let sh = shared.clone();
        set("__gui_add", lua.create_function(move |_, (kind, t): (String, Table)| {
            let id: String = t.get::<Option<String>>("id")?.unwrap_or_else(|| format!("auto{}", sh.lock().unwrap().next));
            let rgb = |v: Option<Table>| -> Option<[u8; 3]> { v.map(|t| [t.get::<f64>(1).unwrap_or(0.0) as u8, t.get::<f64>(2).unwrap_or(0.0) as u8, t.get::<f64>(3).unwrap_or(0.0) as u8]) };
            let kind = match kind.as_str() {
                "frame" => GuiKind::Frame,
                "button" => GuiKind::Button,
                "image" => GuiKind::Image,
                _ => GuiKind::Label,
            };
            let mut s = sh.lock().unwrap();
            s.next += 1;
            let order = s.next;
            let el = GuiElement {
                id: id.clone(),
                sprite:t.get::<Option<u16>>("sprite")?.filter(|id|*id<wolfcraft_assets::gui::COUNT),
                opacity:t.get::<Option<f32>>("opacity")?.filter(|v|v.is_finite()).unwrap_or(1.).clamp(0.,1.),
                text: t.get::<Option<String>>("text")?.unwrap_or_default(),
                x: t.get::<Option<f32>>("x")?.unwrap_or(0.5),
                y: t.get::<Option<f32>>("y")?.unwrap_or(0.5),
                w: t.get::<Option<f32>>("w")?.unwrap_or(if kind == GuiKind::Label { 0.0 } else { 0.2 }),
                h: t.get::<Option<f32>>("h")?.unwrap_or(if kind == GuiKind::Label { 0.0 } else { 0.08 }),
                color: rgb(t.get("color")?),
                text_color: rgb(t.get("text_color")?).unwrap_or([255, 255, 255]),
                size: t.get::<Option<f32>>("size")?.unwrap_or(18.0),
                visible: t.get::<Option<bool>>("visible")?.unwrap_or(true),
                kind,
                order,
            };
            s.gui.retain(|e| e.id != id);
            s.gui.push(el);
            Ok(())
        }).map_err(|e| e.to_string())?)?;
        lua.load(PRELUDE).set_name("prelude").exec().map_err(|e| e.to_string())?;
        // gui.text / show / remove / clear need the table made by the prelude.
        let gui: Table = g.get("gui").map_err(|e| e.to_string())?;
        let sprites=lua.create_table().map_err(|e|e.to_string())?;
        for (name,id) in [("wood",wolfcraft_assets::gui::PANEL_WOOD),("metal",wolfcraft_assets::gui::PANEL_METAL),("resonance",wolfcraft_assets::gui::PANEL_RESONANCE),("magic",wolfcraft_assets::gui::PANEL_MAGIC),("dark",wolfcraft_assets::gui::PANEL_DARK),("play",wolfcraft_assets::gui::PLAY),("heart",wolfcraft_assets::gui::HEART)] {sprites.set(name,id).map_err(|e|e.to_string())?;}
        gui.set("sprites",sprites).map_err(|e|e.to_string())?;
        let sh=shared.clone();
        gui.set("set",lua.create_function(move |_,(id,t):(String,Table)|{
            if let Some(e)=sh.lock().unwrap().gui.iter_mut().find(|e|e.id==id) {
                for (name,target) in [("x",&mut e.x),("y",&mut e.y),("w",&mut e.w),("h",&mut e.h),("size",&mut e.size),("opacity",&mut e.opacity)] {
                    if let Some(v)=t.get::<Option<f32>>(name)?.filter(|v|v.is_finite()) {*target=match name{"x"|"y"=>v.clamp(-2.,3.),"opacity"=>v.clamp(0.,1.),"size"=>v.clamp(1.,128.),_=>v.clamp(0.,4.)};}
                }
                if let Some(sprite)=t.get::<Option<u16>>("sprite")?.filter(|id|*id<wolfcraft_assets::gui::COUNT){e.sprite=Some(sprite);}
            }Ok(())
        }).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        let sh = shared.clone();
        gui.set("text", lua.create_function(move |_, (id, text): (String, String)| {
            if let Some(e) = sh.lock().unwrap().gui.iter_mut().find(|e| e.id == id) {
                e.text = text;
            }
            Ok(())
        }).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let sh = shared.clone();
        gui.set("show", lua.create_function(move |_, (id, on): (String, bool)| {
            if let Some(e) = sh.lock().unwrap().gui.iter_mut().find(|e| e.id == id) {
                e.visible = on;
            }
            Ok(())
        }).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let sh = shared.clone();
        gui.set("remove", lua.create_function(move |_, id: String| {
            sh.lock().unwrap().gui.retain(|e| e.id != id);
            Ok(())
        }).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let sh = shared.clone();
        gui.set("clear", lua.create_function(move |_, ()| {
            sh.lock().unwrap().gui.clear();
            Ok(())
        }).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        Ok(Self { lua, shared, deadline, wants_tick: false })
    }

    /// The GUI the script built (drawn by the game every frame).
    pub fn gui(&self) -> Vec<GuiElement> {
        let mut v = self.shared.lock().unwrap().gui.clone();
        v.sort_by_key(|e| e.order);
        v
    }

    /// Lines the script printed (and errors) since the last call.
    pub fn take_output(&mut self) -> Vec<(ChatKind, String)> {
        std::mem::take(&mut self.shared.lock().unwrap().out)
    }

    pub fn wants_tick(&self) -> bool {
        self.wants_tick
    }

    /// Runs `f` with the world API lent to the script (bound to the session for this call).
    fn with_world(&mut self, s: &mut GameSession, f: impl FnOnce(&Lua) -> mlua::Result<()>) {
        *self.deadline.lock().unwrap() = Instant::now() + BUDGET;
        let cell = RefCell::new(s);
        let out = self.shared.clone();
        let res = self.lua.scope(|scope| {
            let g = self.lua.globals();
            let player = self.lua.create_table()?;
            let world = self.lua.create_table()?;
            let c = &cell;
            g.set("command", scope.create_function_mut(|_, line: String| {
                let line = if line.starts_with('/') { line } else { format!("/{line}") };
                let replies = crate::commands::run(&mut c.borrow_mut(), &line);
                out.lock().unwrap().out.extend(replies);
                Ok(())
            })?)?;
            g.set("chat", scope.create_function_mut(|_, text: String| {
                let name = c.borrow().nickname.clone();
                out.lock().unwrap().out.push((ChatKind::Player, format!("<{name}> {text}")));
                Ok(())
            })?)?;
            player.set("position", scope.create_function(|_, ()| {
                let p = c.borrow().player.body.pos;
                Ok((p.x, p.y, p.z))
            })?)?;
            player.set("health", scope.create_function(|_, ()| Ok(c.borrow().player.survival.health))?)?;
            player.set("set_health", scope.create_function_mut(|_, v: f32| {
                c.borrow_mut().player.survival.health = v.clamp(0.0, 20.0);
                Ok(())
            })?)?;
            player.set("creative", scope.create_function(|_, ()| Ok(c.borrow().is_creative()))?)?;
            player.set("give", scope.create_function_mut(|_, (name, n): (String, Option<u32>)| {
                let Some(id) = wolfcraft_engine::items::find_item(&name) else { return Ok(false) };
                let mut s = c.borrow_mut();
                let max = wolfcraft_engine::items::info(id).max_stack as u32;
                let mut left = n.unwrap_or(1).clamp(1, 2304);
                while left > 0 {
                    let k = left.min(max);
                    if s.player.inventory.add(wolfcraft_engine::items::ItemStack::new(id, k as u8)).is_some() {
                        return Ok(false);
                    }
                    left -= k;
                }
                Ok(true)
            })?)?;
            player.set("teleport", scope.create_function_mut(|_, (x, y, z): (f64, f64, f64)| {
                c.borrow_mut().teleport(glam::DVec3::new(x, y, z));
                Ok(())
            })?)?;
            // Blocks around the player: dx right, dy up, dz forward (cells).
            let around = |s: &GameSession, dx: i32, dy: i32, dz: i32| {
                let g = s.chunks.grid();
                let feet = g.world_to_cell(s.player.body.pos);
                let centre = g.cell_center(feet);
                let ax = |di: i32, dj: i32| g.offset(feet, di, dj, 0).map(|n| (g.cell_center(n) - centre).normalize()).unwrap_or(glam::DVec3::ZERO);
                let (ei, ej) = (ax(1, 0), ax(0, 1));
                let h = s.player.heading;
                let fwd = if h.dot(ei).abs() >= h.dot(ej).abs() { (h.dot(ei).signum() as i32, 0) } else { (0, h.dot(ej).signum() as i32) };
                let r = s.player.right();
                let perp = if fwd.0 != 0 { (0, 1) } else { (1, 0) };
                let rs = r.dot(ei * perp.0 as f64 + ej * perp.1 as f64).signum() as i32;
                g.offset(feet, fwd.0 * dz + perp.0 * rs * dx, fwd.1 * dz + perp.1 * rs * dx, dy)
            };
            world.set("set", scope.create_function_mut(move |_, (dx, dy, dz, name): (i32, i32, i32, String)| {
                let Some(b) = crate::worldedit::parse_block(&name) else { return Ok(false) };
                let mut s = c.borrow_mut();
                let Some(cell) = around(&s, dx, dy, dz) else { return Ok(false) };
                let ok = s.chunks.set_block(cell, b);
                if ok {
                    s.net_out.push((cell, b));
                }
                Ok(ok)
            })?)?;
            world.set("get", scope.create_function(move |_, (dx, dy, dz): (i32, i32, i32)| {
                let s = c.borrow();
                Ok(around(&s, dx, dy, dz).and_then(|cell| s.chunks.get_block(cell)).map(|b| b.info().name.to_string()))
            })?)?;
            world.set("time", scope.create_function(|_, ()| {
                let t = c.borrow().player.world_time;
                Ok(((t / wolfcraft_engine::player::DAY_LENGTH).rem_euclid(1.0) * 24.0) as f32)
            })?)?;
            world.set("set_time", scope.create_function_mut(|_, h: f64| {
                let mut s = c.borrow_mut();
                let day = (s.player.world_time / wolfcraft_engine::player::DAY_LENGTH).floor();
                s.player.world_time = (day + (h / 24.0).rem_euclid(1.0)) * wolfcraft_engine::player::DAY_LENGTH;
                Ok(())
            })?)?;
            // Blocks by cell (the `pos` tables of block events).
            let cell_of = |f: u8, i: i32, j: i32, k: i32| wolfcraft_engine::cubesphere::Cell::new(f, i, j, k);
            g.set("__bget", scope.create_function(move |_, (f, i, j, k): (u8, i32, i32, i32)| {
                let s = c.borrow();
                Ok(s.chunks.get_block(cell_of(f, i, j, k)).map(|b| b.info().name.to_string()))
            })?)?;
            g.set("__bset", scope.create_function_mut(move |_, (f, i, j, k, name): (u8, i32, i32, i32, String)| {
                let b = if matches!(name.to_lowercase().as_str(), "air" | "воздух" | "") { Some(wolfcraft_engine::block::Block::Air) } else { crate::worldedit::parse_block(&name) };
                let Some(b) = b else { return Ok(false) };
                let mut s = c.borrow_mut();
                let cell = cell_of(f, i, j, k);
                let ok = s.chunks.set_block(cell, b);
                if ok {
                    s.net_out.push((cell, b));
                }
                Ok(ok)
            })?)?;
            g.set("__boff", scope.create_function(move |lua, (f, i, j, k, di, dj, dk): (u8, i32, i32, i32, i32, i32, i32)| {
                let s = c.borrow();
                let g = s.chunks.grid();
                match g.offset(cell_of(f, i, j, k), di, dj, dk) {
                    Some(n) => Ok(Some(pos_table(lua, &g, n)?)),
                    None => Ok(None),
                }
            })?)?;
            g.set("player", player)?;
            g.set("world", world)?;
            f(&self.lua)
        });
        if let Err(e) = res {
            self.shared.lock().unwrap().out.push(chat_err(e));
        }
        self.wants_tick = self.lua.globals().get::<mlua::Function>("__has_tick").and_then(|f| f.call::<bool>(())).unwrap_or(false);
    }

    /// Runs a script's source, then fires "start".
    pub fn run(&mut self, s: &mut GameSession, source: &str) {
        let src = source.to_string();
        self.with_world(s, |lua| {
            lua.load(&src).set_name("скрипт мира").exec()?;
            lua.globals().get::<mlua::Function>("__fire")?.call::<()>("start")
        });
    }

    /// One frame: timers and "tick" handlers.
    pub fn tick(&mut self, s: &mut GameSession, dt: f32) {
        if !self.wants_tick {
            return;
        }
        self.with_world(s, |lua| lua.globals().get::<mlua::Function>("__tick")?.call::<()>(dt));
    }

    /// Fires an event with one string argument ("chat", "block_broken", "block_placed").
    pub fn event(&mut self, s: &mut GameSession, name: &str, arg: &str) {
        let (n, a) = (name.to_string(), arg.to_string());
        self.with_world(s, |lua| lua.globals().get::<mlua::Function>("__fire")?.call::<()>((n, a)));
    }

    /// The script handles this block event (so the game can skip work when it does not).
    pub fn has_block_event(&self, kind: &str, block: wolfcraft_engine::block::Block) -> bool {
        self.lua.globals().get::<mlua::Function>("__block_has").and_then(|f| f.call::<bool>((kind, block.info().name.to_lowercase()))).unwrap_or(false)
    }

    /// Fires a block event ("placed", "broken", "interact", "step") on `cell`. Returns true
    /// if a handler asked to stop the usual action.
    pub fn block_event(&mut self, s: &mut GameSession, kind: &str, block: wolfcraft_engine::block::Block, cell: wolfcraft_engine::cubesphere::Cell) -> bool {
        if !self.has_block_event(kind, block) {
            return false;
        }
        let nick = s.nickname.clone();
        let grid = s.chunks.grid();
        let (k, name) = (kind.to_string(), block.info().name.to_lowercase());
        let mut stop = false;
        self.with_world(s, |lua| {
            let pos = pos_table(lua, &grid, cell)?;
            stop = lua.globals().get::<mlua::Function>("__block_fire")?.call::<bool>((k, name, pos, nick))?;
            Ok(())
        });
        stop
    }

    /// A GUI button was pressed.
    pub fn click(&mut self, s: &mut GameSession, id: &str) {
        let id = id.to_string();
        self.with_world(s, |lua| lua.globals().get::<mlua::Function>("__click")?.call::<()>(id));
    }
}

/// `pos` of a block for scripts: world coordinates of its centre and its cell.
fn pos_table(lua: &Lua, grid: &wolfcraft_engine::cubesphere::Grid, c: wolfcraft_engine::cubesphere::Cell) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let p = grid.cell_center(c);
    t.set("x", (p.x * 100.0).round() / 100.0)?;
    t.set("y", (p.y * 100.0).round() / 100.0)?;
    t.set("z", (p.z * 100.0).round() / 100.0)?;
    t.set("f", c.f)?;
    t.set("i", c.i)?;
    t.set("j", c.j)?;
    t.set("k", c.k)?;
    Ok(t)
}

/// Turns an old command-list script (one chat command per line) into Luau.
pub fn from_command_list(text: &str) -> String {
    let mut out = String::from("-- Скрипт мира на Luau (переведён из списка команд)\n");
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(c) = t.strip_prefix('#') {
            out += &format!("--{c}\n");
        } else {
            let cmd = if t.starts_with('/') { t.to_string() } else { format!("/{t}") };
            out += &format!("command({:?})\n", cmd);
        }
    }
    out
}

/// What a new script starts with: a short tour of the API.
pub const TEMPLATE: &str = r#"-- Скрипт мира на Luau (как в Роблокс Студии). Запускается при входе в мир.
print("Мир запущен!")

-- GUI: кнопки и надписи на экране (x, y — центр, доли экрана)
local score = 0
gui.label{ id = "score", text = "Очки: 0", x = 0.5, y = 0.06, size = 22 }
gui.button{ id = "plus", text = "+1", x = 0.5, y = 0.15, w = 0.12, h = 0.07, on_click = function()
    score += 1
    gui.text("score", "Очки: " .. score)
end }

-- События
local broken = 0
gui.label{ id = "broken", text = "Сломано блоков: 0", x = 0.5, y = 0.22, size = 16 }
on("block_broken", function(name)
    broken += 1
    gui.text("broken", "Сломано блоков: " .. broken .. " (последний: " .. name .. ")")
end)

-- Таймер
task.delay(3, function()
    chat("Прошло 3 секунды")
end)

-- Скрипты блоков: уберите «--», чтобы включить.
-- Нажатие на золотой блок — награда, и блок становится камнем:
-- block.on_interact("Золотой блок", function(pos, nick)
--     chat(nick .. " нашёл клад!")
--     player.give("алмаз", 1)
--     block.set(pos, "камень")
-- end)
-- Встал на белую шерсть — сообщение:
-- block.on_step("Белая шерсть", function(pos, nick)
--     print(nick .. " на батуте")
-- end)
-- Поставили факел — над ним появляется светокамень:
-- block.on_placed("Факел", function(pos)
--     block.set(block.above(pos, 2), "светокамень")
-- end)
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{LoadRequest, start};
    use wolfcraft_engine::config::{GameMode, PlanetSize, WorldConfig};

    fn load() -> GameSession {
        let dir = std::env::temp_dir().join("wc_lua_test");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = WorldConfig::new("Lua", 5);
        cfg.size = PlanetSize::Small;
        cfg.structures = false;
        cfg.mode = GameMode::Creative;
        let sh = start(LoadRequest { cfg, worlds_dir: dir, is_new: true });
        while !sh.is_done() {
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut s = GameSession::new(sh.take_result().unwrap().unwrap());
        let t = Instant::now();
        while s.chunks.get_block(s.chunks.grid().world_to_cell(s.player.body.pos)).is_none() {
            s.update(&crate::session::InputFrame::default(), 0.016, 3, true);
            assert!(t.elapsed().as_secs() < 60);
        }
        s
    }

    #[test]
    fn luau_scripts_drive_the_world_and_a_gui() {
        let mut s = load();
        let mut lua = LuaHost::new().unwrap();
        lua.run(&mut s, TEMPLATE);
        let out = lua.take_output();
        assert!(out.iter().any(|(_, t)| t == "Мир запущен!"), "{out:?}");
        let gui = lua.gui();
        assert_eq!(gui.len(), 3);
        // Pressing the button runs its Luau callback.
        lua.click(&mut s, "plus");
        lua.click(&mut s, "plus");
        assert_eq!(lua.gui().iter().find(|e| e.id == "score").unwrap().text, "Очки: 2");
        // Timer fires after its delay.
        for _ in 0..200 {
            lua.tick(&mut s, 0.02);
        }
        assert!(lua.take_output().iter().any(|(_, t)| t.contains("Прошло 3 секунды")));
        // Events and the world API.
        lua.event(&mut s, "block_broken", "Камень");
        assert_eq!(lua.gui().iter().find(|e| e.id == "broken").unwrap().text, "Сломано блоков: 1 (последний: Камень)");
        lua.run(&mut s, r#"
            world.set(1, 3, 2, "камень")
            print(world.get(1, 3, 2))
            player.give("факел", 5)
            local x, y, z = player.position()
            print(type(x))
        "#);
        let out = lua.take_output();
        assert!(out.iter().any(|(_, t)| t == "Камень"), "{out:?}");
        assert!(out.iter().any(|(_, t)| t == "number"));
        assert_eq!(s.player.inventory.count(wolfcraft_engine::block::Block::Torch as u16), 5);
        // Errors and endless loops come back as chat errors instead of freezing the game.
        lua.run(&mut s, "while true do end");
        assert!(lua.take_output().iter().any(|(k, t)| *k == ChatKind::Error && t.contains("долго")));
        lua.run(&mut s, "nope(");
        assert!(lua.take_output().iter().any(|(k, _)| *k == ChatKind::Error));
        // Block events: a tap on stone runs the script instead of placing a block.
        let g = s.chunks.grid();
        let here = g.world_to_cell(s.player.body.pos);
        let spot = g.offset(here, 0, 0, 3).unwrap();
        use wolfcraft_engine::interaction::WorldEdit;
        s.chunks.set_block(spot, wolfcraft_engine::block::Block::Stone);
        lua.run(&mut s, r#"
            block.on_interact("Камень", function(pos, nick)
                block.set(block.above(pos, 1), "камень")
                print("нажали " .. block.get(pos) .. " " .. type(pos.x))
            end)
            block.on_placed("*", function(pos) print("поставили") end)
        "#);
        assert!(lua.has_block_event("interact", wolfcraft_engine::block::Block::Stone));
        assert!(!lua.has_block_event("step", wolfcraft_engine::block::Block::Stone));
        assert!(lua.block_event(&mut s, "interact", wolfcraft_engine::block::Block::Stone, spot), "the tap is taken by the script");
        assert_eq!(s.chunks.get_block(g.offset(spot, 0, 0, 1).unwrap()), Some(wolfcraft_engine::block::Block::Stone));
        assert!(!lua.block_event(&mut s, "placed", wolfcraft_engine::block::Block::Torch, spot));
        let out = lua.take_output();
        assert!(out.iter().any(|(_, t)| t == "нажали Камень number"), "{out:?}");
        assert!(out.iter().any(|(_, t)| t == "поставили"), "{out:?}");
        // Old command-list scripts are translated.
        assert!(from_command_list("/time set день\n# x").contains("command(\"/time set день\")"));
    }
}


#[cfg(test)]
mod atlas_gui_tests {
    use super::*;
    #[test] fn lua_changes_state_while_renderer_keeps_atlas_sprites(){
        let host=LuaHost::new().unwrap();
        host.lua.load(r#"
            gui.image{id="badge",sprite=gui.sprites.heart,x=0.2,y=0.2,w=0.1,h=0.1}
            gui.frame{id="panel",sprite=gui.sprites.wood}
            gui.set("panel",{x=0.75,y=0.4,opacity=0.3,w=0.4})
            gui.show("badge",false)
        "#).exec().unwrap();
        let elements=host.gui();let panel=elements.iter().find(|e|e.id=="panel").unwrap();
        assert_eq!(panel.sprite,Some(wolfcraft_assets::gui::PANEL_WOOD));assert_eq!(panel.x,0.75);assert_eq!(panel.opacity,0.3);
        assert!(!elements.iter().find(|e|e.id=="badge").unwrap().visible);
        host.lua.load("gui.set('panel',{opacity=5,w=-1,sprite=65000})").exec().unwrap();
        let panel=host.gui().into_iter().find(|e|e.id=="panel").unwrap();assert_eq!(panel.opacity,1.);assert_eq!(panel.w,0.);assert_eq!(panel.sprite,Some(wolfcraft_assets::gui::PANEL_WOOD));
    }
}
