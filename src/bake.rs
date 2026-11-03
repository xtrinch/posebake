//! Posing, skinning, part filtering and colouring: from glTF sources to one static mesh.

use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use glam::{Mat4, Vec3};

use crate::pose;
use crate::recipe::Recipe;
use crate::source::Source;
use crate::texture;

/// A static triangle mesh in the sources' model space (glTF is Y up).
#[derive(Debug, Default)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    /// Unit normal per vertex.
    pub normals: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    /// sRGB colour (0-1) per triangle.
    pub colors: Vec<Vec3>,
}

/// Pose, skin, filter and colour every source of the recipe into one mesh. Returns the mesh
/// and notes about anything skipped.
pub fn bake(recipe: &Recipe) -> Result<(Mesh, Vec<String>)> {
    let mut mesh = Mesh::default();
    let mut notes = Vec::new();
    for spec in &recipe.sources {
        let source = Source::load(&spec.gltf)?;
        let (posed, skipped) = pose::apply(&source, &recipe.pose)?;
        let name = spec.gltf.display();
        notes.extend(skipped.into_iter().map(|note| format!("{name}: {note}")));

        let mut keep: Option<HashSet<usize>> = None;
        for joint in &spec.keep_joints {
            let Some(index) = source.node(joint) else {
                bail!("{name}: keep_joints names {joint}, which is not a node in the file");
            };
            keep.get_or_insert_with(HashSet::new)
                .extend(source.subtree(index));
        }

        let before = mesh.triangles.len();
        for node in source.document.nodes() {
            let Some(gltf_mesh) = node.mesh() else {
                continue;
            };
            let names = [node.name(), gltf_mesh.name()];
            if !spec.nodes.is_empty()
                && !names
                    .iter()
                    .flatten()
                    .any(|n| spec.nodes.iter().any(|wanted| wanted == n))
            {
                continue;
            }
            let joint_matrices: Option<Vec<(usize, Mat4)>> = node.skin().map(|skin| {
                let reader = skin.reader(|buffer| Some(&source.buffers[buffer.index()]));
                let inverse_binds: Vec<Mat4> = match reader.read_inverse_bind_matrices() {
                    Some(matrices) => matrices.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
                    None => vec![Mat4::IDENTITY; skin.joints().count()],
                };
                skin.joints()
                    .zip(inverse_binds)
                    .map(|(joint, inverse_bind)| {
                        (joint.index(), posed[joint.index()] * inverse_bind)
                    })
                    .collect()
            });
            for primitive in gltf_mesh.primitives() {
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    notes.push(format!(
                        "{name}: skipped a non-triangle primitive in {:?}",
                        node.name()
                    ));
                    continue;
                }
                add_primitive(
                    &mut mesh,
                    &source,
                    &primitive,
                    joint_matrices.as_deref(),
                    posed[node.index()],
                    keep.as_ref(),
                    recipe.keep_threshold,
                )?;
            }
        }
        if mesh.triangles.len() == before {
            notes.push(format!("{name}: no triangles were kept"));
        }
    }
    Ok((mesh, notes))
}

fn add_primitive(
    mesh: &mut Mesh,
    source: &Source,
    primitive: &gltf::Primitive,
    joint_matrices: Option<&[(usize, Mat4)]>,
    node_matrix: Mat4,
    keep: Option<&HashSet<usize>>,
    keep_threshold: f32,
) -> Result<()> {
    let reader = primitive.reader(|buffer| Some(&source.buffers[buffer.index()]));
    let Some(positions) = reader.read_positions() else {
        return Ok(());
    };
    let positions: Vec<Vec3> = positions.map(Vec3::from).collect();
    let indices: Vec<u32> = match reader.read_indices() {
        Some(indices) => indices.into_u32().collect(),
        None => (0..positions.len() as u32).collect(),
    };
    let normals: Option<Vec<Vec3>> = reader.read_normals().map(|n| n.map(Vec3::from).collect());

    // Skin each vertex: a weighted blend of its joints' matrices. Unskinned meshes, and
    // skinned primitives without joint data, follow their node.
    let skin_data = joint_matrices.and_then(|matrices| {
        let joints: Vec<[u16; 4]> = reader.read_joints(0)?.into_u16().collect();
        let weights: Vec<[f32; 4]> = reader.read_weights(0)?.into_f32().collect();
        Some((matrices, joints, weights))
    });
    let mut vertex_matrix = Vec::with_capacity(positions.len());
    let mut kept_weight = Vec::with_capacity(positions.len());
    for v in 0..positions.len() {
        match &skin_data {
            Some((matrices, joints, weights)) => {
                let mut matrix = Mat4::ZERO;
                let mut kept = 0.0;
                for k in 0..4 {
                    let weight = weights[v][k];
                    if weight == 0.0 {
                        continue;
                    }
                    let Some(&(joint_node, joint_matrix)) = matrices.get(joints[v][k] as usize)
                    else {
                        bail!("a vertex refers to joint {} outside its skin", joints[v][k]);
                    };
                    matrix += joint_matrix * weight;
                    if keep.is_some_and(|set| set.contains(&joint_node)) {
                        kept += weight;
                    }
                }
                vertex_matrix.push(matrix);
                kept_weight.push(kept);
            }
            None => {
                vertex_matrix.push(node_matrix);
                // Rigid geometry belongs entirely to its node.
                kept_weight.push(if keep.is_some() { 0.0 } else { 1.0 });
            }
        }
    }

    // Colour source: the material's base colour factor times its base colour texture.
    let material = primitive.material();
    let pbr = material.pbr_metallic_roughness();
    let factor = Vec3::from_slice(&pbr.base_color_factor()[..3]);
    let texture = pbr.base_color_texture().and_then(|info| {
        let uvs: Vec<[f32; 2]> = reader
            .read_tex_coords(info.tex_coord())?
            .into_f32()
            .collect();
        Some((source.images.get(&info.texture().source().index())?, uvs))
    });

    let mut remap: HashMap<u32, u32> = HashMap::new();
    let first_vertex = mesh.positions.len();
    let first_triangle = mesh.triangles.len();
    for &corners in indices.as_chunks::<3>().0 {
        if corners.iter().any(|&c| c as usize >= positions.len()) {
            bail!("a triangle refers to a vertex past the end of its primitive");
        }
        if keep.is_some()
            && corners
                .iter()
                .any(|&c| kept_weight[c as usize] < keep_threshold)
        {
            continue;
        }
        let color = match &texture {
            Some((image, uvs)) => {
                let u = corners.iter().map(|&c| uvs[c as usize][0]).sum::<f32>() / 3.0;
                let v = corners.iter().map(|&c| uvs[c as usize][1]).sum::<f32>() / 3.0;
                let linear = texture::srgb_to_linear(texture::sample(image, u, v)) * factor;
                texture::linear_to_srgb(linear)
            }
            None => texture::linear_to_srgb(factor),
        };
        let mut triangle = [0u32; 3];
        for (slot, &corner) in triangle.iter_mut().zip(&corners) {
            *slot = *remap.entry(corner).or_insert_with(|| {
                let matrix = vertex_matrix[corner as usize];
                mesh.positions
                    .push(matrix.transform_point3(positions[corner as usize]));
                let normal = normals.as_ref().map_or(Vec3::ZERO, |n| {
                    matrix
                        .transform_vector3(n[corner as usize])
                        .normalize_or_zero()
                });
                mesh.normals.push(normal);
                (mesh.positions.len() - 1) as u32
            });
        }
        mesh.triangles.push(triangle);
        mesh.colors.push(color);
    }

    // Without file normals, give each vertex the area-weighted average of its faces.
    if normals.is_none() {
        for t in first_triangle..mesh.triangles.len() {
            let [a, b, c] = mesh.triangles[t].map(|i| i as usize);
            let face = (mesh.positions[b] - mesh.positions[a])
                .cross(mesh.positions[c] - mesh.positions[a]);
            for i in [a, b, c] {
                mesh.normals[i] += face;
            }
        }
        for normal in &mut mesh.normals[first_vertex..] {
            *normal = normal.normalize_or_zero();
        }
    }
    Ok(())
}
