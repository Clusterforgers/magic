//! The replay of a recorded game that ended, built from its recipe and log for the account that asks.

use mtg_core::ids::PlayerId;
use mtg_protocol::{Replay, spectator_viewer};

use super::Registry;
use super::resume::decode;
use crate::game_id::GameId;
use crate::records::RecordStatus;
use crate::store::EventStore;
use crate::user_id::UserId;

/// Why a replay was not given.
#[derive(Debug, PartialEq, Eq)]
pub enum ReplayRefusal {
    /// No recorded game of that id has ended (or the server keeps no records).
    NotFound,
    /// The game could not be rebuilt or read; the detail went to the log.
    Failed,
}

impl Registry {
    /// The replay of game `id` for `user`: the seat that account held sees its own hidden cards, anybody else the
    /// spectator's public view. Only a recorded game that ended has one.
    pub async fn replay(&self, id: GameId, user: Option<UserId>) -> Result<Replay, ReplayRefusal> {
        let records = self.shared.records.clone().ok_or(ReplayRefusal::NotFound)?;
        let failed = |detail: String| {
            tracing::error!(game = %id, %detail, "replay failed");
            ReplayRefusal::Failed
        };
        let record = records
            .get(id)
            .await
            .map_err(|error| failed(error.to_string()))?
            .filter(|record| record.status == RecordStatus::Finished)
            .ok_or(ReplayRefusal::NotFound)?;
        let recipe = decode(&record).map_err(|unloadable| failed(unloadable.detail))?;
        let catalog = self
            .catalog_of(&recipe)
            .await
            .map_err(|unloadable| failed(unloadable.detail))?;
        let setup = recipe
            .build_with(&catalog)
            .map_err(|error| failed(error.to_string()))?;
        let events = records
            .store()
            .load(id)
            .await
            .map_err(|error| failed(error.to_string()))?;
        let seat = user.and_then(|user| {
            record
                .seats
                .iter()
                .find(|seat| seat.user == Some(user))
                .map(|seat| PlayerId::from_index(seat.seat))
        });
        let start = mtg_engine::replay(&setup, &[]).map_err(|error| failed(error.to_string()))?;
        let viewer = seat.unwrap_or_else(|| spectator_viewer(&start));
        let steps = crate::replay::steps(&setup, &events, viewer)
            .map_err(|error| failed(error.to_string()))?;
        Ok(Replay {
            seat,
            names: record.seats.iter().map(|seat| seat.name.clone()).collect(),
            result: record.result,
            steps,
        })
    }
}

impl Registry {
    /// Each seat's life at the end of recorded game `id` and whether it ended as a Spirit; `None` until the game has
    /// ended, or on a server that keeps no records.
    pub(crate) async fn final_seats(
        &self,
        id: GameId,
    ) -> Result<Option<Vec<(i32, bool)>>, ReplayRefusal> {
        let Some(records) = self.shared.records.clone() else {
            return Ok(None);
        };
        let failed = |detail: String| {
            tracing::error!(game = %id, %detail, "the end of a game could not be read");
            ReplayRefusal::Failed
        };
        let Some(record) = records
            .get(id)
            .await
            .map_err(|error| failed(error.to_string()))?
            .filter(|record| record.status == RecordStatus::Finished)
        else {
            return Ok(None);
        };
        let recipe = decode(&record).map_err(|unloadable| failed(unloadable.detail))?;
        let catalog = self
            .catalog_of(&recipe)
            .await
            .map_err(|unloadable| failed(unloadable.detail))?;
        let setup = recipe
            .build_with(&catalog)
            .map_err(|error| failed(error.to_string()))?;
        let events = records
            .store()
            .load(id)
            .await
            .map_err(|error| failed(error.to_string()))?;
        let state =
            mtg_engine::replay(&setup, &events).map_err(|error| failed(error.to_string()))?;
        Ok(Some(
            state
                .players()
                .iter()
                .map(|player| (player.life(), player.is_spirit()))
                .collect(),
        ))
    }
}
