//! Native-planet encounters. Original stone, forest and volcanic guardians; no copied boss AI.
use crate::{cubesphere::Cell, items::{ItemStack,ids}};
use glam::DVec3;
use std::{collections::HashMap,path::Path};
#[repr(u8)]
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum BossKind {Stone,Root,Ember}
impl BossKind {
    pub fn from_id(id:u8)->Option<Self>{Some(match id{0=>Self::Stone,1=>Self::Root,2=>Self::Ember,_=>return None})}
    pub fn name(self)->&'static str{match self{Self::Stone=>"Каменный страж",Self::Root=>"Хранитель чащи",Self::Ember=>"Сердце вулкана"}}
    pub fn max_hp(self)->f32{match self{Self::Stone=>180.,Self::Root=>160.,Self::Ember=>240.}}
    pub fn color(self)->[f32;3]{match self{Self::Stone=>[0.48,0.57,0.64],Self::Root=>[0.28,0.66,0.35],Self::Ember=>[0.95,0.30,0.10]}}
    pub fn phase(self,hp:f32)->u8{let f=hp/self.max_hp();if f>0.66{1}else if f>0.33{2}else{3}}
    pub fn rewards(self)->Vec<ItemStack>{match self{
        Self::Stone=>vec![ItemStack::new(ids::REVIVAL_HEART,1),ItemStack::new(ids::RESONANCE_RUNE,1)],
        Self::Root=>vec![ItemStack::new(ids::WINGS,1),ItemStack::new(ids::RITUAL_SEAL,1)],
        Self::Ember=>vec![ItemStack::new(ids::VOLCANIC_CORE,1),ItemStack::new(ids::RESONANCE_RUNE,2)],
    }}
}
#[derive(Clone,Debug)]
pub struct BossRole {
    pub kind:BossKind,pub home:Cell,pub anchor:DVec3,pub target:DVec3,pub cooldown:f32,pub casting:f32,
    /// The attack being cast (see `Attack`).
    pub attack:Attack,
    /// Several aimed spots for a volley (fireballs, root spikes).
    pub targets:Vec<DVec3>,
    /// Seconds of a charge left, and its direction.
    pub charge:f32,pub charge_dir:DVec3,
    /// Circling direction for ranged guardians (+1 / -1) and when it flips.
    pub orbit:f64,pub orbit_t:f32,
    /// Seconds without moving although trying (stuck): then it jumps.
    pub stuck:f32,
    /// How many attacks so far (picks the next one).
    pub count:u32,
}
impl BossRole {pub fn new(kind:BossKind,home:Cell,anchor:DVec3)->Self{Self{kind,home,anchor,target:anchor,cooldown:2.0,casting:0.0,attack:Attack::Slam,targets:Vec::new(),charge:0.0,charge_dir:DVec3::ZERO,orbit:1.0,orbit_t:3.0,stuck:0.0,count:0}}}

/// The guardians' attacks. Each is announced by a glowing ring on the ground (or rings),
/// then lands after the cast time: step out of the ring.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Attack {
    /// Stone: a ground slam all round itself (radius 6), throws the player up.
    Slam,
    /// Stone: hurls a boulder where the player stands (radius 2.5).
    Boulder,
    /// Stone (phase 3): charges straight at the player.
    Charge,
    /// Forest: roots burst from the ground under the player and two spots beside it, and
    /// hold you (slowness).
    Roots,
    /// Forest: calls two wolves from the woods.
    Summon,
    /// Forest (phase 3): draws life from the forest (heals while the cast lasts).
    Heal,
    /// Ember: a volley of fireballs: one on the player, more around in later phases.
    Fireballs,
    /// Ember: a ring of fire round itself (radius 4.5).
    FireRing,
}

impl BossKind {
    /// The attack after `count` attacks in `phase` (each guardian has its own rotation).
    pub fn next_attack(self,phase:u8,count:u32,dist:f64)->Attack{
        use Attack::*;
        match self {
            Self::Stone => if dist<5.5 {Slam} else if phase==3&&count%2==0 {Charge} else {Boulder},
            Self::Root => match (phase,count%3) {(3,2)=>Heal,(p,1) if p>=2=>Summon,_=>Roots},
            Self::Ember => if dist<4.5||(phase==3&&count%3==2) {FireRing} else {Fireballs},
        }
    }
    /// How far it likes to stay from the player (melee guardians 0).
    pub fn keep_away(self)->f64{match self{Self::Stone=>0.0,Self::Root=>7.0,Self::Ember=>9.0}}
}
impl Attack {
    pub fn radius(self)->f64{match self{Attack::Slam=>6.0,Attack::Boulder=>2.5,Attack::Roots=>2.2,Attack::Fireballs=>2.0,Attack::FireRing=>4.5,_=>0.0}}
    pub fn damage(self,phase:u8)->f32{let b=match self{Attack::Slam=>6.0,Attack::Boulder=>5.0,Attack::Charge=>8.0,Attack::Roots=>4.0,Attack::Fireballs=>4.0,Attack::FireRing=>6.0,_=>0.0};if phase==3{b*1.5}else{b}}
    pub fn cast_time(self,phase:u8)->f32{let b=match self{Attack::Slam=>1.4,Attack::Boulder=>1.2,Attack::Charge=>0.8,Attack::Roots=>1.1,Attack::Summon=>1.0,Attack::Heal=>2.5,Attack::Fireballs=>1.0,Attack::FireRing=>1.3};if phase==3{b*0.7}else{b}}
}
#[derive(Clone,Debug)]
pub struct BossDeath {pub kind:BossKind,pub home:Cell,pub at:DVec3}
#[derive(Clone,Copy)]
pub struct Record {pub hp:f32,pub defeated:bool}
#[derive(Default)]
pub struct BossBook {pub records:HashMap<(BossKind,Cell),Record>}
impl BossBook {
    pub fn begin(&mut self,kind:BossKind,home:Cell)->Result<f32,&'static str>{
        let r=self.records.entry((kind,home)).or_insert(Record{hp:kind.max_hp(),defeated:false});
        if r.defeated{return Err("Этот хранитель уже побеждён");} Ok(r.hp.clamp(1.,kind.max_hp()))
    }
    pub fn finish(&mut self,kind:BossKind,home:Cell)->bool{
        let r=self.records.entry((kind,home)).or_insert(Record{hp:kind.max_hp(),defeated:false});
        if r.defeated{return false;}r.hp=0.;r.defeated=true;true
    }
    pub fn load(path:&Path)->Self{
        let mut book=Self::default();let Ok(text)=std::fs::read_to_string(path)else{return book};
        for line in text.lines().take(10000){let n:Vec<&str>=line.split_whitespace().collect();if n.len()!=7{continue;}
            let parsed=(||Some((BossKind::from_id(n[0].parse().ok()?)?,Cell::new(n[1].parse().ok()?,n[2].parse().ok()?,n[3].parse().ok()?,n[4].parse().ok()?),n[5].parse::<f32>().ok()?,n[6]=="1")))();
            if let Some((kind,cell,hp,defeated))=parsed{if cell.f<6&&hp.is_finite(){book.records.insert((kind,cell),Record{hp:hp.clamp(0.,kind.max_hp()),defeated});}}
        }book
    }
    pub fn save(&self,path:&Path)->Result<(),String>{
        let mut entries:Vec<_>=self.records.iter().collect();entries.sort_by_key(|((k,c),_)|(*k as u8,c.f,c.i,c.j,c.k));
        let text=entries.into_iter().map(|((k,c),r)|format!("{} {} {} {} {} {} {}\n",*k as u8,c.f,c.i,c.j,c.k,r.hp,r.defeated as u8)).collect::<String>();
        let tmp=path.with_extension("tmp");std::fs::write(&tmp,text).map_err(|e|e.to_string())?;std::fs::rename(tmp,path).map_err(|e|e.to_string())
    }
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn rewards_are_rare_and_encounters_cannot_pay_twice(){let c=Cell::new(0,1,2,3);let mut b=BossBook::default();assert_eq!(b.begin(BossKind::Stone,c),Ok(180.));assert!(b.finish(BossKind::Stone,c));assert!(!b.finish(BossKind::Stone,c));assert!(b.begin(BossKind::Stone,c).is_err());for k in [BossKind::Stone,BossKind::Root,BossKind::Ember]{assert!(k.rewards().iter().all(|s|crate::items::is_valid(s.id)));assert_eq!(k.phase(1.),3);}}
 #[test]fn encounter_state_survives_reload(){let p=std::env::temp_dir().join(format!("wc_boss_{}.txt",std::process::id()));let mut b=BossBook::default();let c=Cell::new(2,3,4,5);b.begin(BossKind::Root,c).unwrap();b.records.get_mut(&(BossKind::Root,c)).unwrap().hp=44.;b.save(&p).unwrap();assert_eq!(BossBook::load(&p).begin(BossKind::Root,c),Ok(44.));std::fs::remove_file(p).unwrap();}
 #[test]fn every_guardian_has_its_own_attacks_and_phase_three_is_harder(){
  use Attack::*;
  // Stone: slams up close, throws boulders from afar, charges in phase 3.
  assert_eq!(BossKind::Stone.next_attack(1,0,3.0),Slam);
  assert_eq!(BossKind::Stone.next_attack(1,0,10.0),Boulder);
  assert_eq!(BossKind::Stone.next_attack(3,0,10.0),Charge);
  // Forest: roots, wolves from phase 2, healing in phase 3.
  assert_eq!(BossKind::Root.next_attack(1,0,8.0),Roots);
  assert_eq!(BossKind::Root.next_attack(1,1,8.0),Roots);
  assert_eq!(BossKind::Root.next_attack(2,1,8.0),Summon);
  assert_eq!(BossKind::Root.next_attack(3,2,8.0),Heal);
  // Ember: fireballs from afar, a fire ring up close or in phase 3.
  assert_eq!(BossKind::Ember.next_attack(1,0,9.0),Fireballs);
  assert_eq!(BossKind::Ember.next_attack(1,0,3.0),FireRing);
  assert_eq!(BossKind::Ember.next_attack(3,2,9.0),FireRing);
  // The ranged ones keep their distance; phase 3 hits harder and casts faster.
  assert!(BossKind::Root.keep_away()>0.0&&BossKind::Ember.keep_away()>BossKind::Root.keep_away()&&BossKind::Stone.keep_away()==0.0);
  for a in [Slam,Boulder,Charge,Roots,Fireballs,FireRing] {assert!(a.damage(3)>a.damage(1)&&a.cast_time(3)<a.cast_time(1));}
 }
}
