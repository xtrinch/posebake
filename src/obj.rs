//! Writing a baked mesh as Wavefront OBJ with an MTL file of diffuse colours.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glam::Vec3;

use crate::bake::Mesh;

/// Paths written and how many colour materials the MTL holds.
pub struct Written {
    pub obj: PathBuf,
    pub mtl: PathBuf,
    pub materials: usize,
}

/// Write `<base>.obj` and `<base>.mtl`. Triangle colours are rounded to `palette_levels` steps
/// per channel (0 keeps 8-bit colours) and each distinct colour becomes one material.
pub fn write(
    mesh: &Mesh,
    base: &Path,
    header: Option<&str>,
    palette_levels: u32,
) -> Result<Written> {
    let obj_path = base.with_extension("obj");
    let mtl_path = base.with_extension("mtl");
    let (obj, mtl, materials) = to_strings(mesh, &mtl_path, header, palette_levels);
    if let Some(folder) = obj_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(folder)
            .with_context(|| format!("cannot create {}", folder.display()))?;
    }
    std::fs::write(&obj_path, obj)
        .with_context(|| format!("cannot write {}", obj_path.display()))?;
    std::fs::write(&mtl_path, mtl)
        .with_context(|| format!("cannot write {}", mtl_path.display()))?;
    Ok(Written {
        obj: obj_path,
        mtl: mtl_path,
        materials,
    })
}

/// The OBJ and MTL text, and the number of materials.
pub fn to_strings(
    mesh: &Mesh,
    mtl_path: &Path,
    header: Option<&str>,
    palette_levels: u32,
) -> (String, String, usize) {
    let mut comment = String::new();
    for line in header.unwrap_or_default().lines() {
        let _ = writeln!(comment, "# {line}");
    }

    let mut obj = comment.clone();
    let mtl_name = mtl_path.file_name().unwrap_or_default().to_string_lossy();
    let _ = writeln!(obj, "mtllib {mtl_name}");
    for p in &mesh.positions {
        let _ = writeln!(obj, "v {} {} {}", num(p.x), num(p.y), num(p.z));
    }
    for n in &mesh.normals {
        let _ = writeln!(obj, "vn {} {} {}", num(n.x), num(n.y), num(n.z));
    }

    let mut palette: HashMap<[u16; 3], usize> = HashMap::new();
    let mut ordered: Vec<[u16; 3]> = Vec::new();
    let mut current = None;
    for (triangle, color) in mesh.triangles.iter().zip(&mesh.colors) {
        let key = quantize(*color, palette_levels);
        let material = *palette.entry(key).or_insert_with(|| {
            ordered.push(key);
            ordered.len() - 1
        });
        if current != Some(material) {
            current = Some(material);
            let _ = writeln!(obj, "usemtl c{material}");
        }
        let [a, b, c] = triangle.map(|i| i + 1);
        let _ = writeln!(obj, "f {a}//{a} {b}//{b} {c}//{c}");
    }

    let mut mtl = comment;
    let steps = steps(palette_levels) as f32;
    for (index, key) in ordered.iter().enumerate() {
        let [r, g, b] = key.map(|c| num(c as f32 / steps));
        let _ = writeln!(mtl, "newmtl c{index}\nKd {r} {g} {b}");
    }
    (obj, mtl, ordered.len())
}

fn steps(palette_levels: u32) -> u32 {
    if palette_levels >= 2 {
        palette_levels - 1
    } else {
        255
    }
}

fn quantize(color: Vec3, palette_levels: u32) -> [u16; 3] {
    let steps = steps(palette_levels) as f32;
    color
        .to_array()
        .map(|c| (c.clamp(0.0, 1.0) * steps).round() as u16)
}

// Up to 5 decimals, without trailing zeros.
fn num(x: f32) -> String {
    let text = format!("{x:.5}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}
