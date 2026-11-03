//! Writing a baked mesh as binary glTF (GLB).

use std::path::{Path, PathBuf};

use anyhow::Result;
use glam::Vec3;
use serde_json::{Value, json};

use crate::bake::Mesh;
use crate::palette::Palette;
use crate::texture::srgb_to_linear;

const FLOAT: u32 = 5126;
const UNSIGNED_INT: u32 = 5125;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;

/// Write `<base>.glb`; returns its path. Colours are stored as `COLOR_0` vertex colours on a
/// single primitive with a white material; per-triangle colours use unshared vertices.
pub fn write(
    mesh: &Mesh,
    palette: &Palette,
    base: &Path,
    header: Option<&str>,
    vertex_colors: bool,
) -> Result<Vec<PathBuf>> {
    let path = base.with_extension("glb");
    crate::write_file(&path, &to_bytes(mesh, palette, header, vertex_colors))?;
    Ok(vec![path])
}

/// The GLB file contents.
pub fn to_bytes(
    mesh: &Mesh,
    palette: &Palette,
    header: Option<&str>,
    vertex_colors: bool,
) -> Vec<u8> {
    let mut bin = Bin::default();
    let mut asset = json!({ "version": "2.0", "generator": "posebake" });
    if let Some(header) = header {
        asset["copyright"] = json!(header);
    }
    let mut document = json!({ "asset": asset, "scene": 0, "scenes": [{ "nodes": [] }] });

    if !mesh.triangles.is_empty() {
        // Per-vertex colours share vertices. Per-triangle colours give every triangle its own
        // three vertices with the triangle's palette colour, so the mesh stays one primitive
        // (one draw call) instead of one per colour.
        let (positions, normals, colors, indices): (Vec<Vec3>, Vec<Vec3>, Vec<Vec3>, Vec<u32>) =
            if vertex_colors {
                (
                    mesh.positions.clone(),
                    mesh.normals.clone(),
                    mesh.vertex_colors.clone(),
                    mesh.triangles.iter().flatten().copied().collect(),
                )
            } else {
                let corners: Vec<(usize, u32)> = mesh
                    .triangles
                    .iter()
                    .zip(&palette.of_triangle)
                    .flat_map(|(triangle, &entry)| triangle.map(|v| (entry, v)))
                    .collect();
                (
                    corners
                        .iter()
                        .map(|&(_, v)| mesh.positions[v as usize])
                        .collect(),
                    corners
                        .iter()
                        .map(|&(_, v)| mesh.normals[v as usize])
                        .collect(),
                    corners
                        .iter()
                        .map(|&(entry, _)| palette.colors[entry])
                        .collect(),
                    (0..corners.len() as u32).collect(),
                )
            };
        // glTF colours are linear.
        let colors: Vec<Vec3> = colors.into_iter().map(srgb_to_linear).collect();

        let min = positions.iter().fold(Vec3::MAX, |a, &b| a.min(b));
        let max = positions.iter().fold(Vec3::MIN, |a, &b| a.max(b));
        let attributes = json!({
            "POSITION": bin.vec3s(&positions, Some((min, max))),
            "NORMAL": bin.vec3s(&normals, None),
            "COLOR_0": bin.vec3s(&colors, None),
        });
        let primitive = json!({
            "attributes": attributes,
            "indices": bin.indices(&indices),
            "material": 0
        });
        // White, so the vertex colours show unchanged.
        let material = json!({ "pbrMetallicRoughness": {
            "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
            "metallicFactor": 0.0,
            "roughnessFactor": 1.0
        }});
        document["scenes"][0]["nodes"] = json!([0]);
        document["nodes"] = json!([{ "mesh": 0 }]);
        document["meshes"] = json!([{ "primitives": [primitive] }]);
        document["materials"] = json!([material]);
        document["accessors"] = json!(bin.accessors);
        document["bufferViews"] = json!(bin.views);
        document["buffers"] = json!([{ "byteLength": bin.data.len() }]);
    }

    let mut json_chunk = serde_json::to_vec(&document).expect("glTF JSON serializes");
    while !json_chunk.len().is_multiple_of(4) {
        json_chunk.push(b' ');
    }
    let mut bin_chunk = bin.data;
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }
    let with_bin = !bin_chunk.is_empty();
    let length = 12 + 8 + json_chunk.len() + if with_bin { 8 + bin_chunk.len() } else { 0 };

    let mut out = Vec::with_capacity(length);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(length as u32).to_le_bytes());
    out.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_chunk);
    if with_bin {
        out.extend_from_slice(&(bin_chunk.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin_chunk);
    }
    out
}

/// The binary chunk being built, with its buffer views and accessors.
#[derive(Default)]
struct Bin {
    data: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Bin {
    fn view(&mut self, bytes: &[u8], target: u32) -> usize {
        let offset = self.data.len();
        self.data.extend_from_slice(bytes);
        self.views.push(json!({
            "buffer": 0, "byteOffset": offset, "byteLength": bytes.len(), "target": target
        }));
        self.views.len() - 1
    }

    fn vec3s(&mut self, values: &[Vec3], bounds: Option<(Vec3, Vec3)>) -> usize {
        let bytes: Vec<u8> = values
            .iter()
            .flat_map(|v| v.to_array())
            .flat_map(f32::to_le_bytes)
            .collect();
        let view = self.view(&bytes, ARRAY_BUFFER);
        let mut accessor = json!({ "bufferView": view, "componentType": FLOAT, "count": values.len(), "type": "VEC3" });
        if let Some((min, max)) = bounds {
            accessor["min"] = json!(min.to_array());
            accessor["max"] = json!(max.to_array());
        }
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    fn indices(&mut self, indices: &[u32]) -> usize {
        let bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let view = self.view(&bytes, ELEMENT_ARRAY_BUFFER);
        self.accessors.push(json!({
            "bufferView": view, "componentType": UNSIGNED_INT, "count": indices.len(), "type": "SCALAR"
        }));
        self.accessors.len() - 1
    }
}
