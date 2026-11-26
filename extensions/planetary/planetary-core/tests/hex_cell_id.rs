use planetary_core::hex::icosa::{DIAMONDS, FACES, corners};
use planetary_core::hex::{CellId, CellIdError, HEX_MAX_LEVEL};

#[test]
fn level_counts() {
    for level in 0..=6u8 {
        let ids: Vec<CellId> = CellId::level_iter(level).collect();
        assert_eq!(ids.len() as u64, 10 * 4u64.pow(u32::from(level)) + 2);
        assert!(ids.windows(2).all(|w| w[0].raw() < w[1].raw()), "ascending");
        assert!(ids.iter().all(|c| c.level() == level));
    }
}

#[test]
fn round_trip_all_cells_to_level_6() {
    for level in 0..=6u8 {
        for c in CellId::level_iter(level) {
            let (i, j) = c.ij();
            assert_eq!(CellId::new(c.base(), level, i, j), Ok(c));
            assert_eq!(CellId::from_raw(c.raw()), Ok(c));
        }
    }
    assert_eq!(CellId::north_pole(3).base(), 0);
    assert_eq!(CellId::south_pole(3).base(), 11);
    assert!(CellId::south_pole(3).is_pole());
}

#[test]
fn rejects_invalid_raw() {
    let ok = CellId::new(3, 5, 7, 9).unwrap().raw();
    assert!(CellId::from_raw(ok).is_ok());
    for bit in 0..7 {
        assert_eq!(
            CellId::from_raw(ok | (1 << bit)),
            Err(CellIdError::StrayBits)
        );
    }
    for base in 12u64..=15 {
        assert_eq!(
            CellId::from_raw(base << 60),
            Err(CellIdError::BadBase(base as u8))
        );
    }
    for level in 25u64..=31 {
        assert_eq!(
            CellId::from_raw((3u64 << 60) | (level << 55)),
            Err(CellIdError::BadLevel(level as u8))
        );
    }
    assert!(CellId::from_raw((3u64 << 60) | (u64::from(HEX_MAX_LEVEL) << 55)).is_ok());
    assert_eq!(CellId::new(1, 3, 8, 0), Err(CellIdError::CoordOutOfRange));
    assert_eq!(CellId::new(1, 3, 0, 8), Err(CellIdError::CoordOutOfRange));
    // Out-of-range morton bits via raw: i = 8 at level 3.
    assert_eq!(
        CellId::from_raw((1u64 << 60) | (3u64 << 55) | (0b0100_0000u64 << 7)),
        Err(CellIdError::CoordOutOfRange)
    );
    assert_eq!(CellId::new(0, 2, 1, 0), Err(CellIdError::PoleCoord));
    assert_eq!(CellId::new(11, 2, 0, 1), Err(CellIdError::PoleCoord));
    assert_eq!(
        CellId::from_raw((11u64 << 60) | (2u64 << 55) | (1u64 << 7)),
        Err(CellIdError::PoleCoord)
    );
    assert!(CellId::new(1, 24, (1 << 24) - 1, (1 << 24) - 1).is_ok());
}

#[test]
fn ordering_is_raw_order() {
    let a = CellId::new(1, 4, 3, 3).unwrap();
    let b = CellId::new(2, 4, 0, 0).unwrap();
    assert!(a < b);
    assert_eq!(a.cmp(&b), a.raw().cmp(&b.raw()));
}

#[test]
fn icosa_tables_consistent() {
    // 30 edges, each shared by exactly 2 faces; every vertex in exactly 5 faces.
    for v in 0..12u8 {
        assert_eq!(FACES.iter().filter(|f| f.contains(&v)).count(), 5);
    }
    for (d, dia) in DIAMONDS.iter().enumerate() {
        let c = corners(d as u8 + 1);
        assert_eq!([dia.origin, dia.i_vertex, dia.j_vertex, dia.far_vertex], c);
        assert_ne!(dia.faces[0], dia.faces[1]);
    }
    let mut origins: Vec<u8> = DIAMONDS.iter().map(|d| d.origin).collect();
    origins.sort_unstable();
    assert_eq!(origins, (1..=10).collect::<Vec<u8>>());
}
