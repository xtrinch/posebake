//! Pose steps: turn joints, and everything below them, around the joint's position.

use anyhow::{Result, bail};
use glam::{Mat4, Quat, Vec3};
use serde::Deserialize;

use crate::source::Source;

/// One pose change. Directions and axes are in the file's model space (glTF is Y up).
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PoseStep {
    /// Turn `joint` so the line from it to `toward` points along `direction`, e.g. lower an
    /// arm by aiming the upper arm toward the hand straight down.
    Aim {
        joint: String,
        toward: String,
        direction: [f32; 3],
    },
    /// Turn `joint` by `degrees` around `axis`, counter-clockwise looking against the axis.
    Rotate {
        joint: String,
        axis: [f32; 3],
        degrees: f32,
    },
}

impl PoseStep {
    fn joints(&self) -> Vec<&str> {
        match self {
            PoseStep::Aim { joint, toward, .. } => vec![joint, toward],
            PoseStep::Rotate { joint, .. } => vec![joint],
        }
    }
}

/// Model-space node transforms after applying the steps in order. Steps naming a joint the
/// source does not have are skipped and reported, so a hair-only file can share a recipe with
/// a full body.
pub fn apply(source: &Source, steps: &[PoseStep]) -> Result<(Vec<Mat4>, Vec<String>)> {
    let mut posed = source.rest.clone();
    let mut skipped = Vec::new();
    for step in steps {
        let missing: Vec<&str> = step
            .joints()
            .into_iter()
            .filter(|name| source.node(name).is_none())
            .collect();
        if !missing.is_empty() {
            skipped.push(format!(
                "skipped a pose step: no joint {}",
                missing.join(", ")
            ));
            continue;
        }
        let (joint, rotation) = match step {
            PoseStep::Aim {
                joint: name,
                toward,
                direction,
            } => {
                let joint = source.node(name).unwrap();
                let current =
                    translation(&posed[source.node(toward).unwrap()]) - translation(&posed[joint]);
                let target = Vec3::from(*direction);
                if current.length() < 1e-6 || target.length() < 1e-6 {
                    bail!("aim step for joint {name} has a zero-length direction");
                }
                (
                    joint,
                    Quat::from_rotation_arc(current.normalize(), target.normalize()),
                )
            }
            PoseStep::Rotate {
                joint,
                axis,
                degrees,
            } => {
                let axis = Vec3::from(*axis);
                if axis.length() < 1e-6 {
                    bail!("rotate step for joint {joint} has a zero-length axis");
                }
                let joint = source.node(joint).unwrap();
                (
                    joint,
                    Quat::from_axis_angle(axis.normalize(), degrees.to_radians()),
                )
            }
        };
        let pivot = translation(&posed[joint]);
        let swing = Mat4::from_translation(pivot)
            * Mat4::from_quat(rotation)
            * Mat4::from_translation(-pivot);
        for node in source.subtree(joint) {
            posed[node] = swing * posed[node];
        }
    }
    Ok((posed, skipped))
}

fn translation(matrix: &Mat4) -> Vec3 {
    matrix.w_axis.truncate()
}
