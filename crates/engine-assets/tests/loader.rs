//! Loader integration tests against a real JobSystem. No sleeps: sync via
//! `start_paused(true)` + `resume()` + blocking `wait_all`.

use engine_assets::{
    AssetClass, AssetError, AssetId, AsyncLoader, ClassBudget, LoadEvent, ResidencyManager,
    ResidencyState,
};
use engine_jobs::{JobSystem, Priority};

fn setup(budget_cpu: u64) -> (ResidencyManager, AssetClass) {
    let class = AssetClass(1);
    let mut m = ResidencyManager::new(1 << 20);
    m.set_class_budget(
        class,
        ClassBudget {
            cpu_bytes: budget_cpu,
            gpu_bytes: 1 << 20,
        },
    );
    (m, class)
}

fn jobs(workers: usize, paused: bool) -> JobSystem {
    JobSystem::builder()
        .workers(workers)
        .start_paused(paused)
        .build()
        .expect("job system")
}

fn id(k: &str) -> AssetId {
    AssetId::from_key(k).expect("key")
}

fn requested(m: &mut ResidencyManager, class: AssetClass, k: &str) -> AssetId {
    let a = id(k);
    m.track(a, class).expect("track");
    m.request(a).expect("request");
    a
}

fn cpu_used(m: &ResidencyManager) -> u64 {
    m.stats().classes[0].cpu_used
}

#[test]
fn loader_load_reaches_cpu_ready() {
    let (mut m, c) = setup(1024);
    let js = jobs(1, true);
    let mut l = AsyncLoader::new();
    let a = requested(&mut m, c, "a");
    l.start(&js, &mut m, a, Priority::Normal, 64, |_| {
        Some(vec![7u8; 64])
    })
    .expect("start");
    assert_eq!(m.state(a), Some(ResidencyState::Generating));
    js.resume();
    let ev = l.wait_all(&mut m);
    assert_eq!(
        ev,
        vec![LoadEvent::Loaded {
            id: a,
            data: vec![7u8; 64]
        }]
    );
    assert_eq!(m.state(a), Some(ResidencyState::CpuReady));
    assert_eq!(cpu_used(&m), 64);
    assert!(m.verify_accounting());
    js.shutdown();
}

#[test]
fn loader_cancel_returns_to_unloaded() {
    let (mut m, c) = setup(1024);
    let js = jobs(1, true);
    let mut l = AsyncLoader::new();
    let a = requested(&mut m, c, "a");
    l.start(&js, &mut m, a, Priority::Normal, 64, |_| {
        Some(vec![1u8; 64])
    })
    .expect("start");
    l.cancel(&mut m, a).expect("cancel");
    assert_eq!(m.state(a), Some(ResidencyState::Unloaded));
    assert_eq!(cpu_used(&m), 0);
    js.resume();
    let ev = l.wait_all(&mut m);
    assert!(!ev.iter().any(|e| matches!(e, LoadEvent::Loaded { .. })));
    assert_eq!(m.state(a), Some(ResidencyState::Unloaded));
    assert!(m.verify_accounting());
    js.shutdown();
}

#[test]
fn loader_panic_marks_failed() {
    let (mut m, c) = setup(1024);
    let js = jobs(1, true);
    let mut l = AsyncLoader::new();
    let a = requested(&mut m, c, "a");
    l.start(&js, &mut m, a, Priority::Normal, 64, |_| {
        panic!("synthetic loader failure")
    })
    .expect("start");
    js.resume();
    assert_eq!(l.wait_all(&mut m), vec![LoadEvent::Failed { id: a }]);
    assert_eq!(m.state(a), Some(ResidencyState::Failed));
    assert_eq!(cpu_used(&m), 0);
    assert!(m.verify_accounting());
    js.shutdown();
}

#[test]
fn loader_producer_none_marks_failed() {
    let (mut m, c) = setup(1024);
    let js = jobs(1, true);
    let mut l = AsyncLoader::new();
    let a = requested(&mut m, c, "a");
    l.start(&js, &mut m, a, Priority::Normal, 64, |_| None)
        .expect("start");
    js.resume();
    assert_eq!(l.wait_all(&mut m), vec![LoadEvent::Failed { id: a }]);
    assert_eq!(m.state(a), Some(ResidencyState::Failed));
    assert_eq!(cpu_used(&m), 0);
    js.shutdown();
}

#[test]
fn loader_stale_generation_ignored() {
    let (mut m, c) = setup(1024);
    let js = jobs(1, true);
    let mut l = AsyncLoader::new();
    let a = requested(&mut m, c, "a");
    l.start(&js, &mut m, a, Priority::Normal, 16, |_| {
        Some(vec![1u8; 16])
    })
    .expect("start A1");
    l.cancel(&mut m, a).expect("cancel");
    m.request(a).expect("re-request");
    l.start(&js, &mut m, a, Priority::Normal, 16, |_| {
        Some(vec![2u8; 16])
    })
    .expect("start A2");
    js.resume();
    let ev = l.wait_all(&mut m);
    assert_eq!(
        ev,
        vec![LoadEvent::Loaded {
            id: a,
            data: vec![2u8; 16]
        }]
    );
    assert_eq!(m.state(a), Some(ResidencyState::CpuReady));
    assert_eq!(cpu_used(&m), 16);
    assert!(m.verify_accounting());
    js.shutdown();
}

#[test]
fn loader_budget_rejects_start() {
    let (mut m, c) = setup(32);
    let js = jobs(1, true);
    let mut l = AsyncLoader::new();
    let a = requested(&mut m, c, "a");
    let r = l.start(&js, &mut m, a, Priority::Normal, 64, |_| {
        Some(vec![0u8; 64])
    });
    assert!(matches!(r, Err(AssetError::OverBudget { .. })));
    assert_eq!(js.queued_len(), 0);
    assert_eq!(l.pending_len(), 0);
    assert_eq!(m.state(a), Some(ResidencyState::Requested));
    js.shutdown();
}

#[test]
fn loader_many_assets_multi_worker() {
    let (mut m, c) = setup(64 * 16);
    let js = jobs(4, false);
    let mut l = AsyncLoader::new();
    for i in 0..64u8 {
        let a = requested(&mut m, c, &format!("asset{i}"));
        l.start(&js, &mut m, a, Priority::Normal, 16, move |_| {
            Some(vec![i; 16])
        })
        .expect("start");
    }
    let ev = l.wait_all(&mut m);
    assert_eq!(ev.len(), 64);
    assert!(ev.iter().all(|e| matches!(e, LoadEvent::Loaded { .. })));
    assert_eq!(m.stats().state_counts[ResidencyState::CpuReady.index()], 64);
    assert!(m.verify_accounting());
    js.shutdown();
}
