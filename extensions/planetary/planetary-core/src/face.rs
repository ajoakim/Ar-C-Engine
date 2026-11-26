//! Cube faces, face-local edges and integer face bases (`u x v = n`, counter-clockwise outside).

/// One of the six cube faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Face {
    PosX = 0,
    NegX = 1,
    PosY = 2,
    NegY = 3,
    PosZ = 4,
    NegZ = 5,
}

/// Integer orthonormal frame of a face: outward normal `n`, grid axes `u` (x) and `v` (y).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceBasis {
    pub n: [i32; 3],
    pub u: [i32; 3],
    pub v: [i32; 3],
}

const fn neg(a: [i32; 3]) -> [i32; 3] {
    [-a[0], -a[1], -a[2]]
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::PosX,
        Face::NegX,
        Face::PosY,
        Face::NegY,
        Face::PosZ,
        Face::NegZ,
    ];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn from_index(i: u8) -> Option<Face> {
        match i {
            0 => Some(Face::PosX),
            1 => Some(Face::NegX),
            2 => Some(Face::PosY),
            3 => Some(Face::NegY),
            4 => Some(Face::PosZ),
            5 => Some(Face::NegZ),
            _ => None,
        }
    }

    pub const fn from_normal(n: [i32; 3]) -> Option<Face> {
        match n {
            [1, 0, 0] => Some(Face::PosX),
            [-1, 0, 0] => Some(Face::NegX),
            [0, 1, 0] => Some(Face::PosY),
            [0, -1, 0] => Some(Face::NegY),
            [0, 0, 1] => Some(Face::PosZ),
            [0, 0, -1] => Some(Face::NegZ),
            _ => None,
        }
    }

    pub const fn basis(self) -> FaceBasis {
        let (n, u, v) = match self {
            Face::PosX => ([1, 0, 0], [0, 1, 0], [0, 0, 1]),
            Face::NegX => ([-1, 0, 0], [0, 0, 1], [0, 1, 0]),
            Face::PosY => ([0, 1, 0], [0, 0, 1], [1, 0, 0]),
            Face::NegY => ([0, -1, 0], [1, 0, 0], [0, 0, 1]),
            Face::PosZ => ([0, 0, 1], [1, 0, 0], [0, 1, 0]),
            Face::NegZ => ([0, 0, -1], [0, 1, 0], [1, 0, 0]),
        };
        FaceBasis { n, u, v }
    }
}

/// A patch edge. `NegU`: x = 0 side, `PosU`: x = K-1, `NegV`: y = 0, `PosV`: y = K-1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Edge {
    NegU = 0,
    PosU = 1,
    NegV = 2,
    PosV = 3,
}

impl Edge {
    pub const ALL: [Edge; 4] = [Edge::NegU, Edge::PosU, Edge::NegV, Edge::PosV];

    pub const fn opposite(self) -> Edge {
        match self {
            Edge::NegU => Edge::PosU,
            Edge::PosU => Edge::NegU,
            Edge::NegV => Edge::PosV,
            Edge::PosV => Edge::NegV,
        }
    }

    /// True for `NegU`/`PosU` (the along-edge coordinate is y / j).
    pub const fn is_u_edge(self) -> bool {
        matches!(self, Edge::NegU | Edge::PosU)
    }

    /// Grid vertex `(i, j)` of the `t`-th vertex along this edge of an `n x n` cell grid,
    /// ordered by increasing along-edge coordinate.
    pub const fn vertex(self, n: u32, t: u32) -> (u32, u32) {
        match self {
            Edge::NegU => (0, t),
            Edge::PosU => (n, t),
            Edge::NegV => (t, 0),
            Edge::PosV => (t, n),
        }
    }
}

impl FaceBasis {
    /// Outward direction (in cube space) of the given edge.
    pub const fn outward(&self, e: Edge) -> [i32; 3] {
        match e {
            Edge::NegU => neg(self.u),
            Edge::PosU => self.u,
            Edge::NegV => neg(self.v),
            Edge::PosV => self.v,
        }
    }

    /// Edge whose outward direction equals `w`, if any.
    pub const fn edge_with_outward(&self, w: [i32; 3]) -> Option<Edge> {
        let mut i = 0;
        while i < 4 {
            let e = Edge::ALL[i];
            let o = self.outward(e);
            if o[0] == w[0] && o[1] == w[1] && o[2] == w[2] {
                return Some(e);
            }
            i += 1;
        }
        None
    }
}
