# posebake

Pose skinned glTF models and bake them into static OBJ or GLB meshes, coloured from their
materials' textures.

Typical use: you have a rigged, textured character in glTF (T-pose, colours in texture images,
body, outfit and hair as separate parts) and a renderer that wants a plain mesh with colours.
posebake merges the parts you want, poses them from one of the model's animations and/or your
own pose steps, and writes a static mesh whose colours are sampled from the textures.

- Merge several glTF/GLB files that share a skeleton, e.g. an outfit, a head and hair.
- Keep only some meshes, or only the triangles that follow certain joints
  (e.g. just the head of a full body).
- Start from a frame of one of the model's animations, at any time.
- Pose with simple steps: aim a bone chain in a direction, or rotate a joint.
- Bake with standard linear-blend skinning; normals follow the pose.
- Colour per triangle or per vertex, from the base colour texture times the base colour
  factor.
- Write OBJ + MTL and/or GLB.

## Install

```sh
cargo install --path .
```

## Usage

```sh
posebake recipe.toml
posebake recipe.toml --output out/character   # e.g. writes out/character.obj and .mtl
```

The recipe describes the whole bake. Paths are relative to the recipe file. Top-level keys
must come before the first `[table]`.

```toml
output = "woman-peasant"          # writes woman-peasant.obj, .mtl and .glb
formats = ["obj", "glb"]
header = "Peasant outfit and head from Quaternius packs (CC0)"

# An outfit, the head of a full base body, and hair, all on the same skeleton.
[[source]]
gltf = "Female_Peasant.gltf"

[[source]]
gltf = "Superhero_Female_FullBody.gltf"
nodes = ["Superhero_Female"]
keep_joints = ["Head", "neck_01"]

[[source]]
gltf = "Superhero_Female_FullBody.gltf"
nodes = ["Eyes", "Eyebrows"]

[[source]]
gltf = "Hair_Long.gltf"

# Lower both arms from the T-pose to 10 degrees out from straight down.
[[pose]]
kind = "aim"
joint = "upperarm_l"
toward = "hand_l"
direction = [0.17365, -0.98481, 0.0]

[[pose]]
kind = "aim"
joint = "upperarm_r"
toward = "hand_r"
direction = [-0.17365, -0.98481, 0.0]
```

Or freeze a character mid-animation:

```toml
output = "runner"

[animation]
name = "Run"
time = 0.25

[[source]]
gltf = "Casual.gltf"
```

## Recipe reference

Top level:

| Key | Default | Meaning |
|---|---|---|
| `output` | none | Output path without extension; `--output` overrides it |
| `formats` | `["obj"]` | Files to write: `"obj"` (OBJ + MTL) and/or `"glb"` |
| `colors` | `"triangle"` | `"triangle"`: one colour per triangle, sampled at its centre. `"vertex"`: one per vertex, sampled at its own texture coordinate |
| `header` | none | Credits or other text: comments in OBJ and MTL, `asset.copyright` in GLB |
| `palette_levels` | 32 | Colour steps per channel when rounding triangle colours; `0` keeps 8-bit colours |
| `keep_threshold` | 0.5 | Minimum share of a vertex's skin weight on `keep_joints` for its triangles to be kept |

`[animation]`, optional starting pose:

| Key | Default | Meaning |
|---|---|---|
| `name` | required | Name of an animation in the sources |
| `time` | 0 | Seconds from its start; clamped to its length |

Each source is posed by its own animation of that name. A source without it starts from its
rest pose, with a note.

`[[source]]`, one per glTF file, merged in order:

| Key | Default | Meaning |
|---|---|---|
| `gltf` | required | Path to a `.gltf` or `.glb` file |
| `nodes` | all | Keep only mesh nodes whose node name or mesh name is listed |
| `keep_joints` | all | Keep only triangles whose vertices all follow these joints (or their children) by at least `keep_threshold` |

`[[pose]]`, applied in order to every source, after the animation frame:

| `kind` | Keys | Meaning |
|---|---|---|
| `aim` | `joint`, `toward`, `direction` | Turn `joint` so the line from it to `toward` points along `direction` |
| `rotate` | `joint`, `axis`, `degrees` | Turn `joint` by `degrees` around `axis` (right-handed) |

Each step turns the joint and everything below it around the joint's position. Directions and
axes are in the model's space; glTF is Y up and models usually face +Z. A step naming a joint
that a source does not have is skipped for that source and reported, so a hair-only file can
share a recipe with a full body.

## Output

- **OBJ + MTL**: triangle colours are rounded to the palette and written as MTL materials
  (`Kd`, sRGB). With `colors = "vertex"`, vertices are also written as `v x y z r g b`, an OBJ
  extension most tools read; the materials stay for tools that do not.
- **GLB**: one mesh, one primitive and one white, non-metallic material, with colours in the
  standard `COLOR_0` attribute (linear, as glTF requires). Per-triangle colours give each
  triangle its own three vertices so the colour stays flat; per-vertex colours share
  vertices. Either way the mesh draws in a single call.

The output stays in the source model's space and units (glTF: Y up, metres).

## How it works

1. Each source is loaded and its node hierarchy evaluated in the rest pose. Only images used
   as base colour textures are decoded; normal and other maps are ignored and need not exist.
2. With `[animation]`, the named animation is sampled at `time` (step, linear with spherical
   rotation blending, or cubic spline) and replaces the animated nodes' local transforms.
3. The pose steps turn joint subtrees around their joint positions, in order.
4. Each vertex is skinned with its joints' posed matrices times their inverse bind matrices,
   blended by weight. Unskinned meshes follow their node's transform.
5. Triangles failing the `nodes` or `keep_joints` filters are dropped; vertices no kept
   triangle uses are not written.
6. Colours are the base colour texture at the triangle's centre (or the vertex's texture
   coordinate), converted to linear light, multiplied by the base colour factor and
   converted back to sRGB.

## Limits

- Per-triangle colours lose texture detail smaller than a triangle; per-vertex colours lose
  detail between vertices.
- Nearest texel, repeating texture coordinates; sampler settings and `KHR_texture_transform`
  are ignored.
- Only the first joint and weight set (`JOINTS_0`, `WEIGHTS_0`) is used.
- Morph targets are ignored.
- Normals are transformed with the skin matrix, which is exact for rotations and uniform
  scale.

## Library use

```rust
let report = posebake::run(std::path::Path::new("recipe.toml"), None)?;
println!("{} triangles", report.triangles);
```

`posebake::bake::bake` returns the mesh in memory; `posebake::obj` and `posebake::glb` turn it
into file contents.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.
