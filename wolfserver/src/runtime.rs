//! Server-side Luau extensions. Each core/plugin has an isolated VM and persistent data.
use crate::profile::{self,Profile};
use mlua::{Lua,Value,Function,VmState};
use std::{cell::RefCell,collections::{HashMap,HashSet},path::Path,sync::{Arc,Mutex},time::{Instant,Duration}};
use wolfcraft_engine::{block::Block,config::WorldConfig,cubesphere::Cell,planet::Planet};
use wolfcraft_net::{server::{ServerHooks,State},S2C};

const BUDGET:Duration=Duration::from_millis(15);
const MAX_SCRIPT:usize=512*1024;
const MAX_WRITES:usize=256;
const PRELUDE:&str=r#"
server = {}
world = {}
for _,name in ipairs({"broadcast","log","kick","players"}) do
  server[name] = function(...) return __server_api[name](...) end
end
for _,name in ipairs({"get","set"}) do
  world[name] = function(...) return __world_api[name](...) end
end
local handlers = {}
on = function(event,fn)
  assert(type(fn)=="function", "handler must be a function")
  handlers[event] = handlers[event] or {}
  table.insert(handlers[event],fn)
end
function __emit(event,data)
  local allowed=true
  for _,fn in ipairs(handlers[event] or {}) do
    if fn(data)==false then allowed=false end
  end
  return allowed
end
"#;
struct Plugin {id:String,lua:Lua,source:String,deadline:Arc<Mutex<Instant>>,started:bool,disabled:bool}
pub struct Runtime {plugins:Mutex<Vec<Plugin>>,planet:Planet,hell:Planet}
fn source(path:&Path)->Result<String,String>{
 let bytes=std::fs::read(path).map_err(|e|format!("{}: {e}",path.display()))?;
 if bytes.len()>MAX_SCRIPT{return Err("скрипт больше 512 КБ".into());}String::from_utf8(bytes).map_err(|e|e.to_string())
}
fn lua_json(lua:&Lua,v:&serde_json::Value)->mlua::Result<Value>{Ok(match v{
 serde_json::Value::Null=>Value::Nil,serde_json::Value::Bool(v)=>Value::Boolean(*v),serde_json::Value::Number(n)=>Value::Number(n.as_f64().unwrap_or(0.)),serde_json::Value::String(s)=>Value::String(lua.create_string(s)?),
 serde_json::Value::Array(a)=>{let t=lua.create_table()?;for (i,v) in a.iter().enumerate(){t.set(i+1,lua_json(lua,v)?)?;}Value::Table(t)},
 serde_json::Value::Object(o)=>{let t=lua.create_table()?;for (k,v) in o{t.set(k.as_str(),lua_json(lua,v)?)?;}Value::Table(t)},
})}
impl Plugin {
 fn new(id:&str,path:&Path,root:&Path)->Result<Self,String>{
  let source_text=source(&profile::contained(root,path)?)?;
  let lua=Lua::new();lua.set_memory_limit(16*1024*1024).map_err(|e|e.to_string())?;
  let deadline=Arc::new(Mutex::new(Instant::now()+BUDGET));let d=deadline.clone();
  lua.set_interrupt(move |_|{if Instant::now()>*d.lock().unwrap(){return Err(mlua::Error::runtime("превышен лимит времени скрипта"));}Ok(VmState::Continue)});
  lua.sandbox(true).map_err(|e|e.to_string())?;
  lua.load(PRELUDE).exec().map_err(|e|e.to_string())?;
  let modules=root.join("modules");let cache=lua.create_table().map_err(|e|e.to_string())?;
  let loading=Arc::new(Mutex::new(HashSet::<String>::new()));
  let require=lua.create_function(move |lua,id:String|{
   if !profile::valid_id(&id){return Err(mlua::Error::runtime("неверное имя модуля"));}
   let cached:Value=cache.get(id.as_str())?;if !matches!(cached,Value::Nil){return Ok(cached);}
   if !loading.lock().unwrap().insert(id.clone()){return Err(mlua::Error::runtime("циклическая зависимость модулей"));}
   let result=(||{let path=profile::contained(&modules,&modules.join(format!("{id}.lua"))).map_err(mlua::Error::runtime)?;let text=source(&path).map_err(mlua::Error::runtime)?;lua.load(&text).set_name(id.as_str()).eval::<Value>()})();
   loading.lock().unwrap().remove(&id);
   let value=result?;let value=if matches!(value,Value::Nil){Value::Boolean(true)}else{value};cache.set(id.as_str(),value.clone())?;Ok(value)
  }).map_err(|e|e.to_string())?;
  lua.globals().set("require",require).map_err(|e|e.to_string())?;
  let data_path=root.join("data").join(format!("{id}.json"));
  let values:HashMap<String,String>=if data_path.exists(){let b=std::fs::read(&data_path).map_err(|e|e.to_string())?;if b.len()>1024*1024{return Err("данные плагина больше 1 МБ".into());}serde_json::from_slice(&b).map_err(|e|format!("{}: {e}",data_path.display()))?}else{HashMap::new()};
  let values=Arc::new(Mutex::new(values));let store=lua.create_table().map_err(|e|e.to_string())?;
  let v=values.clone();store.set("get",lua.create_function(move |_,key:String|Ok(v.lock().unwrap().get(&key).cloned())).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
  store.set("set",lua.create_function(move |_,(key,value):(String,String)|{
    if key.len()>64||value.len()>4096{return Err(mlua::Error::runtime("слишком большой ключ или значение"));}
    let mut values=values.lock().unwrap();let mut next=values.clone();next.insert(key,value);
    let bytes=serde_json::to_vec(&next).map_err(mlua::Error::external)?;
    if bytes.len()>1024*1024{return Err(mlua::Error::runtime("данные плагина больше 1 МБ"));}
    let tmp=data_path.with_extension("tmp");std::fs::write(&tmp,&bytes).map_err(mlua::Error::external)?;std::fs::rename(&tmp,&data_path).map_err(mlua::Error::external)?;*values=next;Ok(())
  }).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
  lua.globals().set("store",store).map_err(|e|e.to_string())?;
  Ok(Self{id:id.into(),lua,source:source_text,deadline,started:false,disabled:false})
 }
 fn call(&mut self,event:&str,data:&serde_json::Value,state:&mut State,planet:&Planet,hell:&Planet)->Result<bool,String>{
  *self.deadline.lock().unwrap()=Instant::now()+BUDGET;
  let state=RefCell::new(state);let writes=RefCell::new(0usize);
   let valid=|f:i32,i:i32,j:i32,k:i32|f>=0&&f<6&&i>=0&&j>=0&&i<planet.grid.n&&j<planet.grid.n&&k>=0&&k<planet.radius as i32+4096;
  self.lua.scope(|scope|{
   let server=self.lua.create_table()?;let world=self.lua.create_table()?;
   server.set("broadcast",scope.create_function_mut(|_,text:String|{let text:String=text.chars().filter(|c|!c.is_control()).take(200).collect();state.borrow_mut().broadcast(&S2C::Chat{from:"Сервер".into(),text},None);Ok(())})?)?;
   server.set("log",scope.create_function_mut(|_,text:String|{state.borrow_mut().log(text.chars().take(500).collect::<String>());Ok(())})?)?;
   server.set("kick",scope.create_function_mut(|_,(id,reason):(u32,String)|{state.borrow_mut().kick(id,&reason.chars().take(200).collect::<String>());Ok(())})?)?;
   server.set("players",scope.create_function(|lua,()|{let s=state.borrow();let rows=lua.create_table()?;for (i,(id,p)) in s.players.iter().enumerate(){let t=lua.create_table()?;t.set("id",*id)?;t.set("nick",p.state.nick.as_str())?;t.set("role",p.role.name())?;rows.set(i+1,t)?;}Ok(rows)})?)?;
   world.set("get",scope.create_function(|_,(f,i,j,k,h):(i32,i32,i32,i32,Option<bool>)|{if !valid(f,i,j,k){return Ok(None);}let h=h.unwrap_or(false);let s=state.borrow();Ok(Some(s.edits[h as usize].get(&[f,i,j,k]).copied().unwrap_or_else(||if h{hell}else{planet}.block_at(Cell::new(f as u8,i,j,k)) as u16)))})?)?;
   world.set("set",scope.create_function_mut(|_,(f,i,j,k,id,h):(i32,i32,i32,i32,u16,Option<bool>)|{
    if !valid(f,i,j,k)||(id!=0&&Block::from_id(id)==Block::Air){return Err(mlua::Error::runtime("неверная ячейка или ID блока"));}
    let mut n=writes.borrow_mut();*n+=1;if *n>MAX_WRITES{return Err(mlua::Error::runtime("слишком много изменений за событие"));}
    let h=h.unwrap_or(false);let cell=[f,i,j,k];let mut s=state.borrow_mut();s.edits[h as usize].insert(cell,id);s.append_edit(h,cell,id);s.broadcast(&S2C::Edit{cell,block:id,hell:h},None);Ok(())
   })?)?;
   self.lua.globals().set("__server_api",server)?;self.lua.globals().set("__world_api",world)?;
   if !self.started {self.started=true;self.lua.load(&self.source).set_name(self.id.as_str()).exec()?;}
   self.lua.globals().get::<Function>("__emit")?.call::<bool>((event,lua_json(&self.lua,data)?))
  }).map_err(|e|e.to_string())
 }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {id:String,#[serde(default="entry")]entry:String,#[serde(default)]modules:Vec<String>}
fn entry()->String{"main.lua".into()}
impl Runtime {
 pub fn load(profile:&Profile,cfg:&WorldConfig)->Result<Self,String>{
  let mut plugins=vec![];
  if profile.config.core!="builtin" {if !profile.config.core.starts_with("core/"){return Err("ядро должно находиться в core/".into());}plugins.push(Plugin::new("core",&profile.root.join(&profile.config.core),&profile.root)?);}
  if profile.place&&profile.root.join("server.lua").exists(){plugins.push(Plugin::new("place",&profile.root.join("server.lua"),&profile.root)?);}
  for id in &profile.config.plugins {
   let folder=profile::contained(&profile.root.join("plugins"),&profile.root.join("plugins").join(id))?;
   let manifest_path=profile::contained(&folder,&folder.join("plugin.toml"))?;
   let manifest:Manifest=toml::from_str(&source(&manifest_path)?).map_err(|e|e.to_string())?;
   if manifest.id!=*id{return Err(format!("ID плагина {id} не совпадает с manifest"));}
   for module in &manifest.modules {if !profile::valid_id(module){return Err("неверный ID модуля".into());}profile::contained(&profile.root.join("modules"),&profile.root.join("modules").join(format!("{module}.lua")))?;}
   let script=profile::contained(&folder,&folder.join(&manifest.entry))?;plugins.push(Plugin::new(id,&script,&profile.root)?);
  }
  Ok(Self{plugins:Mutex::new(plugins),planet:Planet::new(cfg),hell:Planet::new_hell(cfg)})
 }
}
impl ServerHooks for Runtime {
 fn event(&self,name:&str,data:serde_json::Value,state:&mut State)->bool{
  if name=="block_edit" {let c=data["cell"].as_array();let Some(c)=c else{return false};let f=c[0].as_i64().unwrap_or(-1);let i=c[1].as_i64().unwrap_or(-1);let j=c[2].as_i64().unwrap_or(-1);let k=c[3].as_i64().unwrap_or(-1);let id=data["block"].as_u64().unwrap_or(65535) as u16;
   if !(0..6).contains(&f)||i<0||j<0||i>=self.planet.grid.n as i64||j>=self.planet.grid.n as i64||k<0||k>self.planet.radius as i64+4096||(id!=0&&Block::from_id(id)==Block::Air){return false;}}
  // kick() may trigger a leave event while a plugin is running. Do not re-enter its VM.
  let Ok(mut plugins)=self.plugins.try_lock()else{return true};let mut allowed=true;
  if name=="block_edit"&&plugins.iter().any(|p|p.disabled){return false;}
  for p in plugins.iter_mut().filter(|p|!p.disabled){match p.call(name,&data,state,&self.planet,&self.hell){Ok(ok)=>allowed&=ok,Err(e)=>{p.disabled=true;state.log(format!("Плагин {} отключён: {e}",p.id));allowed=false;}}}allowed
 }
 fn block_at(&self,c:[i32;4],hell:bool,state:&State)->Option<u16>{
  if !(0..6).contains(&c[0])||c[1]<0||c[2]<0||c[1]>=self.planet.grid.n||c[2]>=self.planet.grid.n||c[3]<0||c[3]>self.planet.radius as i32+4096{return None;}
  Some(state.edits[hell as usize].get(&c).copied().unwrap_or_else(||if hell{&self.hell}else{&self.planet}.block_at(Cell::new(c[0] as u8,c[1],c[2],c[3])) as u16))
 }
}

#[cfg(test)]mod tests{
 use super::*;
 use std::path::PathBuf;
 use wolfcraft_net::{server::{self,Role,ServerOptions},Client,C2S};
 fn wait(mut f:impl FnMut()->bool){let deadline=Instant::now()+Duration::from_secs(4);while !f(){assert!(Instant::now()<deadline,"timed out");std::thread::sleep(Duration::from_millis(10));}}
 fn fixture(name:&str,place:bool)->(PathBuf,WorldConfig,Profile){let d=std::env::temp_dir().join(format!("wolf_runtime_{}_{name}",std::process::id()));let _=std::fs::remove_dir_all(&d);let mut cfg=WorldConfig::new(name,5);cfg.studio_project=place;let p=profile::load(&d,&d.join("worlds/test"),&cfg).unwrap();(d,cfg,p)}
 fn start(p:&Profile,cfg:&WorldConfig)->server::ServerHandle{let runtime=Arc::new(Runtime::load(p,cfg).unwrap());server::start_with_options("Test",toml::to_string(cfg).unwrap(),&p.root,0,false,ServerOptions{place:p.place,max_players:16,owner_keys:vec![p.config.owner_key.clone()],editor_keys:vec!["editor-key-for-test-with-32-characters".into()],hooks:Some(runtime)}).unwrap()}
 #[test]fn plugin_and_module_execute_on_server_and_keep_data(){
  let(d,cfg,mut p)=fixture("plugins",false);p.config.plugins=vec!["hello".into()];let folder=p.root.join("plugins/hello");std::fs::create_dir_all(&folder).unwrap();
  std::fs::write(folder.join("plugin.toml"),"id='hello'\nmodules=['greet']\n").unwrap();std::fs::write(p.root.join("modules/greet.lua"),"return {text='module works'}").unwrap();
  std::fs::write(folder.join("main.lua"),r#"local greet=require('greet')
    local set=world.set
    on('start',function() store.set('message',greet.text);set(2,10,10,700,1) end)
    on('chat',function(e) if e.text=='ping' then server.broadcast('pong');return false end end)
    on('tick',function() set(2,11,10,700,1) end)
    on('block_edit',function(e) return e.block~=7 end)
  "#).unwrap();
  let s=start(&p,&cfg);let address=format!("127.0.0.1:{}",s.port);let mut c=Client::connect(&address,"Player",0).unwrap();
  wait(||s.shared.lock().unwrap().edits[0].contains_key(&[2,11,10,700]));assert_eq!(s.shared.lock().unwrap().edits[0][&[2,10,10,700]],1);
  c.send(&C2S::Chat("ping".into()));wait(||c.poll().iter().any(|m|matches!(m,S2C::Chat{text,..} if text=="pong")));
  c.send(&C2S::Edit{cell:[2,12,10,700],block:7,hell:false});wait(||c.poll().iter().any(|m|matches!(m,S2C::Edit{cell:[2,12,10,700],..})));
  assert!(!s.shared.lock().unwrap().edits[0].contains_key(&[2,12,10,700]));
  assert!(std::fs::read_to_string(p.root.join("data/hello.json")).unwrap().contains("module works"));s.stop();drop(c);drop(s);std::fs::remove_dir_all(d).unwrap();
 }
 #[test]fn place_edits_require_keys_and_nick_does_not_grant_roles(){
  let(d,cfg,p)=fixture("roles",true);std::fs::write(p.root.join("server.lua"),"on('start',function() world.set(2,5,5,700,1) end)").unwrap();let s=start(&p,&cfg);let a=format!("127.0.0.1:{}",s.port);
  let mut player=Client::connect(&a,"Owner",0).unwrap();assert_eq!(s.shared.lock().unwrap().players[&player.id].role,Role::Player);
  player.send(&C2S::Edit{cell:[2,5,5,700],block:7,hell:false});wait(||player.poll().iter().any(|m|matches!(m,S2C::Edit{cell:[2,5,5,700],block:1,..})));assert_eq!(s.shared.lock().unwrap().edits[0][&[2,5,5,700]],1);
  assert!(Client::connect_with_access(&a,"Owner",0,Some("wrong".into())).is_err());
  let mut editor=Client::connect_with_access(&a,"Owner",0,Some("editor-key-for-test-with-32-characters".into())).unwrap();assert_eq!(s.shared.lock().unwrap().players[&editor.id].role,Role::Editor);
  editor.send(&C2S::Edit{cell:[2,5,5,700],block:7,hell:false});wait(||s.shared.lock().unwrap().edits[0].get(&[2,5,5,700])==Some(&7));
  let owner=Client::connect_with_access(&a,"Player",0,Some(p.config.owner_key.clone())).unwrap();assert_eq!(s.shared.lock().unwrap().players[&owner.id].role,Role::Owner);
  s.stop();drop((owner,editor,player,s));std::fs::remove_dir_all(d).unwrap();
 }
 #[test]fn infinite_server_script_is_disabled_without_hanging_the_server(){
  let(d,cfg,p)=fixture("budget",true);std::fs::write(p.root.join("server.lua"),"on('tick',function() while true do end end)").unwrap();let s=start(&p,&cfg);wait(||s.shared.lock().unwrap().log.iter().any(|line|line.contains("отключён")));assert!(s.is_running());s.stop();drop(s);std::fs::remove_dir_all(d).unwrap();
 }
}
