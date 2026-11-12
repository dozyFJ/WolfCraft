//! Simple RGBA8 image used for sprites and atlases.

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height, pixels: vec![0; (width * height * 4) as usize] }
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.pixels[i], self.pixels[i + 1], self.pixels[i + 2], self.pixels[i + 3]]
    }

    #[inline]
    pub fn set(&mut self, x: u32, y: u32, p: [u8; 4]) {
        let i = ((y * self.width + x) * 4) as usize;
        self.pixels[i..i + 4].copy_from_slice(&p);
    }

    /// Copies `src` to (dx, dy) and extrudes its border pixels `pad` pixels outward.
    /// Extrusion prevents texture bleeding when sampling near sprite edges.
    pub fn blit_extruded(&mut self, src: &Image, dx: u32, dy: u32, pad: u32) {
        let (w, h) = (src.width as i64, src.height as i64);
        let p = pad as i64;
        for y in -p..h + p {
            for x in -p..w + p {
                let tx = dx as i64 + x;
                let ty = dy as i64 + y;
                if tx < 0 || ty < 0 || tx >= self.width as i64 || ty >= self.height as i64 {
                    continue;
                }
                let sx = x.clamp(0, w - 1) as u32;
                let sy = y.clamp(0, h - 1) as u32;
                self.set(tx as u32, ty as u32, src.get(sx, sy));
            }
        }
    }

    /// Extracts a sub-rectangle.
    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> Image {
        let mut out = Image::new(w, h);
        for yy in 0..h {
            let s = (((y + yy) * self.width + x) * 4) as usize;
            let d = (yy * w * 4) as usize;
            out.pixels[d..d + (w * 4) as usize].copy_from_slice(&self.pixels[s..s + (w * 4) as usize]);
        }
        out
    }

    pub fn load_png(path: &std::path::Path) -> Result<Image, String> {
        let img = ::image::open(path).map_err(|e| format!("{}: {e}", path.display()))?.to_rgba8();
        Ok(Image { width: img.width(), height: img.height(), pixels: img.into_raw() })
    }

    pub fn load_png_bytes(bytes: &[u8]) -> Result<Image, String> {
        let img = ::image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8();
        Ok(Image { width: img.width(), height: img.height(), pixels: img.into_raw() })
    }

    pub fn save_png(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        ::image::save_buffer(path, &self.pixels, self.width, self.height, ::image::ExtendedColorType::Rgba8)
            .map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrusion_copies_edges() {
        let mut src = Image::new(2, 2);
        src.set(0, 0, [255, 0, 0, 255]);
        src.set(1, 1, [0, 255, 0, 255]);
        let mut dst = Image::new(6, 6);
        dst.blit_extruded(&src, 2, 2, 2);
        assert_eq!(dst.get(0, 0), [255, 0, 0, 255]); // corner extruded
        assert_eq!(dst.get(5, 5), [0, 255, 0, 255]);
        assert_eq!(dst.crop(2, 2, 2, 2), src);
    }
}
