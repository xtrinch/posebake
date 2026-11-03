//! Pose skinned glTF models and bake them into static OBJ or GLB meshes, coloured from their
//! materials' base colour textures.
//!
//! A bake is described by a [`Recipe`]: glTF sources to merge (optionally only some nodes,
//! or only the triangles that follow certain joints), a starting pose (rest pose or a frame of
//! an animation), pose steps applied to their shared skeleton, the colour mode and the output
//! files. See the README for the recipe format.

pub mod animation;
pub mod bake;
pub mod glb;
pub mod obj;
pub mod palette;
pub mod pose;
pub mod recipe;
pub mod source;
pub mod texture;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub use bake::Mesh;
pub use pose::PoseStep;
pub use recipe::{AnimationFrame, ColorMode, Format, Recipe, SourceSpec};

/// What a bake produced.
pub struct Report {
    pub files: Vec<PathBuf>,
    pub triangles: usize,
    pub vertices: usize,
    /// Distinct triangle colours after rounding to the palette.
    pub colors: usize,
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
    let palette = palette::group(&mesh.colors, recipe.palette_levels);
    let vertex_colors = recipe.colors == ColorMode::Vertex;
    let header = recipe.header.as_deref();

    let mut files = Vec::new();
    for format in &recipe.formats {
        files.extend(match format {
            Format::Obj => obj::write(&mesh, &palette, &base, header, vertex_colors)?,
            Format::Glb => glb::write(&mesh, &palette, &base, header, vertex_colors)?,
        });
    }
    Ok(Report {
        files,
        triangles: mesh.triangles.len(),
        vertices: mesh.positions.len(),
        colors: palette.colors.len(),
        notes,
    })
}

/// Write a file, creating its folder first.
pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(folder) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(folder)
            .with_context(|| format!("cannot create {}", folder.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}
