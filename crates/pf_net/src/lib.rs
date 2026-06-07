//! Rollback netcode wiring (GGRS) and the determinism gate.
//!
//! Phase 0 ships the GGRS [`Config`] and a [`run_synctest`] harness. SyncTest
//! re-simulates past frames every tick and compares checksums, so it fails fast
//! the moment the simulation stops being bit-deterministic. Real P2P transport
//! (matchbox / WebRTC) arrives in Phase 3.

use ggrs::{Config, GgrsError, GgrsRequest, SessionBuilder};
use pf_core::{buttons, Input, World};

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
    input.stick_x = if (phase / 20) % 2 == 0 { 110 } else { -110 };
    if phase % 45 == 0 {
        input.buttons |= buttons::JUMP;
    }
    input
}

/// Run a GGRS SyncTest for `frames` frames against a local [`World`].
///
/// Returns `Err` (a checksum mismatch) if the simulation is not bit-deterministic
/// under rollback — which is exactly what the CI test below asserts never happens.
pub fn run_synctest(frames: u32) -> Result<(), GgrsError> {
    const NUM_PLAYERS: usize = 2;

    let mut session = SessionBuilder::<GgrsConfig>::new()
        .with_num_players(NUM_PLAYERS)
        .with_check_distance(2)
        .start_synctest_session()?;

    let mut state = World::new();

    for frame in 0..frames {
        for handle in 0..NUM_PLAYERS {
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
                    state.advance([inputs[0].0, inputs[1].0]);
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
        run_synctest(300).expect("simulation desynced under rollback");
    }
}
