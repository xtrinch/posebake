//! The TOML recipe that describes what to bake.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::pose::PoseStep;

/// Everything needed for one bake: which glTF sources to merge, how to pose them and
/// where to write the result.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// Output path without extension; `.obj` and `.mtl` are added. Relative to the recipe.
    pub output: Option<PathBuf>,
    /// Comment written at the top of both output files, e.g. credits and licenses.
    #[serde(default)]
    pub header: Option<String>,
    /// Colour levels per channel when grouping triangle colours into materials. Colours are
    /// rounded to this many steps, so similar colours share a material; 0 keeps 8-bit colours.
    #[serde(default = "default_palette_levels")]
    pub palette_levels: u32,
    /// Minimum share of a vertex's skin weight on `keep_joints` for its triangles to be kept.
    #[serde(default = "default_keep_threshold")]
    pub keep_threshold: f32,
    /// glTF files to merge, in order.
    #[serde(rename = "source", default)]
    pub sources: Vec<SourceSpec>,
    /// Pose steps, applied in order to every source that has the named joints.
    #[serde(default)]
    pub pose: Vec<PoseStep>,
}

/// One glTF file and the parts of it to keep.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSpec {
    /// Path to a `.gltf` or `.glb` file, relative to the recipe.
    pub gltf: PathBuf,
    /// Keep only mesh nodes whose node or mesh name is listed. Empty keeps all.
    #[serde(default)]
    pub nodes: Vec<String>,
    /// Keep only triangles whose vertices mostly follow these joints or their children,
    /// e.g. `["Head"]` to take just the head of a full body. Empty keeps all.
    #[serde(default)]
    pub keep_joints: Vec<String>,
}

fn default_palette_levels() -> u32 {
    32
}

fn default_keep_threshold() -> f32 {
    0.5
}

impl Recipe {
    /// Parse a recipe from TOML text. Relative paths inside stay relative.
    pub fn parse(text: &str) -> Result<Self> {
        let recipe: Recipe = toml::from_str(text).context("invalid recipe")?;
        if recipe.sources.is_empty() {
            anyhow::bail!("the recipe has no [[source]] entries");
        }
        Ok(recipe)
    }

    /// Read a recipe file and resolve its relative paths against the file's folder.
    pub fn from_file(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read recipe {}", path.display()))?;
        let mut recipe =
            Self::parse(&text).with_context(|| format!("in recipe {}", path.display()))?;
        let base = path.parent().unwrap_or(Path::new("."));
        for source in &mut recipe.sources {
            source.gltf = base.join(&source.gltf);
        }
        recipe.output = recipe.output.map(|output| base.join(output));
        Ok(recipe)
    }
}
