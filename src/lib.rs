//! Pose skinned glTF models and bake them into static OBJ meshes, with each triangle
//! coloured from its material's base-colour texture.
//!
//! A bake is described by a [`Recipe`]: glTF sources to merge (optionally only some nodes,
//! or only the triangles that follow certain joints), pose steps applied to their shared
//! skeleton, and an output path. See the README for the recipe format.

pub mod bake;
pub mod obj;
pub mod pose;
pub mod recipe;
pub mod source;
pub mod texture;

use std::path::Path;

use anyhow::{Result, bail};

pub use bake::Mesh;
pub use pose::PoseStep;
pub use recipe::{Recipe, SourceSpec};

/// What a bake produced.
pub struct Report {
    pub written: obj::Written,
    pub triangles: usize,
    pub vertices: usize,
    /// Anything skipped along the way, e.g. pose steps for joints a source does not have.
    pub notes: Vec<String>,
}

/// Bake a recipe file. `output` overrides the recipe's own output path (without extension).
pub fn run(recipe_path: &Path, output: Option<&Path>) -> Result<Report> {
    let recipe = Recipe::from_file(recipe_path)?;
    let Some(base) = output
        .map(Path::to_path_buf)
        .or_else(|| recipe.output.clone())
    else {
        bail!("no output path: set `output` in the recipe or pass --output");
    };
    let (mesh, notes) = bake::bake(&recipe)?;
    let written = obj::write(
        &mesh,
        &base,
        recipe.header.as_deref(),
        recipe.palette_levels,
    )?;
    Ok(Report {
        written,
        triangles: mesh.triangles.len(),
        vertices: mesh.positions.len(),
        notes,
    })
}
