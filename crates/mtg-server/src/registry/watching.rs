//! Spectators: any signed-in user may watch any running game, holding no seat.

use mtg_core::ids::PlayerId;

use super::{Registry, SeatAccess};
use crate::game_id::GameId;

impl Registry {
    /// A spectator's access to `game`, for a caller that has already checked the visitor is signed in; `None` for
    /// an unknown game. Whether the game runs is the actor's to say when the spectator joins.
    pub fn authenticate_spectator(&self, game: GameId) -> Option<SeatAccess> {
        let games = self.games();
        let entry = games.get(&game)?;
        Some(SeatAccess {
            game,
            handle: entry.handle.clone(),
            seat: PlayerId::from_index(entry.seats.len()),
            removed: entry.alive.subscribe(),
            spectator: true,
        })
    }
}
