//! Reading colours from decoded glTF textures.

use glam::Vec3;
use gltf::image::{Data, Format};

/// Colour of the texel at `uv` as stored (sRGB-encoded, 0-1). Nearest texel, with the
/// texture repeating outside 0-1; glTF's v runs down the image.
pub fn sample(image: &Data, u: f32, v: f32) -> Vec3 {
    let wrap = |t: f32, size: u32| ((t.rem_euclid(1.0) * size as f32) as u32).min(size - 1);
    let (x, y) = (wrap(u, image.width), wrap(v, image.height));
    texel(image, (y * image.width + x) as usize)
}

fn texel(image: &Data, index: usize) -> Vec3 {
    let channels = match image.format {
        Format::R8 | Format::R16 => 1,
        Format::R8G8 | Format::R16G16 => 2,
        Format::R8G8B8 | Format::R16G16B16 | Format::R32G32B32FLOAT => 3,
        Format::R8G8B8A8 | Format::R16G16B16A16 | Format::R32G32B32A32FLOAT => 4,
    };
    let channel = |c: usize| -> f32 {
        let at = index * channels + c;
        let pixels = &image.pixels;
        match image.format {
            Format::R8 | Format::R8G8 | Format::R8G8B8 | Format::R8G8B8A8 => {
                pixels[at] as f32 / 255.0
            }
            Format::R16 | Format::R16G16 | Format::R16G16B16 | Format::R16G16B16A16 => {
                u16::from_ne_bytes([pixels[at * 2], pixels[at * 2 + 1]]) as f32 / 65535.0
            }
            Format::R32G32B32FLOAT | Format::R32G32B32A32FLOAT => f32::from_ne_bytes([
                pixels[at * 4],
                pixels[at * 4 + 1],
                pixels[at * 4 + 2],
                pixels[at * 4 + 3],
            ]),
        }
    };
    // One or two channels are grey (plus alpha).
    if channels < 3 {
        Vec3::splat(channel(0))
    } else {
        Vec3::new(channel(0), channel(1), channel(2))
    }
}

pub fn srgb_to_linear(c: Vec3) -> Vec3 {
    c.map(|x| {
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    })
}

pub fn linear_to_srgb(c: Vec3) -> Vec3 {
    c.map(|x| {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.0031308 {
            x * 12.92
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    })
}
