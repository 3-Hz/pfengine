//! Rollback netcode wiring (GGRS) and the determinism gate.
//!
//! Phase 0 ships the GGRS [`Config`] and a [`run_synctest`] harness. SyncTest
//! re-simulates past frames every tick and compares checksums, so it fails fast
//! the moment the simulation stops being bit-deterministic. Real P2P transport
//! (matchbox / WebRTC) arrives in Phase 3 and is capped at
//! [`MAX_NETPLAY_PLAYERS`]; local play is not.

use std::error::Error;
use std::fmt;

use ggrs::{Config, GgrsError, GgrsRequest, SessionBuilder};
use pf_core::{buttons, Input, World};

/// The most fighters a netplay session accepts, counted across all machines;
/// with couch + online one machine may own several. Every peer rolls back to
/// the laggiest one and each rollback re-simulates every fighter, so the cap
/// bounds both. Links run between machines, not fighters.
pub const MAX_NETPLAY_PLAYERS: usize = 4;

/// A netplay session asked for more players than [`MAX_NETPLAY_PLAYERS`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TooManyPlayers {
    pub requested: usize,
    pub max: usize,
}

impl fmt::Display for TooManyPlayers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "netplay supports at most {} players, got {}",
            self.max, self.requested
        )
    }
}

impl Error for TooManyPlayers {}

/// Gate for the P2P session builder (Phase 3). SyncTest is local and skips it.
pub fn check_netplay_players(num_players: usize) -> Result<(), TooManyPlayers> {
    if num_players > MAX_NETPLAY_PLAYERS {
        return Err(TooManyPlayers {
            requested: num_players,
            max: MAX_NETPLAY_PLAYERS,
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
    fn netplay_allows_up_to_max_players() {
        for n in 1..=MAX_NETPLAY_PLAYERS {
            assert!(check_netplay_players(n).is_ok(), "n = {n}");
        }
    }

    #[test]
    fn netplay_rejects_more_than_max_players() {
        let requested = MAX_NETPLAY_PLAYERS + 1;
        assert_eq!(
            check_netplay_players(requested),
            Err(TooManyPlayers {
                requested,
                max: MAX_NETPLAY_PLAYERS
            })
        );
    }
}
