use planetary_core::{Edge, Face, MAX_LEVEL, PatchId, PatchIdError};

fn morton(x: u32, y: u32) -> u64 {
    let mut m = 0u64;
    for b in 0..24 {
        m |= u64::from((x >> b) & 1) << (2 * b);
        m |= u64::from((y >> b) & 1) << (2 * b + 1);
    }
    m
}

#[test]
fn round_trip_all_small_levels() {
    for l in 0..=6u8 {
        let k = 1u32 << l;
        for f in Face::ALL {
            for x in 0..k {
                for y in 0..k {
                    let p = PatchId::new(f, l, x, y).unwrap();
                    assert_eq!((p.face(), p.level(), p.xy()), (f, l, (x, y)));
                    assert_eq!(PatchId::from_raw(p.raw()), Ok(p));
                }
            }
        }
    }
}

#[test]
fn round_trip_sampled_deep_levels() {
    for l in 7..=MAX_LEVEL {
        let k = 1u64 << l;
        for i in 0..200u64 {
            let x = ((i * 2654435761) % k) as u32;
            let y = ((i * 40503) % k) as u32;
            let f = Face::ALL[(i % 6) as usize];
            let p = PatchId::new(f, l, x, y).unwrap();
            assert_eq!((p.face(), p.level(), p.xy()), (f, l, (x, y)));
            assert_eq!(PatchId::from_raw(p.raw()), Ok(p));
        }
    }
}

#[test]
fn bit_layout() {
    let p = PatchId::new(Face::NegZ, 3, 0b101, 0b011).unwrap();
    assert_eq!(morton(0b101, 0b011), 0b011011);
    assert_eq!(p.raw(), (5u64 << 61) | (3u64 << 56) | 0b011011);
}

#[test]
fn parent_children_hierarchy() {
    for l in 1..=6u8 {
        for p in PatchId::level_iter(l) {
            let par = p.parent().unwrap();
            let kids = par.children().unwrap();
            assert_eq!(kids.iter().filter(|c| **c == p).count(), 1);
            assert_eq!(p.ancestor_at(l - 1), Some(par));
            assert!(par.is_ancestor_of(p));
            assert!(!p.is_ancestor_of(par));
        }
    }
    let p = PatchId::new(Face::PosY, 2, 1, 3).unwrap();
    let c = p.children().unwrap();
    let want = [(2, 6), (3, 6), (2, 7), (3, 7)];
    for (k, w) in c.iter().zip(want) {
        assert_eq!(k.xy(), w);
        assert_eq!(k.parent(), Some(p));
        assert_eq!(k.level(), 3);
    }
    for f in Face::ALL {
        assert_eq!(PatchId::root(f).parent(), None);
        assert_eq!(PatchId::root(f).ancestor_at(1), None);
        assert!(!PatchId::root(f).is_ancestor_of(PatchId::root(f)));
    }
    let deep = PatchId::new(Face::PosX, MAX_LEVEL, 5, 9).unwrap();
    assert_eq!(deep.children(), None);
}

#[test]
fn from_raw_rejects_malformed() {
    assert_eq!(PatchId::from_raw(6u64 << 61), Err(PatchIdError::BadFace(6)));
    assert_eq!(PatchId::from_raw(7u64 << 61), Err(PatchIdError::BadFace(7)));
    for l in 25..=31u64 {
        assert_eq!(
            PatchId::from_raw(l << 56),
            Err(PatchIdError::BadLevel(l as u8))
        );
    }
    let raw = (2u64 << 56) | (1 << 4);
    assert_eq!(PatchId::from_raw(raw), Err(PatchIdError::StrayBits(raw)));
    let raw0 = 1u64;
    assert_eq!(PatchId::from_raw(raw0), Err(PatchIdError::StrayBits(raw0)));
    assert!(PatchId::from_raw(2u64 << 56 | 0b1111).is_ok());
}

#[test]
fn new_rejects_out_of_range() {
    assert!(matches!(
        PatchId::new(Face::PosX, 2, 4, 0),
        Err(PatchIdError::CoordOutOfRange { .. })
    ));
    assert!(matches!(
        PatchId::new(Face::PosX, 0, 0, 1),
        Err(PatchIdError::CoordOutOfRange { .. })
    ));
    assert_eq!(
        PatchId::new(Face::PosX, 25, 0, 0),
        Err(PatchIdError::BadLevel(25))
    );
    assert!(PatchId::new(Face::PosX, 24, (1 << 24) - 1, (1 << 24) - 1).is_ok());
    assert!(!PatchIdError::BadFace(9).to_string().is_empty());
}

#[test]
fn ordering_and_level_iter() {
    let ids: Vec<_> = PatchId::level_iter(2).collect();
    assert_eq!(ids.len(), 96);
    assert!(
        ids.windows(2)
            .all(|w| w[0] < w[1] && w[0].raw() < w[1].raw())
    );
    assert_eq!(PatchId::level_iter(25).count(), 0);
}

#[test]
fn face_bases() {
    for f in Face::ALL {
        let b = f.basis();
        let c = [
            b.u[1] * b.v[2] - b.u[2] * b.v[1],
            b.u[2] * b.v[0] - b.u[0] * b.v[2],
            b.u[0] * b.v[1] - b.u[1] * b.v[0],
        ];
        assert_eq!(c, b.n);
        assert_eq!(Face::from_normal(b.n), Some(f));
        assert_eq!(Face::from_index(f.index()), Some(f));
        for e in Edge::ALL {
            assert_eq!(b.edge_with_outward(b.outward(e)), Some(e));
        }
        assert_eq!(b.edge_with_outward(b.n), None);
    }
    assert_eq!(Face::from_index(6), None);
    assert_eq!(Face::from_normal([1, 1, 0]), None);
}

#[test]
fn edge_vertices() {
    for t in 0..=8 {
        assert_eq!(Edge::NegU.vertex(8, t), (0, t));
        assert_eq!(Edge::PosU.vertex(8, t), (8, t));
        assert_eq!(Edge::NegV.vertex(8, t), (t, 0));
        assert_eq!(Edge::PosV.vertex(8, t), (t, 8));
    }
    assert_eq!(Edge::NegU.opposite(), Edge::PosU);
    assert_eq!(Edge::PosV.opposite(), Edge::NegV);
    assert!(Edge::PosU.is_u_edge() && !Edge::NegV.is_u_edge());
}
