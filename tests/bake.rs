//! End-to-end tests on a small generated glTF: a three-joint chain (root -> arm -> hand), five
//! vertices and three triangles, and a 2x2 texture. Expected results are worked out by hand.

use std::path::{Path, PathBuf};

use base64::Engine;
use serde_json::json;

const ROOT: u16 = 0;
const ARM: u16 = 1;

// Vertex positions (z = 0) with the joint weights they follow.
//   v0 (0, 0)   root        v3 (0, 1)   root
//   v1 (2, 0)   arm         v4 (3, 0.5) arm
//   v2 (2, 1)   half root, half arm
const POSITIONS: [[f32; 3]; 5] = [
    [0.0, 0.0, 0.0],
    [2.0, 0.0, 0.0],
    [2.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
    [3.0, 0.5, 0.0],
];
const JOINTS: [[u16; 4]; 5] = [
    [ROOT, 0, 0, 0],
    [ARM, 0, 0, 0],
    [ROOT, ARM, 0, 0],
    [ROOT, 0, 0, 0],
    [ARM, 0, 0, 0],
];
const WEIGHTS: [[f32; 4]; 5] = [
    [1.0, 0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0],
    [0.5, 0.5, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0],
];
// Triangle centres land on texel (1, 0) red, (0, 1) green and (1, 0) red.
const UVS: [[f32; 2]; 5] = [
    [0.25, 0.25],
    [0.75, 0.25],
    [0.75, 0.75],
    [0.25, 0.75],
    [0.9, 0.1],
];
const INDICES: [u16; 9] = [0, 1, 2, 0, 2, 3, 1, 4, 2];
// 90 degrees around z as a quaternion (x, y, z, w).
const QUARTER_TURN: [f32; 4] = [
    0.0,
    0.0,
    std::f32::consts::FRAC_1_SQRT_2,
    std::f32::consts::FRAC_1_SQRT_2,
];

/// A fresh, empty folder for one test.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("posebake-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn bytes<T: Copy>(values: &[T], to_le: impl Fn(T) -> Vec<u8>) -> Vec<u8> {
    values.iter().flat_map(|&v| to_le(v)).collect()
}

/// Write the test model to `dir/model.gltf` with an embedded buffer and texture.
fn write_model(dir: &Path, base_color_factor: [f32; 4]) -> PathBuf {
    let f32s = |v: f32| v.to_le_bytes().to_vec();
    let u16s = |v: u16| v.to_le_bytes().to_vec();
    let inverse_bind = |x: f32| {
        [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -x, 0.0, 0.0, 1.0,
        ]
    };
    let views: Vec<Vec<u8>> = vec![
        bytes(&POSITIONS.concat(), f32s),
        bytes(&[[0.0f32, 0.0, 1.0]; 5].concat(), f32s),
        bytes(&UVS.concat(), f32s),
        bytes(&JOINTS.concat(), u16s),
        bytes(&WEIGHTS.concat(), f32s),
        bytes(&INDICES, u16s),
        // Inverse bind matrices of root (at x 0), arm (x 1) and hand (x 2).
        bytes(
            &[inverse_bind(0.0), inverse_bind(1.0), inverse_bind(2.0)].concat(),
            f32s,
        ),
        // Animation keys at 0 and 1 seconds, turning the arm 0 -> 90 degrees around z.
        bytes(&[0.0f32, 1.0], f32s),
        bytes(&[[0.0f32, 0.0, 0.0, 1.0], QUARTER_TURN].concat(), f32s),
        // The same as cubic spline keys (in-tangent, value, out-tangent) with zero tangents.
        bytes(
            &[
                [0.0f32; 4],
                [0.0, 0.0, 0.0, 1.0],
                [0.0; 4],
                [0.0; 4],
                QUARTER_TURN,
                [0.0; 4],
            ]
            .concat(),
            f32s,
        ),
    ];
    let mut buffer = Vec::new();
    let mut buffer_views = Vec::new();
    for view in &views {
        buffer_views
            .push(json!({ "buffer": 0, "byteOffset": buffer.len(), "byteLength": view.len() }));
        buffer.extend_from_slice(view);
    }

    // 2x2 texture: blue, red / green, white.
    let texture =
        image::RgbImage::from_raw(2, 2, vec![0, 0, 255, 255, 0, 0, 0, 255, 0, 255, 255, 255])
            .unwrap();
    let mut png = Vec::new();
    texture
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let base64 = base64::engine::general_purpose::STANDARD;

    let accessor = |view: usize, component: u32, count: usize, kind: &str| json!({ "bufferView": view, "componentType": component, "count": count, "type": kind });
    // An animation rotating the arm (node 1), keyed at accessor 7 with outputs at `output`.
    let animation = |name: &str, output: usize, interpolation: &str| {
        json!({
            "name": name,
            "channels": [{ "sampler": 0, "target": { "node": 1, "path": "rotation" } }],
            "samplers": [{ "input": 7, "output": output, "interpolation": interpolation }]
        })
    };
    let document = json!({
        "asset": { "version": "2.0" },
        "scene": 0,
        "scenes": [{ "nodes": [0, 3] }],
        "nodes": [
            { "name": "root", "children": [1] },
            { "name": "arm", "translation": [1.0, 0.0, 0.0], "children": [2] },
            { "name": "hand", "translation": [1.0, 0.0, 0.0] },
            { "name": "skinned_quad", "mesh": 0, "skin": 0 }
        ],
        "skins": [{ "joints": [0, 1, 2], "inverseBindMatrices": 6 }],
        "meshes": [{
            "name": "quad",
            "primitives": [{
                "attributes": { "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2, "JOINTS_0": 3, "WEIGHTS_0": 4 },
                "indices": 5,
                "material": 0
            }]
        }],
        "materials": [{
            "pbrMetallicRoughness": { "baseColorFactor": base_color_factor, "baseColorTexture": { "index": 0 } }
        }],
        "textures": [{ "source": 0 }],
        "images": [{ "uri": format!("data:image/png;base64,{}", base64.encode(&png)) }],
        "accessors": [
            {
                "bufferView": 0, "componentType": 5126, "count": 5, "type": "VEC3",
                "min": [0.0, 0.0, 0.0], "max": [3.0, 1.0, 0.0]
            },
            accessor(1, 5126, 5, "VEC3"),
            accessor(2, 5126, 5, "VEC2"),
            accessor(3, 5123, 5, "VEC4"),
            accessor(4, 5126, 5, "VEC4"),
            accessor(5, 5123, 9, "SCALAR"),
            accessor(6, 5126, 3, "MAT4"),
            { "bufferView": 7, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0.0], "max": [1.0] },
            accessor(8, 5126, 2, "VEC4"),
            accessor(9, 5126, 6, "VEC4")
        ],
        "animations": [
            animation("Turn", 8, "LINEAR"),
            animation("Snap", 8, "STEP"),
            animation("Smooth", 9, "CUBICSPLINE")
        ],
        "bufferViews": buffer_views,
        "buffers": [{
            "byteLength": buffer.len(),
            "uri": format!("data:application/octet-stream;base64,{}", base64.encode(&buffer))
        }]
    });
    let path = dir.join("model.gltf");
    std::fs::write(&path, serde_json::to_string_pretty(&document).unwrap()).unwrap();
    path
}

/// Parsed output: vertex positions, triangles (0-based) and each triangle's colour.
struct Output {
    positions: Vec<[f32; 3]>,
    triangles: Vec<[usize; 3]>,
    colors: Vec<[f32; 3]>,
    materials: usize,
    notes: Vec<String>,
}

/// Write `recipe` next to a fresh model, bake it and read the result back.
fn bake(name: &str, recipe: &str) -> Output {
    let dir = folder(name);
    write_model(&dir, [1.0, 1.0, 1.0, 1.0]);
    bake_in(&dir, recipe)
}

fn bake_in(dir: &Path, recipe: &str) -> Output {
    let recipe_path = dir.join("recipe.toml");
    std::fs::write(&recipe_path, recipe).unwrap();
    let report = posebake::run(&recipe_path, Some(&dir.join("out"))).unwrap();

    let mut materials = std::collections::HashMap::new();
    let mut current = String::new();
    for line in std::fs::read_to_string(dir.join("out.mtl"))
        .unwrap()
        .lines()
    {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.first() {
            Some(&"newmtl") => current = parts[1].to_string(),
            Some(&"Kd") => {
                let rgb: Vec<f32> = parts[1..4].iter().map(|v| v.parse().unwrap()).collect();
                materials.insert(current.clone(), [rgb[0], rgb[1], rgb[2]]);
            }
            _ => {}
        }
    }

    let mut output = Output {
        positions: Vec::new(),
        triangles: Vec::new(),
        colors: Vec::new(),
        materials: materials.len(),
        notes: report.notes,
    };
    let mut color = [0.0; 3];
    for line in std::fs::read_to_string(dir.join("out.obj"))
        .unwrap()
        .lines()
    {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.first() {
            Some(&"v") => {
                let p: Vec<f32> = parts[1..4].iter().map(|v| v.parse().unwrap()).collect();
                output.positions.push([p[0], p[1], p[2]]);
            }
            Some(&"usemtl") => color = materials[parts[1]],
            Some(&"f") => {
                let corner = |s: &str| s.split('/').next().unwrap().parse::<usize>().unwrap() - 1;
                output
                    .triangles
                    .push([corner(parts[1]), corner(parts[2]), corner(parts[3])]);
                output.colors.push(color);
            }
            _ => {}
        }
    }
    output
}

fn source_line(dir_relative: &str) -> String {
    format!("[[source]]\ngltf = \"{dir_relative}\"\n")
}

fn assert_close(actual: [f32; 3], expected: [f32; 3]) {
    for k in 0..3 {
        assert!(
            (actual[k] - expected[k]).abs() < 1e-4,
            "{actual:?} != {expected:?}"
        );
    }
}

#[test]
fn rest_pose_keeps_positions_and_samples_texture_colours() {
    let out = bake("rest", &source_line("model.gltf"));

    assert_eq!(out.positions.len(), 5);
    for (actual, expected) in out.positions.iter().zip(POSITIONS) {
        assert_close(*actual, expected);
    }
    assert_eq!(out.triangles, vec![[0, 1, 2], [0, 2, 3], [1, 4, 2]]);
    assert_eq!(
        out.colors,
        vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]]
    );
    assert_eq!(out.materials, 2);
}

#[test]
fn rotate_turns_the_joint_and_blends_shared_vertices() {
    let recipe = source_line("model.gltf")
        + "[[pose]]\nkind = \"rotate\"\njoint = \"arm\"\naxis = [0.0, 0.0, 1.0]\ndegrees = 90.0\n";
    let out = bake("rotate", &recipe);

    // Turned 90 degrees counter-clockwise around the arm joint at (1, 0).
    assert_close(out.positions[0], [0.0, 0.0, 0.0]); // root: unchanged
    assert_close(out.positions[1], [1.0, 1.0, 0.0]); // arm: (2, 0) -> (1, 1)
    assert_close(out.positions[2], [1.0, 1.0, 0.0]); // half: midway between (2, 1) and (0, 1)
    assert_close(out.positions[3], [0.0, 1.0, 0.0]); // root: unchanged
    assert_close(out.positions[4], [0.5, 2.0, 0.0]); // arm: (3, 0.5) -> (0.5, 2)
}

#[test]
fn aim_matches_the_equivalent_rotation() {
    let rotate = bake(
        "aim-rotate",
        &(source_line("model.gltf")
            + "[[pose]]\nkind = \"rotate\"\njoint = \"arm\"\naxis = [0.0, 0.0, 1.0]\ndegrees = 90.0\n"),
    );
    // The arm points from (1, 0) to the hand at (2, 0); aiming it up is the same quarter turn.
    let aim = bake(
        "aim",
        &(source_line("model.gltf")
            + "[[pose]]\nkind = \"aim\"\njoint = \"arm\"\ntoward = \"hand\"\ndirection = [0.0, 1.0, 0.0]\n"),
    );
    for (a, r) in aim.positions.iter().zip(&rotate.positions) {
        assert_close(*a, *r);
    }
}

#[test]
fn keep_joints_keeps_only_triangles_that_follow_those_joints() {
    let recipe = "[[source]]\ngltf = \"model.gltf\"\nkeep_joints = [\"arm\"]\n";
    let out = bake("keep", recipe);

    // Only v1, v4 (arm) and v2 (half arm, at the 0.5 threshold) qualify.
    assert_eq!(out.triangles.len(), 1);
    assert_eq!(out.positions.len(), 3, "unused vertices are not written");
    assert_eq!(out.colors, vec![[1.0, 0.0, 0.0]]);
}

#[test]
fn sources_merge_and_node_filters_apply() {
    let recipe = "[[source]]\ngltf = \"model.gltf\"\nnodes = [\"skinned_quad\"]\n\n\
                  [[source]]\ngltf = \"model.gltf\"\nnodes = [\"quad\"]\n\n\
                  [[source]]\ngltf = \"model.gltf\"\nnodes = [\"no_such_node\"]\n";
    let out = bake("merge", recipe);

    // Matched by node name and by mesh name; the third source contributes nothing.
    assert_eq!(out.triangles.len(), 6);
    assert_eq!(out.positions.len(), 10);
    assert!(
        out.notes
            .iter()
            .any(|note| note.contains("no triangles were kept"))
    );
}

#[test]
fn pose_steps_for_missing_joints_are_skipped_with_a_note() {
    let recipe = source_line("model.gltf")
        + "[[pose]]\nkind = \"rotate\"\njoint = \"upperarm_l\"\naxis = [0.0, 0.0, 1.0]\ndegrees = 45.0\n";
    let out = bake("missing-joint", &recipe);

    assert_close(out.positions[1], POSITIONS[1]);
    assert!(
        out.notes
            .iter()
            .any(|note| note.contains("no joint upperarm_l"))
    );
}

#[test]
fn base_colour_factor_multiplies_the_texture_in_linear_light() {
    let dir = folder("factor");
    write_model(&dir, [0.5, 1.0, 1.0, 1.0]);
    let out = bake_in(&dir, &source_line("model.gltf"));

    // Red texel (1.0) times a linear factor of 0.5, shown in sRGB.
    assert!(
        (out.colors[0][0] - 0.7354).abs() < 0.02,
        "{:?}",
        out.colors[0]
    );
}

#[test]
fn unknown_keep_joint_is_an_error() {
    let dir = folder("bad-joint");
    write_model(&dir, [1.0, 1.0, 1.0, 1.0]);
    let recipe = dir.join("recipe.toml");
    std::fs::write(
        &recipe,
        "[[source]]\ngltf = \"model.gltf\"\nkeep_joints = [\"tail\"]\n",
    )
    .unwrap();

    let error = posebake::run(&recipe, Some(&dir.join("out")))
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("tail"), "{error:#}");
}

#[test]
fn recipes_reject_unknown_fields_and_empty_sources() {
    assert!(posebake::Recipe::parse("output = \"x\"\n").is_err());
    assert!(posebake::Recipe::parse("[[source]]\ngltf = \"a.gltf\"\ncolour = 1\n").is_err());
    assert!(posebake::Recipe::parse("[[source]]\ngltf = \"a.gltf\"\n").is_ok());
}

#[test]
fn missing_normals_are_computed_from_the_faces() {
    let dir = folder("no-normals");
    let model = write_model(&dir, [1.0, 1.0, 1.0, 1.0]);
    let mut document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&model).unwrap()).unwrap();
    document["meshes"][0]["primitives"][0]["attributes"]
        .as_object_mut()
        .unwrap()
        .remove("NORMAL");
    std::fs::write(&model, document.to_string()).unwrap();
    bake_in(&dir, &source_line("model.gltf"));

    // The flat quad lies in z = 0, so every computed normal is along z.
    let obj = std::fs::read_to_string(dir.join("out.obj")).unwrap();
    let normals: Vec<Vec<f32>> = obj
        .lines()
        .filter_map(|line| line.strip_prefix("vn "))
        .map(|rest| rest.split(' ').map(|v| v.parse().unwrap()).collect())
        .collect();
    assert_eq!(normals.len(), 5);
    for n in normals {
        assert!(
            n[0].abs() < 1e-5 && n[1].abs() < 1e-5 && (n[2].abs() - 1.0).abs() < 1e-5,
            "{n:?}"
        );
    }
}

/// Bake a recipe next to a fresh model without parsing the output.
fn bake_only(name: &str, recipe: &str) -> (PathBuf, posebake::Report) {
    let dir = folder(name);
    write_model(&dir, [1.0, 1.0, 1.0, 1.0]);
    let recipe_path = dir.join("recipe.toml");
    std::fs::write(&recipe_path, recipe).unwrap();
    let report = posebake::run(&recipe_path, Some(&dir.join("out"))).unwrap();
    (dir, report)
}

#[test]
fn glb_with_triangle_colours_gives_each_triangle_flat_vertex_colours() {
    let recipe = format!("formats = [\"glb\"]\n\n{}", source_line("model.gltf"));
    let (dir, report) = bake_only("glb-triangle", &recipe);
    assert_eq!(report.files, vec![dir.join("out.glb")]);

    let (document, buffers, _) = gltf::import(dir.join("out.glb")).unwrap();
    assert_eq!(document.materials().count(), 1);
    let mesh = document.meshes().next().unwrap();
    assert_eq!(mesh.primitives().count(), 1, "one draw call");
    let primitive = mesh.primitives().next().unwrap();
    let reader = primitive.reader(|b| Some(&buffers[b.index()]));
    let positions: Vec<[f32; 3]> = reader.read_positions().unwrap().collect();
    let colours: Vec<[f32; 3]> = reader.read_colors(0).unwrap().into_rgb_f32().collect();
    let indices: Vec<u32> = reader.read_indices().unwrap().into_u32().collect();

    // Three unshared corners per triangle, in triangle order.
    assert_eq!(indices, (0..9).collect::<Vec<u32>>());
    for (corner, &vertex) in INDICES.iter().enumerate() {
        assert_close(positions[corner], POSITIONS[vertex as usize]);
    }
    let triangle_colours = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]];
    for (corner, colour) in colours.iter().enumerate() {
        assert_close(*colour, triangle_colours[corner / 3]);
    }
}

#[test]
fn vertex_colours_sample_each_vertex_in_glb_and_obj() {
    let recipe = format!(
        "formats = [\"glb\", \"obj\"]\ncolors = \"vertex\"\n\n{}",
        source_line("model.gltf")
    );
    let (dir, _) = bake_only("vertex-colours", &recipe);
    // Texels at each vertex: blue, red, white, green, red. Pure colours are equal in sRGB
    // and linear light.
    let expected = [
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
    ];

    let (document, buffers, _) = gltf::import(dir.join("out.glb")).unwrap();
    let primitive = document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .next()
        .unwrap();
    let reader = primitive.reader(|b| Some(&buffers[b.index()]));
    let colours: Vec<[f32; 3]> = reader.read_colors(0).unwrap().into_rgb_f32().collect();
    assert_eq!(colours.len(), 5);
    for (actual, expected) in colours.iter().zip(expected) {
        assert_close(*actual, expected);
    }

    let obj = std::fs::read_to_string(dir.join("out.obj")).unwrap();
    let vertices: Vec<Vec<f32>> = obj
        .lines()
        .filter_map(|line| line.strip_prefix("v "))
        .map(|rest| rest.split(' ').map(|v| v.parse().unwrap()).collect())
        .collect();
    for (vertex, expected) in vertices.iter().zip(expected) {
        assert_eq!(vertex.len(), 6, "{vertex:?}");
        assert_close([vertex[3], vertex[4], vertex[5]], expected);
    }
}

fn animated(name: &str, animation: &str, time: f32) -> Vec<[f32; 3]> {
    let recipe = format!(
        "[animation]\nname = \"{animation}\"\ntime = {time:?}\n\n{}",
        source_line("model.gltf")
    );
    bake(name, &recipe).positions
}

#[test]
fn animation_frames_set_the_starting_pose() {
    let (c, s) = (
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    );
    // Arm vertices v1 (2, 0) and v4 (3, 0.5) turned around the arm joint at (1, 0).
    let quarter = [[1.0, 1.0, 0.0], [0.5, 2.0, 0.0]];
    let eighth = [
        [1.0 + c, s, 0.0],
        [1.0 + 2.0 * c - 0.5 * s, 2.0 * s + 0.5 * c, 0.0],
    ];
    let rest = [POSITIONS[1], POSITIONS[4]];
    let check = |positions: Vec<[f32; 3]>, expected: [[f32; 3]; 2]| {
        assert_close(positions[1], expected[0]);
        assert_close(positions[4], expected[1]);
    };

    check(animated("turn-end", "Turn", 1.0), quarter);
    check(animated("turn-half", "Turn", 0.5), eighth);
    check(animated("turn-after", "Turn", 5.0), quarter);
    check(animated("snap-half", "Snap", 0.5), rest);
    check(animated("snap-end", "Snap", 1.0), quarter);
    // Zero tangents make the spline's midpoint the halfway rotation.
    check(animated("smooth-half", "Smooth", 0.5), eighth);
}

#[test]
fn pose_steps_apply_on_top_of_the_animation_frame() {
    let recipe = format!(
        "[animation]\nname = \"Turn\"\ntime = 1.0\n\n{}\n\
         [[pose]]\nkind = \"rotate\"\njoint = \"arm\"\naxis = [0.0, 0.0, 1.0]\ndegrees = -90.0\n",
        source_line("model.gltf")
    );
    let out = bake("anim-then-pose", &recipe);
    for (actual, expected) in out.positions.iter().zip(POSITIONS) {
        assert_close(*actual, expected);
    }
}

#[test]
fn a_missing_animation_falls_back_to_the_rest_pose() {
    let recipe = format!(
        "[animation]\nname = \"Dance\"\n\n{}",
        source_line("model.gltf")
    );
    let out = bake("no-animation", &recipe);
    assert_close(out.positions[1], POSITIONS[1]);
    assert!(
        out.notes
            .iter()
            .any(|note| note.contains("no animation Dance"))
    );
}
