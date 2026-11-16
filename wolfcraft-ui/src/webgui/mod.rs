//! WolfHTML: GUI windows for mods written in HTML + CSS with JavaScript logic, drawn by the
//! game itself (no browser), the same on the phone and on the PC.
//!
//! A page is one `.html` file: `<title>` names the window, `<style>` blocks and `style=""`
//! attributes style it, `<script>` blocks run once when it opens. Scripts change the page
//! through `document.getElementById(...)` (`textContent`, `style.*`, `classList`, `value`,
//! `hidden`, `onclick` / `addEventListener('click', …)`), use `setInterval` / `setTimeout`,
//! and talk to the game through `wolf`: `wolf.chat(text)`, `wolf.command("/time set день")`,
//! `wolf.close()`, `wolf.health()`, `wolf.hunger()`, `wolf.pos()`, `wolf.time()`,
//! `wolf.nick()`, `wolf.creative()`, `wolf.store(key, value)` / `wolf.load(key)`.

pub mod css;
pub mod html;
pub mod js;

use crate::theme_manager::bold_font;
use css::{Align, Display, Len, Rgba, Style};
use egui::{Color32, Frame, Margin, RichText, Rounding, Sense, Stroke, Ui};
pub use js::{Effect, Host};

pub struct Page {
    pub name: String,
    pub title: String,
    pub open: bool,
    it: js::Interp,
    rules: Vec<css::Rule>,
    body_style: Style,
    hovered: Option<usize>,
    /// Where `wolf.store` values are kept between sessions.
    pub save_path: Option<std::path::PathBuf>,
}

impl Page {
    /// Parses the page and runs its scripts (errors go to the log, like a browser console).
    pub fn load(name: &str, src: &str, save_path: Option<std::path::PathBuf>) -> Page {
        let dom = html::Dom::parse(src);
        let rules = css::parse(&dom.all_text_of("style").join("\n"));
        let title = dom.find_tag("title").map(|t| dom.text_of(t)).filter(|t| !t.is_empty()).unwrap_or_else(|| name.to_string());
        let scripts = dom.all_text_of("script");
        let mut it = js::Interp::new(dom);
        if let Some(p) = &save_path {
            if let Ok(t) = std::fs::read_to_string(p) {
                for line in t.lines() {
                    if let Some((k, v)) = line.split_once('\t') {
                        it.saved.insert(k.to_string(), v.replace("\\n", "\n"));
                    }
                }
            }
        }
        for s in scripts {
            if let Err(e) = it.run(&s) {
                it.log.push(format!("JS: {e}"));
            }
        }
        let body = it.dom.find_tag("body").unwrap_or(0);
        let body_style = css::compute(&it.dom, &rules, body, &Style::root(), None);
        Page { name: name.to_string(), title, open: true, it, rules, body_style, hovered: None, save_path }
    }

    /// Lines printed by `console.log` and script errors since the last call.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.it.log)
    }

    /// Draws the window, runs the events and timers; returns what the scripts asked of the game.
    pub fn show(&mut self, ctx: &egui::Context, host: &Host, dt: f32) -> Vec<Effect> {
        self.it.host = host.clone();
        self.it.tick(dt as f64);
        let body = self.it.dom.find_tag("body").unwrap_or(0);
        let bs = self.body_style.clone();
        let fill = bs.background.map(|c| col(c, 1.0)).unwrap_or(Color32::from_rgba_unmultiplied(22, 28, 40, 238));
        let frame = Frame::window(&ctx.style())
            .fill(fill)
            .rounding(Rounding::same(if bs.radius > 0.0 { bs.radius } else { 12.0 }))
            .stroke(bs.border.map(|(w, c)| Stroke::new(w, col(c, 1.0))).unwrap_or(Stroke::new(1.0_f32, Color32::from_white_alpha(60))))
            .inner_margin(if bs.padding == [0.0; 4] { Margin::same(12.0) } else { margin(bs.padding) });
        let clickable: Vec<bool> = (0..self.it.dom.nodes.len()).map(|n| self.it.dom.attr(n, "onclick").is_some() || self.it.handler(n, "click").is_some()).collect();
        let mut cx = Cx { dom: &mut self.it.dom, rules: &self.rules, events: Vec::new(), hover: self.hovered, new_hover: None, clickable };
        let mut open = self.open;
        let max_w = (ctx.screen_rect().width() - 24.0).max(200.0);
        let width = match bs.width {
            Len::Px(w) => w.min(max_w),
            Len::Pct(p) => ctx.screen_rect().width() * p / 100.0,
            Len::Auto => 340.0f32.min(max_w),
        };
        egui::Window::new(RichText::new(&self.title).font(bold_font(16.0)))
            .id(egui::Id::new(("wolfhtml", &self.name)))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .frame(frame)
            .default_width(width)
            .max_width(max_w)
            .max_height(ctx.screen_rect().height() - 40.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(width);
                egui::ScrollArea::vertical().max_height(ctx.screen_rect().height() - 110.0).show(ui, |ui| {
                    children(ui, &mut cx, body, &bs);
                });
            });
        let events = std::mem::take(&mut cx.events);
        self.hovered = cx.new_hover;
        self.open = open;
        for (n, ev) in events {
            if let Some(code) = self.it.dom.attr(n, &format!("on{ev}")) {
                if let Err(e) = self.it.run(&code) {
                    self.it.log.push(format!("JS: {e}"));
                }
            }
            if let Some(h) = self.it.handler(n, ev) {
                if let Err(e) = self.it.call_value(&h, vec![]) {
                    self.it.log.push(format!("JS: {e}"));
                }
            }
        }
        if self.it.saved_dirty {
            self.it.saved_dirty = false;
            if let Some(p) = &self.save_path {
                let mut lines: Vec<String> = self.it.saved.iter().map(|(k, v)| format!("{k}\t{}", v.replace('\n', "\\n"))).collect();
                lines.sort();
                let _ = std::fs::write(p, lines.join("\n"));
            }
        }
        let fx = std::mem::take(&mut self.it.effects);
        if fx.contains(&Effect::Close) {
            self.open = false;
        }
        fx
    }
}

struct Cx<'a> {
    dom: &'a mut html::Dom,
    rules: &'a [css::Rule],
    events: Vec<(usize, &'static str)>,
    hover: Option<usize>,
    new_hover: Option<usize>,
    clickable: Vec<bool>,
}

fn col(c: Rgba, opacity: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0, c.1, c.2, (c.3 as f32 * opacity) as u8)
}

fn margin(p: [f32; 4]) -> Margin {
    Margin { top: p[0], right: p[1], bottom: p[2], left: p[3] }
}

fn rich(t: &str, s: &Style) -> RichText {
    let mut r = RichText::new(t).color(col(s.color, s.opacity));
    r = if s.bold { r.font(bold_font(s.font_size)) } else { r.size(s.font_size) };
    if s.italic {
        r = r.italics();
    }
    r
}

/// Lays out the children of `n`: runs of inline content flow in wrapped rows, blocks stack;
/// flex containers put every child in a row (or a column) with the gap between.
fn children(ui: &mut Ui, cx: &mut Cx, n: usize, st: &Style) {
    let kids = cx.dom.nodes[n].children.clone();
    let styles: Vec<Style> = kids.iter().map(|k| css::compute(cx.dom, cx.rules, *k, st, cx.hover)).collect();
    if st.display == Display::Flex {
        let cross = match st.align_items {
            Align::Center => egui::Align::Center,
            Align::End => egui::Align::Max,
            _ => egui::Align::Min,
        };
        let main = match st.justify {
            Align::Center => egui::Align::Center,
            Align::End => egui::Align::Max,
            _ => egui::Align::Min,
        };
        let layout = if st.column { egui::Layout::top_down(cross).with_main_align(main) } else { egui::Layout::left_to_right(cross).with_main_align(main).with_main_wrap(st.wrap) };
        ui.with_layout(layout, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(st.gap, st.gap);
            for (k, ks) in kids.iter().zip(&styles) {
                element(ui, cx, *k, ks, st);
            }
        });
        return;
    }
    let centre = st.text_align == Align::Center;
    let mut i = 0;
    while i < kids.len() {
        if styles[i].display == Display::None {
            i += 1;
            continue;
        }
        if styles[i].display == Display::Inline {
            let start = i;
            while i < kids.len() && (styles[i].display == Display::Inline || styles[i].display == Display::None) && cx.dom.nodes[kids[i]].tag != "br" {
                i += 1;
            }
            let row = |ui: &mut Ui, cx: &mut Cx| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for k in start..i {
                    element(ui, cx, kids[k], &styles[k], st);
                }
            };
            if centre {
                ui.vertical_centered(|ui| ui.horizontal_wrapped(|ui| row(ui, cx)));
            } else if st.text_align == Align::End {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.horizontal_wrapped(|ui| row(ui, cx)));
            } else {
                ui.horizontal_wrapped(|ui| row(ui, cx));
            }
            // A <br> ends the row.
            if i < kids.len() && cx.dom.nodes[kids[i]].tag == "br" {
                i += 1;
            }
            continue;
        }
        if centre {
            let (k, ks) = (kids[i], styles[i].clone());
            ui.vertical_centered(|ui| element(ui, cx, k, &ks, st));
        } else {
            element(ui, cx, kids[i], &styles[i], st);
        }
        i += 1;
    }
}

fn element(ui: &mut Ui, cx: &mut Cx, n: usize, st: &Style, parent: &Style) {
    let _ = parent;
    if st.display == Display::None {
        return;
    }
    let tag = cx.dom.nodes[n].tag.clone();
    match tag.as_str() {
        "#text" => {
            let t = cx.dom.nodes[n].text.clone();
            ui.label(rich(&t, st));
            return;
        }
        "hr" => {
            ui.separator();
            return;
        }
        "br" => return,
        "button" => {
            let label = cx.dom.text_of(n);
            let mut b = egui::Button::new(rich(&label, st)).fill(st.background.map(|c| col(c, st.opacity)).unwrap_or(Color32::TRANSPARENT)).rounding(Rounding::same(st.radius));
            if let Some((w, c)) = st.border {
                b = b.stroke(Stroke::new(w, col(c, st.opacity)));
            }
            if let Len::Px(w) = st.width {
                b = b.min_size(egui::vec2(w, if let Len::Px(h) = st.height { h } else { 0.0 }));
            }
            let disabled = cx.dom.attr(n, "disabled").is_some();
            let add = |ui: &mut Ui| ui.add_enabled(!disabled, b);
            let r = if st.margin != [0.0; 4] { Frame::none().outer_margin(margin(st.margin)).show(ui, add).inner } else { add(ui) };
            if r.hovered() {
                cx.new_hover = Some(n);
            }
            if r.clicked() {
                cx.events.push((n, "click"));
            }
            return;
        }
        "input" => {
            let kind = cx.dom.attr(n, "type").unwrap_or_default();
            if kind == "checkbox" {
                let mut on = cx.dom.nodes[n].value == "on" || (cx.dom.nodes[n].value.is_empty() && cx.dom.attr(n, "checked").is_some());
                if ui.checkbox(&mut on, "").changed() {
                    cx.dom.nodes[n].value = if on { "on".into() } else { "off".into() };
                    cx.events.push((n, "change"));
                }
                return;
            }
            let w = match st.width {
                Len::Px(w) => w,
                Len::Pct(p) => ui.available_width() * p / 100.0,
                Len::Auto => 160.0,
            };
            let hint = cx.dom.attr(n, "placeholder").unwrap_or_default();
            let r = ui.add(egui::TextEdit::singleline(&mut cx.dom.nodes[n].value).hint_text(hint).desired_width(w));
            if r.changed() {
                cx.events.push((n, "input"));
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                cx.events.push((n, "change"));
            }
            return;
        }
        "progress" => {
            let v: f32 = cx.dom.attr(n, "value").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let m: f32 = cx.dom.attr(n, "max").and_then(|v| v.parse().ok()).unwrap_or(1.0);
            let w = match st.width {
                Len::Px(w) => w,
                Len::Pct(p) => ui.available_width() * p / 100.0,
                Len::Auto => 160.0,
            };
            let mut bar = egui::ProgressBar::new((v / m.max(0.0001)).clamp(0.0, 1.0)).desired_width(w);
            if let Some(c) = st.background {
                bar = bar.fill(col(c, st.opacity));
            }
            ui.add(bar);
            return;
        }
        "img" => return,
        _ => {}
    }
    // A box: background, border, rounding, padding and margin around its children.
    let mut frame = Frame::none().inner_margin(margin(st.padding)).outer_margin(margin(st.margin)).rounding(Rounding::same(st.radius));
    if let Some(c) = st.background {
        frame = frame.fill(col(c, st.opacity));
    }
    if let Some((w, c)) = st.border {
        frame = frame.stroke(Stroke::new(w, col(c, st.opacity)));
    }
    if st.shadow {
        frame = frame.shadow(egui::epaint::Shadow { offset: egui::vec2(0.0, 3.0), blur: 10.0, spread: 0.0, color: Color32::from_black_alpha(90) });
    }
    let avail = ui.available_width();
    let pad = st.padding[1] + st.padding[3] + st.margin[1] + st.margin[3];
    let block = st.display != Display::Inline;
    let r = frame.show(ui, |ui| {
        match st.width {
            Len::Px(w) => ui.set_width((w - st.padding[1] - st.padding[3]).max(0.0)),
            Len::Pct(p) => ui.set_width((avail * p / 100.0 - pad).max(0.0)),
            // Blocks take the whole row, like in a browser.
            Len::Auto if block && (st.background.is_some() || st.border.is_some()) => ui.set_width((avail - pad).max(0.0)),
            _ => {}
        }
        if let Len::Px(h) = st.height {
            ui.set_min_height((h - st.padding[0] - st.padding[2]).max(0.0));
        }
        if st.min_width > 0.0 {
            ui.set_min_width(st.min_width);
        }
        children(ui, cx, n, st);
    });
    if cx.clickable.get(n).copied().unwrap_or(false) {
        let resp = ui.interact(r.response.rect, egui::Id::new(("wolfhtml-click", n)), Sense::click());
        if resp.hovered() {
            cx.new_hover = Some(n);
        }
        if resp.clicked() {
            cx.events.push((n, "click"));
        }
    } else if r.response.hovered() && cx.new_hover.is_none() {
        // :hover on plain boxes (only the innermost wins, buttons first).
        cx.new_hover = Some(n);
    }
}

/// The page every new world gets in its `gui` folder: a WolfTech-style control panel that
/// shows how pages talk to the game.
pub const EXAMPLE: &str = r#"<!DOCTYPE html>
<html>
<head>
<title>Пульт WolfCraft</title>
<style>
  body { background: #161c28ee; color: #e8ecf5; width: 360px; border-radius: 14px; border: 1px solid #ffffff40; padding: 14px; }
  h2 { color: #7fd4ff; margin: 0 0 6px 0; }
  .card { background: #ffffff14; border: 1px solid #ffffff30; border-radius: 10px; padding: 10px; margin: 6px 0; }
  .row { display: flex; gap: 8px; align-items: center; }
  .stat { font-size: 13px; color: #b9c3d6; }
  .big { font-size: 22px; font-weight: bold; color: gold; }
  button { background: #3a78d8; color: white; border-radius: 8px; padding: 6px 12px; }
  button:hover { background: #5590f0; }
  .warn { background: #c0453a; }
  .ok { color: #7ee07e; }
</style>
</head>
<body>
  <h2>Пульт</h2>
  <div class="card">
    <div class="row"><span class="stat">Здоровье:</span> <progress id="hp" value="20" max="20"></progress> <span id="hpText">20</span></div>
    <div class="row"><span class="stat">Где я:</span> <span id="pos">…</span></div>
    <div class="row"><span class="stat">Время:</span> <span id="time">…</span></div>
  </div>
  <div class="card">
    <div class="row"><span class="stat">Кликов:</span> <span id="clicks" class="big">0</span></div>
    <div class="row">
      <button id="add">+1</button>
      <button onclick="reset()" class="warn">Сброс</button>
      <button onclick="wolf.command('time set день')">☀ День</button>
    </div>
  </div>
  <div class="card">
    <div class="row"><input id="msg" placeholder="Сообщение в чат"> <button id="send">Отправить</button></div>
    <p id="status" class="stat">Окно из HTML + CSS + JS</p>
  </div>
<script>
  let clicks = Number(wolf.load('clicks') || 0);
  const out = document.getElementById('clicks');
  out.textContent = clicks;
  document.getElementById('add').addEventListener('click', () => {
    clicks++;
    out.textContent = clicks;
    wolf.store('clicks', clicks);
    if (clicks % 10 === 0) wolf.chat('Уже ' + clicks + ' кликов!');
  });
  function reset() {
    clicks = 0;
    out.textContent = 0;
    wolf.store('clicks', 0);
  }
  document.getElementById('send').onclick = function () {
    const m = document.getElementById('msg');
    if (m.value.length > 0) {
      wolf.chat(wolf.nick() + ': ' + m.value);
      m.value = '';
      const s = document.getElementById('status');
      s.textContent = 'Отправлено ✓';
      s.classList.add('ok');
    }
  };
  setInterval(() => {
    const p = wolf.pos();
    document.getElementById('pos').textContent = Math.round(p.x) + ', ' + Math.round(p.y) + ', ' + Math.round(p.z);
    document.getElementById('time').textContent = wolf.time();
    const hp = document.getElementById('hp');
    hp.setAttribute('value', wolf.health());
    document.getElementById('hpText').textContent = wolf.health().toFixed(0);
  }, 250);
</script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webgui_example_page_loads_without_errors() {
        let mut p = Page::load("пульт", EXAMPLE, None);
        assert_eq!(p.title, "Пульт WolfCraft");
        assert!(p.take_log().is_empty());
        p.it.host = Host { health: 15.0, max_health: 20.0, pos: [1.4, 2.6, -3.0], time: "12:00".into(), nick: "Волк".into(), ..Default::default() };
        p.it.tick(0.3);
        assert!(p.take_log().is_empty());
        assert_eq!(p.it.dom.text_of(p.it.dom.by_id("pos").unwrap()), "1, 3, -3");
        let add = p.it.dom.by_id("add").unwrap();
        for _ in 0..10 {
            let h = p.it.handler(add, "click").unwrap();
            p.it.call_value(&h, vec![]).unwrap();
        }
        assert_eq!(p.it.dom.text_of(p.it.dom.by_id("clicks").unwrap()), "10");
        assert_eq!(p.it.effects, vec![Effect::Chat("Уже 10 кликов!".into())]);
        p.it.run("reset()").unwrap();
        assert_eq!(p.it.saved.get("clicks").map(String::as_str), Some("0"));
    }

    #[test]
    fn webgui_draws_in_egui() {
        let mut p = Page::load("пульт", EXAMPLE, None);
        let ctx = egui::Context::default();
        crate::theme_manager::install(&ctx);
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                let fx = p.show(ctx, &Host::default(), 0.016);
                assert!(fx.is_empty());
            });
        }
        assert!(p.take_log().is_empty(), "{:?}", p.take_log());
    }
}
