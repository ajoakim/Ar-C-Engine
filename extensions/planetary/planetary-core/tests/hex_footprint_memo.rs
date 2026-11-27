//! The per-thread footprint memo of the hex topology returns exactly what the direct mapping
//! computes (07.1-27): a hit, a miss and an evicting collision are all bit-equal.

use engine_sdk::DVec3;
use planetary_core::hex::mapping;
use planetary_core::{HexSphere, Topology};

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit(state: &mut u64) -> DVec3 {
    let f = |s: &mut u64| (splitmix(s) >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0;
    loop {
        let v = DVec3::new(f(state), f(state), f(state));
        let l = v.length();
        if l > 0.1 && l <= 1.0 {
            return v / l;
        }
    }
}

fn bits(f: &planetary_core::lod::Footprint) -> [u64; 5] {
    [
        f.center.x.to_bits(),
        f.center.y.to_bits(),
        f.center.z.to_bits(),
        f.cos_delta.to_bits(),
        f.sin_delta.to_bits(),
    ]
}

#[test]
fn memoised_footprint_is_bit_equal_to_the_direct_mapping() {
    let topo = HexSphere::new(16).expect("hex");
    let mut st = 0x07_1027;
    // 5000 seeded cells over all levels: more than the memo's slot count over time, so slots are
    // reused (collision path) while earlier cells are queried again (hit and re-miss paths).
    let mut cells = Vec::new();
    for i in 0..5000u32 {
        let level = u8::try_from(i % 17).expect("level");
        cells.push(topo.locate(unit(&mut st), level));
    }
    for round in 0..3 {
        for &c in &cells {
            let direct = mapping::footprint(c);
            let memo = topo.footprint(c);
            assert_eq!(bits(&direct), bits(&memo), "round {round} cell {c:?}");
        }
    }
    // Immediately repeated queries hit the memo.
    for &c in cells.iter().take(200) {
        let a = topo.footprint(c);
        let b = topo.footprint(c);
        assert_eq!(bits(&a), bits(&b));
        assert_eq!(bits(&a), bits(&mapping::footprint(c)));
    }
}

#[test]
fn memo_is_per_thread_and_not_shared_across_topologies_of_different_n() {
    // The footprint does not depend on the sector side, so two topologies agree.
    let a = HexSphere::new(16).expect("hex");
    let b = HexSphere::new(8).expect("hex");
    let mut st = 99;
    for i in 0..300u32 {
        let c = a.locate(unit(&mut st), u8::try_from(i % 15).expect("level"));
        assert_eq!(bits(&a.footprint(c)), bits(&b.footprint(c)));
    }
    let c = a.locate(DVec3::X, 9);
    let here = bits(&a.footprint(c));
    let there = std::thread::spawn(move || {
        let t = HexSphere::new(16).expect("hex");
        bits(&t.footprint(c))
    })
    .join()
    .expect("thread");
    assert_eq!(here, there);
}
