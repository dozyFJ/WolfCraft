//! One server runtime, two creation profiles. Existing worlds always retain their settings.
use std::path::{Path,PathBuf};
use wolfcraft_engine::config::{WorldConfig,WorldType,GameMode,Difficulty,PlanetSize,slugify};

pub fn load_or_create(data:&Path,name:&str,args:&[String])->Result<(PathBuf,WorldConfig),String>{
    let value=|key:&str|args.iter().position(|a|a==key).and_then(|i|args.get(i+1)).filter(|v|!v.starts_with("--"));
    let has=|key:&str|args.iter().any(|a|a==key);
    if has("--place")&&has("--planet"){return Err("выберите --place или --planet".into());}
    if has("--creative")&&has("--survival"){return Err("выберите --creative или --survival".into());}
    let id=if has("--world"){
        let id=value("--world").ok_or("после --world нужен ID мира")?;
        if id.trim().is_empty()||id.contains(['/', '\\', '.'])||id.chars().any(char::is_control){return Err("недопустимый ID мира".into());}
        id.clone()
    }else{slugify(name)};
    let dir=data.join("worlds").join(&id);
    if dir.join("world.toml").exists(){
        let cfg=WorldConfig::load(&dir).map_err(|e|format!("не удалось прочитать {}: {e}; файл оставлен без изменений",dir.join("world.toml").display()))?;
        return Ok((dir,cfg));
    }
    // An explicit folder means open that world, never silently create a replacement.
    if has("--world"){return Err(format!("мир «{id}» не найден"));}
    let seed=if has("--seed"){value("--seed").ok_or("после --seed нужно число")?.parse::<i64>().map_err(|_|"неверный seed")? as u64}
        else{wolfcraft_common::platform::unix_now().wrapping_mul(6364136223846793005)};
    let mut cfg=WorldConfig::new(name,seed);cfg.id=id;
    let place=has("--place");cfg.studio_project=place;
    if place {
        cfg.world_type=WorldType::Baseplate;cfg.structures=false;cfg.size=PlanetSize::Small;cfg.difficulty=Difficulty::Peaceful;
        cfg.mode=GameMode::Creative;
    }
    if has("--creative"){cfg.mode=GameMode::Creative;}
    if has("--survival"){cfg.mode=GameMode::Survival;}
    cfg.save(&dir).map_err(|e|format!("не удалось сохранить мир: {e}"))?;
    Ok((dir,cfg))
}

#[cfg(test)]mod tests{
 use super::*;
 fn dir(name:&str)->PathBuf{let p=std::env::temp_dir().join(format!("wolf_server_profiles_{}_{name}",std::process::id()));let _=std::fs::remove_dir_all(&p);p}
 fn args(a:&[&str])->Vec<String>{a.iter().map(|s|s.to_string()).collect()}
 #[test]fn ordinary_and_place_have_separate_creation_profiles(){
  let d=dir("profiles");let (_,w)=load_or_create(&d,"Survival",&args(&["--seed","42"])).unwrap();
  assert!(!w.studio_project);assert_ne!(w.world_type,WorldType::Baseplate);assert_eq!(w.mode,GameMode::Survival);assert_eq!(w.seed,42);
  let (_,p)=load_or_create(&d,"Place",&args(&["--place"])).unwrap();assert!(p.studio_project);assert_eq!(p.world_type,WorldType::Baseplate);assert_eq!(p.mode,GameMode::Creative);assert!(!p.structures);
  std::fs::remove_dir_all(d).unwrap();
 }
 #[test]fn reopening_preserves_settings_and_corrupt_world_is_never_overwritten(){
  let d=dir("preserve");let (p,old)=load_or_create(&d,"Place",&args(&["--place","--seed","123"])).unwrap();
  let (_,new)=load_or_create(&d,"Ignored",&args(&["--world",&old.id,"--planet","--survival"])).unwrap();assert_eq!(new.seed,123);assert!(new.studio_project);assert_eq!(new.mode,GameMode::Creative);
  std::fs::write(p.join("world.toml"),"broken = [").unwrap();assert!(load_or_create(&d,"Place",&args(&[])).is_err());assert_eq!(std::fs::read_to_string(p.join("world.toml")).unwrap(),"broken = [");std::fs::remove_dir_all(d).unwrap();
 }
 #[test]fn explicit_world_and_conflicting_flags_do_not_create_surprise_worlds(){
  let d=dir("invalid");for a in [&["--world"][..],&["--world","../escape"],&["--world","missing"],&["--place","--planet"],&["--creative","--survival"]]{assert!(load_or_create(&d,"Test",&args(a)).is_err());}assert!(!d.exists());
 }
}
