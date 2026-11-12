//! Shelf-based rectangle packer with padding. Produces power-of-two atlases.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackedRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Result of packing: atlas size and one rect per input (same order as input).
#[derive(Debug)]
pub struct PackResult {
    pub width: u32,
    pub height: u32,
    pub rects: Vec<PackedRect>,
}

/// Packs `sizes` (w, h) into the smallest power-of-two atlas up to `max_size`.
/// `padding` pixels are reserved on every side of each rect (used for edge extrusion).
pub fn pack(sizes: &[(u32, u32)], padding: u32, max_size: u32) -> Option<PackResult> {
    if sizes.is_empty() {
        return Some(PackResult { width: 1, height: 1, rects: vec![] });
    }
    let area: u64 = sizes.iter().map(|&(w, h)| ((w + 2 * padding) as u64) * ((h + 2 * padding) as u64)).sum();
    let widest = sizes.iter().map(|s| s.0 + 2 * padding).max().unwrap_or(1);
    let mut width = (((area as f64).sqrt()) as u32).max(widest).next_power_of_two().max(16);
    while width <= max_size {
        // Try height == width, then half width rectangle if that also fits.
        if let Some(r) = try_pack(sizes, padding, width, max_size) {
            return Some(r);
        }
        width *= 2;
    }
    None
}

fn try_pack(sizes: &[(u32, u32)], padding: u32, width: u32, max_h: u32) -> Option<PackResult> {
    // Sort by height descending for tight shelves (stable for determinism).
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by(|&a, &b| sizes[b].1.cmp(&sizes[a].1).then(sizes[b].0.cmp(&sizes[a].0)).then(a.cmp(&b)));
    let mut rects = vec![PackedRect { x: 0, y: 0, w: 0, h: 0 }; sizes.len()];
    let (mut x, mut y, mut shelf_h) = (0u32, 0u32, 0u32);
    for i in order {
        let (w, h) = sizes[i];
        let pw = w + 2 * padding;
        let ph = h + 2 * padding;
        if pw > width {
            return None;
        }
        if x + pw > width {
            x = 0;
            y += shelf_h;
            shelf_h = 0;
        }
        rects[i] = PackedRect { x: x + padding, y: y + padding, w, h };
        x += pw;
        shelf_h = shelf_h.max(ph);
    }
    let used_h = (y + shelf_h).next_power_of_two();
    if used_h > max_h.min(width) {
        return None;
    }
    Some(PackResult { width, height: used_h.max(1), rects })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlaps(a: &PackedRect, b: &PackedRect, pad: u32) -> bool {
        let ax0 = a.x - pad;
        let ay0 = a.y - pad;
        let bx0 = b.x - pad;
        let by0 = b.y - pad;
        ax0 < b.x + b.w + pad && bx0 < a.x + a.w + pad && ay0 < b.y + b.h + pad && by0 < a.y + a.h + pad
    }

    #[test]
    fn packs_without_overlap() {
        let sizes: Vec<(u32, u32)> = (0..40).map(|i| (8 + (i % 5) * 8, 8 + (i % 3) * 16)).collect();
        let r = pack(&sizes, 2, 4096).unwrap();
        assert!(r.width.is_power_of_two() && r.height.is_power_of_two());
        for (i, a) in r.rects.iter().enumerate() {
            assert_eq!((a.w, a.h), sizes[i]);
            assert!(a.x + a.w + 2 <= r.width && a.y + a.h + 2 <= r.height);
            for b in &r.rects[i + 1..] {
                assert!(!overlaps(a, b, 2), "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn too_big_fails() {
        assert!(pack(&[(300, 300)], 0, 256).is_none());
    }
}
