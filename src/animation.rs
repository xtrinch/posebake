//! Sampling a glTF animation at one point in time, as a starting pose.

use glam::{Quat, Vec3, Vec4};
use gltf::animation::util::ReadOutputs;
use gltf::animation::{Interpolation, Property};

use crate::source::{Source, Trs};

/// The source's node local transforms with the named animation applied at `time` seconds
/// (clamped to the animation's range). `None` if the source has no animation of that name.
pub fn sample(source: &Source, name: &str, time: f32) -> Option<Vec<Trs>> {
    let animation = source
        .document
        .animations()
        .find(|animation| animation.name() == Some(name))?;
    let mut locals = source.rest_locals.clone();
    for channel in animation.channels() {
        let reader = channel.reader(|buffer| Some(&source.buffers[buffer.index()]));
        let (Some(inputs), Some(outputs)) = (reader.read_inputs(), reader.read_outputs()) else {
            continue;
        };
        let times: Vec<f32> = inputs.collect();
        if times.is_empty() {
            continue;
        }
        let interpolation = channel.sampler().interpolation();
        let local = &mut locals[channel.target().node().index()];
        match (channel.target().property(), outputs) {
            (Property::Translation, ReadOutputs::Translations(values)) => {
                let values: Vec<Vec3> = values.map(Vec3::from).collect();
                local.translation = sample_vec3(&times, &values, interpolation, time);
            }
            (Property::Scale, ReadOutputs::Scales(values)) => {
                let values: Vec<Vec3> = values.map(Vec3::from).collect();
                local.scale = sample_vec3(&times, &values, interpolation, time);
            }
            (Property::Rotation, ReadOutputs::Rotations(values)) => {
                let values: Vec<Quat> = values.into_f32().map(Quat::from_array).collect();
                local.rotation = sample_quat(&times, &values, interpolation, time);
            }
            // Morph target weights do not move joints.
            _ => {}
        }
    }
    Some(locals)
}

/// Keyframes around `time`: (index before, index after, blend 0-1, seconds between them).
fn segment(times: &[f32], time: f32) -> (usize, usize, f32, f32) {
    let last = times.len() - 1;
    if time <= times[0] {
        return (0, 0, 0.0, 0.0);
    }
    if time >= times[last] {
        return (last, last, 0.0, 0.0);
    }
    let after = times.partition_point(|&t| t <= time);
    let before = after - 1;
    let span = times[after] - times[before];
    (before, after, (time - times[before]) / span, span)
}

// Cubic spline keyframes store (in-tangent, value, out-tangent) per key.
fn key<T: Copy>(values: &[T], index: usize, interpolation: Interpolation) -> T {
    match interpolation {
        Interpolation::CubicSpline => values[index * 3 + 1],
        _ => values[index],
    }
}

// Cubic Hermite blend of two keys from the glTF spline layout, per component.
fn hermite(p0: Vec4, out0: Vec4, in1: Vec4, p1: Vec4, s: f32, span: f32) -> Vec4 {
    let (s2, s3) = (s * s, s * s * s);
    p0 * (2.0 * s3 - 3.0 * s2 + 1.0)
        + out0 * span * (s3 - 2.0 * s2 + s)
        + p1 * (-2.0 * s3 + 3.0 * s2)
        + in1 * span * (s3 - s2)
}

fn sample_vec3(times: &[f32], values: &[Vec3], interpolation: Interpolation, time: f32) -> Vec3 {
    let (a, b, s, span) = segment(times, time);
    match interpolation {
        _ if a == b => key(values, a, interpolation),
        Interpolation::Step => key(values, a, interpolation),
        Interpolation::Linear => {
            key(values, a, interpolation).lerp(key(values, b, interpolation), s)
        }
        Interpolation::CubicSpline => hermite(
            values[a * 3 + 1].extend(0.0),
            values[a * 3 + 2].extend(0.0),
            values[b * 3].extend(0.0),
            values[b * 3 + 1].extend(0.0),
            s,
            span,
        )
        .truncate(),
    }
}

fn sample_quat(times: &[f32], values: &[Quat], interpolation: Interpolation, time: f32) -> Quat {
    let (a, b, s, span) = segment(times, time);
    let quat = match interpolation {
        _ if a == b => key(values, a, interpolation),
        Interpolation::Step => key(values, a, interpolation),
        Interpolation::Linear => {
            key(values, a, interpolation).slerp(key(values, b, interpolation), s)
        }
        Interpolation::CubicSpline => Quat::from_vec4(hermite(
            Vec4::from(values[a * 3 + 1]),
            Vec4::from(values[a * 3 + 2]),
            Vec4::from(values[b * 3]),
            Vec4::from(values[b * 3 + 1]),
            s,
            span,
        )),
    };
    quat.normalize()
}
