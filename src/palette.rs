//! Grouping triangle colours into a small set of materials.

use std::collections::HashMap;

use glam::Vec3;

/// Distinct rounded colours and, for each triangle, the index of its colour.
pub struct Palette {
    /// sRGB colours (0-1), in order of first use.
    pub colors: Vec<Vec3>,
    pub of_triangle: Vec<usize>,
}

/// Round colours to `levels` steps per channel (0 or 1 keeps 8-bit colours) so similar colours
/// share a palette entry.
pub fn group(colors: &[Vec3], levels: u32) -> Palette {
    let steps = if levels >= 2 { levels - 1 } else { 255 } as f32;
    let mut index: HashMap<[u16; 3], usize> = HashMap::new();
    let mut palette = Palette {
        colors: Vec::new(),
        of_triangle: Vec::with_capacity(colors.len()),
    };
    for color in colors {
        let key = color
            .to_array()
            .map(|c| (c.clamp(0.0, 1.0) * steps).round() as u16);
        let entry = *index.entry(key).or_insert_with(|| {
            palette
                .colors
                .push(Vec3::from_array(key.map(|c| c as f32 / steps)));
            palette.colors.len() - 1
        });
        palette.of_triangle.push(entry);
    }
    palette
}
