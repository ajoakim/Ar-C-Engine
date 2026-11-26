//! Residency state machine.
//!
//! Legal transitions (17 edges, everything else is illegal):
//!
//! | From        | To                                  |
//! |-------------|-------------------------------------|
//! | Unloaded    | Requested                           |
//! | Requested   | Generating, Failed, Unloaded        |
//! | Generating  | CpuReady, Failed, Unloaded          |
//! | CpuReady    | Uploading, Evicting, Failed         |
//! | Uploading   | GpuResident, Failed, CpuReady       |
//! | GpuResident | Evicting                            |
//! | Evicting    | Unloaded                            |
//! | Failed      | Requested, Unloaded                 |

use crate::error::AssetError;

/// Residency state of an asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ResidencyState {
    /// Nothing resident.
    Unloaded,
    /// Load requested.
    Requested,
    /// CPU data being generated.
    Generating,
    /// CPU data ready.
    CpuReady,
    /// GPU upload in progress.
    Uploading,
    /// Resident on the GPU.
    GpuResident,
    /// Being evicted.
    Evicting,
    /// Failed.
    Failed,
}

impl ResidencyState {
    /// All states, in index order.
    pub const ALL: [ResidencyState; 8] = [
        ResidencyState::Unloaded,
        ResidencyState::Requested,
        ResidencyState::Generating,
        ResidencyState::CpuReady,
        ResidencyState::Uploading,
        ResidencyState::GpuResident,
        ResidencyState::Evicting,
        ResidencyState::Failed,
    ];

    /// Position in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            ResidencyState::Unloaded => 0,
            ResidencyState::Requested => 1,
            ResidencyState::Generating => 2,
            ResidencyState::CpuReady => 3,
            ResidencyState::Uploading => 4,
            ResidencyState::GpuResident => 5,
            ResidencyState::Evicting => 6,
            ResidencyState::Failed => 7,
        }
    }

    /// Whether `self -> to` is a legal edge.
    pub fn can_transition(self, to: Self) -> bool {
        use ResidencyState::*;
        match self {
            Unloaded => matches!(to, Requested),
            Requested => matches!(to, Generating | Failed | Unloaded),
            Generating => matches!(to, CpuReady | Failed | Unloaded),
            CpuReady => matches!(to, Uploading | Evicting | Failed),
            Uploading => matches!(to, GpuResident | Failed | CpuReady),
            GpuResident => matches!(to, Evicting),
            Evicting => matches!(to, Unloaded),
            Failed => matches!(to, Requested | Unloaded),
        }
    }

    /// Apply a transition, leaving `self` unchanged on error.
    pub fn transition(&mut self, to: Self) -> Result<(), AssetError> {
        if self.can_transition(to) {
            *self = to;
            Ok(())
        } else {
            Err(AssetError::IllegalTransition { from: *self, to })
        }
    }

    /// True for Requested, Generating and Uploading.
    pub fn is_in_flight(self) -> bool {
        matches!(
            self,
            ResidencyState::Requested | ResidencyState::Generating | ResidencyState::Uploading
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ResidencyState::{self, *};
    use crate::error::AssetError;

    #[test]
    fn residency_state_matrix_is_exact() {
        // Rows: from (ALL order); columns: to (ALL order).
        const T: bool = true;
        const F: bool = false;
        let expected: [[bool; 8]; 8] = [
            //  Unl Req Gen Cpu Upl Gpu Evi Fai
            [F, T, F, F, F, F, F, F], // Unloaded
            [T, F, T, F, F, F, F, T], // Requested
            [T, F, F, T, F, F, F, T], // Generating
            [F, F, F, F, T, F, T, T], // CpuReady
            [F, F, F, T, F, T, F, T], // Uploading
            [F, F, F, F, F, F, T, F], // GpuResident
            [T, F, F, F, F, F, F, F], // Evicting
            [T, T, F, F, F, F, F, F], // Failed
        ];
        let mut legal = 0;
        for a in ResidencyState::ALL {
            for b in ResidencyState::ALL {
                assert_eq!(
                    a.can_transition(b),
                    expected[a.index()][b.index()],
                    "{a:?}->{b:?}"
                );
                if a.can_transition(b) {
                    legal += 1;
                }
            }
        }
        assert_eq!(legal, 17);
    }

    #[test]
    fn residency_state_self_transitions_illegal() {
        for s in ResidencyState::ALL {
            assert!(!s.can_transition(s));
        }
    }

    #[test]
    fn residency_state_transition_ok_and_err() {
        let mut s = Unloaded;
        assert!(s.transition(Requested).is_ok());
        assert_eq!(s, Requested);
        assert_eq!(
            s.transition(GpuResident),
            Err(AssetError::IllegalTransition {
                from: Requested,
                to: GpuResident
            })
        );
        assert_eq!(s, Requested);
    }

    #[test]
    fn residency_state_in_flight() {
        for s in ResidencyState::ALL {
            assert_eq!(
                s.is_in_flight(),
                matches!(s, Requested | Generating | Uploading)
            );
        }
    }
}
