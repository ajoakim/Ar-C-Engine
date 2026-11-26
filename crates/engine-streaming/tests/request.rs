mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use common::*;
use engine_streaming::{
    AssetClass, AssetError, CancelToken, FrameReport, JobSystem, Priority, ResidencyState,
    StreamConfig, StreamError, StreamKey, StreamManager, StreamRequest,
};

fn jobs(paused: bool) -> JobSystem {
    JobSystem::builder()
        .workers(2)
        .start_paused(paused)
        .build()
        .expect("jobs")
}

fn req(k: TestKey, p: Priority) -> StreamRequest<TestKey> {
    StreamRequest::new(k, p, CLASS)
}

fn cfg(pending: usize, starts: u32, comps: u32) -> StreamConfig {
    StreamConfig {
        max_pending: pending,
        max_starts_per_frame: starts,
        max_completions_per_frame: comps,
    }
}

#[test]
fn request_carries_all_fields() {
    let k = TestKey::new(1, 2);
    let r = StreamRequest::new(k, Priority::High, AssetClass(7));
    assert_eq!(r.key, k);
    assert_eq!(r.priority, Priority::High);
    assert_eq!(r.budget_class, AssetClass(7));
    assert!(!r.cancel.is_cancelled());
    let tok = CancelToken::new();
    let r = r.with_cancel(tok.clone());
    tok.cancel();
    assert!(r.cancel.is_cancelled());
}

#[test]
fn request_priority_order() {
    let j = jobs(true);
    let mut m = StreamManager::new(
        cfg(16, 1, 8),
        residency(1000, 1000, 1000),
        Arc::new(GatedSource::new(10)),
    )
    .expect("mgr");
    let (a, b, c, d) = (
        TestKey::new(0, 1),
        TestKey::new(0, 2),
        TestKey::new(0, 3),
        TestKey::new(0, 4),
    );
    m.request(req(a, Priority::Low)).unwrap();
    m.request(req(b, Priority::High)).unwrap();
    m.request(req(c, Priority::Normal)).unwrap();
    m.request(req(d, Priority::High)).unwrap();
    assert_eq!(m.pending_keys(), vec![b, d, c, a]);
    let mut up = RecordingUploader::default();
    m.update(&j, 1, &mut up).unwrap();
    assert_eq!(m.state(&b), Some(ResidencyState::Generating));
    assert_eq!(m.pending_keys(), vec![d, c, a]);
    j.resume();
    m.settle();
    j.shutdown();
}

#[test]
fn request_queue_full_rejects() {
    let mut m = StreamManager::new(
        cfg(3, 1, 1),
        residency(1000, 1000, 1000),
        Arc::new(GatedSource::new(10)),
    )
    .expect("mgr");
    let keys: Vec<TestKey> = (0..4).map(|i| TestKey::new(0, i)).collect();
    for k in &keys[..3] {
        m.request(req(*k, Priority::Normal)).unwrap();
    }
    assert_eq!(
        m.request(req(keys[3], Priority::Critical)),
        Err(StreamError::QueueFull { max: 3 })
    );
    assert_eq!(m.pending_len(), 3);
    assert_eq!(m.pending_keys(), keys[..3].to_vec());
}

#[test]
fn request_duplicate_rejected() {
    let mut m = StreamManager::new(
        cfg(8, 1, 1),
        residency(1000, 1000, 1000),
        Arc::new(GatedSource::new(10)),
    )
    .expect("mgr");
    let k = TestKey::new(0, 1);
    m.request(req(k, Priority::Normal)).unwrap();
    assert_eq!(
        m.request(req(k, Priority::High)),
        Err(StreamError::Duplicate(k.asset_id()))
    );
}

#[test]
fn request_unknown_class_errors() {
    let mut m = StreamManager::new(
        cfg(8, 1, 1),
        residency(1000, 1000, 1000),
        Arc::new(GatedSource::new(10)),
    )
    .expect("mgr");
    let r = StreamRequest::new(TestKey::new(0, 1), Priority::Normal, AssetClass(99));
    assert!(matches!(
        m.request(r),
        Err(StreamError::Asset(AssetError::UnknownClass(_)))
    ));
}

#[test]
fn request_invalid_config_rejected() {
    let r = StreamManager::<TestKey, GatedSource>::new(
        cfg(0, 1, 1),
        residency(1, 1, 1),
        Arc::new(GatedSource::new(1)),
    );
    assert!(matches!(r, Err(StreamError::InvalidConfig(_))));
}

fn run_caps_scenario() -> Vec<FrameReport> {
    let j = jobs(false);
    let keys: Vec<TestKey> = (0..10).map(|i| TestKey::new(0, i)).collect();
    let (src, released) = GatedSource::gated_at(&keys, 100, 1);
    let mut m = StreamManager::new(cfg(32, 3, 2), residency(10_000, 10_000, 250), Arc::new(src))
        .expect("mgr");
    for k in &keys {
        m.request(req(*k, Priority::Normal)).unwrap();
    }
    let mut up = RecordingUploader::default();
    let mut reports = Vec::new();
    for frame in 1..=40u64 {
        // Close the gate so jobs started this frame cannot finish during update's poll.
        released.store(0, Ordering::Release);
        let r = m.update(&j, frame, &mut up).unwrap();
        // Open it and settle so completion visibility is deterministic.
        released.store(1, Ordering::Release);
        m.settle();
        assert!(r.started <= 3);
        assert!(r.completions_applied <= 2);
        assert!(r.uploaded_bytes <= 250);
        reports.push(r);
        if keys
            .iter()
            .all(|k| m.state(k) == Some(ResidencyState::GpuResident))
        {
            break;
        }
    }
    for k in &keys {
        assert_eq!(m.state(k), Some(ResidencyState::GpuResident));
    }
    assert!(m.residency().verify_accounting());
    j.shutdown();
    reports
}

#[test]
fn request_caps_hold_per_frame() {
    let a = run_caps_scenario();
    let b = run_caps_scenario();
    assert_eq!(a, b);
    assert!(a.len() < 40);
}

#[test]
fn request_over_budget_waits() {
    let j = jobs(false);
    let (a, b) = (TestKey::new(0, 1), TestKey::new(0, 2));
    let (s, released) = GatedSource::gated(&[a, b], 100);
    let mut m =
        StreamManager::new(cfg(8, 4, 4), residency(150, 1000, 1000), Arc::new(s)).expect("mgr");
    m.request(req(a, Priority::High)).unwrap();
    m.request(req(b, Priority::Normal)).unwrap();
    let mut up = RecordingUploader::default();
    for f in 1..=5 {
        m.update(&j, f, &mut up).unwrap();
        assert_eq!(m.pending_len(), 1);
        assert_eq!(m.state(&b), Some(ResidencyState::Unloaded));
        assert_eq!(m.state(&a), Some(ResidencyState::Generating));
    }
    released.store(u64::MAX, Ordering::Release);
    m.settle();
    j.shutdown();
}
