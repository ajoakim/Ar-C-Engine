use planetary_core::hex::CellId;
use planetary_core::hex::adjacency::{
    canonical, children, is_pentagon, neighbors, parents, primary_parent,
};
use std::collections::BTreeSet;

fn nb(c: CellId) -> Vec<CellId> {
    let mut v = Vec::new();
    neighbors(c, &mut v);
    v
}

fn ch(c: CellId) -> Vec<CellId> {
    let mut v = Vec::new();
    children(c, &mut v);
    v
}

fn pa(c: CellId) -> Vec<CellId> {
    let mut v = Vec::new();
    parents(c, &mut v);
    v
}

fn count_for(level: u8) -> usize {
    10 * 4usize.pow(u32::from(level)) + 2
}

#[test]
fn exactly_twelve_pentagons_per_level() {
    for level in 0..=6u8 {
        let mut pentagons = 0;
        let mut total = 0;
        for c in CellId::level_iter(level) {
            total += 1;
            let n = nb(c).len();
            if is_pentagon(c) {
                pentagons += 1;
                assert_eq!(n, 5, "pentagon {c:?} L{level}");
            } else {
                assert_eq!(n, 6, "hex {c:?} L{level}");
            }
        }
        assert_eq!(total, count_for(level));
        assert_eq!(pentagons, 12, "pentagons at L{level}");
    }
}

fn check_reciprocal(c: CellId) {
    let ns = nb(c);
    let set: BTreeSet<CellId> = ns.iter().copied().collect();
    assert_eq!(set.len(), ns.len(), "duplicate neighbour of {c:?}");
    assert!(!set.contains(&c), "self neighbour {c:?}");
    assert!(ns.iter().all(|x| x.level() == c.level()));
    for n in &ns {
        assert!(nb(*n).contains(&c), "not reciprocal: {c:?} -> {n:?}");
    }
}

#[test]
fn neighbors_reciprocal() {
    for level in 0..=6u8 {
        for c in CellId::level_iter(level) {
            check_reciprocal(c);
        }
    }
}

#[test]
fn canonical_matches_in_range() {
    let c = CellId::new(4, 3, 5, 2).unwrap();
    assert_eq!(canonical(4, 3, 5, 2), c);
    assert_eq!(canonical(0, 3, 0, 0), CellId::north_pole(3));
}

#[test]
fn children_counts() {
    for level in 0..=5u8 {
        for c in CellId::level_iter(level) {
            let v = ch(c);
            let set: BTreeSet<CellId> = v.iter().copied().collect();
            assert_eq!(set.len(), v.len(), "duplicate children of {c:?}");
            assert_eq!(v.len(), if is_pentagon(c) { 6 } else { 7 }, "{c:?}");
            assert!(v.iter().all(|x| x.level() == level + 1));
            let (i, j) = c.ij();
            assert_eq!(v[0].base(), c.base());
            assert_eq!(v[0].ij(), (2 * i, 2 * j), "centre child first");
        }
    }
    let mut v = Vec::new();
    children(CellId::new(1, 24, 3, 3).unwrap(), &mut v);
    assert!(v.is_empty(), "no children beyond the max level");
}

#[test]
fn parents_round_trip() {
    for level in 0..=5u8 {
        for c in CellId::level_iter(level) {
            let v = ch(c);
            assert_eq!(pa(v[0]), vec![c], "centre child of {c:?}");
            for e in &v[1..] {
                let ps = pa(*e);
                assert_eq!(ps.len(), 2, "edge child {e:?} of {c:?}");
                assert!(ps[0] < ps[1], "ascending raw");
                assert!(ps.contains(&c));
                assert!(ps.iter().all(|p| p.level() == level));
                assert!(nb(ps[0]).contains(&ps[1]), "parents of {e:?} not adjacent");
            }
            for x in &v {
                assert!(pa(*x).contains(&c));
            }
        }
    }
    assert!(pa(CellId::new(2, 0, 0, 0).unwrap()).is_empty());
    for level in 1..=6u8 {
        for c in CellId::level_iter(level) {
            let ps = pa(c);
            assert!(matches!(ps.len(), 1 | 2), "{c:?}");
            assert_eq!(primary_parent(c), Some(ps[0]));
            assert!(ps.iter().all(|p| ch(*p).contains(&c)));
        }
    }
}

#[test]
fn pentagons_never_edge_children() {
    for level in 1..=6u8 {
        for c in CellId::level_iter(level).filter(|c| is_pentagon(*c)) {
            let ps = pa(c);
            assert_eq!(ps.len(), 1, "{c:?}");
            assert!(is_pentagon(ps[0]));
            assert_eq!(ch(ps[0])[0], c, "pentagon is centre child of a pentagon");
        }
    }
}

#[test]
fn children_cover_next_level() {
    for level in 0..=5u8 {
        let mut count = vec![0u32; 0];
        let next: Vec<CellId> = CellId::level_iter(level + 1).collect();
        count.resize(next.len(), 0);
        for c in CellId::level_iter(level) {
            for x in ch(c) {
                let idx = next.binary_search(&x).expect("child is a level+1 cell");
                count[idx] += 1;
            }
        }
        let mut two = 0usize;
        for (x, k) in next.iter().zip(&count) {
            let np = pa(*x).len() as u32;
            assert_eq!(*k, np, "{x:?} listed by exactly its parents");
            if np == 2 {
                two += 1;
            }
        }
        assert!(count.iter().all(|k| *k >= 1), "coverage at L{}", level + 1);
        assert_eq!(two, 3 * count_for(level) - 6, "edge cells = lattice edges");
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

#[test]
fn deep_levels_sampled() {
    let mut rng = Lcg(0x5EED_1234);
    for level in [12u8, 24] {
        let n = 1u64 << level;
        for s in 0..1000 {
            let base = 1 + (rng.next() % 10) as u8;
            // Every 4th sample hugs a diamond boundary to exercise the wraps.
            let pick = |rng: &mut Lcg| match s % 4 {
                0 => rng.next() % 3,
                1 => n - 1 - rng.next() % 3,
                _ => rng.next() % n,
            };
            let (i, j) = (pick(&mut rng) as u32, pick(&mut rng) as u32);
            let c = CellId::new(base, level, i, j).unwrap();
            check_reciprocal(c);
            let ps = pa(c);
            assert!(matches!(ps.len(), 1 | 2));
            for p in &ps {
                assert!(ch(*p).contains(&c), "{c:?} missing from children of {p:?}");
            }
            if level < 24 {
                for x in ch(c) {
                    assert!(pa(x).contains(&c));
                }
            }
        }
    }
    // The 12 pentagons at the deepest level.
    let pentagons: Vec<CellId> = (1..=10u8)
        .map(|d| CellId::new(d, 24, 0, 0).unwrap())
        .chain([CellId::north_pole(24), CellId::south_pole(24)])
        .collect();
    for p in pentagons {
        assert!(is_pentagon(p));
        assert_eq!(nb(p).len(), 5);
        check_reciprocal(p);
    }
}
