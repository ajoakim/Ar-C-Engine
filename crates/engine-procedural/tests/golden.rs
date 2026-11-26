//! Golden-hash suite: determinism across thread counts, submission orders and
//! layer compositions (PROC-01, PROC-02, ADR-0008).

use std::collections::BTreeMap;

use engine_procedural::{
    Fnv1a64, Generator, LayerStack, Params, StableId, fbm_3d, hash_values, splitmix64, unit_f64,
    value_noise_2d, value_noise_3d,
};

const WORLD_SEED: u64 = 0x0123_4567_89AB_CDEF;

// Pinned on Windows x86_64; asserted on Linux/macOS/Windows CI (PROC-02). Never loosen; fix the drifting op.
const GOLDEN_ALL: u64 = 0x9c64_23c0_be89_8ef7;
const GOLDEN_BASE: u64 = 0xaba8_2342_011c_3f1c;
const GOLDEN_DETAIL: u64 = 0xc388_4739_1ba9_cd5d;
const GOLDEN_MASK: u64 = 0xf500_77dd_f6ed_f6ff;

fn ids() -> Vec<StableId> {
    let mut v: Vec<StableId> = (0..256).map(StableId).collect();
    v.push(StableId(u64::MAX));
    v.push(StableId(1 << 63));
    v.push(StableId(0xDEAD_BEEF));
    v.push(StableId(0x0000_0400_0020_0001));
    v
}

fn coords(id: StableId) -> (f64, f64, f64) {
    (
        (id.0 & 0x1F_FFFF) as f64,
        ((id.0 >> 21) & 0x1F_FFFF) as f64,
        (id.0 >> 42) as f64,
    )
}

struct BaseLayer;
impl Generator for BaseLayer {
    fn name(&self) -> &'static str {
        "base"
    }
    fn version(&self) -> u32 {
        1
    }
    fn output_len(&self) -> usize {
        1
    }
    fn generate(&self, id: StableId, seed: u64, params: &Params, out: &mut [f64]) {
        let (x, y, z) = coords(id);
        let f = params.get_or("frequency", 0.03125);
        out[0] = fbm_3d(seed, x * f, y * f, z * f, 4);
    }
}

struct DetailLayer {
    version: u32,
}
impl Generator for DetailLayer {
    fn name(&self) -> &'static str {
        "detail"
    }
    fn version(&self) -> u32 {
        self.version
    }
    fn output_len(&self) -> usize {
        2
    }
    fn generate(&self, id: StableId, seed: u64, _params: &Params, out: &mut [f64]) {
        let (x, y, z) = coords(id);
        out[0] = value_noise_3d(seed, x * 0.5, y * 0.5, z * 0.5) - 0.5;
        out[1] = unit_f64(splitmix64(seed ^ id.0));
    }
}

struct MaskLayer;
impl Generator for MaskLayer {
    fn name(&self) -> &'static str {
        "mask"
    }
    fn version(&self) -> u32 {
        1
    }
    fn output_len(&self) -> usize {
        1
    }
    fn generate(&self, id: StableId, seed: u64, params: &Params, out: &mut [f64]) {
        let (x, y, _) = coords(id);
        out[0] = if value_noise_2d(seed, x * 0.25, y * 0.25) > params.get_or("threshold", 0.5) {
            1.0
        } else {
            0.0
        };
    }
}

fn layer(name: &str, detail_version: u32) -> Box<dyn Generator> {
    match name {
        "base" => Box::new(BaseLayer),
        "detail" => Box::new(DetailLayer {
            version: detail_version,
        }),
        "mask" => Box::new(MaskLayer),
        _ => panic!("unknown layer"),
    }
}

fn stack_of(seed: u64, names: &[&str], detail_version: u32) -> LayerStack {
    let mut s = LayerStack::new(seed);
    for n in names {
        s.push(layer(n, detail_version)).unwrap();
    }
    s
}

fn params() -> Params {
    Params::new()
        .with("frequency", 0.03125)
        .with("threshold", 0.5)
}

fn aggregate(m: &BTreeMap<StableId, u64>) -> u64 {
    let mut h = Fnv1a64::new();
    for (id, v) in m {
        h.write_u64(id.0);
        h.write_u64(*v);
    }
    h.finish()
}

/// Per-id result computed by `f`, run on `threads` std threads, merged by key.
fn run<F>(ids: &[StableId], threads: usize, f: F) -> BTreeMap<StableId, u64>
where
    F: Fn(StableId) -> u64 + Sync,
{
    let f = &f;
    let mut merged = BTreeMap::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                scope.spawn(move || {
                    ids.iter()
                        .skip(t)
                        .step_by(threads)
                        .map(|id| (*id, f(*id)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in handles {
            for (id, v) in h.join().unwrap() {
                merged.insert(id, v);
            }
        }
    });
    merged
}

fn whole_hash(stack: &LayerStack, id: StableId) -> u64 {
    stack.generate(id, &params()).content_hash().unwrap()
}

fn layer_agg(stack: &LayerStack, ids: &[StableId], name: &'static str) -> u64 {
    aggregate(&run(ids, 1, |id| {
        let o = stack.generate(id, &params());
        hash_values(name, o.layer(name).unwrap()).unwrap()
    }))
}

fn shuffled(mut v: Vec<StableId>) -> Vec<StableId> {
    let mut state = 0x5EED_u64;
    for i in (1..v.len()).rev() {
        state = splitmix64(state);
        let j = (state % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
    v
}

const ALL: [&str; 3] = ["base", "detail", "mask"];

#[test]
fn golden_all_threads_and_orders() {
    let stack = stack_of(WORLD_SEED, &ALL, 1);
    let fwd = ids();
    let mut rev = fwd.clone();
    rev.reverse();
    let shuf = shuffled(fwd.clone());
    assert_ne!(shuf, fwd);
    for order in [&fwd, &rev, &shuf] {
        for threads in [1, 2, 4, 8] {
            let got = aggregate(&run(order, threads, |id| whole_hash(&stack, id)));
            assert_eq!(got, GOLDEN_ALL, "got {got:#018x} threads={threads}");
        }
    }
}

#[test]
fn golden_per_layer() {
    let stack = stack_of(WORLD_SEED, &ALL, 1);
    let ids = ids();
    let b = layer_agg(&stack, &ids, "base");
    let d = layer_agg(&stack, &ids, "detail");
    let m = layer_agg(&stack, &ids, "mask");
    assert_eq!(b, GOLDEN_BASE, "base got {b:#018x}");
    assert_eq!(d, GOLDEN_DETAIL, "detail got {d:#018x}");
    assert_eq!(m, GOLDEN_MASK, "mask got {m:#018x}");
}

#[test]
fn golden_layer_isolation() {
    let ids = ids();
    let reference = layer_agg(&stack_of(WORLD_SEED, &["base"], 1), &ids, "base");
    for names in [
        &["detail", "base"][..],
        &["base", "detail", "mask"][..],
        &["mask", "base"][..],
    ] {
        let s = stack_of(WORLD_SEED, names, 1);
        assert_eq!(layer_agg(&s, &ids, "base"), reference, "stack {names:?}");
    }
}

#[test]
fn golden_version_bump_isolated() {
    let ids = ids();
    let v1 = stack_of(WORLD_SEED, &ALL, 1);
    let v2 = stack_of(WORLD_SEED, &ALL, 2);
    assert_ne!(
        layer_agg(&v1, &ids, "detail"),
        layer_agg(&v2, &ids, "detail")
    );
    assert_eq!(layer_agg(&v1, &ids, "base"), layer_agg(&v2, &ids, "base"));
    assert_eq!(layer_agg(&v1, &ids, "mask"), layer_agg(&v2, &ids, "mask"));
}

#[test]
fn golden_world_seed_changes_everything() {
    let ids = ids();
    let a = stack_of(WORLD_SEED, &ALL, 1);
    let b = stack_of(WORLD_SEED + 1, &ALL, 1);
    for name in ["base", "detail", "mask"] {
        assert_ne!(
            layer_agg(&a, &ids, name),
            layer_agg(&b, &ids, name),
            "{name}"
        );
    }
}

#[test]
fn golden_purity_repeat() {
    let stack = stack_of(WORLD_SEED, &ALL, 1);
    let p = params();
    let first = stack.generate(StableId(5), &p);
    let _ = stack.generate(StableId(9), &p);
    let second = stack.generate(StableId(5), &p);
    assert_eq!(first, second);
    let fresh = stack_of(WORLD_SEED, &ALL, 1).generate(StableId(5), &p);
    assert_eq!(first, fresh);
}
