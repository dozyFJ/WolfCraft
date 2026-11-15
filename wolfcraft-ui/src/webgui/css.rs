//! CSS for mod GUIs: rules with simple selectors (`tag`, `.class`, `#id`, `tag.class`,
//! descendant `a b`, `*`, `:hover`), specificity and the cascade, and a computed style with
//! the properties the renderer understands.

use super::html::Dom;

#[derive(Clone, Debug, PartialEq)]
struct Compound {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    hover: bool,
}

#[derive(Clone, Debug)]
pub struct Rule {
    /// Descendant chain, outermost first.
    sel: Vec<Compound>,
    spec: (u32, u32, u32),
    order: usize,
    decls: Vec<(String, String)>,
}

pub fn parse(src: &str) -> Vec<Rule> {
    // Strip comments.
    let mut s = String::new();
    let mut rest = src;
    while let Some(a) = rest.find("/*") {
        s.push_str(&rest[..a]);
        rest = rest[a + 2..].find("*/").map(|b| &rest[a + 2 + b + 2..]).unwrap_or("");
    }
    s.push_str(rest);
    let mut rules = Vec::new();
    let mut rest = s.as_str();
    while let Some(open) = rest.find('{') {
        let head = rest[..open].trim();
        let Some(close) = rest[open..].find('}') else { break };
        let body = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];
        if head.starts_with('@') {
            continue;
        }
        let decls = parse_decls(body);
        for sel in head.split(',') {
            let chain: Vec<Compound> = sel.split_whitespace().filter(|p| *p != ">").map(compound).collect();
            if chain.is_empty() {
                continue;
            }
            let spec = chain.iter().fold((0, 0, 0), |a, c| (a.0 + c.id.is_some() as u32, a.1 + c.classes.len() as u32 + c.hover as u32, a.2 + c.tag.is_some() as u32));
            rules.push(Rule { sel: chain, spec, order: rules.len(), decls: decls.clone() });
        }
    }
    rules
}

pub fn parse_decls(body: &str) -> Vec<(String, String)> {
    body.split(';')
        .filter_map(|d| {
            let (k, v) = d.split_once(':')?;
            let (k, v) = (k.trim().to_lowercase(), v.trim().trim_end_matches("!important").trim().to_string());
            (!k.is_empty() && !v.is_empty()).then_some((k, v))
        })
        .collect()
}

fn compound(s: &str) -> Compound {
    let mut c = Compound { tag: None, id: None, classes: vec![], hover: false };
    let mut s = s.to_string();
    if let Some(x) = s.find(":hover") {
        c.hover = true;
        s.replace_range(x..x + 6, "");
    }
    let mut cur = String::new();
    let mut kind = 't';
    let flush = |c: &mut Compound, kind: char, cur: &mut String| {
        if !cur.is_empty() {
            match kind {
                '#' => c.id = Some(cur.clone()),
                '.' => c.classes.push(cur.clone()),
                _ if cur.as_str() != "*" => c.tag = Some(cur.to_lowercase()),
                _ => {}
            }
        }
        cur.clear();
    };
    for ch in s.chars() {
        if ch == '#' || ch == '.' {
            flush(&mut c, kind, &mut cur);
            kind = ch;
        } else {
            cur.push(ch);
        }
    }
    flush(&mut c, kind, &mut cur);
    c
}

fn matches(dom: &Dom, n: usize, c: &Compound, hovered: Option<usize>) -> bool {
    let node = &dom.nodes[n];
    if node.tag.starts_with('#') {
        return false;
    }
    if c.tag.as_ref().is_some_and(|t| *t != node.tag) {
        return false;
    }
    if c.id.as_ref().is_some_and(|i| dom.attr(n, "id").as_deref() != Some(i.as_str())) {
        return false;
    }
    if !c.classes.is_empty() {
        let cl = dom.classes(n);
        if !c.classes.iter().all(|x| cl.contains(x)) {
            return false;
        }
    }
    if c.hover && hovered != Some(n) {
        return false;
    }
    true
}

fn rule_matches(dom: &Dom, n: usize, r: &Rule, hovered: Option<usize>) -> bool {
    let (last, up) = r.sel.split_last().unwrap();
    if !matches(dom, n, last, hovered) {
        return false;
    }
    // Ancestors, innermost first, must contain the rest of the chain in order.
    let mut need = up.iter().rev().peekable();
    let mut p = dom.nodes[n].parent;
    while let (Some(want), Some(a)) = (need.peek(), p) {
        if matches(dom, a, want, hovered) {
            need.next();
        }
        p = dom.nodes[a].parent;
    }
    need.peek().is_none()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Len {
    Auto,
    Px(f32),
    Pct(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    Block,
    Inline,
    Flex,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Between,
}

/// The style of one element after the cascade (inherited values filled from the parent).
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub color: Rgba,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub text_align: Align,
    pub background: Option<Rgba>,
    pub width: Len,
    pub height: Len,
    pub min_width: f32,
    pub padding: [f32; 4],
    pub margin: [f32; 4],
    pub border: Option<(f32, Rgba)>,
    pub radius: f32,
    pub display: Display,
    pub column: bool,
    pub gap: f32,
    pub justify: Align,
    pub align_items: Align,
    pub opacity: f32,
    pub wrap: bool,
    pub shadow: bool,
}

impl Style {
    pub fn root() -> Style {
        Style {
            color: Rgba(235, 238, 245, 255),
            font_size: 15.0,
            bold: false,
            italic: false,
            text_align: Align::Start,
            background: None,
            width: Len::Auto,
            height: Len::Auto,
            min_width: 0.0,
            padding: [0.0; 4],
            margin: [0.0; 4],
            border: None,
            radius: 0.0,
            display: Display::Block,
            column: false,
            gap: 0.0,
            justify: Align::Start,
            align_items: Align::Start,
            opacity: 1.0,
            wrap: true,
            shadow: false,
        }
    }

    /// What a child starts from: only the inherited properties.
    fn inherit(&self) -> Style {
        Style { color: self.color, font_size: self.font_size, bold: self.bold, italic: self.italic, text_align: self.text_align, ..Style::root() }
    }
}

const INLINE: &[&str] = &["span", "b", "strong", "i", "em", "a", "button", "input", "progress", "img", "label", "small", "code", "#text", "br"];

/// Built-in look of tags (like a browser's default style sheet).
fn defaults(tag: &str, s: &mut Style) {
    s.display = if INLINE.contains(&tag) { Display::Inline } else { Display::Block };
    match tag {
        "h1" => {
            s.font_size = 26.0;
            s.bold = true;
            s.margin = [4.0, 0.0, 6.0, 0.0];
        }
        "h2" => {
            s.font_size = 21.0;
            s.bold = true;
            s.margin = [4.0, 0.0, 4.0, 0.0];
        }
        "h3" => {
            s.font_size = 17.0;
            s.bold = true;
            s.margin = [2.0, 0.0, 2.0, 0.0];
        }
        "p" => s.margin = [2.0, 0.0, 4.0, 0.0],
        "b" | "strong" | "th" => s.bold = true,
        "i" | "em" => s.italic = true,
        "small" => s.font_size *= 0.82,
        "a" => s.color = Rgba(110, 180, 255, 255),
        "head" | "script" | "style" | "title" | "meta" | "link" => s.display = Display::None,
        "button" => {
            s.background = Some(Rgba(70, 120, 210, 255));
            s.color = Rgba(255, 255, 255, 255);
            s.padding = [5.0, 12.0, 5.0, 12.0];
            s.radius = 6.0;
        }
        "ul" | "ol" => s.padding = [0.0, 0.0, 0.0, 16.0],
        _ => {}
    }
}

/// Computes the style of node `n` given its parent's style.
pub fn compute(dom: &Dom, rules: &[Rule], n: usize, parent: &Style, hovered: Option<usize>) -> Style {
    let node = &dom.nodes[n];
    let mut s = parent.inherit();
    defaults(&node.tag, &mut s);
    if node.tag == "#text" {
        return s;
    }
    let mut hits: Vec<&Rule> = rules.iter().filter(|r| rule_matches(dom, n, r, hovered)).collect();
    hits.sort_by_key(|r| (r.spec, r.order));
    for r in hits {
        for (k, v) in &r.decls {
            apply(&mut s, k, v, parent);
        }
    }
    if let Some(inline) = dom.attr(n, "style") {
        for (k, v) in parse_decls(&inline) {
            apply(&mut s, &k, &v, parent);
        }
    }
    for (k, v) in &node.js_style {
        apply(&mut s, k, v, parent);
    }
    if node.hidden {
        s.display = Display::None;
    }
    s
}

fn apply(s: &mut Style, k: &str, v: &str, parent: &Style) {
    let v = v.trim();
    match k {
        "color" => {
            if let Some(c) = color(v) {
                s.color = c;
            }
        }
        "background" | "background-color" => {
            s.background = if v == "none" || v == "transparent" { None } else { color(v.split_whitespace().next().unwrap_or("")).or_else(|| gradient_first(v)).or(s.background) };
        }
        "font-size" => {
            if let Some(px) = px_rel(v, parent.font_size) {
                s.font_size = px.clamp(6.0, 96.0);
            }
        }
        "font-weight" => s.bold = v == "bold" || v.parse::<u32>().is_ok_and(|w| w >= 600),
        "font-style" => s.italic = v == "italic",
        "text-align" => s.text_align = align(v),
        "width" => s.width = len(v),
        "height" => s.height = len(v),
        "min-width" => s.min_width = px_rel(v, s.font_size).unwrap_or(0.0),
        "padding" => s.padding = sides(v, s.font_size),
        "margin" => s.margin = sides(v, s.font_size),
        "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => s.padding[side(k)] = px_rel(v, s.font_size).unwrap_or(0.0),
        "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => s.margin[side(k)] = px_rel(v, s.font_size).unwrap_or(0.0),
        "border" => {
            if v == "none" || v == "0" {
                s.border = None;
            } else {
                let w = v.split_whitespace().find_map(|p| px_rel(p, s.font_size)).unwrap_or(1.0);
                let c = v.split_whitespace().find_map(color).unwrap_or(Rgba(128, 128, 128, 255));
                s.border = Some((w, c));
            }
        }
        "border-color" => {
            if let (Some((w, _)), Some(c)) = (s.border, color(v)) {
                s.border = Some((w, c));
            }
        }
        "border-radius" => s.radius = px_rel(v.split_whitespace().next().unwrap_or("0"), s.font_size).unwrap_or(0.0),
        "display" => {
            s.display = match v {
                "none" => Display::None,
                "flex" | "inline-flex" => Display::Flex,
                "inline" | "inline-block" => Display::Inline,
                _ => Display::Block,
            }
        }
        "flex-direction" => s.column = v.starts_with("column"),
        "flex-wrap" => s.wrap = v != "nowrap",
        "gap" => s.gap = px_rel(v.split_whitespace().next().unwrap_or("0"), s.font_size).unwrap_or(0.0),
        "justify-content" => s.justify = align(v),
        "align-items" => s.align_items = align(v),
        "opacity" => s.opacity = v.parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0),
        "box-shadow" => s.shadow = v != "none",
        "visibility" if v == "hidden" => s.opacity = 0.0,
        _ => {}
    }
}

fn side(k: &str) -> usize {
    if k.ends_with("top") {
        0
    } else if k.ends_with("right") {
        1
    } else if k.ends_with("bottom") {
        2
    } else {
        3
    }
}

fn align(v: &str) -> Align {
    match v {
        "center" => Align::Center,
        "right" | "end" | "flex-end" => Align::End,
        "space-between" | "space-around" | "space-evenly" => Align::Between,
        _ => Align::Start,
    }
}

fn len(v: &str) -> Len {
    if let Some(p) = v.strip_suffix('%') {
        return p.trim().parse().map(Len::Pct).unwrap_or(Len::Auto);
    }
    px_rel(v, 16.0).map(Len::Px).unwrap_or(Len::Auto)
}

/// `12px`, `1.5em`, `2rem`, `0`, bare numbers.
fn px_rel(v: &str, em: f32) -> Option<f32> {
    let v = v.trim();
    if let Some(n) = v.strip_suffix("px") {
        return n.trim().parse().ok();
    }
    if let Some(n) = v.strip_suffix("rem") {
        return n.trim().parse::<f32>().ok().map(|x| x * 16.0);
    }
    if let Some(n) = v.strip_suffix("em") {
        return n.trim().parse::<f32>().ok().map(|x| x * em);
    }
    v.parse().ok()
}

fn sides(v: &str, em: f32) -> [f32; 4] {
    let p: Vec<f32> = v.split_whitespace().map(|x| px_rel(x, em).unwrap_or(0.0)).collect();
    match p.len() {
        1 => [p[0]; 4],
        2 => [p[0], p[1], p[0], p[1]],
        3 => [p[0], p[1], p[2], p[1]],
        n if n >= 4 => [p[0], p[1], p[2], p[3]],
        _ => [0.0; 4],
    }
}

fn gradient_first(v: &str) -> Option<Rgba> {
    let inner = v.split_once('(')?.1;
    inner.split(',').find_map(|p| color(p.split_whitespace().next().unwrap_or("")).or_else(|| color(p.trim().trim_end_matches(')'))))
}

pub fn color(v: &str) -> Option<Rgba> {
    let v = v.trim().to_lowercase();
    if let Some(h) = v.strip_prefix('#') {
        let d = |s: &str| u8::from_str_radix(s, 16).ok();
        return match h.len() {
            3 | 4 => {
                let c: Vec<u8> = h.chars().filter_map(|c| d(&c.to_string()).map(|x| x * 17)).collect();
                (c.len() == h.len()).then(|| Rgba(c[0], c[1], c[2], *c.get(3).unwrap_or(&255)))
            }
            6 | 8 => Some(Rgba(d(&h[0..2])?, d(&h[2..4])?, d(&h[4..6])?, if h.len() == 8 { d(&h[6..8])? } else { 255 })),
            _ => None,
        };
    }
    if let Some(args) = v.strip_prefix("rgba(").or_else(|| v.strip_prefix("rgb(")) {
        let n: Vec<f32> = args.trim_end_matches(')').split([',', ' ', '/']).filter(|x| !x.is_empty()).filter_map(|x| x.trim().parse().ok()).collect();
        if n.len() >= 3 {
            let a = n.get(3).map(|a| if *a <= 1.0 { a * 255.0 } else { *a }).unwrap_or(255.0);
            return Some(Rgba(n[0] as u8, n[1] as u8, n[2] as u8, a as u8));
        }
        return None;
    }
    Some(match v.as_str() {
        "white" => Rgba(255, 255, 255, 255),
        "black" => Rgba(0, 0, 0, 255),
        "red" => Rgba(230, 60, 50, 255),
        "green" => Rgba(40, 170, 70, 255),
        "lime" => Rgba(120, 230, 70, 255),
        "blue" => Rgba(50, 110, 230, 255),
        "navy" => Rgba(20, 30, 100, 255),
        "yellow" => Rgba(250, 220, 60, 255),
        "gold" => Rgba(255, 200, 40, 255),
        "orange" => Rgba(250, 150, 40, 255),
        "purple" => Rgba(150, 70, 200, 255),
        "pink" => Rgba(250, 140, 190, 255),
        "brown" => Rgba(140, 90, 50, 255),
        "gray" | "grey" => Rgba(128, 128, 128, 255),
        "silver" => Rgba(192, 192, 192, 255),
        "cyan" | "aqua" => Rgba(60, 220, 240, 255),
        "teal" => Rgba(30, 140, 140, 255),
        "magenta" | "fuchsia" => Rgba(230, 60, 220, 255),
        "transparent" => Rgba(0, 0, 0, 0),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_cascade_by_specificity_inline_and_inheritance() {
        let dom = Dom::parse("<div class='card' id='c'><p class='big'>Текст</p><button>OK</button></div>");
        let rules = parse("/* тема */ .card { background: #20304080; padding: 8px 12px; color: gold } p { font-size: 12px } .card .big { font-size: 1.5em; } #c { border: 2px solid white; border-radius: 10px }");
        let root = Style::root();
        let c = dom.by_id("c").unwrap();
        let card = compute(&dom, &rules, c, &root, None);
        assert_eq!(card.background, Some(Rgba(0x20, 0x30, 0x40, 0x80)));
        assert_eq!(card.padding, [8.0, 12.0, 8.0, 12.0]);
        assert_eq!(card.border, Some((2.0, Rgba(255, 255, 255, 255))));
        let p = dom.query("p").unwrap();
        let ps = compute(&dom, &rules, p, &card, None);
        assert_eq!(ps.font_size, 22.5);
        assert_eq!(ps.color, Rgba(255, 200, 40, 255));
        assert_eq!(compute(&dom, &rules, dom.query("button").unwrap(), &card, None).display, Display::Inline);
        assert_eq!(color("rgba(10, 20, 30, 0.5)"), Some(Rgba(10, 20, 30, 127)));
    }
}
