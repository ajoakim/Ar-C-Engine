//! Engine lifecycle state machine with an explicit transition table.

use crate::error::LifecycleError;

/// Phases of the engine lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnginePhase {
    /// Modules are being initialized.
    Initialize,
    /// Assets/resources are loading.
    Load,
    /// Frames are being driven.
    Running,
    /// Paused; may resume.
    Suspend,
    /// Terminal phase.
    Shutdown,
}

impl EnginePhase {
    /// Every phase, in declaration order.
    pub const ALL: [EnginePhase; 5] = [
        EnginePhase::Initialize,
        EnginePhase::Load,
        EnginePhase::Running,
        EnginePhase::Suspend,
        EnginePhase::Shutdown,
    ];
}

/// Whether `from -> to` is an allowed transition.
pub fn is_legal_transition(from: EnginePhase, to: EnginePhase) -> bool {
    use EnginePhase::*;
    matches!(
        (from, to),
        (Initialize, Load)
            | (Load, Running)
            | (Running, Suspend)
            | (Suspend, Running)
            | (Running, Shutdown)
            | (Suspend, Shutdown)
            | (Load, Shutdown)
            | (Initialize, Shutdown)
    )
}

/// Lifecycle state holder.
#[derive(Debug)]
pub struct Lifecycle {
    phase: EnginePhase,
}

impl Lifecycle {
    /// Start in [`EnginePhase::Initialize`].
    pub fn new() -> Self {
        Self {
            phase: EnginePhase::Initialize,
        }
    }

    /// Current phase.
    pub fn phase(&self) -> EnginePhase {
        self.phase
    }

    /// Move to `to`, or fail leaving the phase unchanged.
    pub fn transition(&mut self, to: EnginePhase) -> Result<(), LifecycleError> {
        let from = self.phase;
        if !is_legal_transition(from, to) {
            return Err(LifecycleError::IllegalTransition { from, to });
        }
        tracing::debug!(?from, ?to, "lifecycle transition");
        self.phase = to;
        Ok(())
    }
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive_to(target: EnginePhase) -> Lifecycle {
        use EnginePhase::*;
        let path: &[EnginePhase] = match target {
            Initialize => &[],
            Load => &[Load],
            Running => &[Load, Running],
            Suspend => &[Load, Running, Suspend],
            Shutdown => &[Shutdown],
        };
        let mut l = Lifecycle::new();
        for p in path {
            assert!(l.transition(*p).is_ok());
        }
        assert_eq!(l.phase(), target);
        l
    }

    #[test]
    fn lifecycle_all_pairs() {
        let mut legal = 0;
        for from in EnginePhase::ALL {
            for to in EnginePhase::ALL {
                let mut l = drive_to(from);
                let r = l.transition(to);
                if is_legal_transition(from, to) {
                    legal += 1;
                    assert!(r.is_ok(), "{from:?}->{to:?}");
                    assert_eq!(l.phase(), to);
                } else {
                    assert_eq!(r, Err(LifecycleError::IllegalTransition { from, to }));
                    assert_eq!(l.phase(), from);
                }
            }
        }
        assert_eq!(legal, 8);
    }

    #[test]
    fn lifecycle_starts_initialize() {
        assert_eq!(Lifecycle::default().phase(), EnginePhase::Initialize);
    }

    #[test]
    fn lifecycle_shutdown_is_terminal() {
        for to in EnginePhase::ALL {
            let mut l = drive_to(EnginePhase::Shutdown);
            assert!(l.transition(to).is_err());
        }
    }

    #[test]
    fn lifecycle_full_path() {
        use EnginePhase::*;
        let mut l = Lifecycle::new();
        for p in [Load, Running, Suspend, Running, Shutdown] {
            assert!(l.transition(p).is_ok());
        }
    }
}
