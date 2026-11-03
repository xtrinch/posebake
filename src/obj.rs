//! Writing a baked mesh as Wavefront OBJ with an MTL file of diffuse colours.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::bake::Mesh;
use crate::palette::Palette;

/// Write `<base>.obj` and `<base>.mtl`; returns their paths. Each palette colour becomes one
/// material. With `vertex_colors`, vertices are also written as `v x y z r g b`, a widely
/// supported OBJ extension.
pub fn write(
    mesh: &Mesh,
    palette: &Palette,
    base: &Path,
    header: Option<&str>,
    vertex_colors: bool,
) -> Result<Vec<PathBuf>> {
    let obj_path = base.with_extension("obj");
    let mtl_path = base.with_extension("mtl");
    let (obj, mtl) = to_strings(mesh, palette, &mtl_path, header, vertex_colors);
    crate::write_file(&obj_path, obj.as_bytes())?;
    crate::write_file(&mtl_path, mtl.as_bytes())?;
    Ok(vec![obj_path, mtl_path])
}

/// The OBJ and MTL text.
pub fn to_strings(
    mesh: &Mesh,
    palette: &Palette,
    mtl_path: &Path,
    header: Option<&str>,
    vertex_colors: bool,
) -> (String, String) {
    let mut comment = String::new();
    for line in header.unwrap_or_default().lines() {
        let _ = writeln!(comment, "# {line}");
    }

    let mut obj = comment.clone();
    let mtl_name = mtl_path.file_name().unwrap_or_default().to_string_lossy();
    let _ = writeln!(obj, "mtllib {mtl_name}");
    for (index, p) in mesh.positions.iter().enumerate() {
        let _ = write!(obj, "v {} {} {}", num(p.x), num(p.y), num(p.z));
        if vertex_colors {
            let c = mesh.vertex_colors[index];
            let _ = write!(obj, " {} {} {}", num(c.x), num(c.y), num(c.z));
        }
        obj.push('\n');
    }
    for n in &mesh.normals {
        let _ = writeln!(obj, "vn {} {} {}", num(n.x), num(n.y), num(n.z));
    }
    let mut current = None;
    for (triangle, &material) in mesh.triangles.iter().zip(&palette.of_triangle) {
        if current != Some(material) {
            current = Some(material);
            let _ = writeln!(obj, "usemtl c{material}");
        }
        let [a, b, c] = triangle.map(|i| i + 1);
        let _ = writeln!(obj, "f {a}//{a} {b}//{b} {c}//{c}");
    }

    let mut mtl = comment;
    for (index, color) in palette.colors.iter().enumerate() {
        let [r, g, b] = color.to_array().map(num);
        let _ = writeln!(mtl, "newmtl c{index}\nKd {r} {g} {b}");
    }
    (obj, mtl)
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
