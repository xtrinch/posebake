//! A loaded glTF file and its node hierarchy in rest pose.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use glam::{Mat4, Quat, Vec3};

/// A node's local transform as translation, rotation and scale.
#[derive(Clone, Copy, Debug)]
pub struct Trs {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Trs {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

pub struct Source {
    pub document: gltf::Document,
    pub buffers: Vec<gltf::buffer::Data>,
    /// Decoded images by image index; only those used as a base colour texture.
    pub images: HashMap<usize, gltf::image::Data>,
    /// Every node's local transform in the file's rest pose, by node index.
    pub rest_locals: Vec<Trs>,
    /// Model-space transform of every node in the file's rest pose, by node index.
    pub rest: Vec<Mat4>,
    parents: Vec<Option<usize>>,
}

impl Source {
    pub fn load(path: &Path) -> Result<Self> {
        let context = || format!("cannot load {}", path.display());
        let gltf::Gltf { document, blob } = gltf::Gltf::open(path).with_context(context)?;
        let folder = path.parent();
        let buffers = gltf::import_buffers(&document, folder, blob).with_context(context)?;

        // Normal, roughness and other maps are never read, so they are neither decoded nor
        // required to exist.
        let mut images = HashMap::new();
        for material in document.materials() {
            let Some(info) = material.pbr_metallic_roughness().base_color_texture() else {
                continue;
            };
            let image = info.texture().source();
            if images.contains_key(&image.index()) {
                continue;
            }
            let data = gltf::image::Data::from_source(image.source(), folder, &buffers)
                .with_context(|| {
                    format!("cannot decode a base colour texture of {}", path.display())
                })?;
            images.insert(image.index(), data);
        }

        let mut parents = vec![None; document.nodes().len()];
        for node in document.nodes() {
            for child in node.children() {
                parents[child.index()] = Some(node.index());
            }
        }
        let rest_locals: Vec<Trs> = document
            .nodes()
            .map(|node| {
                let (translation, rotation, scale) = node.transform().decomposed();
                Trs {
                    translation: Vec3::from(translation),
                    rotation: Quat::from_array(rotation),
                    scale: Vec3::from(scale),
                }
            })
            .collect();

        let mut source = Self {
            document,
            buffers,
            images,
            rest_locals,
            rest: Vec::new(),
            parents,
        };
        source.rest = source.globals(&source.rest_locals);
        Ok(source)
    }

    /// Model-space transforms of all nodes for the given local transforms.
    pub fn globals(&self, locals: &[Trs]) -> Vec<Mat4> {
        let locals: Vec<Mat4> = locals.iter().map(Trs::matrix).collect();
        let mut done = vec![None; locals.len()];
        for index in 0..locals.len() {
            global(index, &self.parents, &locals, &mut done);
        }
        done.into_iter().map(Option::unwrap).collect()
    }

    /// Index of the first node with this name.
    pub fn node(&self, name: &str) -> Option<usize> {
        self.document
            .nodes()
            .find(|node| node.name() == Some(name))
            .map(|node| node.index())
    }

    /// The node and every node below it.
    pub fn subtree(&self, index: usize) -> Vec<usize> {
        let mut nodes = vec![index];
        let mut next = 0;
        while next < nodes.len() {
            if let Some(node) = self.document.nodes().nth(nodes[next]) {
                nodes.extend(node.children().map(|child| child.index()));
            }
            next += 1;
        }
        nodes
    }
}

// Model-space transform of a node: its parent's, times its own local transform.
fn global(
    index: usize,
    parents: &[Option<usize>],
    locals: &[Mat4],
    done: &mut [Option<Mat4>],
) -> Mat4 {
    if let Some(matrix) = done[index] {
        return matrix;
    }
    let matrix = match parents[index] {
        Some(parent) => global(parent, parents, locals, done) * locals[index],
        None => locals[index],
    };
    done[index] = Some(matrix);
    matrix
}
