mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use common::*;
use engine_streaming::{
    JobSystem, Priority, ResidencyState, StreamConfig, StreamManager, StreamRequest,
};

fn jobs() -> JobSystem {
    JobSystem::builder().workers(2).build().expect("jobs")
}

fn cfg(comps: u32) -> StreamConfig {
    StreamConfig {
        max_pending: 16,
        max_starts_per_frame: 4,
        max_completions_per_frame: comps,
    }
}

#[test]
fn cancel_pending_holds_no_bytes() {
    let k = TestKey::new(0, 1);
    let mut m = StreamManager::new(
        cfg(4),
        residency(1000, 1000, 1000),
        Arc::new(GatedSource::new(100)),
    )
    .expect("mgr");
    m.request(StreamRequest::new(k, Priority::Normal, CLASS))
        .unwrap();
    assert_eq!(m.cancel(&k), Ok(true));
    assert_eq!(m.pending_len(), 0);
    assert_eq!(cpu_used(m.residency(), CLASS), 0);
    assert_eq!(m.cancel(&k), Ok(false));
}

#[test]
fn cancel_generating_frees_budget_immediately() {
    let j = jobs();
    let k = TestKey::new(0, 1);
    let (s, released) = GatedSource::gated(&[k], 600);
    let mut m = StreamManager::new(cfg(4), residency(1000, 1000, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    m.request(StreamRequest::new(k, Priority::Normal, CLASS))
        .unwrap();
    m.update(&j, 1, &mut up).unwrap();
    assert_eq!(cpu_used(m.residency(), CLASS), 600);
    assert_eq!(m.cancel(&k), Ok(true));
    assert_eq!(cpu_used(m.residency(), CLASS), 0);
    assert_eq!(m.state(&k), Some(ResidencyState::Unloaded));
    assert!(m.residency().verify_accounting());
    released.store(u64::MAX, Ordering::Release);
    m.settle();
    m.update(&j, 2, &mut up).unwrap();
    assert_eq!(m.state(&k), Some(ResidencyState::Unloaded));
    assert_eq!(cpu_used(m.residency(), CLASS), 0);
    assert!(up.uploaded.is_empty());
    j.shutdown();
}

#[test]
fn cancel_token_frees_budget_on_next_update() {
    let j = jobs();
    let k = TestKey::new(0, 1);
    let (s, released) = GatedSource::gated(&[k], 600);
    let mut m = StreamManager::new(cfg(4), residency(1000, 1000, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    let r = StreamRequest::new(k, Priority::Normal, CLASS);
    let token = r.cancel.clone();
    m.request(r).unwrap();
    m.update(&j, 1, &mut up).unwrap();
    assert_eq!(cpu_used(m.residency(), CLASS), 600);
    token.cancel();
    assert_eq!(cpu_used(m.residency(), CLASS), 600);
    let rep = m.update(&j, 2, &mut up).unwrap();
    assert_eq!(cpu_used(m.residency(), CLASS), 0);
    assert_eq!(rep.cancelled, 1);
    assert!(m.residency().verify_accounting());
    released.store(u64::MAX, Ordering::Release);
    m.settle();
    j.shutdown();
}

#[test]
fn cancel_cpu_ready_evicts() {
    let j = jobs();
    let (a, b) = (TestKey::new(0, 1), TestKey::new(0, 2));
    let (s, released) = GatedSource::gated_at(&[a, b], 100, 1);
    let mut m = StreamManager::new(cfg(1), residency(1000, 1000, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    m.request(StreamRequest::new(a, Priority::High, CLASS))
        .unwrap();
    m.request(StreamRequest::new(b, Priority::Normal, CLASS))
        .unwrap();
    // Producers stay gated while update polls, so frame 1 applies nothing.
    m.update(&j, 1, &mut up).unwrap();
    released.store(1, Ordering::Release);
    m.settle();
    // One completion per frame: exactly one key is applied and uploaded, the
    // other stays deferred (CpuReady). Event order follows asset id order.
    m.update(&j, 2, &mut up).unwrap();
    let (done, deferred) = if m.state(&a) == Some(ResidencyState::GpuResident) {
        (a, b)
    } else {
        (b, a)
    };
    assert_eq!(m.state(&done), Some(ResidencyState::GpuResident));
    assert_eq!(m.state(&deferred), Some(ResidencyState::CpuReady));
    assert_eq!(m.cancel(&deferred), Ok(true));
    assert_eq!(m.state(&deferred), Some(ResidencyState::Unloaded));
    assert_eq!(cpu_used(m.residency(), CLASS), 100);
    assert!(m.residency().verify_accounting());
    m.update(&j, 3, &mut up).unwrap();
    assert!(!up.uploaded.contains_key(&deferred));
    assert_eq!(m.state(&deferred), Some(ResidencyState::Unloaded));
    j.shutdown();
}

#[test]
fn cancel_failed_producer_releases() {
    let j = jobs();
    let k = TestKey::new(0, 1);
    let (mut s, released) = GatedSource::gated_at(&[k], 100, 1);
    s.fail.insert(k);
    let mut m = StreamManager::new(cfg(4), residency(1000, 1000, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    m.request(StreamRequest::new(k, Priority::Normal, CLASS))
        .unwrap();
    m.update(&j, 1, &mut up).unwrap();
    released.store(1, Ordering::Release);
    m.settle();
    let rep = m.update(&j, 2, &mut up).unwrap();
    assert_eq!(m.state(&k), Some(ResidencyState::Failed));
    assert_eq!(cpu_used(m.residency(), CLASS), 0);
    assert_eq!(rep.failed, 1);
    assert!(
        m.request(StreamRequest::new(k, Priority::Normal, CLASS))
            .is_ok()
    );
    j.shutdown();
}
