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
