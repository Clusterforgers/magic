//! Spectators: the watch links the players of a game hand out, and the host's switch that allows them.

use mtg_core::ids::PlayerId;

use super::{HOST, Registry, SeatAccess, TableError};
use crate::game_id::GameId;
use crate::secret::{SeatToken, TokenDigest, looks_like_token};

/// The watch links a game keeps; a new one past this many forgets the oldest.
pub const MAX_WATCH_LINKS: usize = 32;

/// Why no watch link was made.
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    /// No such game.
    #[error("no such game")]
    NotFound,
    /// The host does not allow spectators.
    #[error("the host does not allow spectators")]
    NotAllowed,
    /// The system's random source failed.
    #[error("no random token")]
    Random,
}

impl Registry {
    /// A new watch link to `game` for a player (`by`, any seat) to hand out: its token opens the spectator's view
    /// while the host allows spectators. The game keeps the last [`MAX_WATCH_LINKS`] links, in memory only.
    pub fn watch_link(&self, game: GameId, by: PlayerId) -> Result<SeatToken, WatchError> {
        let token = SeatToken::generate().map_err(|_| WatchError::Random)?;
        let mut games = self.games();
        let entry = games.get_mut(&game).ok_or(WatchError::NotFound)?;
        if by.index() >= entry.seats.len() {
            return Err(WatchError::NotFound);
        }
        if !*entry.spectators.borrow() {
            return Err(WatchError::NotAllowed);
        }
        if entry.watch_links.len() >= MAX_WATCH_LINKS {
            entry.watch_links.remove(0);
        }
        entry.watch_links.push(token.digest());
        Ok(token)
    }

    /// The host (`by` is [`HOST`]) allows spectators or stops them; stopping forgets every watch link and closes the
    /// spectators' sockets.
    pub fn set_spectators(
        &self,
        game: GameId,
        by: PlayerId,
        allowed: bool,
    ) -> Result<(), TableError> {
        let mut games = self.games();
        let entry = games.get_mut(&game).ok_or(TableError::NotFound)?;
        if by != HOST {
            return Err(TableError::NotAllowed);
        }
        if !allowed {
            entry.watch_links.clear();
        }
        entry.spectators.send_replace(allowed);
        Ok(())
    }

    /// Maps a watch link's token to the spectator's access; `None` for an unknown game, a wrong token and a game
    /// whose host does not allow spectators alike.
    pub fn authenticate_watcher(&self, game: GameId, token: &str) -> Option<SeatAccess> {
        if !looks_like_token(token) {
            return None;
        }
        let presented = TokenDigest::of(token);
        let games = self.games();
        let entry = games.get(&game)?;
        // Check every link so the time taken does not say which one matched.
        let found = entry
            .watch_links
            .iter()
            .fold(false, |found, digest| digest.matches(&presented) | found);
        if !found || !*entry.spectators.borrow() {
            return None;
        }
        Some(SeatAccess {
            game,
            handle: entry.handle.clone(),
            seat: PlayerId::from_index(entry.seats.len()),
            removed: entry.alive.subscribe(),
            spectators: Some(entry.spectators.subscribe()),
        })
    }
}
