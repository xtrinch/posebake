# Working in this repository

posebake poses skinned glTF models and bakes them into static OBJ/GLB meshes with colours
sampled from their textures. It is a library (`src/lib.rs`) and a CLI (`src/main.rs`) driven
by a TOML recipe; the README documents the recipe and is the user-facing reference.

## Architecture
- `recipe.rs` parses and validates the recipe; paths resolve relative to the recipe file.
- `source.rs` loads a glTF/GLB, evaluates its node hierarchy and decodes only images used as
  base colour textures. Other maps (normal, metallic, ...) are never loaded and need not exist.
- `animation.rs` samples one named animation at a time (step, linear with slerp, cubic spline)
  into node local transforms.
- `pose.rs` applies `aim` and `rotate` steps after the animation frame. A step naming a joint a
  source lacks is skipped and reported as a note, not an error.
- `bake.rs` does linear-blend skinning, the `nodes`/`keep_joints` filters and colour sampling,
  producing one in-memory `Mesh`.
- `texture.rs` holds sampling and sRGB/linear conversion; `palette.rs` rounds triangle colours.
- `obj.rs` and `glb.rs` turn a `Mesh` into file contents; `lib.rs::run` ties it together and
  returns a `Report`.

## Rules
- Output stays in the source model's space and units (glTF: Y up, metres). Do not add axis or
  unit conversion for a particular engine; that belongs in the consumer.
- Colours: textures and OBJ/MTL are sRGB; GLB `COLOR_0` is linear. Multiply by the base colour
  factor in linear light.
- GLB output is one mesh, one primitive and one material, so it draws in one call. Per-triangle
  colours use unshared vertices rather than a primitive per colour.
- Keep the recipe format backwards compatible; new keys need defaults. Update the README's
  recipe reference with every recipe change.
- Keep dependencies small; do not add a dependency for what a few lines do.
- Keep changes scoped to the requested feature or fix; avoid optional refactoring.

## Commands and checks
- `cargo test` runs the tests in `tests/bake.rs`. They build a small glTF fixture in code
  (root -> arm -> hand, a 2x2 texture, step/linear/cubic animations) and check hand-worked
  expected values. Add a test there for each behaviour change.
- `cargo clippy --all-targets` and `cargo fmt --check` must be clean.
- Validate GLB changes with the Khronos glTF validator (`gltf_validator file.glb`): no errors
  or warnings.
- Do not commit test models, baked output or `target/`. Third-party models are only used
  locally for manual checks.

## Commits
- Do not add `Co-Authored-By` trailers or other AI attribution to commits, tags, releases or
  PRs.

## Releases
- Published on crates.io as `posebake`. Bump the version in `Cargo.toml`, run
  `cargo publish --dry-run`, publish, then tag `vX.Y.Z` on the published commit and create a
  GitHub release.
- A published version cannot be deleted, only yanked.
