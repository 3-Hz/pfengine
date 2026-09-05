//! The rollback session `pf_app` runs every tick through.
//!
//! Local play is the all-local case: every handle is local and the socket
//! has no peers, so GGRS starts in the Running state and never rolls back.
//! Phase 3 adds a constructor that takes a transport and remote handles; the
//! app loop does not change.

use std::error::Error;
use std::fmt;

use ggrs::{GgrsError, Message, NonBlockingSocket, P2PSession, PlayerType, SessionBuilder};

use crate::GgrsConfig;

/// Index of a player in the session, `0..num_players`. Slot `n` in `pf_app`
/// drives handle `n`.
pub type PlayerHandle = usize;

/// A socket with no peers: drops every send, never receives. With no remote
/// endpoints GGRS skips synchronization and starts Running.
struct NullSocket;

impl NonBlockingSocket<usize> for NullSocket {
    fn send_to(&mut self, _msg: &Message, _addr: &usize) {}

    fn receive_all_messages(&mut self) -> Vec<(usize, Message)> {
        Vec::new()
    }
}

/// What one call to [`Session::advance`] did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Advanced {
    /// World ticks run by this call: 0 while waiting, more than 1 after a
    /// rollback.
    pub frames: u32,
    /// Whether GGRS rewound the world before re-simulating.
    pub rolled_back: bool,
}

/// Why a session could not be built or advanced.
#[derive(Debug)]
pub enum SessionError {
    /// A session needs at least one player.
    NoPlayers,
    /// GGRS refused: a bad handle, a missing local input, or a session
    /// misconfiguration. Every case is a contract bug in the caller.
    Ggrs(GgrsError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::NoPlayers => write!(f, "a session needs at least one player"),
            SessionError::Ggrs(e) => write!(f, "ggrs: {e}"),
        }
    }
}

impl Error for SessionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SessionError::NoPlayers => None,
            SessionError::Ggrs(e) => Some(e),
        }
    }
}

impl From<GgrsError> for SessionError {
    fn from(e: GgrsError) -> Self {
        SessionError::Ggrs(e)
    }
}

/// A GGRS rollback session and the request loop that drives a [`World`]
/// through it.
///
/// [`World`]: pf_core::World
pub struct Session {
    inner: P2PSession<GgrsConfig>,
    local_handles: Vec<PlayerHandle>,
}

impl Session {
    /// Local play: every handle in `0..num_players` is local. There is no
    /// player cap; [`MAX_NETPLAY_MACHINES`] applies to netplay only.
    ///
    /// GGRS defaults stay: input delay 0, prediction window 8, desync
    /// detection off. All three are netplay knobs for Phase 3.
    ///
    /// [`MAX_NETPLAY_MACHINES`]: crate::MAX_NETPLAY_MACHINES
    pub fn local(num_players: usize) -> Result<Session, SessionError> {
        if num_players == 0 {
            return Err(SessionError::NoPlayers);
        }
        let mut builder = SessionBuilder::<GgrsConfig>::new().with_num_players(num_players);
        for handle in 0..num_players {
            builder = builder.add_player(PlayerType::Local, handle)?;
        }
        let inner = builder.start_p2p_session(NullSocket)?;
        Ok(Session {
            inner,
            local_handles: (0..num_players).collect(),
        })
    }

    pub fn num_players(&self) -> usize {
        self.inner.num_players()
    }

    /// The handles this machine supplies input for.
    pub fn local_handles(&self) -> &[PlayerHandle] {
        &self.local_handles
    }

    /// Frames the session has advanced; equals `world.frame` between calls.
    pub fn frame_count(&self) -> u32 {
        // GGRS counts frames as i32 with -1 for "none"; the session starts
        // at 0 and only counts up.
        self.inner.current_frame().max(0) as u32
    }

    /// The newest frame whose inputs are final. `None` before the first one.
    pub fn confirmed_frame(&self) -> Option<u32> {
        let frame = self.inner.confirmed_frame();
        (frame >= 0).then_some(frame as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_NETPLAY_MACHINES;

    #[test]
    fn local_session_marks_every_handle_local() {
        let session = Session::local(3).unwrap();
        assert_eq!(session.num_players(), 3);
        assert_eq!(session.local_handles(), &[0, 1, 2]);
        assert_eq!(session.frame_count(), 0);
        assert_eq!(session.confirmed_frame(), None);
    }

    #[test]
    fn local_session_rejects_zero_players() {
        assert!(matches!(Session::local(0), Err(SessionError::NoPlayers)));
    }

    #[test]
    fn local_session_has_no_player_cap() {
        let n = MAX_NETPLAY_MACHINES * 2;
        assert_eq!(Session::local(n).unwrap().num_players(), n);
    }
}
