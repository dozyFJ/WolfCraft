fn main() {
    for pack in ["builtin:16", "builtin:toon", "builtin:64", "builtin:128"] {
        let t = std::time::Instant::now();
        let a = wolfcraft_assets::AtlasManager::load(pack, None, None).unwrap();
        let ms = t.elapsed().as_millis();
        let t2 = std::time::Instant::now();
        let _ = a.relief();
        let rms = t2.elapsed().as_millis();
        let mut total = 0u64;
        let mut desc = String::new();
        for k in wolfcraft_assets::AtlasKind::ALL {
            let img = a.get(k).image();
            total += (img.width * img.height * 4) as u64;
            desc += &format!("{:?} {}x{} ({} спр.)  ", k, img.width, img.height, a.get(k).len());
        }
        let r = a.relief().image();
        desc += &format!("relief {}x{}", r.width, r.height);
        total += (r.width * r.height) as u64;
        println!("{pack}: {ms} ms (+рельеф {rms} ms), {:.1} MB  {desc}", total as f64 / 1e6);
    }
}
