mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use common::*;
use engine_streaming::{
    FrameReport, JobSystem, Priority, ResidencyState, Resolved, StreamConfig, StreamManager,
    StreamRequest,
};

fn jobs(workers: usize) -> JobSystem {
    JobSystem::builder().workers(workers).build().expect("jobs")
}

fn cfg(starts: u32, comps: u32) -> StreamConfig {
    StreamConfig {
        max_pending: 16,
        max_starts_per_frame: starts,
        max_completions_per_frame: comps,
    }
}

type Mgr = StreamManager<TestKey, GatedSource>;

/// Make `p` GpuResident: update (starts, gate closed), open gate, settle,
/// update (applies and uploads). Leaves `released` at 1.
fn load_root(
    m: &mut Mgr,
    j: &JobSystem,
    up: &mut RecordingUploader,
    released: &AtomicU64,
    p: TestKey,
) {
    m.request(StreamRequest::new(p, Priority::Normal, CLASS))
        .unwrap();
    m.update(j, 1, up).unwrap();
    released.store(1, Ordering::Release);
    m.settle();
    m.update(j, 2, up).unwrap();
    assert_eq!(m.state(&p), Some(ResidencyState::GpuResident));
}

/// Root gated at level 1, listed children gated at `child_level`.
fn source_for(p: TestKey, children: &[TestKey], child_level: u64) -> (GatedSource, Arc<AtomicU64>) {
    let (mut s, r) = GatedSource::gated_at(&[p], 100, 1);
    for c in children {
        s.release_at.insert(*c, child_level);
    }
    (s, r)
}

#[test]
fn slow_load_parent_served_until_child_resident() {
    let j = jobs(2);
    let p = TestKey::new(0, 0);
    let c = TestKey::new(1, 0);
    let (s, released) = source_for(p, &[c], 100);
    let mut m = Mgr::new(cfg(4, 4), residency(1000, 1000, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    load_root(&mut m, &j, &mut up, &released, p);

    m.request(StreamRequest::new(c, Priority::Normal, CLASS))
        .unwrap();
    for frame in 3..=60 {
        m.update(&j, frame, &mut up).expect("update ok");
        assert_eq!(
            m.resolve(&c),
            Some(Resolved {
                key: p,
                exact: false
            })
        );
        assert_eq!(m.pin_count(&p), 1);
        assert_eq!(m.state(&c), Some(ResidencyState::Generating));
    }
    released.store(100, Ordering::Release);
    m.settle();
    m.update(&j, 61, &mut up).unwrap();
    assert_eq!(
        m.resolve(&c),
        Some(Resolved {
            key: c,
            exact: true
        })
    );
    assert_eq!(m.pin_count(&p), 0);
    j.shutdown();
}

#[test]
fn slow_load_grandchild_falls_back_two_levels() {
    let j = jobs(2);
    let p = TestKey::new(0, 0);
    let g = TestKey::new(2, 0);
    let (s, released) = source_for(p, &[g], u64::MAX);
    let mut m = Mgr::new(cfg(4, 4), residency(1000, 1000, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    load_root(&mut m, &j, &mut up, &released, p);

    m.request(StreamRequest::new(g, Priority::Normal, CLASS))
        .unwrap();
    m.update(&j, 3, &mut up).unwrap();
    assert_eq!(
        m.resolve(&g),
        Some(Resolved {
            key: p,
            exact: false
        })
    );
    assert_eq!(m.pin_count(&p), 1);
    released.store(u64::MAX, Ordering::Release);
    m.settle();
    m.update(&j, 4, &mut up).unwrap();
    assert_eq!(m.pin_count(&p), 0);
    j.shutdown();
}

#[test]
fn slow_load_nothing_resident_returns_none() {
    let mut m = Mgr::new(
        cfg(4, 4),
        residency(1000, 1000, 1000),
        Arc::new(GatedSource::new(100)),
    )
    .expect("mgr");
    assert_eq!(m.resolve(&TestKey::new(3, 5)), None);
}

fn run_caps_scenario() -> Vec<FrameReport> {
    let j = jobs(1);
    let keys: Vec<TestKey> = (1..=12).map(|i| TestKey::new(0, i)).collect();
    let mut s = GatedSource::new(100);
    // Groups of four share a release tick: 10, 40, 70.
    for (i, k) in keys.iter().enumerate() {
        s.release_at.insert(*k, 30 * (i as u64 / 4) + 10);
    }
    let released = Arc::clone(&s.released);
    let config = cfg(2, 1);
    let mut m = Mgr::new(config, residency(2000, 2000, 150), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    let mut reports = Vec::new();
    for f in 1..=120u64 {
        // Gate opened only just before settle, closed again during update, so
        // visibility of completions is deterministic.
        if f == 10 || f == 40 || f == 70 {
            released.store(f, Ordering::Release);
            m.settle();
            released.store(0, Ordering::Release);
        }
        if f == 1 || f == 31 || f == 61 {
            let g = (f / 30) as usize;
            for k in &keys[4 * g..4 * g + 4] {
                m.request(StreamRequest::new(*k, Priority::Normal, CLASS))
                    .unwrap();
            }
        }
        let r = m.update(&j, f, &mut up).expect("update ok");
        assert!(r.started <= 2, "started cap: {r:?}");
        assert!(r.completions_applied <= 1, "completion cap: {r:?}");
        assert!(r.uploaded_bytes <= 150, "upload cap: {r:?}");
        assert!(r.pending <= config.max_pending);
        reports.push(r);
    }
    for k in &keys {
        assert_eq!(m.state(k), Some(ResidencyState::GpuResident), "{k:?}");
    }
    assert!(reports.iter().any(|r| r.deferred_completions > 0));
    assert_eq!(reports.iter().map(|r| r.uploads).sum::<u32>(), 12);
    released.store(u64::MAX, Ordering::Release);
    j.shutdown();
    reports
}

#[test]
fn slow_load_caps_hold_every_frame_under_gated_workers() {
    let a = run_caps_scenario();
    let b = run_caps_scenario();
    assert_eq!(a, b, "frame of each refinement must be deterministic");
}

#[test]
fn slow_load_update_never_waits() {
    let j = jobs(1);
    let keys = [TestKey::new(0, 1), TestKey::new(0, 2), TestKey::new(0, 3)];
    let (s, released) = GatedSource::gated(&keys, 100);
    let mut m = Mgr::new(cfg(2, 2), residency(1000, 1000, 1000), Arc::new(s)).expect("mgr");
    let mut up = RecordingUploader::default();
    for k in &keys {
        m.request(StreamRequest::new(*k, Priority::Normal, CLASS))
            .unwrap();
    }
    for f in 1..=500u64 {
        let r = m.update(&j, f, &mut up).expect("update ok");
        assert_eq!(r.completions_applied, 0);
        assert_eq!(r.uploads, 0);
    }
    assert_eq!(m.in_flight_len(), 3);
    released.store(u64::MAX, Ordering::Release);
    m.settle();
    for f in 501..=506u64 {
        m.update(&j, f, &mut up).unwrap();
    }
    for k in &keys {
        assert_eq!(m.state(k), Some(ResidencyState::GpuResident));
    }
    j.shutdown();
}
