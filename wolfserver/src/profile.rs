//! Separate extension roots: an ordinary server owns its runtime; each place owns its own.
use serde::{Deserialize, Serialize};
use std::path::{Path,PathBuf};
use wolfcraft_engine::config::WorldConfig;
#[derive(Serialize,Deserialize,Clone)]
#[serde(default,deny_unknown_fields)]
pub struct Config {
    pub version:u32,
    pub core:String,
    pub max_players:usize,
    pub owner_key:String,
    pub editor_keys:Vec<String>,
    pub plugins:Vec<String>,
}
impl Default for Config {fn default()->Self{Self{version:1,core:"builtin".into(),max_players:16,owner_key:String::new(),editor_keys:vec![],plugins:vec![]}}}
pub struct Profile {pub root:PathBuf,pub config:Config,pub place:bool}
pub fn valid_id(id:&str)->bool{!id.is_empty()&&id.len()<=64&&id.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'_'||c==b'-')}
pub fn secret()->Result<String,String>{let mut bytes=[0u8;32];getrandom::fill(&mut bytes).map_err(|e|e.to_string())?;Ok(bytes.iter().map(|b|format!("{b:02x}")).collect())}
pub fn load(data:&Path,world:&Path,cfg:&WorldConfig)->Result<Profile,String>{
    let place=cfg.studio_project;let root=if place{world}else{data}.to_path_buf();
    let path=root.join(if place{"place.toml"}else{"server.toml"});
    std::fs::create_dir_all(&root).map_err(|e|e.to_string())?;
    let mut config:Config=if path.exists(){toml::from_str(&std::fs::read_to_string(&path).map_err(|e|e.to_string())?).map_err(|e|format!("{}: {e}",path.display()))?}else{let mut c=Config::default();c.owner_key=secret()?;c};
    if config.version!=1||!(1..=128).contains(&config.max_players){return Err("неподдерживаемая версия или max_players (1..128)".into());}
    if config.owner_key.len()<32||config.editor_keys.iter().any(|k|k.len()<32){return Err("ключи владельца и редакторов должны содержать не менее 32 символов".into());}
    if config.editor_keys.iter().any(|k|k==&config.owner_key){return Err("ключ редактора не должен совпадать с ключом владельца".into());}
    if config.plugins.len()>32||config.plugins.iter().any(|id|!valid_id(id)||id=="core"||id=="place"){return Err("недопустимый ID плагина".into());}
    config.plugins.sort();config.plugins.dedup();
    if !path.exists(){let text=toml::to_string_pretty(&config).map_err(|e|e.to_string())?;std::fs::write(&path,text).map_err(|e|e.to_string())?;}
    for folder in ["plugins","modules","core","data"]{std::fs::create_dir_all(root.join(folder)).map_err(|e|e.to_string())?;}
    Ok(Profile{root,config,place})
}
/// Canonical containment also prevents module and plugin symlinks from escaping the root.
pub fn contained(root:&Path,path:&Path)->Result<PathBuf,String>{
    let root=root.canonicalize().map_err(|e|e.to_string())?;let path=path.canonicalize().map_err(|e|e.to_string())?;
    if !path.starts_with(&root){return Err("путь выходит за папку профиля".into());}Ok(path)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn roots_keys_and_configs_are_independent(){let d=std::env::temp_dir().join(format!("wolf_profiles_{}",std::process::id()));let _=std::fs::remove_dir_all(&d);let w=d.join("worlds/place");let mut c=WorldConfig::new("test",1);let a=load(&d,&w,&c).unwrap();c.studio_project=true;let b=load(&d,&w,&c).unwrap();assert_ne!(a.root,b.root);assert_ne!(a.config.owner_key,b.config.owner_key);assert!(d.join("server.toml").exists());assert!(w.join("place.toml").exists());assert_eq!(load(&d,&w,&c).unwrap().config.owner_key,b.config.owner_key);std::fs::write(w.join("place.toml"),"bad = [").unwrap();assert!(load(&d,&w,&c).is_err());assert_eq!(std::fs::read_to_string(w.join("place.toml")).unwrap(),"bad = [");std::fs::remove_dir_all(d).unwrap();}
 #[test]fn module_names_cannot_be_paths(){for id in ["../evil","","a/b","a.b","a\\b"]{assert!(!valid_id(id));}assert!(valid_id("my_module-1"));}
}
