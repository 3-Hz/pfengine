//! Rollback netcode wiring (GGRS) and the determinism gate.
//!
//! [`Session`] is what `pf_app` runs every tick through; local play is the
//! all-local case of it. [`run_synctest`] is the CI gate: SyncTest
//! re-simulates past frames every tick and compares checksums, so it fails
//! fast the moment the simulation stops being bit-deterministic. Real P2P
//! transport (matchbox / WebRTC) arrives in Phase 3 and is capped at
//! [`MAX_NETPLAY_MACHINES`] machines; fighters are not capped.

use std::error::Error;
use std::fmt;

use ggrs::{Config, GgrsError, GgrsRequest, SessionBuilder};
use pf_core::{buttons, Input, World};

pub mod session;

pub use session::{Advanced, PlayerHandle, Session, SessionError};

/// The most machines a netplay session accepts, counting this one. Fighters
/// are uncapped: a machine may own several handles (couch + online). Links
/// and prediction cost scale with machines; re-simulation cost scales with
/// fighters and stays cheap until Phase 5 adds real mechanics.
pub const MAX_NETPLAY_MACHINES: usize = 4;

/// A netplay session asked for more machines than [`MAX_NETPLAY_MACHINES`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TooManyMachines {
    pub requested: usize,
    pub max: usize,
}

impl fmt::Display for TooManyMachines {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "netplay supports at most {} machines, got {}",
            self.max, self.requested
        )
    }
}

impl Error for TooManyMachines {}

/// Gate for the Phase 3 P2P constructor. `num_machines` is the distinct peer
/// addresses plus this machine. Local sessions and SyncTest have no cap.
pub fn check_netplay_machines(num_machines: usize) -> Result<(), TooManyMachines> {
    if num_machines > MAX_NETPLAY_MACHINES {
        return Err(TooManyMachines {
            requested: num_machines,
            max: MAX_NETPLAY_MACHINES,
        });
    }
    Ok(())
}

/// The GGRS configuration: what we send (`Input`), what we snapshot (`World`),
/// and how peers are addressed (a handle index for now).
#[derive(Debug)]
pub struct GgrsConfig;

impl Config for GgrsConfig {
    type Input = Input;
    type State = World;
    type Address = usize;
}

/// Deterministic scripted input so the simulation actually moves during a
/// SyncTest — a motionless game can't reveal nondeterminism.
pub fn scripted_input(handle: usize, frame: u32) -> Input {
    let mut input = Input::default();
    let phase = frame.wrapping_add(handle as u32 * 17);
    input.stick_x = if (phase / 20).is_multiple_of(2) {
        110
    } else {
        -110
    };
    if phase.is_multiple_of(45) {
        input.buttons |= buttons::JUMP;
    }
    input
}

/// Run a GGRS SyncTest for `frames` frames against a local [`World`] of
/// `num_players` fighters.
///
/// Returns `Err` (a checksum mismatch) if the simulation is not bit-deterministic
/// under rollback — which is exactly what the CI test below asserts never happens.
pub fn run_synctest(num_players: usize, frames: u32) -> Result<(), GgrsError> {
    let mut session = SessionBuilder::<GgrsConfig>::new()
        .with_num_players(num_players)
        .with_check_distance(2)
        .start_synctest_session()?;

    let mut state = World::new(num_players);
    let mut frame_inputs = Vec::with_capacity(num_players);

    for frame in 0..frames {
        for handle in 0..num_players {
            session.add_local_input(handle, scripted_input(handle, frame))?;
        }

        for request in session.advance_frame()? {
            match request {
                GgrsRequest::SaveGameState { cell, frame } => {
                    cell.save(frame, Some(state.clone()), Some(state.checksum()));
                }
                GgrsRequest::LoadGameState { cell, .. } => {
                    state = cell.load().expect("synctest provided no state to load");
                }
                GgrsRequest::AdvanceFrame { inputs } => {
                    frame_inputs.clear();
                    frame_inputs.extend(inputs.iter().map(|(input, _)| *input));
                    state.advance(&frame_inputs);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The determinism guardrail. Keep this green at all costs.
    #[test]
    fn determinism_holds_under_rollback() {
        for n in [1, 2, 4, 8] {
            run_synctest(n, 300)
                .unwrap_or_else(|e| panic!("{n} players desynced under rollback: {e}"));
        }
    }

    #[test]
    fn netplay_allows_up_to_max_machines() {
        for n in 1..=MAX_NETPLAY_MACHINES {
            assert!(check_netplay_machines(n).is_ok(), "n = {n}");
        }
    }

    #[test]
    fn netplay_rejects_more_than_max_machines() {
        let requested = MAX_NETPLAY_MACHINES + 1;
        assert_eq!(
            check_netplay_machines(requested),
            Err(TooManyMachines {
                requested,
                max: MAX_NETPLAY_MACHINES
            })
        );
    }
}
