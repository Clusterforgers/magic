//! What a persistent game's actor writes to its records as the game moves on.
//!
//! The records never decide anything: a failed write is logged and the game goes on, because the
//! event log is the truth and boot repairs a status that lags behind it.

use mtg_core::ids::PlayerId;
use mtg_engine::state::GameResult;

use crate::game_id::GameId;
use crate::recipe::GameRecipe;
use crate::records::{GameRecords, RecordError};

/// The actor's hook into the `games` and `seats` tables.
#[derive(Debug)]
pub(crate) struct Recorder {
    records: GameRecords,
    game: GameId,
    /// Seats already noted as joined, so each is written once.
    joined: Vec<bool>,
}

impl Recorder {
    /// A recorder for `game`; `joined` says which seats the records already show as joined.
    pub(crate) fn new(records: GameRecords, game: GameId, joined: Vec<bool>) -> Recorder {
        Recorder {
            records,
            game,
            joined,
        }
    }

    /// Notes the first connection to `seat`.
    pub(crate) async fn joined(&mut self, seat: PlayerId) {
        let Some(flag) = self.joined.get_mut(seat.index()) else {
            return;
        };
        if std::mem::replace(flag, true) {
            return;
        }
        if let Err(error) = self.records.mark_joined(self.game, seat).await {
            // Try again at the seat's next join.
            *flag = false;
            tracing::warn!(game = %self.game, %error, "could not record a seat joining");
        }
    }

    /// Forgets that `seat` ever joined, for a seat given up in the lobby that somebody else may take.
    pub(crate) fn forget(&mut self, seat: PlayerId) {
        if let Some(flag) = self.joined.get_mut(seat.index()) {
            *flag = false;
        }
    }

    /// Stores the recipe with the decks the seats chose. Unlike the notes below, the game must not
    /// start without it: a stored log replays under the recipe, so the caller refuses the start
    /// when this fails.
    pub(crate) async fn decks(&self, recipe: &GameRecipe) -> Result<(), RecordError> {
        self.records.set_recipe(self.game, recipe).await
    }

    /// Notes that the lobby is over.
    pub(crate) async fn started(&self) {
        if let Err(error) = self.records.mark_running(self.game).await {
            tracing::warn!(game = %self.game, %error, "could not record the game starting");
        }
    }

    /// Notes how the game ended.
    pub(crate) async fn finished(&self, result: GameResult) {
        if let Err(error) = self.records.mark_finished(self.game, result).await {
            tracing::warn!(game = %self.game, %error, "could not record the game ending");
        }
    }
}
