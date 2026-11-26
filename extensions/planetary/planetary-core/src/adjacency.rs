//! Same-level neighbor derivation across cube faces, computed from the integer face bases.

use crate::face::{Edge, Face};
use crate::patch_id::PatchId;

/// Result of a same-level neighbor query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Neighbor {
    pub patch: PatchId,
    /// The neighbor's edge facing back at the queried patch.
    pub edge: Edge,
    /// True when the along-edge coordinate runs in the opposite direction on the neighbor.
    pub flipped: bool,
}

const fn dot(a: [i32; 3], b: [i32; 3]) -> i32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Derives the cross-face link `(face g, back edge ge, flipped)` for edge `e` of face `f`.
/// The back edge of `g` is the one whose outward vector equals `+n_f`.
const fn derive_link(f: Face, e: Edge) -> (Face, Edge, bool) {
    let bf = f.basis();
    let g = match Face::from_normal(bf.outward(e)) {
        Some(g) => g,
        None => return (f, e.opposite(), false),
    };
    let bg = g.basis();
    let ge = match bg.edge_with_outward(bf.n) {
        Some(ge) => ge,
        None => return (f, e.opposite(), false),
    };
    let along_f = if e.is_u_edge() { bf.v } else { bf.u };
    let along_g = if ge.is_u_edge() { bg.v } else { bg.u };
    (g, ge, dot(along_f, along_g) < 0)
}

const fn build_links() -> [[(Face, Edge, bool); 4]; 6] {
    let mut t = [[(Face::PosX, Edge::NegU, false); 4]; 6];
    let mut f = 0;
    while f < 6 {
        let face = Face::ALL[f];
        let mut e = 0;
        while e < 4 {
            t[f][e] = derive_link(face, Edge::ALL[e]);
            e += 1;
        }
        f += 1;
    }
    t
}

/// Cross-face links `[face][edge] -> (neighbor face, neighbor back edge, flipped)`.
pub(crate) const LINKS: [[(Face, Edge, bool); 4]; 6] = build_links();

/// Same-level neighbor across `e`.
pub fn neighbor(p: PatchId, e: Edge) -> Neighbor {
    let f = p.face();
    let l = p.level();
    let k = 1u32 << l;
    let (x, y) = p.xy();
    let inner = match e {
        Edge::NegU if x > 0 => Some((x - 1, y)),
        Edge::PosU if x + 1 < k => Some((x + 1, y)),
        Edge::NegV if y > 0 => Some((x, y - 1)),
        Edge::PosV if y + 1 < k => Some((x, y + 1)),
        _ => None,
    };
    if let Some((nx, ny)) = inner {
        return Neighbor {
            patch: PatchId::from_parts(f, l, nx, ny),
            edge: e.opposite(),
            flipped: false,
        };
    }
    let (g, ge, flipped) = LINKS[f.index() as usize][e as usize];
    let t = if e.is_u_edge() { y } else { x };
    let t2 = if flipped { k - 1 - t } else { t };
    let (nx, ny) = match ge {
        Edge::NegU => (0, t2),
        Edge::PosU => (k - 1, t2),
        Edge::NegV => (t2, 0),
        Edge::PosV => (t2, k - 1),
    };
    Neighbor {
        patch: PatchId::from_parts(g, l, nx, ny),
        edge: ge,
        flipped,
    }
}

/// The two children of `neighbor(p, e).patch` touching the shared edge, ordered by increasing
/// along-edge coordinate of `p`. `None` when `p` is at the maximum level.
pub fn neighbor_children(p: PatchId, e: Edge) -> Option<[PatchId; 2]> {
    let n = neighbor(p, e);
    let kids = n.patch.children()?;
    // Children order: [(2x,2y), (2x+1,2y), (2x,2y+1), (2x+1,2y+1)]; pick the pair on n.edge in
    // increasing along-edge order of the neighbor.
    let pair = match n.edge {
        Edge::NegU => [kids[0], kids[2]],
        Edge::PosU => [kids[1], kids[3]],
        Edge::NegV => [kids[0], kids[1]],
        Edge::PosV => [kids[2], kids[3]],
    };
    Some(if n.flipped { [pair[1], pair[0]] } else { pair })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn const_links_match_dynamic_derivation() {
        for f in Face::ALL {
            for e in Edge::ALL {
                let bf = f.basis();
                let g = Face::from_normal(bf.outward(e));
                assert!(g.is_some());
                let g = g.unwrap_or(f);
                let ge = g.basis().edge_with_outward(bf.n);
                assert!(ge.is_some());
                let ge = ge.unwrap_or(e);
                let af = if e.is_u_edge() { bf.v } else { bf.u };
                let ag = if ge.is_u_edge() {
                    g.basis().v
                } else {
                    g.basis().u
                };
                let flipped = dot(af, ag) < 0;
                assert_eq!(LINKS[f.index() as usize][e as usize], (g, ge, flipped));
            }
        }
    }
}
