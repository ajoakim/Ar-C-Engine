//! Validators (partition, level jump) and canonical seam viewpoints.

use std::collections::BTreeSet;

use engine_sdk::DVec3;

use crate::adjacency::neighbor;
use crate::face::Edge;
use crate::patch_id::{MAX_LEVEL, PatchId};

/// True iff the leaves tile the sphere exactly: integer area sums to six full faces and no leaf
/// is an ancestor of another (or duplicated).
#[must_use]
pub fn is_partition(leaves: &[PatchId]) -> bool {
    let set: BTreeSet<PatchId> = leaves.iter().copied().collect();
    if set.len() != leaves.len() || set.is_empty() {
        return false;
    }
    let mut area: u128 = 0;
    for p in &set {
        area += 1u128 << (2 * u32::from(MAX_LEVEL - p.level()));
        let mut cur = p.parent();
        while let Some(a) = cur {
            if set.contains(&a) {
                return false;
            }
            cur = a.parent();
        }
    }
    area == 6u128 << (2 * u32::from(MAX_LEVEL))
}

/// Maximum level difference across any shared edge.
#[must_use]
pub fn max_level_jump(leaves: &[PatchId]) -> u8 {
    let set: BTreeSet<PatchId> = leaves.iter().copied().collect();
    let mut jump = 0u8;
    for &p in &set {
        for e in Edge::ALL {
            let mut cur = Some(neighbor(p, e).patch);
            while let Some(q) = cur {
                if set.contains(&q) {
                    jump = jump.max(p.level() - q.level());
                    break;
                }
                cur = q.parent();
            }
        }
    }
    jump
}

#[must_use]
pub fn is_balanced(leaves: &[PatchId]) -> bool {
    max_level_jump(leaves) <= 1
}

fn norm(x: f64, y: f64, z: f64) -> DVec3 {
    let l = (x * x + y * y + z * z).sqrt();
    DVec3::new(x / l, y / l, z / l)
}

fn scaled(d: DVec3, r: f64) -> DVec3 {
    DVec3::new(d.x * r, d.y * r, d.z * r)
}

/// Canonical verification eye positions in deterministic order.
#[must_use]
pub fn reference_views(radius: f64, max_relief: f64) -> Vec<(String, DVec3)> {
    let base = radius + max_relief;
    let mut corners = Vec::new();
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                corners.push(norm(sx, sy, sz));
            }
        }
    }
    let mut edges = Vec::new();
    for s in [-1.0, 1.0] {
        for t in [-1.0, 1.0] {
            edges.push(norm(s, t, 0.0));
            edges.push(norm(s, 0.0, t));
            edges.push(norm(0.0, s, t));
        }
    }
    let mut out = Vec::new();
    for (i, d) in corners.iter().enumerate() {
        out.push((format!("corner{i}_1km"), scaled(*d, base + 1000.0)));
    }
    for (i, d) in corners.iter().enumerate() {
        out.push((format!("corner{i}_50m"), scaled(*d, base + 50.0)));
    }
    for (i, d) in edges.iter().enumerate() {
        out.push((format!("edge{i}_1km"), scaled(*d, base + 1000.0)));
    }
    out.push((
        "face_center_1km".to_string(),
        DVec3::new(base + 1000.0, 0.0, 0.0),
    ));
    out.push(("orbit_2r".to_string(), DVec3::new(2.0 * radius, 0.0, 0.0)));
    out
}
