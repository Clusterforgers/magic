//! The admin's view of the live games and ending any of them.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mtg_protocol::rest::{AdminGame, AdminGameSeat};

use super::listing::utc_stamp;
use super::{Registry, Removal};
use crate::game_id::GameId;
use crate::handle::GameHandle;
use crate::records::{RecordError, RecordStatus};

/// What [`Registry::end_by_admin`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminEnd {
    /// The game was in its lobby or running and is now ended.
    Ended,
    /// The game had already ended; nothing changed.
    AlreadyOver,
    /// There is no such game.
    NotFound,
}

impl Registry {
    /// Every lobby and running game, tutorials included, newest first; a game whose actor is gone or that has a
    /// result is left out.
    pub async fn admin_games(&self) -> Vec<AdminGame> {
        let (now, wall) = (Instant::now(), SystemTime::now());
        let stamp = |at: Instant| {
            let ago = now.saturating_duration_since(at);
            utc_stamp(wall.checked_sub(ago).unwrap_or(UNIX_EPOCH))
        };
        let mut candidates: Vec<(Instant, GameHandle, AdminGame)> = self
            .games()
            .iter()
            .map(|(id, entry)| {
                let row = AdminGame {
                    game: id.to_string(),
                    format: entry.format.as_str().to_owned(),
                    visibility: entry.visibility.as_str().to_owned(),
                    started: false,
                    tutorial: entry.unlisted,
                    seats: entry
                        .seats
                        .iter()
                        .map(|slot| AdminGameSeat {
                            name: slot.name.clone(),
                            connected: false,
                        })
                        .collect(),
                    created_at: stamp(entry.created),
                    last_activity: stamp(entry.last_connected.max(entry.created)),
                };
                (entry.created, entry.handle.clone(), row)
            })
            .collect();
        candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));
        let mut rows = Vec::with_capacity(candidates.len());
        for (_, handle, mut row) in candidates {
            let Ok(status) = handle.status().await else {
                continue;
            };
            if status.result.is_some() {
                continue;
            }
            row.started = status.started;
            for seat in &status.connected {
                if let Some(listed) = row.seats.get_mut(seat.index()) {
                    listed.connected = true;
                }
            }
            if !status.connected.is_empty() {
                row.last_activity = utc_stamp(wall);
            }
            rows.push(row);
        }
        rows
    }

    /// Ends a game in its lobby or running, for an admin. The actor tells every seat and spectator `reason` and
    /// stops (bots stop with it), the record becomes `aborted` with the reason, and the game leaves the registry, so
    /// its sockets close with 4003 and its links open nothing. A game that already ended is left alone, and a
    /// record that missed an earlier end is marked now, so a repeat is harmless.
    pub async fn end_by_admin(
        &self,
        game: GameId,
        reason: Option<String>,
    ) -> Result<AdminEnd, RecordError> {
        let live = self
            .games()
            .get(&game)
            .map(|entry| (entry.handle.clone(), std::sync::Arc::clone(&entry.turn)));
        let Some((handle, turn)) = live else {
            return self.end_record(game, reason.as_deref()).await;
        };
        let _turn = turn.lock().await;
        // A close, or the janitor, may have taken the game while this waited.
        if !self.games().contains_key(&game) {
            return self.end_record(game, reason.as_deref()).await;
        }
        // An actor that is gone already leaves only the record and the entry to end.
        let aborted = handle.abort(reason.clone()).await.unwrap_or(true);
        if !aborted {
            return Ok(AdminEnd::AlreadyOver);
        }
        let recorded = self.games().get(&game).is_some_and(|entry| entry.recorded);
        let written = match (&self.shared.records, recorded) {
            (Some(records), true) => records.mark_aborted(game, reason.as_deref()).await,
            _ => Ok(true),
        };
        if let Some(entry) = self.games().remove(&game) {
            // Said before the entry is dropped, which is what closes the sockets and stops the bots.
            entry.alive.send_replace(Removal::EndedByAdmin);
        }
        tracing::info!(%game, "game ended by an admin");
        // A failed write leaves the record running; asking again marks it, since the game is gone.
        written.map(|_| AdminEnd::Ended)
    }

    /// Ends the record of a game that is not in the registry.
    async fn end_record(
        &self,
        game: GameId,
        reason: Option<&str>,
    ) -> Result<AdminEnd, RecordError> {
        let Some(records) = &self.shared.records else {
            return Ok(AdminEnd::NotFound);
        };
        let Some(record) = records.get(game).await? else {
            return Ok(AdminEnd::NotFound);
        };
        match record.status {
            RecordStatus::Lobby | RecordStatus::Running => {
                records.mark_aborted(game, reason).await?;
                Ok(AdminEnd::Ended)
            }
            _ => Ok(AdminEnd::AlreadyOver),
        }
    }
}
