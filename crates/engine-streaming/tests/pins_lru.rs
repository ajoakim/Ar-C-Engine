mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use common::*;
use engine_streaming::{
    CancelToken, JobSystem, Priority, ResidencyState, StreamConfig, StreamError, StreamKey,
    StreamManager, StreamRequest, StreamSource,
};

fn jobs() -> JobSystem {
    JobSystem::builder().workers(2).build().expect("jobs")
}

fn cfg() -> StreamConfig {
    StreamConfig {
        max_pending: 16,
        max_starts_per_frame: 4,
        max_completions_per_frame: 4,
    }
}

type Mgr = StreamManager<TestKey, GatedSource>;

fn req(k: TestKey) -> StreamRequest<TestKey> {
    StreamRequest::new(k, Priority::Normal, CLASS)
}

/// Request `roots`, update (gate closed), open gate to level 1, settle,
/// update: every root ends GpuResident by frame 2.
fn load_roots(
    m: &mut Mgr,
    j: &JobSystem,
    up: &mut RecordingUploader,
    released: &AtomicU64,
    roots: &[TestKey],
) {
    for k in roots {
        m.request(req(*k)).unwrap();
    }
    m.update(j, 1, up).unwrap();
    released.store(1, Ordering::Release);
    m.settle();
    m.update(j, 2, up).unwrap();
    for k in roots {
        assert_eq!(m.state(k), Some(ResidencyState::GpuResident));
    }
}

/// Roots gated at level 1, `children` gated at `child_level`.
fn source_for(
    roots: &[TestKey],
    children: &[TestKey],
    child_level: u64,
) -> (GatedSource, Arc<AtomicU64>) {
    let (mut s, r) = GatedSource::gated_at(roots, 100, 1);
    for c in children {
        s.release_at.insert(*c, child_level);
    }
    (s, r)
}

#[test]
fn pins_lru_pinned_parent_survives_pressure() {
    let j = jobs();
    let (p, a, b) = (TestKey::new(0, 0), TestKey::new(0, 1), TestKey::new(0, 2));
    let c = TestKey::new(1, 0); // parent = p
    let (s, released) = source_for(&[p, a, b], &[c], u64::MAX);
    let mut m = Mgr::new(cfg(), residency(300, 300, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    load_roots(&mut m, &j, &mut up, &released, &[p, a, b]);

    // Frame 3 shows P, frame 4 shows A, frame 5 shows B: LRU order P, A, B.
    for (frame, k) in [(3, p), (4, a), (5, b)] {
        m.update(&j, frame, &mut up).unwrap();
        assert!(m.resolve(&k).is_some());
    }
    // Frame 10: A (shown at 4) and B (shown at 5) are not recently shown.
    m.request(req(c)).unwrap();
    let r = m.update(&j, 10, &mut up).unwrap();
    assert_eq!(r.started, 1);
    assert_eq!(r.evicted, 1);
    assert_eq!(m.state(&p), Some(ResidencyState::GpuResident));
    assert_eq!(m.state(&a), Some(ResidencyState::Unloaded));
    assert_eq!(m.state(&b), Some(ResidencyState::GpuResident));
    assert_eq!(m.pin_count(&p), 1);
    assert!(m.residency().verify_accounting());

    released.store(u64::MAX, Ordering::Release);
    m.settle();
    m.update(&j, 11, &mut up).unwrap();
    assert_eq!(m.pin_count(&p), 0);
    j.shutdown();
}

#[test]
fn pins_lru_backpressure_when_only_pins() {
    let j = jobs();
    let p = TestKey::new(0, 0);
    let (c1, c2) = (TestKey::new(1, 0), TestKey::new(1, 1)); // both children of p
    let (s, released) = source_for(&[p], &[c1, c2], u64::MAX);
    let mut m = Mgr::new(cfg(), residency(200, 200, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    load_roots(&mut m, &j, &mut up, &released, &[p]);

    m.request(req(c1)).unwrap();
    m.update(&j, 3, &mut up).unwrap();
    assert_eq!(m.pin_count(&p), 1);
    m.request(req(c2)).unwrap();
    for frame in 4..=14 {
        let r = m
            .update(&j, frame, &mut up)
            .expect("no error under pressure");
        assert_eq!(r.evicted, 0);
        assert!(m.is_pending(&c2));
        assert_eq!(m.state(&p), Some(ResidencyState::GpuResident));
        assert_eq!(m.pin_count(&p), 1);
    }
    released.store(u64::MAX, Ordering::Release);
    m.settle();
    m.update(&j, 15, &mut up).unwrap();
    m.settle();
    m.update(&j, 16, &mut up).unwrap();
    j.shutdown();
}

#[test]
fn pins_lru_explicit_evict_refuses_pinned() {
    let j = jobs();
    let p = TestKey::new(0, 0);
    let c = TestKey::new(1, 0);
    let (s, released) = source_for(&[p], &[c], u64::MAX);
    let mut m = Mgr::new(cfg(), residency(300, 300, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    load_roots(&mut m, &j, &mut up, &released, &[p]);

    m.request(req(c)).unwrap();
    m.update(&j, 3, &mut up).unwrap();
    assert_eq!(m.evict(&p), Err(StreamError::Pinned(p.asset_id())));
    assert_eq!(m.state(&p), Some(ResidencyState::GpuResident));

    released.store(u64::MAX, Ordering::Release);
    m.settle();
    m.update(&j, 4, &mut up).unwrap();
    assert_eq!(m.state(&c), Some(ResidencyState::GpuResident));
    assert_eq!(m.evict(&p), Ok(true));
    assert_eq!(m.state(&p), Some(ResidencyState::Unloaded));
    j.shutdown();
}

#[test]
fn pins_lru_unpinned_on_failure() {
    let j = jobs();
    let p = TestKey::new(0, 0);
    let c = TestKey::new(1, 0);
    let (mut s, released) = source_for(&[p], &[c], 2);
    s.fail.insert(c);
    let mut m = Mgr::new(cfg(), residency(300, 300, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    load_roots(&mut m, &j, &mut up, &released, &[p]);

    m.request(req(c)).unwrap();
    m.update(&j, 3, &mut up).unwrap();
    assert_eq!(m.pin_count(&p), 1);
    released.store(2, Ordering::Release);
    m.settle();
    let r = m.update(&j, 4, &mut up).unwrap();
    assert_eq!(r.failed, 1);
    assert_eq!(m.state(&c), Some(ResidencyState::Failed));
    assert_eq!(m.pin_count(&p), 0);
    j.shutdown();
}

struct PanicSource {
    target: TestKey,
}

impl StreamSource<TestKey> for PanicSource {
    fn estimate_bytes(&self, _key: &TestKey) -> u64 {
        100
    }
    fn produce(&self, key: &TestKey, _cancel: &CancelToken) -> Option<Vec<u8>> {
        if *key == self.target {
            panic!("synthetic");
        }
        Some(vec![0; 100])
    }
}

#[test]
fn pins_lru_unpinned_on_panic() {
    let j = jobs();
    let p = TestKey::new(0, 0);
    let c = TestKey::new(1, 0);
    let mut m = StreamManager::new(
        cfg(),
        residency(300, 300, 1000),
        Arc::new(PanicSource { target: c }),
    )
    .expect("mgr");
    let mut up = RecordingUploader::default();
    m.request(req(p)).unwrap();
    m.update(&j, 1, &mut up).unwrap();
    m.settle();
    m.update(&j, 2, &mut up).unwrap();
    assert_eq!(m.state(&p), Some(ResidencyState::GpuResident));

    m.request(req(c)).unwrap();
    m.update(&j, 3, &mut up).unwrap();
    m.settle();
    m.update(&j, 4, &mut up).unwrap();
    assert_eq!(m.state(&c), Some(ResidencyState::Failed));
    assert_eq!(m.pin_count(&p), 0);
    j.shutdown();
}

#[test]
fn pins_lru_unpinned_on_cancel() {
    let j = jobs();
    let p = TestKey::new(0, 0);
    let (c, c2) = (TestKey::new(1, 0), TestKey::new(1, 1));
    let (s, released) = source_for(&[p], &[c, c2], u64::MAX);
    let mut m = Mgr::new(cfg(), residency(400, 400, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    load_roots(&mut m, &j, &mut up, &released, &[p]);

    m.request(req(c)).unwrap();
    let r2 = req(c2);
    let token = r2.cancel.clone();
    m.request(r2).unwrap();
    m.update(&j, 3, &mut up).unwrap();
    assert_eq!(m.pin_count(&p), 2);

    assert_eq!(m.cancel(&c), Ok(true));
    assert_eq!(m.pin_count(&p), 1);
    token.cancel();
    m.update(&j, 4, &mut up).unwrap();
    assert_eq!(m.pin_count(&p), 0);

    released.store(u64::MAX, Ordering::Release);
    m.settle();
    j.shutdown();
}

#[test]
fn pins_lru_recently_shown_not_evicted() {
    let j = jobs();
    let (r, n) = (TestKey::new(0, 1), TestKey::new(0, 2));
    let (s, released) = source_for(&[r], &[], 0);
    let mut m = Mgr::new(cfg(), residency(100, 100, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    load_roots(&mut m, &j, &mut up, &released, &[r]);

    // R is shown at frame 5; at frame 6 it is protected even though it is the
    // only (hence LRU) candidate, so N waits.
    m.update(&j, 5, &mut up).unwrap();
    assert!(m.resolve(&r).is_some());
    m.request(req(n)).unwrap();
    let rep = m.update(&j, 6, &mut up).unwrap();
    assert_eq!(rep.evicted, 0);
    assert_eq!(m.state(&r), Some(ResidencyState::GpuResident));
    assert!(m.is_pending(&n));

    // At frame 7 the protection has lapsed and R is evicted for N.
    let rep = m.update(&j, 7, &mut up).unwrap();
    assert_eq!(rep.evicted, 1);
    assert_eq!(m.state(&r), Some(ResidencyState::Unloaded));
    assert_eq!(rep.started, 1);
    m.settle();
    m.update(&j, 8, &mut up).unwrap();
    j.shutdown();
}
