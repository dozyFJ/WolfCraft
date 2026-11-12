//! Persistent personal resonance storage, portable-crate payloads and item frames.
//! Portable items carry a 16-bit payload tag in their otherwise-unused damage field.
use crate::{cubesphere::Cell,items::{self,ItemStack}};
use std::{collections::HashMap,path::Path};
type Slots=Vec<Option<ItemStack>>;
pub struct StorageBank {pub shared:Slots,pub portable:HashMap<u16,Slots>,pub frames:HashMap<(bool,Cell),ItemStack>}
impl Default for StorageBank {fn default()->Self{Self{shared:vec![None;27],portable:HashMap::new(),frames:HashMap::new()}}}
impl StorageBank {
    pub fn pack(&mut self,slots:Slots)->Result<u16,Slots>{
        if self.portable.len()>=4096{return Err(slots);}
        let Some(tag)=(1..=u16::MAX).find(|id|!self.portable.contains_key(id))else{return Err(slots)};
        self.portable.insert(tag,slots);Ok(tag)
    }
    pub fn unpack(&mut self,tag:u16)->Option<Slots>{self.portable.remove(&tag)}
    pub fn load(path:&Path)->Self{
        let Ok(bytes)=std::fs::read(path)else{return Self::default()};
        match Self::decode(&bytes){Some(bank)=>bank,None=>{log::error!("invalid storage bank {}, preserving a backup",path.display());let _=std::fs::copy(path,path.with_extension("corrupt.bak"));Self::default()}}
    }
    pub fn save(&self,path:&Path)->Result<(),String>{let tmp=path.with_extension("tmp");std::fs::write(&tmp,self.encode()).map_err(|e|e.to_string())?;std::fs::rename(tmp,path).map_err(|e|e.to_string())}
    pub fn encode(&self)->Vec<u8>{
        let mut out=b"WCB1".to_vec();write_slots(&mut out,&self.shared);
        out.extend((self.portable.len() as u16).to_le_bytes());
        let mut payloads:Vec<_>=self.portable.iter().collect();payloads.sort_by_key(|(id,_)|**id);
        for (id,slots) in payloads{out.extend(id.to_le_bytes());write_slots(&mut out,slots);}
        out.extend((self.frames.len() as u32).to_le_bytes());
        for ((hell,c),stack) in &self.frames{out.extend([*hell as u8,c.f]);for n in [c.i,c.j,c.k]{out.extend(n.to_le_bytes());}write_slot(&mut out,Some(*stack));}
        out
    }
    pub fn decode(bytes:&[u8])->Option<Self>{
        if bytes.len()>16*1024*1024||bytes.get(..4)?!=b"WCB1"{return None;}let mut rd=Reader{b:bytes,i:4};
        let shared=rd.slots()?;if shared.len()!=27{return None;}
        let n=rd.u16()? as usize;if n>4096{return None;}let mut portable=HashMap::new();
        for _ in 0..n{let tag=rd.u16()?;let slots=rd.slots()?;if tag==0||portable.insert(tag,slots).is_some(){return None;}}
        let n=rd.u32()? as usize;if n>4096{return None;}let mut frames=HashMap::new();
        for _ in 0..n{let hell=rd.u8()?!=0;let f=rd.u8()?;let c=Cell::new(f,rd.i32()?,rd.i32()?,rd.i32()?);let stack=rd.slot()?;if f>=6{return None;}if let Some(s)=stack{frames.insert((hell,c),s);}}
        (rd.i==bytes.len()).then_some(Self{shared,portable,frames})
    }
}
fn write_slot(out:&mut Vec<u8>,slot:Option<ItemStack>){let s=slot.unwrap_or(ItemStack{id:u16::MAX,count:0,damage:0,ench:0});out.extend(s.id.to_le_bytes());out.push(s.count);out.extend(s.damage.to_le_bytes());out.extend(s.ench.to_le_bytes());}
fn write_slots(out:&mut Vec<u8>,slots:&Slots){out.extend((slots.len() as u16).to_le_bytes());for s in slots{write_slot(out,*s);}}
struct Reader<'a>{b:&'a[u8],i:usize}
impl Reader<'_>{
 fn bytes<const N:usize>(&mut self)->Option<[u8;N]>{let s=self.b.get(self.i..self.i+N)?;self.i+=N;s.try_into().ok()}
 fn u8(&mut self)->Option<u8>{Some(self.bytes::<1>()?[0])}fn u16(&mut self)->Option<u16>{Some(u16::from_le_bytes(self.bytes()?))}fn u32(&mut self)->Option<u32>{Some(u32::from_le_bytes(self.bytes()?))}fn i32(&mut self)->Option<i32>{Some(i32::from_le_bytes(self.bytes()?))}
 fn slot(&mut self)->Option<Option<ItemStack>>{let id=self.u16()?;let count=self.u8()?;let damage=self.u16()?;let ench=self.u32()?;if id==u16::MAX{return Some(None);}if !items::is_valid(id)||count==0||count>items::info(id).max_stack{return None;}Some(Some(ItemStack{id,count,damage,ench}))}
 fn slots(&mut self)->Option<Slots>{let n=self.u16()? as usize;if n>54{return None;}let mut slots=Vec::with_capacity(n);for _ in 0..n{slots.push(self.slot()?);}Some(slots)}
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn crates_keep_metadata_and_are_unpacked_once(){let mut b=StorageBank::default();let stack=ItemStack{id:items::ids::DIAMOND,count:7,damage:4,ench:17};let tag=b.pack(vec![Some(stack);27]).unwrap();b.shared[0]=Some(stack);let mut loaded=StorageBank::decode(&b.encode()).unwrap();assert_eq!(loaded.shared[0],Some(stack));assert_eq!(loaded.unpack(tag).unwrap()[0],Some(stack));assert!(loaded.unpack(tag).is_none());}
 #[test]fn broken_bank_does_not_decode_as_empty_storage(){let b=StorageBank::default().encode();assert!(StorageBank::decode(&b[..b.len()-1]).is_none());let mut b=b;b.push(1);assert!(StorageBank::decode(&b).is_none());}
}
