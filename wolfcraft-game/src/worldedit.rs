//! Basic WorldEdit commands, typed with a double slash like the mod:
//!
//! * selection: `//pos1`, `//pos2` (where you stand), `//hpos1`, `//hpos2` (the block you look
//!   at), `//size`, `//desel`
//! * region: `//set <блок>`, `//replace <что> <на что>`, `//walls <блок>`, `//outline <блок>`
//! * clipboard: `//copy`, `//cut`, `//paste` (relative to where you stood when copying)
//! * shapes around you: `//sphere <блок> <r>`, `//hsphere <блок> <r>`, `//cyl <блок> <r> [h]`
//! * `//drain [r]` removes water and lava around you
//! * history: `//undo`, `//redo`
//!
//! Regions live on one face of the cube-sphere planet (cell coordinates i, j, k).

use crate::commands::Reply;
use crate::session::GameSession;
use wolfcraft_engine::block::Block;
use wolfcraft_engine::cubesphere::Cell;
use wolfcraft_engine::items;
use wolfcraft_ui::chat_ui::ChatKind;

/// Most blocks one command may change.
const MAX_BLOCKS: i64 = 250_000;
/// Undo steps kept.
const HISTORY: usize = 20;

#[derive(Default)]
pub struct WorldEdit {
    pub pos1: Option<Cell>,
    pub pos2: Option<Cell>,
    /// Copied blocks as offsets from where the player stood, plus the block.
    clipboard: Vec<((i32, i32, i32), Block)>,
    undo: Vec<Vec<(Cell, Block)>>,
    redo: Vec<Vec<(Cell, Block)>>,
    /// A brush stroke is going on: its dabs join one undo step.
    stroke: bool,
    stroke_started: bool,
    /// Studio models: groups of blocks (each brush stroke that adds blocks makes one).
    pub models: Vec<Model>,
    models_loaded: bool,
    /// Blocks added by the brush stroke going on (they become a model when it ends).
    stroke_cells: Vec<Cell>,
}

/// A group of blocks in the studio («Модель», like in Roblox Studio).
#[derive(Clone, Debug)]
pub struct Model {
    pub name: String,
    pub cells: Vec<Cell>,
}

/// What can be done to a model.
pub use wolfcraft_ui::studio_ui::ModelCmd;

const MODELS_FILE: &str = "models.txt";

/// Reads the place's models once (`models.txt` next to the world).
pub fn load_models(s: &mut GameSession) {
    if s.we.models_loaded {
        return;
    }
    s.we.models_loaded = true;
    let Ok(text) = std::fs::read_to_string(s.dir.join(MODELS_FILE)) else { return };
    let mut cur: Option<Model> = None;
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("# ") {
            if let Some(m) = cur.take() {
                s.we.models.push(m);
            }
            cur = Some(Model { name: name.to_string(), cells: Vec::new() });
        } else if let Some(m) = &mut cur {
            let v: Vec<i32> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if v.len() == 4 {
                m.cells.push(Cell { f: v[0] as u8, i: v[1], j: v[2], k: v[3] });
            }
        }
    }
    if let Some(m) = cur {
        s.we.models.push(m);
    }
}

pub fn save_models(s: &GameSession) {
    let mut out = String::new();
    for m in &s.we.models {
        out += &format!("# {}\n", m.name);
        for c in &m.cells {
            out += &format!("{} {} {} {}\n", c.f, c.i, c.j, c.k);
        }
    }
    if let Err(e) = std::fs::write(s.dir.join(MODELS_FILE), out) {
        log::warn!("models: {e}");
    }
}

/// Moves, turns, resizes, deletes or ungroups a model. Returns a message for the output.
pub fn model_cmd(s: &mut GameSession, idx: usize, cmd: ModelCmd) -> Result<String, String> {
    let Some(m) = s.we.models.get(idx).cloned() else { return Err("Модели нет".into()) };
    let g = s.chunks.grid();
    let Some(f) = m.cells.first().map(|c| c.f) else {
        s.we.models.remove(idx);
        save_models(s);
        return Err("Модель пустая — убрана".into());
    };
    // Blocks of the model (on its main face) relative to the corner of its box.
    let cells: Vec<(Cell, Block)> = m.cells.iter().filter(|c| c.f == f).filter_map(|c| s.chunks.get_block(*c).filter(|b| *b != Block::Air).map(|b| (*c, b))).collect();
    if cells.is_empty() && !matches!(cmd, ModelCmd::Ungroup | ModelCmd::Delete) {
        return Err("Блоки модели ещё не загружены или уже убраны".into());
    }
    let lo = [cells.iter().map(|c| c.0.i).min().unwrap_or(0), cells.iter().map(|c| c.0.j).min().unwrap_or(0), cells.iter().map(|c| c.0.k).min().unwrap_or(0)];
    let hi = [cells.iter().map(|c| c.0.i).max().unwrap_or(0), cells.iter().map(|c| c.0.j).max().unwrap_or(0), cells.iter().map(|c| c.0.k).max().unwrap_or(0)];
    let rel: Vec<([i32; 3], Block)> = cells.iter().map(|(c, b)| ([c.i - lo[0], c.j - lo[1], c.k - lo[2]], *b)).collect();
    let place = |r: [i32; 3], base: [i32; 3]| g.map_cell(f, base[0] + r[0], base[1] + r[1], base[2] + r[2]);
    let new: Vec<([i32; 3], Block)> = match cmd {
        ModelCmd::Ungroup => {
            s.we.models.remove(idx);
            save_models(s);
            return Ok(format!("«{}» разгруппирована: блоки остались", m.name));
        }
        ModelCmd::Select => {
            s.we.pos1 = Some(Cell { f, i: lo[0], j: lo[1], k: lo[2] });
            s.we.pos2 = Some(Cell { f, i: hi[0], j: hi[1], k: hi[2] });
            return Ok(format!("«{}» выделена: Копия, Заполнить и другие инструменты Модели работают с ней", m.name));
        }
        ModelCmd::Delete => Vec::new(),
        ModelCmd::Move(d) => rel.iter().map(|(r, b)| ([r[0] + d[0], r[1] + d[1], r[2] + d[2]], *b)).collect(),
        ModelCmd::Rotate => {
            // A quarter turn about the vertical, inside the same box corner.
            let h = hi[1] - lo[1];
            rel.iter().map(|(r, b)| ([h - r[1], r[0], r[2]], *b)).collect()
        }
        ModelCmd::Grow => {
            if rel.len() * 8 > 40_000 {
                return Err("Слишком большая модель для увеличения".into());
            }
            let mut v = Vec::new();
            for (r, b) in &rel {
                for a in 0..8 {
                    v.push(([r[0] * 2 + (a & 1), r[1] * 2 + ((a >> 1) & 1), r[2] * 2 + ((a >> 2) & 1)], *b));
                }
            }
            v
        }
        ModelCmd::Shrink => {
            let mut seen = std::collections::HashSet::new();
            rel.iter().filter_map(|(r, b)| {
                let q = [r[0].div_euclid(2), r[1].div_euclid(2), r[2].div_euclid(2)];
                seen.insert(q).then_some((q, *b))
            }).collect()
        }
    };
    let mut ops: Vec<(Cell, Block)> = cells.iter().map(|(c, _)| (*c, Block::Air)).collect();
    let mut out_cells = Vec::new();
    for (r, b) in &new {
        let Some(c) = place(*r, lo) else { return Err("Модель вышла бы за край грани планеты".into()) };
        ops.push((c, *b));
        out_cells.push(c);
    }
    let (n, missed) = apply(s, ops);
    if missed > 0 && n == 0 {
        return Err("Мир там ещё не загрузился".into());
    }
    if new.is_empty() {
        s.we.models.remove(idx);
    } else if let Some(m) = s.we.models.get_mut(idx) {
        m.cells = out_cells;
    }
    save_models(s);
    Ok(match cmd {
        ModelCmd::Delete => format!("«{}» удалена", m.name),
        ModelCmd::Rotate => format!("«{}» повёрнута на 90°", m.name),
        ModelCmd::Grow => format!("«{}» увеличена ×2", m.name),
        ModelCmd::Shrink => format!("«{}» уменьшена ×2", m.name),
        _ => format!("«{}» сдвинута", m.name),
    })
}

fn sys(t: impl Into<String>) -> Reply {
    vec![(ChatKind::System, t.into())]
}
fn err(t: impl Into<String>) -> Reply {
    vec![(ChatKind::Error, t.into())]
}

/// A block by name (Russian name from the game, a few English ids, or the number).
pub fn parse_block(name: &str) -> Option<Block> {
    let q = name.trim().to_lowercase();
    let alias = match q.as_str() {
        "air" | "воздух" | "0" => return Some(Block::Air),
        "stone" => "Камень",
        "dirt" => "Земля",
        "grass" => "Дёрн",
        "sand" => "Песок",
        "glass" => "Стекло",
        "water" => "Вода",
        "lava" => "Лава",
        "planks" | "wood" => "Дубовые доски",
        "cobble" | "cobblestone" => "Булыжник",
        "bricks" | "кирпичи" => "Кирпич",
        _ => name,
    };
    let id = items::find_item(alias)?;
    (id < items::ITEM_BASE || id >= 1024).then(|| Block::from_id(id)).filter(|b| *b != Block::Air || id == 0)
}

impl WorldEdit {
    fn region(&self) -> Result<(u8, [i32; 3], [i32; 3]), String> {
        let (Some(a), Some(b)) = (self.pos1, self.pos2) else { return Err("Сначала задайте выделение: //pos1 и //pos2 (или //hpos1, //hpos2)".into()) };
        if a.f != b.f {
            return Err("Точки выделения на разных гранях планеты: выберите обе на одной".into());
        }
        Ok((a.f, [a.i.min(b.i), a.j.min(b.j), a.k.min(b.k)], [a.i.max(b.i), a.j.max(b.j), a.k.max(b.k)]))
    }
}

fn volume(lo: [i32; 3], hi: [i32; 3]) -> i64 {
    (0..3).map(|a| (hi[a] - lo[a] + 1) as i64).product()
}

/// Sets blocks, recording what was there for undo. Returns how many changed and how many
/// could not be set (chunk not loaded).
pub(crate) fn apply(s: &mut GameSession, cells: impl IntoIterator<Item = (Cell, Block)>) -> (usize, usize) {
    let mut log = Vec::new();
    let mut missed = 0;
    for (c, b) in cells {
        let Some(old) = s.chunks.get_block(c) else {
            missed += 1;
            continue;
        };
        if old == b {
            continue;
        }
        if s.chunks.set_block(c, b) {
            // The brush adds blocks: they will be one model.
            if s.we.stroke && old == Block::Air && b != Block::Air {
                s.we.stroke_cells.push(c);
            }
            // A block that goes leaves its model.
            if b == Block::Air && !s.we.models.is_empty() {
                for m in &mut s.we.models {
                    m.cells.retain(|x| *x != c);
                }
            }
            log.push((c, old));
            s.net_out.push((c, b));
        } else {
            missed += 1;
        }
    }
    let n = log.len();
    if n > 0 && s.we.stroke && s.we.stroke_started {
        // Later dabs of the same brush stroke: one undo step for the whole stroke.
        if let Some(last) = s.we.undo.last_mut() {
            last.extend(log);
        }
        s.we.redo.clear();
    } else if n > 0 {
        s.we.stroke_started = s.we.stroke;
        s.we.undo.push(log);
        if s.we.undo.len() > HISTORY {
            s.we.undo.remove(0);
        }
        s.we.redo.clear();
    }
    (n, missed)
}

fn done(n: usize, missed: usize) -> Reply {
    if missed > 0 {
        sys(format!("Изменено блоков: {n}. Не загружено: {missed} (подойдите ближе)"))
    } else {
        sys(format!("Изменено блоков: {n}"))
    }
}

fn feet(s: &GameSession) -> Cell {
    s.chunks.grid().world_to_cell(s.player.body.pos)
}

/// Runs `//<cmd>`; `cmd` comes without the slashes. `None` when it is not a WorldEdit command.
pub fn run(s: &mut GameSession, cmd: &str, args: &[&str]) -> Option<Reply> {
    let known = ["pos1", "pos2", "hpos1", "hpos2", "size", "desel", "set", "replace", "walls", "outline", "copy", "cut", "paste", "sphere", "hsphere", "cyl", "drain", "undo", "redo", "help", "wand"];
    if !known.contains(&cmd) {
        return None;
    }
    if !s.is_creative() {
        return Some(err("WorldEdit работает только в творческом режиме"));
    }
    let g = s.chunks.grid();
    Some(match cmd {
        "help" | "wand" => vec![
            (ChatKind::System, "WorldEdit: //pos1 //pos2 — углы там, где вы стоите; //hpos1 //hpos2 — блок под прицелом".into()),
            (ChatKind::System, "//set <блок> · //replace <что> <на что> · //walls <блок> · //outline <блок> · //size · //desel".into()),
            (ChatKind::System, "//copy //cut //paste · //sphere <блок> <r> · //hsphere <блок> <r> · //cyl <блок> <r> [h] · //drain [r] · //undo //redo".into()),
        ],
        "pos1" | "pos2" | "hpos1" | "hpos2" => {
            let c = if cmd.starts_with('h') {
                match s.hit {
                    Some(h) => h.cell,
                    None => return Some(err("Смотрите на блок")),
                }
            } else {
                feet(s)
            };
            if cmd.ends_with('1') {
                s.we.pos1 = Some(c);
            } else {
                s.we.pos2 = Some(c);
            }
            let size = s.we.region().map(|(_, lo, hi)| format!(" · выделено {} блоков", volume(lo, hi))).unwrap_or_default();
            sys(format!("Точка {}: {} {} {}{size}", if cmd.ends_with('1') { 1 } else { 2 }, c.i, c.j, c.k))
        }
        "size" => match s.we.region() {
            Ok((_, lo, hi)) => sys(format!("Выделение {}×{}×{} = {} блоков", hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1, volume(lo, hi))),
            Err(e) => err(e),
        },
        "desel" => {
            s.we.pos1 = None;
            s.we.pos2 = None;
            sys("Выделение снято")
        }
        "set" | "walls" | "outline" | "replace" => {
            let (f, lo, hi) = match s.we.region() {
                Ok(r) => r,
                Err(e) => return Some(err(e)),
            };
            if volume(lo, hi) > MAX_BLOCKS {
                return Some(err(format!("Слишком большое выделение ({} блоков, можно до {MAX_BLOCKS})", volume(lo, hi))));
            }
            let (from, to) = if cmd == "replace" {
                let (Some(a), Some(b)) = (args.first(), args.get(1)) else { return Some(err("Использование: //replace <что> <на что>")) };
                match (parse_block(a), parse_block(b)) {
                    (Some(x), Some(y)) => (Some(x), y),
                    _ => return Some(err("Блок не найден")),
                }
            } else {
                let name = args.join(" ");
                match parse_block(&name) {
                    Some(b) => (None, b),
                    None => return Some(err(if name.is_empty() { format!("Использование: //{cmd} <блок>") } else { format!("Блок не найден: {name}") })),
                }
            };
            let mut cells = Vec::new();
            for i in lo[0]..=hi[0] {
                for j in lo[1]..=hi[1] {
                    for k in lo[2]..=hi[2] {
                        let side = i == lo[0] || i == hi[0] || j == lo[1] || j == hi[1];
                        let shell = side || k == lo[2] || k == hi[2];
                        let take = match cmd {
                            "walls" => side,
                            "outline" => shell,
                            _ => true,
                        };
                        if !take {
                            continue;
                        }
                        let c = Cell::new(f, i, j, k);
                        if let Some(x) = from {
                            if s.chunks.get_block(c) != Some(x) {
                                continue;
                            }
                        }
                        cells.push((c, to));
                    }
                }
            }
            let (n, m) = apply(s, cells);
            done(n, m)
        }
        "copy" | "cut" => {
            let (f, lo, hi) = match s.we.region() {
                Ok(r) => r,
                Err(e) => return Some(err(e)),
            };
            if volume(lo, hi) > MAX_BLOCKS {
                return Some(err("Слишком большое выделение для буфера"));
            }
            let me = feet(s);
            if me.f != f {
                return Some(err("Встаньте на той же грани планеты, что и выделение"));
            }
            let mut clip = Vec::new();
            let mut clear = Vec::new();
            for i in lo[0]..=hi[0] {
                for j in lo[1]..=hi[1] {
                    for k in lo[2]..=hi[2] {
                        let c = Cell::new(f, i, j, k);
                        let b = s.chunks.get_block(c).unwrap_or(Block::Air);
                        clip.push(((i - me.i, j - me.j, k - me.k), b));
                        if cmd == "cut" {
                            clear.push((c, Block::Air));
                        }
                    }
                }
            }
            let n = clip.len();
            s.we.clipboard = clip;
            if cmd == "cut" {
                let (_, m) = apply(s, clear);
                sys(format!("Вырезано в буфер: {n} блоков{}", if m > 0 { format!(" (не загружено: {m})") } else { String::new() }))
            } else {
                sys(format!("Скопировано в буфер: {n} блоков. //paste вставит их относительно того места, где вы встанете"))
            }
        }
        "paste" => {
            if s.we.clipboard.is_empty() {
                return Some(err("Буфер пуст: сначала //copy"));
            }
            let me = feet(s);
            let skip_air = args.first().is_some_and(|a| *a == "-a");
            let cells: Vec<(Cell, Block)> = s.we.clipboard.iter().filter(|(_, b)| !(skip_air && *b == Block::Air)).filter_map(|((di, dj, dk), b)| g.offset(me, *di, *dj, *dk).map(|c| (c, *b))).collect();
            let (n, m) = apply(s, cells);
            done(n, m)
        }
        "sphere" | "hsphere" | "cyl" => {
            let (Some(name), Some(r)) = (args.first(), args.get(1).and_then(|r| r.parse::<i32>().ok())) else {
                return Some(err(format!("Использование: //{cmd} <блок> <радиус>{}", if cmd == "cyl" { " [высота]" } else { "" })));
            };
            let Some(b) = parse_block(name) else { return Some(err(format!("Блок не найден: {name}"))) };
            let r = r.clamp(1, 40);
            let h = args.get(2).and_then(|h| h.parse::<i32>().ok()).unwrap_or(1).clamp(1, 64);
            let me = feet(s);
            let rr = (r as f64 + 0.5).powi(2);
            let inner = (r as f64 - 0.5).powi(2);
            let mut cells = Vec::new();
            for di in -r..=r {
                for dj in -r..=r {
                    let ks: Vec<i32> = if cmd == "cyl" { (0..h).collect() } else { (-r..=r).collect() };
                    for dk in ks {
                        let d2 = (di * di + dj * dj + if cmd == "cyl" { 0 } else { dk * dk }) as f64;
                        if d2 > rr || (cmd == "hsphere" && d2 < inner) {
                            continue;
                        }
                        if let Some(c) = g.offset(me, di, dj, dk) {
                            cells.push((c, b));
                        }
                    }
                }
            }
            let (n, m) = apply(s, cells);
            done(n, m)
        }
        "drain" => {
            let r = args.first().and_then(|r| r.parse::<i32>().ok()).unwrap_or(8).clamp(1, 30);
            let me = feet(s);
            let mut cells = Vec::new();
            for di in -r..=r {
                for dj in -r..=r {
                    for dk in -r..=r {
                        if let Some(c) = g.offset(me, di, dj, dk) {
                            if s.chunks.get_block(c).is_some_and(|b| b.is_fluid()) {
                                cells.push((c, Block::Air));
                            }
                        }
                    }
                }
            }
            let (n, m) = apply(s, cells);
            done(n, m)
        }
        "undo" | "redo" => {
            let step = if cmd == "undo" { s.we.undo.pop() } else { s.we.redo.pop() };
            let Some(step) = step else { return Some(err(if cmd == "undo" { "Нечего отменять" } else { "Нечего повторять" })) };
            let mut back = Vec::new();
            for (c, b) in step.into_iter().rev() {
                if let Some(old) = s.chunks.get_block(c) {
                    if s.chunks.set_block(c, b) {
                        back.push((c, old));
                        s.net_out.push((c, b));
                    }
                }
            }
            let n = back.len();
            if cmd == "undo" {
                s.we.redo.push(back);
                sys(format!("Отменено: {n} блоков"))
            } else {
                s.we.undo.push(back);
                sys(format!("Повторено: {n} блоков"))
            }
        }
        _ => return None,
    })
}


/// What the studio brush does (like sculpting brushes in Blender, on blocks).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BrushOp {
    /// Adds a ball of the block where you point.
    #[default]
    Add,
    /// Removes blocks in the ball.
    Erase,
    /// Repaints the visible surface in the ball with the block.
    Paint,
    /// Grows the ground by one layer of the block.
    Raise,
    /// Evens out bumps and holes.
    Smooth,
    /// Levels everything to the height of the point.
    Flatten,
}

impl BrushOp {
    pub const ALL: [BrushOp; 6] = [BrushOp::Add, BrushOp::Erase, BrushOp::Paint, BrushOp::Raise, BrushOp::Smooth, BrushOp::Flatten];
    pub fn name(self) -> &'static str {
        match self {
            BrushOp::Add => "Добавить",
            BrushOp::Erase => "Стереть",
            BrushOp::Paint => "Покрасить",
            BrushOp::Raise => "Поднять",
            BrushOp::Smooth => "Сгладить",
            BrushOp::Flatten => "Выровнять",
        }
    }
}

/// Starts or ends a brush stroke (all dabs while the finger is down are one undo step).
pub fn stroke(s: &mut GameSession, on: bool) {
    let was = s.we.stroke;
    s.we.stroke = on;
    if !on {
        s.we.stroke_started = false;
        // The stroke is over: the blocks it added become a model.
        if was && !s.we.stroke_cells.is_empty() {
            load_models(s);
            let mut cells = std::mem::take(&mut s.we.stroke_cells);
            cells.sort_by_key(|c| (c.f, c.i, c.j, c.k));
            cells.dedup();
            let n = s.we.models.len() + 1;
            s.we.models.push(Model { name: format!("Модель {n}"), cells });
            save_models(s);
        }
        s.we.stroke_cells.clear();
    }
}

/// One dab of the brush at the block the player points at. `cube` uses a cube instead of a
/// ball; `block` is the block in hand (for adding and painting). Returns blocks changed.
pub fn brush(s: &mut GameSession, op: BrushOp, radius: i32, cube: bool, block: Block) -> usize {
    let Some(hit) = s.hit else { return 0 };
    let g = s.chunks.grid();
    let r = radius.clamp(1, 12);
    let centre = if matches!(op, BrushOp::Add) { hit.front_cell(&g).unwrap_or(hit.cell) } else { hit.cell };
    let inside = |di: i32, dj: i32, dk: i32| cube || ((di * di + dj * dj + dk * dk) as f64) <= (r as f64 + 0.4).powi(2);
    let get = |s: &GameSession, c: Cell| s.chunks.get_block(c).unwrap_or(Block::Air);
    let solid = |b: Block| b.is_solid() && !b.is_fluid();
    let mut cells = Vec::new();
    for di in -r..=r {
        for dj in -r..=r {
            for dk in -r..=r {
                if !inside(di, dj, dk) {
                    continue;
                }
                let Some(c) = g.offset(centre, di, dj, dk) else { continue };
                let b = get(s, c);
                let up = g.neighbor(c, 2, 1).map(|u| get(s, u)).unwrap_or(Block::Air);
                let down = g.neighbor(c, 2, -1).map(|d| get(s, d)).unwrap_or(Block::Air);
                let target = match op {
                    BrushOp::Add => (!solid(b)).then_some(block),
                    BrushOp::Erase => (b != Block::Air).then_some(Block::Air),
                    BrushOp::Paint => (solid(b) && !solid(up)).then_some(block),
                    BrushOp::Raise => (!solid(b) && solid(down)).then_some(block),
                    BrushOp::Flatten => {
                        if dk > 0 && b != Block::Air {
                            Some(Block::Air)
                        } else if dk <= 0 && !solid(b) {
                            Some(if dk == 0 { block } else { Block::Dirt })
                        } else {
                            None
                        }
                    }
                    BrushOp::Smooth => {
                        // Six neighbours vote: mostly air around a block removes it, mostly
                        // ground around a gap fills it with the commonest neighbour.
                        let ns: Vec<Block> = [(0, 1), (0, -1), (1, 1), (1, -1), (2, 1), (2, -1)].iter().filter_map(|(a, sg)| g.neighbor(c, *a, *sg)).map(|n| get(s, n)).collect();
                        let full = ns.iter().filter(|n| solid(**n)).count();
                        if solid(b) && full <= 2 {
                            Some(Block::Air)
                        } else if !solid(b) && full >= 4 {
                            let mut best = (Block::Dirt, 0);
                            for n in ns.iter().filter(|n| solid(**n)) {
                                let k = ns.iter().filter(|m| *m == n).count();
                                if k > best.1 {
                                    best = (*n, k);
                                }
                            }
                            Some(best.0)
                        } else {
                            None
                        }
                    }
                };
                if let Some(t) = target {
                    cells.push((c, t));
                }
            }
        }
    }
    apply(s, cells).0
}

