# posebake

Pose skinned glTF models and bake them into static OBJ meshes, with each triangle coloured
from its material's texture.

Typical use: you have a rigged, textured character in glTF (T-pose, colours in texture images,
body, outfit and hair as separate parts) and a renderer that wants a plain mesh with colours.
posebake merges the parts you want, lowers the arms or sets any other pose, and writes an
OBJ + MTL pair where every triangle has a solid colour sampled from the texture.

- Merge several glTF/GLB files that share a skeleton, e.g. an outfit, a head and hair.
- Keep only some meshes, or only the triangles that follow certain joints
  (e.g. just the head of a full body).
- Pose with simple steps: aim a bone chain in a direction, or rotate a joint.
- Bake with standard linear-blend skinning; normals follow the pose.
- Colour each triangle from its base colour texture times its base colour factor.
- Write OBJ + MTL, with similar colours grouped into a small palette of materials.

## Install

```sh
cargo install --path .
```

## Usage

```sh
posebake recipe.toml
posebake recipe.toml --output out/character   # writes out/character.obj and .mtl
```

The recipe describes the whole bake. Paths are relative to the recipe file.

```toml
output = "woman-peasant"          # writes woman-peasant.obj and woman-peasant.mtl
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

## Recipe reference

Top level:

| Key | Default | Meaning |
|---|---|---|
| `output` | none | Output path without extension; `--output` overrides it |
| `header` | none | Comment written at the top of the OBJ and MTL, e.g. credits |
| `palette_levels` | 32 | Colour steps per channel when grouping colours into materials; `0` keeps 8-bit colours |
| `keep_threshold` | 0.5 | Minimum share of a vertex's skin weight on `keep_joints` for its triangles to be kept |

`[[source]]`, one per glTF file, merged in order:

| Key | Default | Meaning |
|---|---|---|
| `gltf` | required | Path to a `.gltf` or `.glb` file |
| `nodes` | all | Keep only mesh nodes whose node name or mesh name is listed |
| `keep_joints` | all | Keep only triangles whose vertices all follow these joints (or their children) by at least `keep_threshold` |

`[[pose]]`, applied in order to every source:

| `kind` | Keys | Meaning |
|---|---|---|
| `aim` | `joint`, `toward`, `direction` | Turn `joint` so the line from it to `toward` points along `direction` |
| `rotate` | `joint`, `axis`, `degrees` | Turn `joint` by `degrees` around `axis` (right-handed) |

Each step turns the joint and everything below it around the joint's position. Directions and
axes are in the model's space; glTF is Y up and models usually face +Z. A step naming a joint
that a source does not have is skipped for that source and reported, so a hair-only file can
share a recipe with a full body.

## How it works

1. Each source is loaded and its node hierarchy evaluated in the rest pose. Only images used
   as base colour textures are decoded; normal and other maps are ignored and need not exist.
2. The pose steps turn joint subtrees around their joint positions, in order.
3. Each vertex is skinned with its joints' posed matrices times their inverse bind matrices,
   blended by weight. Unskinned meshes follow their node's transform.
4. Triangles failing the `nodes` or `keep_joints` filters are dropped; vertices no kept
   triangle uses are not written.
5. Each triangle's colour is the base colour texture sampled at the average of its corners'
   texture coordinates, converted to linear light, multiplied by the base colour factor and
   converted back to sRGB.
6. Colours are rounded to the palette and written as MTL materials; the OBJ references them.

The output stays in the source model's space and units (glTF: Y up, metres).

## Limits

- One colour per triangle: texture detail smaller than a triangle is lost.
- Nearest texel, repeating texture coordinates; sampler settings and `KHR_texture_transform`
  are ignored.
- Only the first joint and weight set (`JOINTS_0`, `WEIGHTS_0`) is used.
- Morph targets and animations are ignored; poses come from the recipe.
- Normals are transformed with the skin matrix, which is exact for rotations and uniform
  scale.
- Output is OBJ + MTL only.

## Library use

```rust
let report = posebake::run(std::path::Path::new("recipe.toml"), None)?;
println!("{} triangles", report.triangles);
```

`posebake::bake::bake` returns the mesh in memory if you want to write another format.
