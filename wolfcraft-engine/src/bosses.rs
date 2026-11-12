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
pub struct BossRole {pub kind:BossKind,pub home:Cell,pub anchor:DVec3,pub target:DVec3,pub cooldown:f32,pub casting:f32}
impl BossRole {pub fn new(kind:BossKind,home:Cell,anchor:DVec3)->Self{Self{kind,home,anchor,target:anchor,cooldown:2.0,casting:0.0}}}
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
}
