//! Duel rules: two fighters on a round arena, strikes with wind-up / hit / recovery, block,
//! dodge, stagger, shadow power, rounds. No rendering here, so it is all testable.

use glam::Vec2;

pub const ARENA_RADIUS: f32 = 8.5;
pub const ROUND_TIME: f32 = 60.0;
pub const ROUNDS_TO_WIN: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Jab,
    Hook,
    Kick,
    Shadow,
}

pub struct MoveSpec {
    pub windup: f32,
    pub active: f32,
    pub recover: f32,
    pub damage: f32,
    pub range: f32,
    pub knock: f32,
    /// Share of the damage that still goes through a block.
    pub chip: f32,
}

pub fn spec(m: Move) -> MoveSpec {
    match m {
        Move::Jab => MoveSpec { windup: 0.10, active: 0.08, recover: 0.16, damage: 6.0, range: 1.6, knock: 0.15, chip: 0.1 },
        Move::Hook => MoveSpec { windup: 0.30, active: 0.10, recover: 0.32, damage: 14.0, range: 1.65, knock: 0.6, chip: 0.3 },
        Move::Kick => MoveSpec { windup: 0.24, active: 0.10, recover: 0.36, damage: 11.0, range: 2.3, knock: 0.9, chip: 0.2 },
        Move::Shadow => MoveSpec { windup: 0.42, active: 0.16, recover: 0.5, damage: 24.0, range: 2.7, knock: 1.8, chip: 0.5 },
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Idle,
    Attack { m: Move, t: f32, hit: bool },
    Block,
    Dodge { t: f32, dir: Vec2 },
    Stagger { t: f32 },
    Ko { t: f32 },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// x: strafe right, y: towards the opponent.
    pub mv: Vec2,
    pub jab: bool,
    pub hook: bool,
    pub kick: bool,
    pub shadow: bool,
    pub block: bool,
    pub dodge: bool,
}

#[derive(Clone, Debug)]
pub struct Fighter {
    pub pos: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    /// Shadow power 0..1; full = the shadow strike is ready.
    pub power: f32,
    pub state: State,
    pub walk: f32,
    pub moving: f32,
    pub combo: u32,
    pub combo_t: f32,
    pub flash: f32,
    pub dodge_cd: f32,
    pub queued: Option<(Move, f32)>,
    pub dmg_mult: f32,
    pub speed: f32,
}

impl Fighter {
    pub fn new(pos: Vec2, hp: f32) -> Self {
        Self { pos, hp, max_hp: hp, power: 0.0, state: State::Idle, walk: 0.0, moving: 0.0, combo: 0, combo_t: 0.0, flash: 0.0, dodge_cd: 0.0, queued: None, dmg_mult: 1.0, speed: 1.0 }
    }
    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }
    pub fn can_act(&self) -> bool {
        match self.state {
            State::Idle | State::Block => true,
            // Combos: the recovery of a strike that landed can be cut into the next one.
            State::Attack { m, t, hit } => {
                let s = spec(m);
                hit && t > s.windup + s.active + 0.03 && m != Move::Shadow
            }
            _ => false,
        }
    }
    /// Invulnerable part of a dodge.
    pub fn evading(&self) -> bool {
        matches!(self.state, State::Dodge { t, .. } if t < 0.26)
    }
    pub fn windup_of(&self) -> Option<(Move, f32)> {
        match self.state {
            State::Attack { m, t, .. } if t < spec(m).windup => Some((m, t / spec(m).windup)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// true: the player was hit.
    pub on_player: bool,
    pub blocked: bool,
    pub damage: f32,
    pub m: Move,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Intro { t: f32 },
    Fight,
    RoundEnd { t: f32, player_won: bool },
    Over { player_won: bool },
}

/// How a shadow fights.
#[derive(Clone, Copy, Debug)]
pub struct Ai {
    pub aggression: f32,
    pub reaction: f32,
    pub block_skill: f32,
    pub dodge_skill: f32,
    pub kick_bias: f32,
    pub hook_bias: f32,
    pub speed: f32,
    pub damage: f32,
    pub hp: f32,
    pub uses_shadow: bool,
}

struct AiState {
    think: f32,
    react: f32,
    seen_windup: bool,
    strafe: f32,
    rng: u32,
    want_block: f32,
}

impl AiState {
    fn rand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x % 10_000) as f32 / 10_000.0
    }
}

pub struct Duel {
    pub player: Fighter,
    pub enemy: Fighter,
    pub ai: Ai,
    st: AiState,
    pub round: u8,
    pub wins: [u8; 2],
    pub timer: f32,
    pub phase: Phase,
    pub hits: Vec<Hit>,
    pub time: f32,
}

impl Duel {
    pub fn new(ai: Ai, seed: u32) -> Self {
        let mut d = Self {
            player: Fighter::new(Vec2::new(0.0, -2.2), 100.0),
            enemy: Fighter::new(Vec2::new(0.0, 2.2), ai.hp),
            ai,
            st: AiState { think: 0.6, react: 0.0, seen_windup: false, strafe: 1.0, rng: seed | 1, want_block: 0.0 },
            round: 1,
            wins: [0, 0],
            timer: ROUND_TIME,
            phase: Phase::Intro { t: 0.0 },
            hits: Vec::new(),
            time: 0.0,
        };
        d.reset_round();
        d
    }

    fn reset_round(&mut self) {
        let keep_p = self.player.power;
        let keep_e = self.enemy.power;
        self.player = Fighter::new(Vec2::new(0.0, -2.2), 100.0);
        self.player.power = keep_p;
        self.enemy = Fighter::new(Vec2::new(0.0, 2.2), self.ai.hp);
        self.enemy.power = keep_e;
        self.enemy.dmg_mult = self.ai.damage;
        self.enemy.speed = self.ai.speed;
        self.timer = ROUND_TIME;
    }

    pub fn distance(&self) -> f32 {
        (self.enemy.pos - self.player.pos).length()
    }

    pub fn update(&mut self, dt: f32, input: Input) {
        self.hits.clear();
        self.time += dt;
        match self.phase {
            Phase::Intro { t } => {
                let t = t + dt;
                self.phase = if t > 2.0 { Phase::Fight } else { Phase::Intro { t } };
                self.idle_tick(dt);
                return;
            }
            Phase::RoundEnd { t, player_won } => {
                let t = t + dt;
                self.idle_tick(dt);
                if t > 3.0 {
                    if self.wins[0] >= ROUNDS_TO_WIN || self.wins[1] >= ROUNDS_TO_WIN {
                        self.phase = Phase::Over { player_won: self.wins[0] > self.wins[1] };
                    } else {
                        self.round += 1;
                        self.reset_round();
                        self.phase = Phase::Intro { t: 0.0 };
                    }
                } else {
                    self.phase = Phase::RoundEnd { t, player_won };
                }
                return;
            }
            Phase::Over { .. } => {
                self.idle_tick(dt);
                return;
            }
            Phase::Fight => {}
        }
        let ai_in = self.think(dt);
        let (p_pos, e_pos) = (self.player.pos, self.enemy.pos);
        step(&mut self.player, dt, input, e_pos);
        step(&mut self.enemy, dt, ai_in, p_pos);
        separate(&mut self.player, &mut self.enemy);
        let h1 = resolve(&mut self.player, &mut self.enemy, false);
        let h2 = resolve(&mut self.enemy, &mut self.player, true);
        self.hits.extend(h1);
        self.hits.extend(h2);
        self.timer -= dt;
        let p_dead = !self.player.alive();
        let e_dead = !self.enemy.alive();
        if p_dead || e_dead || self.timer <= 0.0 {
            let player_won = if e_dead && !p_dead {
                true
            } else if p_dead && !e_dead {
                false
            } else {
                self.player.hp / self.player.max_hp >= self.enemy.hp / self.enemy.max_hp
            };
            if player_won {
                self.wins[0] += 1;
                if self.enemy.alive() {
                    self.enemy.state = State::Ko { t: 0.0 };
                }
            } else {
                self.wins[1] += 1;
                if self.player.alive() {
                    self.player.state = State::Ko { t: 0.0 };
                }
            }
            self.timer = self.timer.max(0.0);
            self.phase = Phase::RoundEnd { t: 0.0, player_won };
        }
    }

    /// Between rounds: fighters only finish what they do and breathe.
    fn idle_tick(&mut self, dt: f32) {
        for f in [&mut self.player, &mut self.enemy] {
            f.flash = (f.flash - dt * 3.0).max(0.0);
            f.moving = (f.moving - dt * 4.0).max(0.0);
            match &mut f.state {
                State::Ko { t } => *t += dt,
                State::Attack { .. } | State::Stagger { .. } | State::Dodge { .. } | State::Block => f.state = State::Idle,
                State::Idle => {}
            }
        }
    }

    fn think(&mut self, dt: f32) -> Input {
        let ai = self.ai;
        let mut out = Input::default();
        let d = self.distance();
        let st = &mut self.st;
        st.think -= dt;
        // Reacting to the player's strike (after the reaction time).
        if let Some((m, _)) = self.player.windup_of() {
            if !st.seen_windup {
                st.seen_windup = true;
                st.react = ai.reaction;
            }
            st.react -= dt;
            if st.react <= 0.0 && d < spec(m).range + 0.6 {
                let r = st.rand();
                if r < ai.dodge_skill * 0.5 && self.enemy.dodge_cd <= 0.0 {
                    out.dodge = true;
                    out.mv = Vec2::new(if st.rand() < 0.5 { -1.0 } else { 1.0 }, -0.4);
                } else if r < ai.block_skill {
                    st.want_block = 0.5;
                }
                st.react = 99.0;
            }
        } else {
            st.seen_windup = false;
        }
        if st.want_block > 0.0 {
            st.want_block -= dt;
            out.block = true;
            return out;
        }
        // Moving: closer when aggressive, circle a bit.
        let want = 1.25 + (1.0 - ai.aggression) * 1.1;
        out.mv.y = ((d - want) * 1.5).clamp(-1.0, 1.0);
        if st.rand() < dt * 0.6 {
            st.strafe = -st.strafe;
        }
        out.mv.x = st.strafe * 0.35;
        if st.think <= 0.0 {
            st.think = 0.25 + (1.0 - ai.aggression) * 0.7 + st.rand() * 0.4;
            let r = st.rand();
            if ai.uses_shadow && self.enemy.power >= 1.0 && d < 2.6 {
                out.shadow = true;
            } else if d < 1.75 {
                if r < ai.hook_bias {
                    out.hook = true;
                } else {
                    out.jab = true;
                    // Quick follow-up keeps the combo going.
                    st.think = 0.12;
                }
            } else if d < 2.35 && r < ai.kick_bias + 0.3 {
                out.kick = true;
            } else if d > 2.4 {
                out.mv.y = 1.0;
            }
        }
        out
    }
}

fn step(f: &mut Fighter, dt: f32, i: Input, target: Vec2) {
    f.flash = (f.flash - dt * 3.0).max(0.0);
    f.dodge_cd = (f.dodge_cd - dt).max(0.0);
    f.combo_t -= dt;
    if f.combo_t <= 0.0 {
        f.combo = 0;
    }
    if let Some((_, t)) = &mut f.queued {
        *t -= dt;
        if *t <= 0.0 {
            f.queued = None;
        }
    }
    let want = if i.shadow && f.power >= 1.0 {
        Some(Move::Shadow)
    } else if i.hook {
        Some(Move::Hook)
    } else if i.kick {
        Some(Move::Kick)
    } else if i.jab {
        Some(Move::Jab)
    } else {
        None
    };
    if let Some(m) = want {
        f.queued = Some((m, 0.22));
    }
    let fwd = (target - f.pos).normalize_or_zero();
    let right = Vec2::new(fwd.y, -fwd.x);
    // Advance the current state.
    match &mut f.state {
        State::Attack { m, t, .. } => {
            *t += dt;
            let s = spec(*m);
            let (mm, tt) = (*m, *t);
            // A small lunge on the hit.
            if tt > s.windup && tt < s.windup + s.active && mm != Move::Jab {
                f.pos += fwd * 1.6 * dt;
            }
            if tt >= s.windup + s.active + s.recover {
                f.state = State::Idle;
            }
        }
        State::Dodge { t, dir } => {
            *t += dt;
            let k = (1.0 - *t / 0.38).max(0.0);
            let d = *dir;
            f.pos += d * 7.5 * k * dt;
            if *t >= 0.38 {
                f.state = State::Idle;
            }
        }
        State::Stagger { t } => {
            *t -= dt;
            if *t <= 0.0 {
                f.state = State::Idle;
            }
        }
        State::Ko { t } => *t += dt,
        State::Block | State::Idle => {}
    }
    if !f.alive() {
        if !matches!(f.state, State::Ko { .. }) {
            f.state = State::Ko { t: 0.0 };
        }
        return;
    }
    if f.can_act() {
        if i.dodge && f.dodge_cd <= 0.0 {
            let mv = if i.mv.length() > 0.2 { i.mv.normalize() } else { Vec2::new(0.0, -1.0) };
            f.state = State::Dodge { t: 0.0, dir: (right * mv.x + fwd * mv.y).normalize_or_zero() };
            f.dodge_cd = 0.55;
            f.queued = None;
        } else if let Some((m, _)) = f.queued.take() {
            if m == Move::Shadow {
                f.power = 0.0;
            }
            f.state = State::Attack { m, t: 0.0, hit: false };
        } else if i.block {
            f.state = State::Block;
        } else if f.state == State::Block {
            f.state = State::Idle;
        }
    }
    // Walking.
    let free = matches!(f.state, State::Idle | State::Block);
    if free && i.mv.length() > 0.05 {
        let mv = i.mv.clamp_length_max(1.0);
        let k = if f.state == State::Block { 0.45 } else { 1.0 };
        let fw = if mv.y > 0.0 { 2.6 } else { 2.0 };
        f.pos += (fwd * mv.y * fw + right * mv.x * 2.2) * k * f.speed * dt;
        f.walk += dt * 9.0 * mv.length();
        f.moving = (f.moving + dt * 6.0).min(1.0);
    } else {
        f.moving = (f.moving - dt * 6.0).max(0.0);
    }
    if f.pos.length() > ARENA_RADIUS {
        f.pos = f.pos.normalize() * ARENA_RADIUS;
    }
}

fn separate(a: &mut Fighter, b: &mut Fighter) {
    let d = b.pos - a.pos;
    let l = d.length();
    let min = 0.85;
    if l < min {
        let n = if l > 1e-4 { d / l } else { Vec2::Y };
        let push = (min - l) / 2.0;
        a.pos -= n * push;
        b.pos += n * push;
    }
}

fn resolve(att: &mut Fighter, def: &mut Fighter, on_player: bool) -> Option<Hit> {
    let State::Attack { m, t, hit } = att.state else { return None };
    let s = spec(m);
    if hit || t < s.windup || t > s.windup + s.active {
        return None;
    }
    let d = (def.pos - att.pos).length();
    if d > s.range + 0.25 || !def.alive() || def.evading() {
        return None;
    }
    att.state = State::Attack { m, t, hit: true };
    let dir = (def.pos - att.pos).normalize_or_zero();
    let blocked = def.state == State::Block;
    let mut dmg = s.damage * att.dmg_mult;
    if blocked {
        dmg *= s.chip;
        def.pos += dir * s.knock * 0.4;
        att.power = (att.power + 0.04).min(1.0);
        def.power = (def.power + 0.08).min(1.0);
        // A heavy blow breaks the guard.
        if matches!(m, Move::Hook | Move::Shadow) {
            def.state = State::Stagger { t: 0.25 };
        }
    } else {
        att.combo += 1;
        att.combo_t = 1.1;
        // Every third hit in a row lands harder.
        if att.combo >= 3 {
            dmg *= 1.25;
        }
        def.pos += dir * s.knock;
        def.state = State::Stagger { t: if m == Move::Jab { 0.22 } else { 0.42 } };
        def.flash = 1.0;
        def.queued = None;
        att.power = (att.power + 0.12).min(1.0);
        def.power = (def.power + 0.06).min(1.0);
    }
    def.hp = (def.hp - dmg).max(0.0);
    if def.pos.length() > ARENA_RADIUS {
        def.pos = def.pos.normalize() * ARENA_RADIUS;
    }
    Some(Hit { on_player, blocked, damage: dmg, m })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy() -> Ai {
        Ai { aggression: 0.0, reaction: 9.0, block_skill: 0.0, dodge_skill: 0.0, kick_bias: 0.0, hook_bias: 0.0, speed: 0.0, damage: 1.0, hp: 100.0, uses_shadow: false }
    }

    fn fight(ai: Ai) -> Duel {
        let mut d = Duel::new(ai, 7);
        d.phase = Phase::Fight;
        d.enemy.pos = Vec2::new(0.0, -0.9);
        d
    }

    fn run(d: &mut Duel, secs: f32, i: Input) {
        let mut t = 0.0;
        while t < secs {
            d.update(1.0 / 60.0, i);
            t += 1.0 / 60.0;
        }
    }

    #[test]
    fn a_jab_in_range_hurts() {
        let mut d = fight(dummy());
        d.update(1.0 / 60.0, Input { jab: true, ..Default::default() });
        run(&mut d, 0.4, Input::default());
        assert!(d.enemy.hp < 100.0 && d.enemy.hp > 90.0, "hp {}", d.enemy.hp);
    }

    #[test]
    fn blocking_takes_much_less() {
        let mut a = Fighter::new(Vec2::ZERO, 100.0);
        let mut open = Fighter::new(Vec2::new(0.0, 1.2), 100.0);
        let mut guard = open.clone();
        guard.state = State::Block;
        let hook = State::Attack { m: Move::Hook, t: spec(Move::Hook).windup + 0.01, hit: false };
        a.state = hook;
        resolve(&mut a, &mut open, false).unwrap();
        a.state = hook;
        a.combo = 0;
        let h = resolve(&mut a, &mut guard, false).unwrap();
        assert!(h.blocked);
        let (full, chip) = (100.0 - open.hp, 100.0 - guard.hp);
        assert!(chip > 0.0 && chip < full * 0.4, "open {full}, blocked {chip}");
        // The heavy blow breaks the guard.
        assert!(matches!(guard.state, State::Stagger { .. }));
    }

    #[test]
    fn a_dodge_avoids_the_hit() {
        let mut d = fight(dummy());
        d.enemy.pos = d.player.pos + Vec2::new(0.0, 1.2);
        d.player.state = State::Attack { m: Move::Jab, t: 0.05, hit: false };
        d.enemy.state = State::Dodge { t: 0.0, dir: Vec2::ZERO };
        run(&mut d, 0.2, Input::default());
        assert_eq!(d.enemy.hp, 100.0);
    }

    #[test]
    fn shadow_strike_needs_full_power() {
        let mut d = fight(dummy());
        d.update(1.0 / 60.0, Input { shadow: true, ..Default::default() });
        assert!(!matches!(d.player.state, State::Attack { m: Move::Shadow, .. }));
        d.player.power = 1.0;
        run(&mut d, 0.5, Input::default());
        d.update(1.0 / 60.0, Input { shadow: true, ..Default::default() });
        assert!(matches!(d.player.state, State::Attack { m: Move::Shadow, .. }));
        assert_eq!(d.player.power, 0.0);
    }

    #[test]
    fn every_chapter_shadow_finishes_a_match_against_a_passive_player() {
        for ch in crate::story::CHAPTERS.iter() {
            let mut d = Duel::new(ch.ai, 3);
            let mut t = 0.0;
            while !matches!(d.phase, Phase::Over { .. }) && t < 400.0 {
                d.update(1.0 / 60.0, Input::default());
                t += 1.0 / 60.0;
            }
            assert!(matches!(d.phase, Phase::Over { player_won: false }), "{}: {:?} after {t}s", ch.foe, d.phase);
        }
    }

    #[test]
    fn a_spamming_player_can_beat_the_first_shadow() {
        let ai = crate::story::CHAPTERS[0].ai;
        let mut d = Duel::new(ai, 11);
        let mut t = 0.0;
        let mut k = 0u32;
        while !matches!(d.phase, Phase::Over { .. }) && t < 400.0 {
            k += 1;
            let i = Input { mv: Vec2::new(0.0, if d.distance() > 1.3 { 1.0 } else { 0.0 }), jab: k % 9 == 0, hook: k % 31 == 0, ..Default::default() };
            d.update(1.0 / 60.0, i);
            t += 1.0 / 60.0;
        }
        assert!(matches!(d.phase, Phase::Over { player_won: true }), "{:?}", d.phase);
    }
}
