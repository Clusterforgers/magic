//! An account that is deleted leaves its tables: a seat a join took is given back, the lobby the
//! account hosts is closed, and every other seat it held stays with its link and no account.

use mtg_core::ids::PlayerId;

use super::{HOST, Registry, TableError};
use crate::game_id::GameId;
use crate::records::SeatState;
use crate::user_id::UserId;

/// What happened to the seats of a deleted account, game by game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Departure {
    /// Seats a join took, given back in their lobby.
    pub released: usize,
    /// Lobbies the account hosted that were closed.
    pub closed: usize,
    /// Seats that stay (the game runs, or the seat came with the game's link) and lost their
    /// account; the connection of each was closed.
    pub unbound: usize,
    /// Seats whose change the database refused; the table still shows them as the account's.
    pub failed: usize,
}

impl Registry {
    /// Takes `user` out of every live game, as a leave. Call it once the account is gone from the
    /// database. A seat a join took in a lobby is released and the lobby the account hosts is
    /// closed; a seat of a game that runs, or one that came with the game's link, is kept without
    /// the account and its connection is closed (the game waits for the seat on its clock, as
    /// for any player who left). A seat the database cannot change is counted in
    /// [`Departure::failed`] and logged.
    pub async fn forget_user(&self, user: UserId) -> Departure {
        let held: Vec<(GameId, PlayerId, SeatState)> = self
            .games()
            .iter()
            .flat_map(|(id, entry)| {
                entry
                    .seats
                    .iter()
                    .enumerate()
                    .filter(move |(_, slot)| slot.user == Some(user))
                    .map(move |(seat, slot)| (*id, PlayerId::from_index(seat), slot.state))
            })
            .collect();

        let mut departure = Departure::default();
        for (game, seat, state) in held {
            let outcome = match state {
                SeatState::Claimed => self.release(game, seat, seat).await,
                SeatState::Issued if seat == HOST => self.close(game, HOST).await,
                SeatState::Issued | SeatState::Open => Err(TableError::NotClaimed),
            };
            match outcome {
                Ok(()) if state == SeatState::Claimed => departure.released += 1,
                Ok(()) => departure.closed += 1,
                // The game ended or was closed meanwhile; its seats are gone with it.
                Err(TableError::NotFound) => {}
                Err(TableError::Started | TableError::NotClaimed | TableError::NotAllowed) => {
                    self.unbind(game, seat, user).await;
                    departure.unbound += 1;
                }
                Err(error) => {
                    tracing::warn!(%game, seat = seat.get(), %error, "could not free a seat of a deleted account");
                    self.unbind(game, seat, user).await;
                    departure.failed += 1;
                }
            }
        }
        if departure != Departure::default() {
            tracing::info!(%user, ?departure, "seats of a deleted account freed");
        }
        departure
    }

    /// Forgets that `seat` of `game` is the account's and closes its connection. The database has
    /// dropped the link already (the account row is gone).
    async fn unbind(&self, game: GameId, seat: PlayerId, user: UserId) {
        let handle = {
            let mut games = self.games();
            let Some(entry) = games.get_mut(&game) else {
                return;
            };
            if let Some(slot) = entry.seats.get_mut(seat.index())
                && slot.user == Some(user)
            {
                slot.user = None;
            }
            entry.handle.clone()
        };
        handle.leave(seat).await;
    }
}
