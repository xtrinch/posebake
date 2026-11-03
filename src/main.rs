use std::path::PathBuf;

use clap::Parser;

/// Pose skinned glTF models and bake them into static OBJ or GLB meshes with colours sampled
/// from their textures.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Recipe file (TOML) describing the sources, pose and output.
    recipe: PathBuf,
    /// Output path without extension; overrides the recipe's `output`.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let report = posebake::run(&args.recipe, args.output.as_deref())?;
    for note in &report.notes {
        eprintln!("note: {note}");
    }
    for file in &report.files {
        println!("wrote {}", file.display());
    }
    println!(
        "{} triangles, {} vertices, {} colours",
        report.triangles, report.vertices, report.colors
    );
    Ok(())
}
