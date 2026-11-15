//! A small JavaScript interpreter for mod GUIs (WolfHTML). It covers what UI scripts need:
//! `let/const/var`, functions and arrow functions (closures), `if/else`, `while`, `for`,
//! objects and arrays, strings and numbers with the usual operators, `+=` and `++`,
//! `?:`, `&&`/`||`, template-free string concatenation, `typeof`, and a host API
//! (`document`, `console`, `Math`, `wolf`, timers). Every run has a step budget, so an
//! endless loop stops with an error instead of freezing the game.

use std::collections::HashMap;
use std::rc::Rc;

use super::html::Dom;

// ---------------------------------------------------------------- tokens

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Id(String),
    P(&'static str),
    Eof,
}

const PUNCT: &[&str] = &[
    "===", "!==", "**", "=>", "==", "!=", "<=", ">=", "&&", "||", "++", "--", "+=", "-=", "*=", "/=", "%=", "?.", "??", "{", "}", "(", ")", "[", "]", ";", ",", ".", "+", "-", "*", "/", "%", "<",
    ">", "=", "!", "?", ":",
];

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if ch.is_ascii_digit() || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let s = i;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '.') {
                i += 1;
            }
            let t: String = c[s..i].iter().collect();
            let v = if let Some(h) = t.strip_prefix("0x") { i64::from_str_radix(h, 16).map(|v| v as f64).ok() } else { t.parse().ok() };
            out.push(Tok::Num(v.ok_or(format!("неверное число {t}"))?));
        } else if ch == '"' || ch == '\'' || ch == '`' {
            let q = ch;
            i += 1;
            let mut s = String::new();
            while i < c.len() && c[i] != q {
                if c[i] == '\\' && i + 1 < c.len() {
                    i += 1;
                    s.push(match c[i] {
                        'n' => '\n',
                        't' => '\t',
                        x => x,
                    });
                } else {
                    s.push(c[i]);
                }
                i += 1;
            }
            if i >= c.len() {
                return Err("строка не закрыта кавычкой".into());
            }
            i += 1;
            out.push(Tok::Str(s));
        } else if ch.is_alphabetic() || ch == '_' || ch == '$' {
            let s = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '$') {
                i += 1;
            }
            out.push(Tok::Id(c[s..i].iter().collect()));
        } else {
            let rest: String = c[i..(i + 3).min(c.len())].iter().collect();
            let p = PUNCT.iter().find(|p| rest.starts_with(**p)).ok_or(format!("непонятный символ «{ch}»"))?;
            i += p.chars().count();
            out.push(Tok::P(p));
        }
    }
    out.push(Tok::Eof);
    Ok(out)
}

// ---------------------------------------------------------------- syntax

#[derive(Debug)]
pub struct FuncDef {
    params: Vec<String>,
    body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Str(Rc<str>),
    Bool(bool),
    Null,
    Undef,
    Id(String),
    Array(Vec<Expr>),
    Object(Vec<(String, Expr)>),
    Func(Rc<FuncDef>),
    Unary(&'static str, Box<Expr>),
    Bin(&'static str, Box<Expr>, Box<Expr>),
    Assign(&'static str, Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    New(Box<Expr>, Vec<Expr>),
    Member(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Update(bool, &'static str, Box<Expr>),
}

#[derive(Debug, Clone)]
enum Stmt {
    Var(Vec<(String, Option<Expr>)>),
    Expr(Expr),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    For(Option<Box<Stmt>>, Option<Expr>, Option<Expr>, Box<Stmt>),
    ForOf(String, Expr, Box<Stmt>),
    Block(Vec<Stmt>),
    Return(Option<Expr>),
    Break,
    Continue,
    Func(String, Rc<FuncDef>),
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.t[self.i]
    }
    fn at(&self, k: usize) -> &Tok {
        &self.t[(self.i + k).min(self.t.len() - 1)]
    }
    fn next(&mut self) -> Tok {
        let t = self.t[self.i].clone();
        if self.i + 1 < self.t.len() {
            self.i += 1;
        }
        t
    }
    fn is(&self, p: &str) -> bool {
        matches!(self.peek(), Tok::P(x) if *x == p)
    }
    fn is_kw(&self, k: &str) -> bool {
        matches!(self.peek(), Tok::Id(x) if x == k)
    }
    fn eat(&mut self, p: &str) -> bool {
        if self.is(p) {
            self.next();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, p: &str) -> Result<(), String> {
        if self.eat(p) { Ok(()) } else { Err(format!("ожидалось «{p}», а здесь {}", show(self.peek()))) }
    }
    fn ident(&mut self) -> Result<String, String> {
        match self.next() {
            Tok::Id(s) => Ok(s),
            t => Err(format!("ожидалось имя, а здесь {}", show(&t))),
        }
    }
    fn semi(&mut self) {
        self.eat(";");
    }

    fn program(&mut self) -> Result<Vec<Stmt>, String> {
        let mut v = Vec::new();
        while *self.peek() != Tok::Eof {
            v.push(self.stmt()?);
        }
        Ok(v)
    }

    fn block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect("{")?;
        let mut v = Vec::new();
        while !self.is("}") {
            if *self.peek() == Tok::Eof {
                return Err("не закрыта «}»".into());
            }
            v.push(self.stmt()?);
        }
        self.next();
        Ok(v)
    }

    fn stmt(&mut self) -> Result<Stmt, String> {
        if self.is("{") {
            return Ok(Stmt::Block(self.block()?));
        }
        if self.eat(";") {
            return Ok(Stmt::Block(vec![]));
        }
        if let Tok::Id(k) = self.peek().clone() {
            match k.as_str() {
                "let" | "const" | "var" => {
                    self.next();
                    let v = self.decl()?;
                    self.semi();
                    return Ok(v);
                }
                "function" if matches!(self.at(1), Tok::Id(_)) => {
                    self.next();
                    let name = self.ident()?;
                    let f = self.func_rest()?;
                    return Ok(Stmt::Func(name, f));
                }
                "if" => {
                    self.next();
                    self.expect("(")?;
                    let c = self.expr()?;
                    self.expect(")")?;
                    let a = Box::new(self.stmt()?);
                    let b = if self.is_kw("else") {
                        self.next();
                        Some(Box::new(self.stmt()?))
                    } else {
                        None
                    };
                    return Ok(Stmt::If(c, a, b));
                }
                "while" => {
                    self.next();
                    self.expect("(")?;
                    let c = self.expr()?;
                    self.expect(")")?;
                    return Ok(Stmt::While(c, Box::new(self.stmt()?)));
                }
                "for" => {
                    self.next();
                    self.expect("(")?;
                    // for (const x of list)
                    if matches!(self.peek(), Tok::Id(k) if k == "let" || k == "const" || k == "var") && matches!(self.at(2), Tok::Id(o) if o == "of") {
                        self.next();
                        let name = self.ident()?;
                        self.next();
                        let list = self.expr()?;
                        self.expect(")")?;
                        return Ok(Stmt::ForOf(name, list, Box::new(self.stmt()?)));
                    }
                    let init = if self.is(";") {
                        None
                    } else if matches!(self.peek(), Tok::Id(k) if k == "let" || k == "const" || k == "var") {
                        self.next();
                        Some(Box::new(self.decl()?))
                    } else {
                        Some(Box::new(Stmt::Expr(self.expr()?)))
                    };
                    self.expect(";")?;
                    let cond = if self.is(";") { None } else { Some(self.expr()?) };
                    self.expect(";")?;
                    let step = if self.is(")") { None } else { Some(self.expr()?) };
                    self.expect(")")?;
                    return Ok(Stmt::For(init, cond, step, Box::new(self.stmt()?)));
                }
                "return" => {
                    self.next();
                    let v = if self.is(";") || self.is("}") { None } else { Some(self.expr()?) };
                    self.semi();
                    return Ok(Stmt::Return(v));
                }
                "break" => {
                    self.next();
                    self.semi();
                    return Ok(Stmt::Break);
                }
                "continue" => {
                    self.next();
                    self.semi();
                    return Ok(Stmt::Continue);
                }
                _ => {}
            }
        }
        let e = self.expr()?;
        self.semi();
        Ok(Stmt::Expr(e))
    }

    fn decl(&mut self) -> Result<Stmt, String> {
        let mut v = Vec::new();
        loop {
            let n = self.ident()?;
            let init = if self.eat("=") { Some(self.assign()?) } else { None };
            v.push((n, init));
            if !self.eat(",") {
                break;
            }
        }
        Ok(Stmt::Var(v))
    }

    fn func_rest(&mut self) -> Result<Rc<FuncDef>, String> {
        self.expect("(")?;
        let mut params = Vec::new();
        while !self.eat(")") {
            params.push(self.ident()?);
            self.eat(",");
        }
        let body = self.block()?;
        Ok(Rc::new(FuncDef { params, body }))
    }

    fn expr(&mut self) -> Result<Expr, String> {
        let mut e = self.assign()?;
        while self.eat(",") {
            let r = self.assign()?;
            e = Expr::Bin(",", Box::new(e), Box::new(r));
        }
        Ok(e)
    }

    /// `(a, b) => …` / `x => …` ahead?
    fn arrow_ahead(&self) -> bool {
        if matches!(self.peek(), Tok::Id(_)) && matches!(self.at(1), Tok::P("=>")) {
            return true;
        }
        if !self.is("(") {
            return false;
        }
        let mut depth = 0;
        let mut k = 0;
        loop {
            match self.at(k) {
                Tok::P("(") => depth += 1,
                Tok::P(")") => {
                    depth -= 1;
                    if depth == 0 {
                        return matches!(self.at(k + 1), Tok::P("=>"));
                    }
                }
                Tok::Eof => return false,
                _ => {}
            }
            k += 1;
        }
    }

    fn arrow(&mut self) -> Result<Expr, String> {
        let mut params = Vec::new();
        if self.eat("(") {
            while !self.eat(")") {
                params.push(self.ident()?);
                self.eat(",");
            }
        } else {
            params.push(self.ident()?);
        }
        self.expect("=>")?;
        let body = if self.is("{") { self.block()? } else { vec![Stmt::Return(Some(self.assign()?))] };
        Ok(Expr::Func(Rc::new(FuncDef { params, body })))
    }

    fn assign(&mut self) -> Result<Expr, String> {
        if self.arrow_ahead() {
            return self.arrow();
        }
        let l = self.cond()?;
        for op in ["=", "+=", "-=", "*=", "/=", "%="] {
            if self.eat(op) {
                if !matches!(l, Expr::Id(_) | Expr::Member(..) | Expr::Index(..)) {
                    return Err("слева от «=» должно быть имя или поле".into());
                }
                let r = self.assign()?;
                return Ok(Expr::Assign(op, Box::new(l), Box::new(r)));
            }
        }
        Ok(l)
    }

    fn cond(&mut self) -> Result<Expr, String> {
        let c = self.bin(0)?;
        if self.eat("?") {
            let a = self.assign()?;
            self.expect(":")?;
            let b = self.assign()?;
            return Ok(Expr::Cond(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
    }

    fn bin(&mut self, level: usize) -> Result<Expr, String> {
        const LEVELS: &[&[&str]] = &[&["??"], &["||"], &["&&"], &["===", "!==", "==", "!="], &["<=", ">=", "<", ">"], &["+", "-"], &["*", "/", "%"], &["**"]];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut l = self.bin(level + 1)?;
        loop {
            let Some(op) = LEVELS[level].iter().find(|o| self.is(o)).copied() else { break };
            self.next();
            let r = self.bin(level + 1)?;
            l = Expr::Bin(op, Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        for op in ["!", "-", "+"] {
            if self.eat(op) {
                return Ok(Expr::Unary(op, Box::new(self.unary()?)));
            }
        }
        for op in ["++", "--"] {
            if self.eat(op) {
                return Ok(Expr::Update(true, op, Box::new(self.unary()?)));
            }
        }
        if self.is_kw("typeof") {
            self.next();
            return Ok(Expr::Unary("typeof", Box::new(self.unary()?)));
        }
        let e = self.postfix()?;
        for op in ["++", "--"] {
            if self.eat(op) {
                return Ok(Expr::Update(false, op, Box::new(e)));
            }
        }
        Ok(e)
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.primary()?;
        loop {
            if self.eat(".") || self.eat("?.") {
                e = Expr::Member(Box::new(e), self.ident()?);
            } else if self.eat("[") {
                let k = self.expr()?;
                self.expect("]")?;
                e = Expr::Index(Box::new(e), Box::new(k));
            } else if self.is("(") {
                let args = self.args()?;
                e = Expr::Call(Box::new(e), args);
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn args(&mut self) -> Result<Vec<Expr>, String> {
        self.expect("(")?;
        let mut v = Vec::new();
        while !self.eat(")") {
            v.push(self.assign()?);
            if !self.is(")") {
                self.expect(",")?;
            }
        }
        Ok(v)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Str(s) => Ok(Expr::Str(s.into())),
            Tok::P("(") => {
                let e = self.expr()?;
                self.expect(")")?;
                Ok(e)
            }
            Tok::P("[") => {
                let mut v = Vec::new();
                while !self.eat("]") {
                    v.push(self.assign()?);
                    if !self.is("]") {
                        self.expect(",")?;
                    }
                }
                Ok(Expr::Array(v))
            }
            Tok::P("{") => {
                let mut v = Vec::new();
                while !self.eat("}") {
                    let k = match self.next() {
                        Tok::Id(s) | Tok::Str(s) => s,
                        Tok::Num(n) => fmt_num(n),
                        t => return Err(format!("ожидалось имя поля, а здесь {}", show(&t))),
                    };
                    let val = if self.eat(":") { self.assign()? } else { Expr::Id(k.clone()) };
                    v.push((k, val));
                    if !self.is("}") {
                        self.expect(",")?;
                    }
                }
                Ok(Expr::Object(v))
            }
            Tok::Id(k) => Ok(match k.as_str() {
                "true" => Expr::Bool(true),
                "false" => Expr::Bool(false),
                "null" => Expr::Null,
                "undefined" => Expr::Undef,
                "function" => {
                    if matches!(self.peek(), Tok::Id(_)) {
                        self.next();
                    }
                    Expr::Func(self.func_rest()?)
                }
                "new" => {
                    let c = self.primary()?;
                    let args = if self.is("(") { self.args()? } else { vec![] };
                    Expr::New(Box::new(c), args)
                }
                _ => Expr::Id(k),
            }),
            t => Err(format!("неожиданно {}", show(&t))),
        }
    }
}

fn show(t: &Tok) -> String {
    match t {
        Tok::Num(n) => fmt_num(*n),
        Tok::Str(s) => format!("\"{s}\""),
        Tok::Id(s) => format!("«{s}»"),
        Tok::P(p) => format!("«{p}»"),
        Tok::Eof => "конец скрипта".into(),
    }
}

// ---------------------------------------------------------------- values

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Undef,
    Null,
    Bool(bool),
    Num(f64),
    Str(Rc<str>),
    /// Heap object (plain object, array, function, DOM element…).
    Obj(usize),
    /// A host function, bound to `this` when taken from an object.
    Native(Native, Option<Box<Value>>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    GetById,
    QuerySelector,
    Log,
    Floor,
    Round,
    Ceil,
    Abs,
    Min,
    Max,
    Random,
    Sqrt,
    ToFixed,
    ToStr,
    Number,
    ParseInt,
    Push,
    Join,
    Includes,
    IndexOf,
    ToUpper,
    ToLower,
    SetTimeout,
    SetInterval,
    ClearTimer,
    AddListener,
    SetAttr,
    GetAttr,
    Chat,
    Command,
    Close,
    Health,
    Hunger,
    Pos,
    Time,
    Nick,
    Creative,
    Store,
    Load,
}

enum Kind {
    Plain,
    Array(Vec<Value>),
    Func(Rc<FuncDef>, usize),
    Elem(usize),
    Style(usize),
    ClassList(usize),
}

struct Obj {
    props: Vec<(String, Value)>,
    kind: Kind,
}

struct Env {
    vars: HashMap<String, Value>,
    parent: Option<usize>,
}

enum Flow {
    Normal,
    Ret(Value),
    Break,
    Continue,
}

/// The world as the script sees it (filled by the game every frame).
#[derive(Clone, Debug, Default)]
pub struct Host {
    pub health: f32,
    pub max_health: f32,
    pub hunger: f32,
    pub pos: [f64; 3],
    pub time: String,
    pub nick: String,
    pub creative: bool,
}

/// What a script asked the game to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Chat(String),
    Command(String),
    Close,
}

struct Timer {
    id: u32,
    due: f64,
    every: Option<f64>,
    f: Value,
}

pub struct Interp {
    heap: Vec<Obj>,
    envs: Vec<Env>,
    pub dom: Dom,
    pub host: Host,
    pub effects: Vec<Effect>,
    pub log: Vec<String>,
    /// Values kept with `wolf.store` (saved by the game between sessions).
    pub saved: HashMap<String, String>,
    /// `wolf.store` changed something since the game last saved.
    pub saved_dirty: bool,
    steps: u64,
    timers: Vec<Timer>,
    next_timer: u32,
    clock: f64,
    seed: u64,
}

const BUDGET: u64 = 400_000;

fn fmt_num(n: f64) -> String {
    if n.is_nan() {
        "NaN".into()
    } else if n.is_infinite() {
        if n > 0.0 { "Infinity".into() } else { "-Infinity".into() }
    } else if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{n}");
        s
    }
}

impl Interp {
    pub fn new(dom: Dom) -> Self {
        let mut it = Interp { heap: Vec::new(), envs: vec![Env { vars: HashMap::new(), parent: None }], dom, host: Host::default(), effects: Vec::new(), log: Vec::new(), saved: HashMap::new(), saved_dirty: false, steps: 0, timers: Vec::new(), next_timer: 1, clock: 0.0, seed: 0x9E37_79B9_7F4A_7C15 };
        let n = |f| Value::Native(f, None);
        let document = it.object(vec![("getElementById", n(Native::GetById)), ("querySelector", n(Native::QuerySelector))]);
        let console = it.object(vec![("log", n(Native::Log)), ("error", n(Native::Log)), ("warn", n(Native::Log))]);
        let math = it.object(vec![
            ("floor", n(Native::Floor)),
            ("round", n(Native::Round)),
            ("ceil", n(Native::Ceil)),
            ("abs", n(Native::Abs)),
            ("min", n(Native::Min)),
            ("max", n(Native::Max)),
            ("random", n(Native::Random)),
            ("sqrt", n(Native::Sqrt)),
            ("PI", Value::Num(std::f64::consts::PI)),
        ]);
        let wolf = it.object(vec![
            ("chat", n(Native::Chat)),
            ("command", n(Native::Command)),
            ("close", n(Native::Close)),
            ("health", n(Native::Health)),
            ("hunger", n(Native::Hunger)),
            ("pos", n(Native::Pos)),
            ("time", n(Native::Time)),
            ("nick", n(Native::Nick)),
            ("creative", n(Native::Creative)),
            ("store", n(Native::Store)),
            ("load", n(Native::Load)),
        ]);
        for (k, v) in [
            ("document", document),
            ("console", console),
            ("Math", math),
            ("wolf", wolf),
            ("setTimeout", n(Native::SetTimeout)),
            ("setInterval", n(Native::SetInterval)),
            ("clearTimeout", n(Native::ClearTimer)),
            ("clearInterval", n(Native::ClearTimer)),
            ("String", n(Native::ToStr)),
            ("Number", n(Native::Number)),
            ("parseInt", n(Native::ParseInt)),
            ("parseFloat", n(Native::Number)),
        ] {
            it.envs[0].vars.insert(k.into(), v);
        }
        it
    }

    fn object(&mut self, props: Vec<(&str, Value)>) -> Value {
        self.heap.push(Obj { props: props.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), kind: Kind::Plain });
        Value::Obj(self.heap.len() - 1)
    }

    fn alloc(&mut self, kind: Kind) -> Value {
        self.heap.push(Obj { props: Vec::new(), kind });
        Value::Obj(self.heap.len() - 1)
    }

    fn elem(&mut self, node: usize) -> Value {
        // One handle per node, so `a === b` works and handlers stick.
        if let Some(i) = self.heap.iter().position(|o| matches!(o.kind, Kind::Elem(n) if n == node)) {
            return Value::Obj(i);
        }
        self.alloc(Kind::Elem(node))
    }

    /// Runs a whole script (a `<script>` block or an `onclick` attribute).
    pub fn run(&mut self, src: &str) -> Result<(), String> {
        let prog = Parser { t: lex(src)?, i: 0 }.program()?;
        self.steps = 0;
        self.hoist(&prog, 0);
        match self.block(&prog, 0)? {
            Flow::Ret(_) | Flow::Normal | Flow::Break | Flow::Continue => Ok(()),
        }
    }

    /// Calls a function value with no arguments (event handlers, timers).
    pub fn call_value(&mut self, f: &Value, args: Vec<Value>) -> Result<Value, String> {
        self.steps = 0;
        self.call(f.clone(), None, args)
    }

    /// The handler a script set on a node with `el.onclick = …` or `addEventListener`.
    pub fn handler(&self, node: usize, event: &str) -> Option<Value> {
        let o = self.heap.iter().find(|o| matches!(o.kind, Kind::Elem(n) if n == node))?;
        let key = format!("on{event}");
        o.props.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()).filter(|v| !matches!(v, Value::Undef | Value::Null))
    }

    /// Advances the clock and fires due timers. Errors go to the log.
    pub fn tick(&mut self, dt: f64) {
        self.clock += dt;
        let mut guard = 0;
        while let Some(i) = self.timers.iter().position(|t| t.due <= self.clock) {
            guard += 1;
            if guard > 64 {
                break;
            }
            let f = self.timers[i].f.clone();
            match self.timers[i].every {
                Some(e) => self.timers[i].due = self.clock + e.max(0.016),
                None => {
                    self.timers.remove(i);
                }
            }
            if let Err(e) = self.call_value(&f, vec![]) {
                self.log.push(format!("JS: {e}"));
                if let Some(i) = self.timers.iter().position(|t| t.f == f) {
                    self.timers.remove(i);
                }
            }
        }
    }

    fn step(&mut self) -> Result<(), String> {
        self.steps += 1;
        if self.steps > BUDGET { Err("скрипт работает слишком долго (бесконечный цикл?) и остановлен".into()) } else { Ok(()) }
    }

    fn hoist(&mut self, prog: &[Stmt], env: usize) {
        for s in prog {
            if let Stmt::Func(name, f) = s {
                let v = self.alloc(Kind::Func(f.clone(), env));
                self.envs[env].vars.insert(name.clone(), v);
            }
        }
    }

    fn block(&mut self, prog: &[Stmt], env: usize) -> Result<Flow, String> {
        for s in prog {
            match self.stmt(s, env)? {
                Flow::Normal => {}
                f => return Ok(f),
            }
        }
        Ok(Flow::Normal)
    }

    fn child(&mut self, parent: usize) -> usize {
        self.envs.push(Env { vars: HashMap::new(), parent: Some(parent) });
        self.envs.len() - 1
    }

    fn stmt(&mut self, s: &Stmt, env: usize) -> Result<Flow, String> {
        self.step()?;
        match s {
            Stmt::Var(list) => {
                for (n, e) in list {
                    let v = match e {
                        Some(e) => self.eval(e, env)?,
                        None => Value::Undef,
                    };
                    self.envs[env].vars.insert(n.clone(), v);
                }
            }
            Stmt::Expr(e) => {
                self.eval(e, env)?;
            }
            Stmt::Func(..) => {}
            Stmt::If(c, a, b) => {
                if truthy(&self.eval(c, env)?) {
                    return self.stmt(a, env);
                } else if let Some(b) = b {
                    return self.stmt(b, env);
                }
            }
            Stmt::While(c, body) => {
                while truthy(&self.eval(c, env)?) {
                    match self.stmt(body, env)? {
                        Flow::Break => break,
                        Flow::Ret(v) => return Ok(Flow::Ret(v)),
                        _ => {}
                    }
                }
            }
            Stmt::For(init, cond, step, body) => {
                let e = self.child(env);
                if let Some(i) = init {
                    self.stmt(i, e)?;
                }
                loop {
                    if let Some(c) = cond {
                        if !truthy(&self.eval(c, e)?) {
                            break;
                        }
                    }
                    // A fresh scope per pass, so closures keep that pass's variables.
                    let pass = self.child(e);
                    match self.stmt(body, pass)? {
                        Flow::Break => break,
                        Flow::Ret(v) => return Ok(Flow::Ret(v)),
                        _ => {}
                    }
                    if let Some(st) = step {
                        self.eval(st, e)?;
                    }
                }
            }
            Stmt::ForOf(name, list, body) => {
                let l = self.eval(list, env)?;
                let items = match &l {
                    Value::Obj(i) => match &self.heap[*i].kind {
                        Kind::Array(v) => v.clone(),
                        _ => vec![],
                    },
                    Value::Str(s) => s.chars().map(|c| Value::Str(c.to_string().into())).collect(),
                    _ => vec![],
                };
                for it in items {
                    let pass = self.child(env);
                    self.envs[pass].vars.insert(name.clone(), it);
                    match self.stmt(body, pass)? {
                        Flow::Break => break,
                        Flow::Ret(v) => return Ok(Flow::Ret(v)),
                        _ => {}
                    }
                }
            }
            Stmt::Block(b) => {
                let e = self.child(env);
                self.hoist(b, e);
                return self.block(b, e);
            }
            Stmt::Return(e) => {
                let v = match e {
                    Some(e) => self.eval(e, env)?,
                    None => Value::Undef,
                };
                return Ok(Flow::Ret(v));
            }
            Stmt::Break => return Ok(Flow::Break),
            Stmt::Continue => return Ok(Flow::Continue),
        }
        Ok(Flow::Normal)
    }

    fn lookup(&self, name: &str, mut env: usize) -> Option<usize> {
        loop {
            if self.envs[env].vars.contains_key(name) {
                return Some(env);
            }
            env = self.envs[env].parent?;
        }
    }

    fn eval(&mut self, e: &Expr, env: usize) -> Result<Value, String> {
        self.step()?;
        Ok(match e {
            Expr::Num(n) => Value::Num(*n),
            Expr::Str(s) => Value::Str(s.clone()),
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Null => Value::Null,
            Expr::Undef => Value::Undef,
            Expr::Id(n) => match self.lookup(n, env) {
                Some(e) => self.envs[e].vars[n].clone(),
                None => return Err(format!("«{n}» не определено")),
            },
            Expr::Array(items) => {
                let mut v = Vec::with_capacity(items.len());
                for i in items {
                    v.push(self.eval(i, env)?);
                }
                self.alloc(Kind::Array(v))
            }
            Expr::Object(props) => {
                let o = self.alloc(Kind::Plain);
                for (k, e) in props {
                    let v = self.eval(e, env)?;
                    self.set(&o, k, v)?;
                }
                o
            }
            Expr::Func(f) => self.alloc(Kind::Func(f.clone(), env)),
            Expr::Unary(op, a) => {
                let v = self.eval(a, env)?;
                match *op {
                    "!" => Value::Bool(!truthy(&v)),
                    "-" => Value::Num(-self.num(&v)),
                    "+" => Value::Num(self.num(&v)),
                    _ => Value::Str(self.type_of(&v).into()),
                }
            }
            Expr::Bin(op, a, b) => match *op {
                "&&" => {
                    let l = self.eval(a, env)?;
                    if truthy(&l) { self.eval(b, env)? } else { l }
                }
                "||" => {
                    let l = self.eval(a, env)?;
                    if truthy(&l) { l } else { self.eval(b, env)? }
                }
                "??" => {
                    let l = self.eval(a, env)?;
                    if matches!(l, Value::Undef | Value::Null) { self.eval(b, env)? } else { l }
                }
                "," => {
                    self.eval(a, env)?;
                    self.eval(b, env)?
                }
                _ => {
                    let l = self.eval(a, env)?;
                    let r = self.eval(b, env)?;
                    self.binop(op, &l, &r)
                }
            },
            Expr::Assign(op, target, value) => {
                let mut v = self.eval(value, env)?;
                if *op != "=" {
                    let old = self.eval(target, env)?;
                    v = self.binop(&op[..1], &old, &v);
                }
                self.store(target, v.clone(), env)?;
                v
            }
            Expr::Update(prefix, op, target) => {
                let old = self.eval(target, env)?;
                let n = self.num(&old);
                let new = Value::Num(if *op == "++" { n + 1.0 } else { n - 1.0 });
                self.store(target, new.clone(), env)?;
                if *prefix { new } else { Value::Num(n) }
            }
            Expr::Cond(c, a, b) => {
                if truthy(&self.eval(c, env)?) {
                    self.eval(a, env)?
                } else {
                    self.eval(b, env)?
                }
            }
            Expr::Member(o, k) => {
                let o = self.eval(o, env)?;
                self.get(&o, k)?
            }
            Expr::Index(o, k) => {
                let o = self.eval(o, env)?;
                let k = self.eval(k, env)?;
                if let (Value::Obj(i), Value::Num(n)) = (&o, &k) {
                    if let Kind::Array(v) = &self.heap[*i].kind {
                        return Ok(v.get(*n as usize).cloned().unwrap_or(Value::Undef));
                    }
                }
                let k = self.to_str(&k);
                self.get(&o, &k)?
            }
            Expr::Call(callee, args) => {
                let (f, this) = match &**callee {
                    Expr::Member(o, k) => {
                        let o = self.eval(o, env)?;
                        (self.get(&o, k)?, Some(o))
                    }
                    c => (self.eval(c, env)?, None),
                };
                let mut a = Vec::with_capacity(args.len());
                for x in args {
                    a.push(self.eval(x, env)?);
                }
                if matches!(f, Value::Undef | Value::Null) {
                    return Err(format!("{} — не функция", describe(callee)));
                }
                self.call(f, this, a)?
            }
            Expr::New(c, args) => {
                // Only `new Array()` / `new Object()`: plain values.
                let _ = args;
                match &**c {
                    Expr::Id(n) if n == "Array" => self.alloc(Kind::Array(vec![])),
                    _ => self.alloc(Kind::Plain),
                }
            }
        })
    }

    fn store(&mut self, target: &Expr, v: Value, env: usize) -> Result<(), String> {
        match target {
            Expr::Id(n) => {
                let e = self.lookup(n, env).unwrap_or(0);
                self.envs[e].vars.insert(n.clone(), v);
                Ok(())
            }
            Expr::Member(o, k) => {
                let o = self.eval(o, env)?;
                self.set(&o, k, v)
            }
            Expr::Index(o, k) => {
                let o = self.eval(o, env)?;
                let k = self.eval(k, env)?;
                if let (Value::Obj(i), Value::Num(n)) = (&o, &k) {
                    if let Kind::Array(arr) = &mut self.heap[*i].kind {
                        let n = *n as usize;
                        if n < 100_000 {
                            if arr.len() <= n {
                                arr.resize(n + 1, Value::Undef);
                            }
                            arr[n] = v;
                        }
                        return Ok(());
                    }
                }
                let k = self.to_str(&k);
                self.set(&o, &k, v)
            }
            _ => Err("сюда нельзя присвоить".into()),
        }
    }

    fn call(&mut self, f: Value, this: Option<Value>, args: Vec<Value>) -> Result<Value, String> {
        self.step()?;
        match f {
            Value::Native(n, bound) => self.native(n, bound.map(|b| *b).or(this), args),
            Value::Obj(i) => {
                let Kind::Func(def, closure) = &self.heap[i].kind else { return Err("это не функция".into()) };
                let (def, closure) = (def.clone(), *closure);
                if self.envs.len() > 200_000 {
                    return Err("слишком много вызовов".into());
                }
                let e = self.child(closure);
                for (k, p) in def.params.iter().enumerate() {
                    self.envs[e].vars.insert(p.clone(), args.get(k).cloned().unwrap_or(Value::Undef));
                }
                if let Some(t) = this {
                    self.envs[e].vars.insert("this".into(), t);
                }
                self.hoist(&def.body, e);
                match self.block(&def.body, e)? {
                    Flow::Ret(v) => Ok(v),
                    _ => Ok(Value::Undef),
                }
            }
            _ => Err("это не функция".into()),
        }
    }

    // ------------------------------------------------------------ objects

    fn get(&mut self, o: &Value, k: &str) -> Result<Value, String> {
        let bind = |n: Native, o: &Value| Value::Native(n, Some(Box::new(o.clone())));
        match o {
            Value::Undef | Value::Null => Err(format!("нельзя прочитать «{k}» у {}", if *o == Value::Null { "null" } else { "undefined" })),
            Value::Num(_) => Ok(match k {
                "toFixed" => bind(Native::ToFixed, o),
                "toString" => bind(Native::ToStr, o),
                _ => Value::Undef,
            }),
            Value::Str(s) => Ok(match k {
                "length" => Value::Num(s.chars().count() as f64),
                "toUpperCase" => bind(Native::ToUpper, o),
                "toLowerCase" => bind(Native::ToLower, o),
                "includes" => bind(Native::Includes, o),
                "indexOf" => bind(Native::IndexOf, o),
                "toString" => bind(Native::ToStr, o),
                _ => Value::Undef,
            }),
            Value::Bool(_) | Value::Native(..) => Ok(Value::Undef),
            Value::Obj(i) => {
                let i = *i;
                match &self.heap[i].kind {
                    Kind::Array(v) => {
                        let len = v.len();
                        return Ok(match k {
                            "length" => Value::Num(len as f64),
                            "push" => bind(Native::Push, o),
                            "join" => bind(Native::Join, o),
                            "includes" => bind(Native::Includes, o),
                            "indexOf" => bind(Native::IndexOf, o),
                            _ => Value::Undef,
                        });
                    }
                    Kind::Elem(n) => {
                        let n = *n;
                        match k {
                            "textContent" | "innerText" | "innerHTML" => return Ok(Value::Str(self.dom.text_of(n).into())),
                            "value" => return Ok(Value::Str(self.dom.nodes[n].value.clone().into())),
                            "id" => return Ok(Value::Str(self.dom.attr(n, "id").unwrap_or_default().into())),
                            "className" => return Ok(Value::Str(self.dom.attr(n, "class").unwrap_or_default().into())),
                            "hidden" => return Ok(Value::Bool(self.dom.nodes[n].hidden)),
                            "style" => {
                                if let Some((_, v)) = self.heap[i].props.iter().find(|(k, _)| k == "style") {
                                    return Ok(v.clone());
                                }
                                let s = self.alloc(Kind::Style(n));
                                self.heap[i].props.push(("style".into(), s.clone()));
                                return Ok(s);
                            }
                            "classList" => {
                                if let Some((_, v)) = self.heap[i].props.iter().find(|(k, _)| k == "classList") {
                                    return Ok(v.clone());
                                }
                                let s = self.alloc(Kind::ClassList(n));
                                self.heap[i].props.push(("classList".into(), s.clone()));
                                return Ok(s);
                            }
                            "addEventListener" => return Ok(bind(Native::AddListener, o)),
                            "setAttribute" => return Ok(bind(Native::SetAttr, o)),
                            "getAttribute" => return Ok(bind(Native::GetAttr, o)),
                            _ => {}
                        }
                    }
                    Kind::Style(n) => {
                        let css = camel_to_css(k);
                        return Ok(self.dom.nodes[*n].js_style.iter().find(|(p, _)| *p == css).map(|(_, v)| Value::Str(v.clone().into())).unwrap_or(Value::Str("".into())));
                    }
                    Kind::ClassList(_) => {
                        return Ok(match k {
                            "add" | "remove" | "toggle" | "contains" => Value::Native(Native::SetAttr, Some(Box::new(Value::Str(format!("class:{k}:{i}").into())))),
                            _ => Value::Undef,
                        });
                    }
                    _ => {}
                }
                Ok(self.heap[i].props.iter().find(|(p, _)| p == k).map(|(_, v)| v.clone()).unwrap_or(Value::Undef))
            }
        }
    }

    fn set(&mut self, o: &Value, k: &str, v: Value) -> Result<(), String> {
        let Value::Obj(i) = o else { return Err(format!("нельзя записать «{k}» сюда")) };
        let i = *i;
        match &self.heap[i].kind {
            Kind::Elem(n) => {
                let n = *n;
                match k {
                    "textContent" | "innerText" | "innerHTML" => {
                        let t = self.to_str(&v);
                        self.dom.set_text(n, &t);
                        return Ok(());
                    }
                    "value" => {
                        self.dom.nodes[n].value = self.to_str(&v);
                        return Ok(());
                    }
                    "className" => {
                        let t = self.to_str(&v);
                        self.dom.set_attr(n, "class", &t);
                        return Ok(());
                    }
                    "hidden" => {
                        self.dom.nodes[n].hidden = truthy(&v);
                        return Ok(());
                    }
                    _ => {}
                }
            }
            Kind::Style(n) => {
                let (n, css, val) = (*n, camel_to_css(k), self.to_str(&v));
                let st = &mut self.dom.nodes[n].js_style;
                st.retain(|(p, _)| *p != css);
                st.push((css, val));
                return Ok(());
            }
            _ => {}
        }
        let props = &mut self.heap[i].props;
        match props.iter_mut().find(|(p, _)| p == k) {
            Some(slot) => slot.1 = v,
            None => props.push((k.to_string(), v)),
        }
        Ok(())
    }

    // ------------------------------------------------------------ helpers

    fn type_of(&self, v: &Value) -> &'static str {
        match v {
            Value::Undef => "undefined",
            Value::Null => "object",
            Value::Bool(_) => "boolean",
            Value::Num(_) => "number",
            Value::Str(_) => "string",
            Value::Native(..) => "function",
            Value::Obj(i) => {
                if matches!(self.heap[*i].kind, Kind::Func(..)) {
                    "function"
                } else {
                    "object"
                }
            }
        }
    }

    fn num(&self, v: &Value) -> f64 {
        match v {
            Value::Num(n) => *n,
            Value::Bool(b) => *b as u8 as f64,
            Value::Null => 0.0,
            Value::Str(s) => {
                let t = s.trim();
                if t.is_empty() { 0.0 } else { t.parse().unwrap_or(f64::NAN) }
            }
            _ => f64::NAN,
        }
    }

    pub fn to_str(&self, v: &Value) -> String {
        match v {
            Value::Undef => "undefined".into(),
            Value::Null => "null".into(),
            Value::Bool(b) => b.to_string(),
            Value::Num(n) => fmt_num(*n),
            Value::Str(s) => s.to_string(),
            Value::Native(..) => "function".into(),
            Value::Obj(i) => match &self.heap[*i].kind {
                Kind::Array(a) => a.iter().map(|x| self.to_str(x)).collect::<Vec<_>>().join(","),
                Kind::Func(..) => "function".into(),
                Kind::Elem(_) => "[HTMLElement]".into(),
                _ => "[object Object]".into(),
            },
        }
    }

    fn binop(&self, op: &str, l: &Value, r: &Value) -> Value {
        match op {
            "+" => {
                if matches!(l, Value::Str(_) | Value::Obj(_)) || matches!(r, Value::Str(_) | Value::Obj(_)) {
                    Value::Str(format!("{}{}", self.to_str(l), self.to_str(r)).into())
                } else {
                    Value::Num(self.num(l) + self.num(r))
                }
            }
            "-" => Value::Num(self.num(l) - self.num(r)),
            "*" => Value::Num(self.num(l) * self.num(r)),
            "/" => Value::Num(self.num(l) / self.num(r)),
            "%" => Value::Num(self.num(l) % self.num(r)),
            "**" => Value::Num(self.num(l).powf(self.num(r))),
            "===" | "!==" | "==" | "!=" => {
                let eq = match (l, r) {
                    (Value::Num(a), Value::Num(b)) => a == b,
                    (Value::Str(a), Value::Str(b)) => a == b,
                    (Value::Undef | Value::Null, Value::Undef | Value::Null) => op.len() == 2 || std::mem::discriminant(l) == std::mem::discriminant(r),
                    (Value::Num(_), Value::Str(_)) | (Value::Str(_), Value::Num(_)) if op.len() == 2 => self.num(l) == self.num(r),
                    _ => l == r,
                };
                Value::Bool(if op.starts_with('!') { !eq } else { eq })
            }
            "<" | ">" | "<=" | ">=" => {
                let o = if let (Value::Str(a), Value::Str(b)) = (l, r) {
                    a.partial_cmp(b)
                } else {
                    self.num(l).partial_cmp(&self.num(r))
                };
                Value::Bool(match (op, o) {
                    (_, None) => false,
                    ("<", Some(o)) => o.is_lt(),
                    (">", Some(o)) => o.is_gt(),
                    ("<=", Some(o)) => o.is_le(),
                    (_, Some(o)) => o.is_ge(),
                })
            }
            _ => Value::Undef,
        }
    }

    fn rand(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    fn native(&mut self, n: Native, this: Option<Value>, a: Vec<Value>) -> Result<Value, String> {
        let arg = |k: usize| a.get(k).cloned().unwrap_or(Value::Undef);
        let this_v = this.clone().unwrap_or(Value::Undef);
        Ok(match n {
            Native::GetById => {
                let id = self.to_str(&arg(0));
                match self.dom.by_id(&id) {
                    Some(node) => self.elem(node),
                    None => Value::Null,
                }
            }
            Native::QuerySelector => {
                let q = self.to_str(&arg(0));
                match self.dom.query(&q) {
                    Some(node) => self.elem(node),
                    None => Value::Null,
                }
            }
            Native::Log => {
                let line = a.iter().map(|v| self.to_str(v)).collect::<Vec<_>>().join(" ");
                self.log.push(line);
                Value::Undef
            }
            Native::Floor => Value::Num(self.num(&arg(0)).floor()),
            Native::Round => Value::Num(self.num(&arg(0)).round()),
            Native::Ceil => Value::Num(self.num(&arg(0)).ceil()),
            Native::Abs => Value::Num(self.num(&arg(0)).abs()),
            Native::Sqrt => Value::Num(self.num(&arg(0)).sqrt()),
            Native::Min => Value::Num(a.iter().map(|v| self.num(v)).fold(f64::INFINITY, f64::min)),
            Native::Max => Value::Num(a.iter().map(|v| self.num(v)).fold(f64::NEG_INFINITY, f64::max)),
            Native::Random => Value::Num(self.rand()),
            Native::ToFixed => {
                let d = self.num(&arg(0)).clamp(0.0, 20.0) as usize;
                Value::Str(format!("{:.*}", d, self.num(&this_v)).into())
            }
            Native::ToStr => {
                let v = if this.is_some() { this_v.clone() } else { a.first().cloned().unwrap_or(Value::Str("".into())) };
                Value::Str(self.to_str(&v).into())
            }
            Native::Number => Value::Num(self.num(&arg(0))),
            Native::ParseInt => {
                let s = self.to_str(&arg(0));
                let t: String = s.trim().chars().enumerate().take_while(|(i, c)| c.is_ascii_digit() || (*i == 0 && *c == '-')).map(|(_, c)| c).collect();
                Value::Num(t.parse().unwrap_or(f64::NAN))
            }
            Native::Push => {
                if let Value::Obj(i) = this_v {
                    if let Kind::Array(v) = &mut self.heap[i].kind {
                        v.extend(a);
                        return Ok(Value::Num(v.len() as f64));
                    }
                }
                Value::Undef
            }
            Native::Join => {
                let sep = if a.is_empty() { ",".to_string() } else { self.to_str(&arg(0)) };
                if let Value::Obj(i) = &this_v {
                    if let Kind::Array(v) = &self.heap[*i].kind {
                        return Ok(Value::Str(v.iter().map(|x| self.to_str(x)).collect::<Vec<_>>().join(&sep).into()));
                    }
                }
                Value::Str("".into())
            }
            Native::Includes | Native::IndexOf => {
                let pos: Option<usize> = match &this_v {
                    Value::Str(s) => {
                        let q = self.to_str(&arg(0));
                        s.find(&q).map(|b| s[..b].chars().count())
                    }
                    Value::Obj(i) => match &self.heap[*i].kind {
                        Kind::Array(v) => {
                            let q = arg(0);
                            v.iter().position(|x| *x == q)
                        }
                        _ => None,
                    },
                    _ => None,
                };
                if n == Native::Includes { Value::Bool(pos.is_some()) } else { Value::Num(pos.map(|p| p as f64).unwrap_or(-1.0)) }
            }
            Native::ToUpper => Value::Str(self.to_str(&this_v).to_uppercase().into()),
            Native::ToLower => Value::Str(self.to_str(&this_v).to_lowercase().into()),
            Native::SetTimeout | Native::SetInterval => {
                let ms = self.num(&arg(1)).max(0.0) / 1000.0;
                let ms = if ms.is_nan() { 0.0 } else { ms };
                let id = self.next_timer;
                self.next_timer += 1;
                if self.timers.len() < 64 {
                    self.timers.push(Timer { id, due: self.clock + ms, every: (n == Native::SetInterval).then_some(ms), f: arg(0) });
                }
                Value::Num(id as f64)
            }
            Native::ClearTimer => {
                let id = self.num(&arg(0)) as u32;
                self.timers.retain(|t| t.id != id);
                Value::Undef
            }
            Native::AddListener => {
                let ev = self.to_str(&arg(0));
                self.set(&this_v, &format!("on{ev}"), arg(1))?;
                Value::Undef
            }
            Native::SetAttr => {
                // classList.add/remove/toggle/contains come here bound to "class:<op>:<obj>".
                if let Value::Str(tag) = &this_v {
                    if let Some(rest) = tag.strip_prefix("class:") {
                        let (op, obj) = rest.split_once(':').unwrap_or(("", "0"));
                        let Kind::ClassList(node) = self.heap[obj.parse::<usize>().unwrap_or(0)].kind else { return Ok(Value::Undef) };
                        let c = self.to_str(&arg(0));
                        let mut list: Vec<String> = self.dom.attr(node, "class").unwrap_or_default().split_whitespace().map(String::from).collect();
                        let has = list.contains(&c);
                        match op {
                            "contains" => return Ok(Value::Bool(has)),
                            "add" if !has => list.push(c),
                            "remove" => list.retain(|x| *x != c),
                            "toggle" if has => list.retain(|x| *x != c),
                            "toggle" => list.push(c),
                            _ => {}
                        }
                        self.dom.set_attr(node, "class", &list.join(" "));
                        return Ok(Value::Undef);
                    }
                }
                if let Value::Obj(i) = this_v {
                    if let Kind::Elem(node) = self.heap[i].kind {
                        let (k, v) = (self.to_str(&arg(0)), self.to_str(&arg(1)));
                        self.dom.set_attr(node, &k, &v);
                    }
                }
                Value::Undef
            }
            Native::GetAttr => {
                if let Value::Obj(i) = this_v {
                    if let Kind::Elem(node) = self.heap[i].kind {
                        let k = self.to_str(&arg(0));
                        return Ok(self.dom.attr(node, &k).map(|s| Value::Str(s.into())).unwrap_or(Value::Null));
                    }
                }
                Value::Null
            }
            Native::Chat => {
                let t = a.iter().map(|v| self.to_str(v)).collect::<Vec<_>>().join(" ");
                self.effects.push(Effect::Chat(t));
                Value::Undef
            }
            Native::Command => {
                let t = self.to_str(&arg(0));
                self.effects.push(Effect::Command(if t.starts_with('/') { t } else { format!("/{t}") }));
                Value::Undef
            }
            Native::Close => {
                self.effects.push(Effect::Close);
                Value::Undef
            }
            Native::Health => Value::Num(self.host.health as f64),
            Native::Hunger => Value::Num(self.host.hunger as f64),
            Native::Pos => {
                let [x, y, z] = self.host.pos;
                self.object(vec![("x", Value::Num(x)), ("y", Value::Num(y)), ("z", Value::Num(z))])
            }
            Native::Time => Value::Str(self.host.time.clone().into()),
            Native::Nick => Value::Str(self.host.nick.clone().into()),
            Native::Creative => Value::Bool(self.host.creative),
            Native::Store => {
                let (k, v) = (self.to_str(&arg(0)), self.to_str(&arg(1)));
                if self.saved.len() < 256 || self.saved.contains_key(&k) {
                    self.saved.insert(k, v.chars().take(4096).collect());
                    self.saved_dirty = true;
                }
                Value::Undef
            }
            Native::Load => {
                let k = self.to_str(&arg(0));
                self.saved.get(&k).map(|s| Value::Str(s.clone().into())).unwrap_or(Value::Null)
            }
        })
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Undef | Value::Null => false,
        Value::Bool(b) => *b,
        Value::Num(n) => *n != 0.0 && !n.is_nan(),
        Value::Str(s) => !s.is_empty(),
        _ => true,
    }
}

fn describe(e: &Expr) -> String {
    match e {
        Expr::Id(n) => format!("«{n}»"),
        Expr::Member(o, k) => format!("{}.{k}", describe(o).trim_matches(['«', '»'])),
        _ => "выражение".into(),
    }
}

/// `backgroundColor` → `background-color`.
fn camel_to_css(k: &str) -> String {
    let mut s = String::new();
    for c in k.chars() {
        if c.is_ascii_uppercase() {
            s.push('-');
            s.push(c.to_ascii_lowercase());
        } else {
            s.push(c);
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str) -> Interp {
        let mut it = Interp::new(Dom::parse("<div id='a'>x</div>"));
        it.run(src).unwrap();
        it
    }

    #[test]
    fn js_runs_closures_loops_objects_and_the_dom() {
        let it = run(
            "let n = 0; function add(k) { n += k; return n; }
             for (let i = 1; i <= 4; i++) { add(i); }
             const o = { a: 2, b: [1, 2, 3] };
             o.c = o.b.length * o.a;
             const f = (x) => x * 2;
             let s = 'n=' + n + ' c=' + o.c + ' f=' + f(5) + ' ' + (n > 5 ? 'big' : 'small');
             for (const v of o.b) { s += v; }
             document.getElementById('a').textContent = s;
             document.getElementById('a').style.backgroundColor = '#ff0000';
             console.log((3.14159).toFixed(2), typeof f);",
        );
        let a = it.dom.by_id("a").unwrap();
        assert_eq!(it.dom.text_of(a), "n=10 c=6 f=10 big123");
        assert_eq!(it.dom.nodes[a].js_style, vec![("background-color".to_string(), "#ff0000".to_string())]);
        assert_eq!(it.log, vec!["3.14 function".to_string()]);
    }

    #[test]
    fn js_stops_endless_loops_and_reports_errors() {
        let mut it = Interp::new(Dom::parse("<p></p>"));
        assert!(it.run("while (true) {}").unwrap_err().contains("долго"));
        assert!(it.run("nope()").unwrap_err().contains("nope"));
        assert!(it.run("let x = ;").is_err());
    }

    #[test]
    fn js_handlers_timers_and_wolf_api() {
        let mut it = Interp::new(Dom::parse("<button id='b'>go</button><span id='t'></span>"));
        it.run(
            "let clicks = 0;
             document.getElementById('b').addEventListener('click', () => { clicks++; wolf.chat('клик', clicks); });
             setTimeout(function () { document.getElementById('t').textContent = 'готово'; wolf.command('time set день'); }, 500);",
        )
        .unwrap();
        let b = it.dom.by_id("b").unwrap();
        let h = it.handler(b, "click").unwrap();
        it.call_value(&h, vec![]).unwrap();
        assert_eq!(it.effects, vec![Effect::Chat("клик 1".into())]);
        it.tick(0.3);
        assert_eq!(it.dom.text_of(it.dom.by_id("t").unwrap()), "");
        it.tick(0.3);
        assert_eq!(it.dom.text_of(it.dom.by_id("t").unwrap()), "готово");
        assert_eq!(it.effects.last(), Some(&Effect::Command("/time set день".into())));
    }
}
