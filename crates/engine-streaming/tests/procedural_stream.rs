//! End-to-end streamed procedural determinism on the real `JobSystem`
//! (PROC-02, STRM-01, STRM-02, ADR-0008).

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::*;
use engine_procedural::{
    Fnv1a64, Generator, LayerStack, Params, StableId, fbm_3d, splitmix64, unit_f64, value_noise_3d,
};
use engine_streaming::{
    CancelToken, JobSystem, Priority, ResidencyState, StreamConfig, StreamManager, StreamRequest,
    StreamSource,
};

const WORLD_SEED: u64 = 0x0123_4567_89AB_CDEF;

// Pinned on Windows x86_64; asserted on the 3-OS CI matrix (PROC-02).
const GOLDEN_STREAMED: u64 = 0x43c8_6516_7d16_b0fe;

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

struct DetailLayer;
impl Generator for DetailLayer {
    fn name(&self) -> &'static str {
        "detail"
    }
    fn version(&self) -> u32 {
        1
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

fn id_of(k: &TestKey) -> u64 {
    (u64::from(k.depth) << 56) | k.index
}

fn make_stack() -> LayerStack {
    let mut s = LayerStack::new(WORLD_SEED);
    s.push(Box::new(BaseLayer)).unwrap();
    s.push(Box::new(DetailLayer)).unwrap();
    s
}

fn make_params() -> Params {
    Params::new().with("frequency", 0.03125)
}

struct ProcSource {
    stack: LayerStack,
    params: Params,
}

impl StreamSource<TestKey> for ProcSource {
    fn estimate_bytes(&self, _key: &TestKey) -> u64 {
        (self.stack.output_len() as u64) * 8
    }
    fn produce(&self, key: &TestKey, cancel: &CancelToken) -> Option<Vec<u8>> {
        if cancel.is_cancelled() {
            return None;
        }
        self.stack
            .generate(StableId(id_of(key)), &self.params)
            .to_le_bytes()
            .ok()
    }
}

fn keys() -> Vec<TestKey> {
    let mut v = Vec::new();
    for depth in 0..4u8 {
        for index in 0..16u64 {
            v.push(TestKey::new(depth, index));
        }
    }
    v
}

fn run(
    workers: usize,
    order: &[TestKey],
    prio: impl Fn(usize) -> Priority,
) -> BTreeMap<TestKey, Vec<u8>> {
    let jobs = JobSystem::builder().workers(workers).build().expect("jobs");
    let cfg = StreamConfig {
        max_pending: 64,
        max_starts_per_frame: 8,
        max_completions_per_frame: 4,
    };
    let src = ProcSource {
        stack: make_stack(),
        params: make_params(),
    };
    let mut m =
        StreamManager::new(cfg, residency(1 << 20, 1 << 20, 1 << 16), Arc::new(src)).expect("mgr");
    let mut up = RecordingUploader::default();
    for (i, k) in order.iter().enumerate() {
        m.request(StreamRequest::new(*k, prio(i), CLASS))
            .expect("request");
    }
    let mut done = false;
    for frame in 0..1000u64 {
        m.update(&jobs, frame, &mut up).expect("update");
        m.settle();
        if order
            .iter()
            .all(|k| m.state(k) == Some(ResidencyState::GpuResident))
        {
            done = true;
            break;
        }
    }
    assert!(done, "streaming did not finish before frame 1000");
    up.uploaded
}

fn golden_of(m: &BTreeMap<TestKey, Vec<u8>>) -> u64 {
    let mut h = Fnv1a64::new();
    for (k, bytes) in m {
        h.write_u32(u32::from(k.depth));
        h.write_u64(k.index);
        h.write_u64(bytes.len() as u64);
        h.write(bytes);
    }
    h.finish()
}

fn uniform(_: usize) -> Priority {
    Priority::Normal
}

#[test]
fn procedural_stream_identical_across_workers() {
    let ks = keys();
    let a = run(1, &ks, uniform);
    let b = run(4, &ks, uniform);
    assert_eq!(a.len(), 64);
    assert_eq!(a, b);
}

#[test]
fn procedural_stream_matches_direct_generation() {
    let ks = keys();
    let got = run(4, &ks, uniform);
    let stack = make_stack();
    let params = make_params();
    for k in &ks {
        let want = stack
            .generate(StableId(id_of(k)), &params)
            .to_le_bytes()
            .unwrap();
        assert_eq!(got.get(k), Some(&want), "{k:?}");
    }
}

#[test]
fn procedural_stream_golden() {
    let ks = keys();
    for workers in [1, 4] {
        let got = golden_of(&run(workers, &ks, uniform));
        assert_eq!(got, GOLDEN_STREAMED, "got {got:#018x} workers={workers}");
    }
}

#[test]
fn procedural_stream_priority_order_irrelevant() {
    let ks = keys();
    let mut rev = ks.clone();
    rev.reverse();
    let mixed = |i: usize| match i % 4 {
        0 => Priority::Background,
        1 => Priority::Low,
        2 => Priority::Normal,
        _ => Priority::High,
    };
    let a = run(1, &ks, uniform);
    let b = run(4, &rev, mixed);
    assert_eq!(a, b);
}
