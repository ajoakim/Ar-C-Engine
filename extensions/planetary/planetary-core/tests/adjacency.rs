//! Exhaustive adjacency tests with an independent integer-geometry oracle (own basis table).

use planetary_core::{Edge, Face, PatchId, neighbor, neighbor_children};

type V3 = [i64; 3];

/// Oracle basis table (n, u, v), deliberately duplicated from the spec, not from the crate.
fn obasis(f: Face) -> (V3, V3, V3) {
    match f {
        Face::PosX => ([1, 0, 0], [0, 1, 0], [0, 0, 1]),
        Face::NegX => ([-1, 0, 0], [0, 0, 1], [0, 1, 0]),
        Face::PosY => ([0, 1, 0], [0, 0, 1], [1, 0, 0]),
        Face::NegY => ([0, -1, 0], [1, 0, 0], [0, 0, 1]),
        Face::PosZ => ([0, 0, 1], [1, 0, 0], [0, 1, 0]),
        Face::NegZ => ([0, 0, -1], [0, 1, 0], [1, 0, 0]),
    }
}

/// Lattice point of corner (ci, cj) of patch (f, level, x, y), scaled by K = 2^level.
fn oracle_corner(f: Face, level: u8, x: u32, y: u32, ci: i64, cj: i64) -> V3 {
    let k = 1i64 << level;
    let (n, u, v) = obasis(f);
    let a = 2 * (i64::from(x) + ci) - k;
    let b = 2 * (i64::from(y) + cj) - k;
    [0, 1, 2].map(|i| k * n[i] + a * u[i] + b * v[i])
}

/// The two corners of edge `e`, ordered by increasing along-edge coordinate.
fn edge_corners(p: PatchId, e: Edge) -> [V3; 2] {
    let (x, y) = p.xy();
    let c = |ci, cj| oracle_corner(p.face(), p.level(), x, y, ci, cj);
    match e {
        Edge::NegU => [c(0, 0), c(0, 1)],
        Edge::PosU => [c(1, 0), c(1, 1)],
        Edge::NegV => [c(0, 0), c(1, 0)],
        Edge::PosV => [c(0, 1), c(1, 1)],
    }
}

fn max_level() -> u8 {
    6
}

#[test]
fn reciprocity_levels_0_to_6() {
    let mut total = 0u64;
    for l in 0..=max_level() {
        for p in PatchId::level_iter(l) {
            for e in Edge::ALL {
                let n = neighbor(p, e);
                assert_eq!(n.patch.level(), l);
                let back = neighbor(n.patch, n.edge);
                assert_eq!(back.patch, p, "{p:?} {e:?}");
                assert_eq!(back.edge, e);
                assert_eq!(back.flipped, n.flipped);
                total += 1;
            }
        }
    }
    // sum over levels of 6 * 4^L * 4
    assert_eq!(total, (0..=6u32).map(|l| 24 * 4u64.pow(l)).sum::<u64>());
}

#[test]
fn geometry_oracle_shared_edges() {
    for l in 0..=max_level() {
        for p in PatchId::level_iter(l) {
            for e in Edge::ALL {
                let n = neighbor(p, e);
                let a = edge_corners(p, e);
                let b = edge_corners(n.patch, n.edge);
                // same corner set
                assert!(
                    (a[0] == b[0] && a[1] == b[1]) || (a[0] == b[1] && a[1] == b[0]),
                    "corner sets differ for {p:?} {e:?}"
                );
                assert_ne!(a[0], a[1]);
                // flip oracle: t=0 ends coincide iff not flipped
                assert_eq!(a[0] == b[0], !n.flipped, "flip mismatch for {p:?} {e:?}");
            }
        }
    }
}

#[test]
fn level0_neighbor_faces_are_the_four_non_opposite() {
    for f in Face::ALL {
        let mut got: Vec<Face> = Edge::ALL
            .iter()
            .map(|&e| neighbor(PatchId::root(f), e).patch.face())
            .collect();
        got.sort();
        got.dedup();
        assert_eq!(got.len(), 4);
        let (n, _, _) = obasis(f);
        for g in got {
            let (gn, _, _) = obasis(g);
            let d: i64 = (0..3).map(|i| n[i] * gn[i]).sum();
            assert_eq!(d, 0, "{f:?} -> {g:?} must be perpendicular");
        }
    }
}

#[test]
fn cross_face_counts_and_interior_rule() {
    for l in 0..=max_level() {
        let mut cross = 0u64;
        for p in PatchId::level_iter(l) {
            for e in Edge::ALL {
                let n = neighbor(p, e);
                if n.patch.face() != p.face() {
                    cross += 1;
                } else {
                    assert_eq!(n.edge, e.opposite());
                    assert!(!n.flipped);
                }
            }
        }
        assert_eq!(cross, 24 * (1u64 << l), "level {l}");
    }
}

#[test]
fn neighbor_children_touch_shared_edge_in_order() {
    for l in 0..=5u8 {
        for p in PatchId::level_iter(l) {
            for e in Edge::ALL {
                let n = neighbor(p, e);
                let kids = neighbor_children(p, e).expect("below max level");
                let shared = edge_corners(p, e);
                let mut prev_dist = -1i64;
                for c in kids {
                    assert_eq!(c.level(), l + 1);
                    assert_eq!(c.parent(), Some(n.patch));
                    // child's edge facing back lies on the shared segment, at double resolution
                    let mut on_edge = false;
                    for ce in Edge::ALL {
                        let cc = edge_corners(c, ce);
                        let s0 = [0, 1, 2].map(|i| shared[0][i] * 2);
                        let s1 = [0, 1, 2].map(|i| shared[1][i] * 2);
                        let inside = |q: V3| {
                            // q on segment s0..s1 (collinear, between)
                            let d = [0, 1, 2].map(|i| s1[i] - s0[i]);
                            let w = [0, 1, 2].map(|i| q[i] - s0[i]);
                            let cr = [
                                d[1] * w[2] - d[2] * w[1],
                                d[2] * w[0] - d[0] * w[2],
                                d[0] * w[1] - d[1] * w[0],
                            ];
                            let dd: i64 = (0..3).map(|i| d[i] * w[i]).sum();
                            let len2: i64 = (0..3).map(|i| d[i] * d[i]).sum();
                            cr == [0, 0, 0] && dd >= 0 && dd <= len2
                        };
                        if inside(cc[0]) && inside(cc[1]) {
                            on_edge = true;
                        }
                    }
                    assert!(on_edge, "child {c:?} does not touch shared edge");
                    // ordering: distance of child's nearest shared corner from p's t=0 corner
                    let s0 = [0, 1, 2].map(|i| shared[0][i] * 2);
                    let mut best = i64::MAX;
                    for ce in Edge::ALL {
                        for q in edge_corners(c, ce) {
                            let d: i64 = (0..3).map(|i| (q[i] - s0[i]).pow(2)).sum();
                            // only consider corners on the shared segment (collinear with s0..s1)
                            let s1 = [0, 1, 2].map(|i| shared[1][i] * 2);
                            let dv = [0, 1, 2].map(|i| s1[i] - s0[i]);
                            let w = [0, 1, 2].map(|i| q[i] - s0[i]);
                            let cr = [
                                dv[1] * w[2] - dv[2] * w[1],
                                dv[2] * w[0] - dv[0] * w[2],
                                dv[0] * w[1] - dv[1] * w[0],
                            ];
                            if cr == [0, 0, 0] {
                                best = best.min(d);
                            }
                        }
                    }
                    assert!(best > prev_dist, "children not ordered along p's edge");
                    prev_dist = best;
                }
                assert_ne!(kids[0], kids[1]);
            }
        }
    }
    let deep = PatchId::new(Face::PosX, planetary_core::MAX_LEVEL, 0, 0).expect("valid");
    assert_eq!(neighbor_children(deep, Edge::PosU), None);
}

#[test]
fn cube_corners_have_three_mutually_adjacent_patches() {
    let level = 2u8;
    let k = 1i64 << level;
    let mut corners_checked = 0;
    for sx in [-1i64, 1] {
        for sy in [-1i64, 1] {
            for sz in [-1i64, 1] {
                let target = [sx * k, sy * k, sz * k];
                let mut touching = Vec::new();
                for p in PatchId::level_iter(level) {
                    let (x, y) = p.xy();
                    let hit = (0..2).any(|ci| {
                        (0..2).any(|cj| oracle_corner(p.face(), level, x, y, ci, cj) == target)
                    });
                    if hit {
                        touching.push(p);
                    }
                }
                assert_eq!(touching.len(), 3, "corner {target:?}");
                let mut faces: Vec<_> = touching.iter().map(|p| p.face()).collect();
                faces.sort();
                faces.dedup();
                assert_eq!(faces.len(), 3);
                for a in &touching {
                    for b in &touching {
                        if a != b {
                            assert!(
                                Edge::ALL.iter().any(|&e| neighbor(*a, e).patch == *b),
                                "{a:?} not adjacent to {b:?}"
                            );
                        }
                    }
                }
                corners_checked += 1;
            }
        }
    }
    assert_eq!(corners_checked, 8);
}
