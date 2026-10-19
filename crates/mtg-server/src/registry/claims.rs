//! Open seats: a visitor claims one and is given its token, a player gives one back, the host
//! removes a player, changes who may find the game or closes it. All of it only while the game is
//! in its lobby.
//!
//! A seat is claimed in three steps: the row in the database (which decides between racing
//! claims), the actor (which names the seat in the lobby and tells the table) and the registry's
//! own slot. Giving a seat back runs the actor first, because a lobby with an open seat cannot
//! start, so once the actor has emptied the seat nothing can begin the game under the change. A
//! game's changes run one at a time (`Entry::turn`), whichever of these or `bind_seat` makes them.

use std::sync::Arc;

use mtg_core::ids::PlayerId;
use thiserror::Error;

use super::{Entry, Registry, Removal, SeatSlot};
use crate::api::MAX_NAME_CHARS;
use crate::game_id::GameId;
use crate::handle::GameHandle;
use crate::records::{Claimed, GameRecords, RecordError, SeatState, Visibility};
use crate::secret::{RandomError, SeatToken};
use crate::user_id::UserId;

/// The seat that belongs to the creator of a game: it can remove players and change who may find
/// the game, and is never a seat a join takes.
pub const HOST: PlayerId = PlayerId(0);

/// Who asks for a seat and what they want to be called there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatClaim {
    /// The name the table sees, 1 to [`MAX_NAME_CHARS`] characters once trimmed. The caller has
    /// checked it for control characters, as for the names of a new game.
    pub name: String,
    /// The signed-in visitor's account, if any: the seat is bound to it, and an account holds one
    /// seat per game.
    pub user: Option<UserId>,
}

/// How a visitor reached the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entrance {
    /// By the game's id, from the game browser: only a public game lets anybody in this way.
    Listing,
    /// With the game's join code, which the caller has resolved with [`Registry::find_by_code`]:
    /// any game, public or not.
    Code,
}

/// A seat a visitor holds after a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seated {
    /// The game.
    pub game: GameId,
    /// The seat.
    pub seat: PlayerId,
    /// The seat's token, shown here and never again. `None` when the account already held the
    /// seat and is told so: it opens the seat through its session, and no second token is made.
    pub token: Option<SeatToken>,
}

/// Why a seat could not be claimed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClaimError {
    /// No such game, or a game this entrance does not show: an invite game reached by its id.
    #[error("no such game")]
    NotFound,
    /// Every seat is held.
    #[error("every seat is taken")]
    Full,
    /// The game has begun.
    #[error("the game has started")]
    Started,
    /// The name is empty or longer than [`MAX_NAME_CHARS`] characters.
    #[error("the name must be 1 to {MAX_NAME_CHARS} characters")]
    BadName,
    /// The claim could not be saved.
    #[error(transparent)]
    Record(#[from] RecordError),
    /// The operating system could not supply random bytes for the token.
    #[error(transparent)]
    Random(#[from] RandomError),
}

/// Why a change to the table was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TableError {
    /// No such game.
    #[error("no such game")]
    NotFound,
    /// Only the seat's holder and the host may do this.
    #[error("not allowed")]
    NotAllowed,
    /// The seat is not one a join claimed: it is open, or its token was made with the game.
    #[error("the seat was not claimed by a join")]
    NotClaimed,
    /// The game has begun.
    #[error("the game has started")]
    Started,
    /// The change could not be saved.
    #[error(transparent)]
    Record(#[from] RecordError),
}

/// What a change to one game's table works with, taken from its entry so no lock is held across
/// an await.
struct Table {
    handle: GameHandle,
    turn: Arc<tokio::sync::Mutex<()>>,
    /// The records to write to: present when the game is in the database.
    records: Option<GameRecords>,
}

impl Registry {
    /// A visitor takes the first open seat of a game in its lobby, under `claim.name`, and is
    /// given its token; the table is told. A signed-in visitor who already holds a seat of the
    /// game gets that seat back, without a token.
    ///
    /// `Listing` finds public games only; for any other game it answers [`ClaimError::NotFound`],
    /// as for a game that does not exist. A started game answers [`ClaimError::Started`] before
    /// any seat is looked for (even for an account that holds one), a full lobby
    /// [`ClaimError::Full`].
    pub async fn claim(
        &self,
        game: GameId,
        entrance: Entrance,
        claim: SeatClaim,
    ) -> Result<Seated, ClaimError> {
        let name = claim.name.trim().to_owned();
        if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
            return Err(ClaimError::BadName);
        }
        let table = self
            .table(game, |entry| {
                entrance == Entrance::Code || entry.visibility == Visibility::Public
            })
            .ok_or(ClaimError::NotFound)?;
        let _turn = table.turn.lock().await;
        let status = table.handle.status().await;
        if status.map_err(|_| ClaimError::NotFound)?.started {
            return Err(ClaimError::Started);
        }

        let seat = {
            let games = self.games();
            let entry = games.get(&game).ok_or(ClaimError::NotFound)?;
            let own = claim
                .user
                .and_then(|user| entry.seats.iter().position(|slot| slot.user == Some(user)));
            if let Some(own) = own {
                return Ok(Seated {
                    game,
                    seat: PlayerId::from_index(own),
                    token: None,
                });
            }
            let open = entry.seats.iter().position(|s| s.state == SeatState::Open);
            PlayerId::from_index(open.ok_or(ClaimError::Full)?)
        };
        let token = SeatToken::generate()?;
        let digest = token.digest();

        if let Some(records) = &table.records {
            match records
                .claim_seat(game, seat, &name, digest.as_bytes(), claim.user)
                .await?
            {
                Claimed::Done => {}
                Claimed::NotOpen => return Err(ClaimError::Full),
                Claimed::Holds(own) => {
                    return Ok(Seated {
                        game,
                        seat: PlayerId::from_index(own),
                        token: None,
                    });
                }
            }
        }
        let refusal = match table.handle.occupy(seat, name.clone()).await {
            Ok(true) => None,
            Ok(false) => Some(ClaimError::Started),
            Err(_) => Some(ClaimError::NotFound),
        };
        if let Some(refusal) = refusal {
            self.undo_claim(&table, game, seat).await;
            return Err(refusal);
        }
        let mut games = self.games();
        let slot = games
            .get_mut(&game)
            .and_then(|entry| entry.seats.get_mut(seat.index()))
            .ok_or(ClaimError::NotFound)?;
        *slot = SeatSlot {
            state: SeatState::Claimed,
            name,
            digest: Some(digest),
            user: claim.user,
        };
        tracing::info!(%game, seat = seat.get(), bound = claim.user.is_some(), "seat claimed");
        Ok(Seated {
            game,
            seat,
            token: Some(token),
        })
    }

    /// Opens a claimed seat again before the game starts: its token stops working, its connection
    /// closes, its `Ready` and account are gone and the table is told. `by` is the seat that asks
    /// (the caller has authenticated it): a player may give up their own seat, and the host
    /// ([`HOST`]) may remove anybody. Only seats a join claimed can be given back.
    pub async fn release(
        &self,
        game: GameId,
        seat: PlayerId,
        by: PlayerId,
    ) -> Result<(), TableError> {
        let table = self.table(game, |_| true).ok_or(TableError::NotFound)?;
        if by != seat && by != HOST {
            return Err(TableError::NotAllowed);
        }
        let _turn = table.turn.lock().await;
        let name = {
            let games = self.games();
            let entry = games.get(&game).ok_or(TableError::NotFound)?;
            match entry.seats.get(seat.index()) {
                Some(slot) if slot.state == SeatState::Claimed => slot.name.clone(),
                _ => return Err(TableError::NotClaimed),
            }
        };
        match table.handle.vacate(seat).await {
            Ok(true) => {}
            Ok(false) => return Err(TableError::Started),
            Err(_) => return Err(TableError::NotFound),
        }
        if let Some(records) = &table.records {
            let freed = records.release_seat(game, seat).await;
            if !matches!(freed, Ok(true)) {
                // The record still shows the seat claimed, so the table goes back to match it.
                let _ = table.handle.occupy(seat, name).await;
                return Err(freed
                    .err()
                    .map_or(TableError::NotClaimed, TableError::Record));
            }
        }
        if let Some(slot) = self
            .games()
            .get_mut(&game)
            .and_then(|entry| entry.seats.get_mut(seat.index()))
        {
            *slot = SeatSlot::open();
        }
        tracing::info!(%game, seat = seat.get(), by = by.get(), "seat released");
        Ok(())
    }

    /// Lists the game, or hides it again, before it starts. Only the host ([`HOST`], as `by`) may.
    pub async fn set_visibility(
        &self,
        game: GameId,
        by: PlayerId,
        visibility: Visibility,
    ) -> Result<(), TableError> {
        let table = self.table(game, |_| true).ok_or(TableError::NotFound)?;
        if by != HOST {
            return Err(TableError::NotAllowed);
        }
        let _turn = table.turn.lock().await;
        let status = table.handle.status().await;
        if status.map_err(|_| TableError::NotFound)?.started {
            return Err(TableError::Started);
        }
        if let Some(records) = &table.records
            && !records.set_visibility(game, visibility).await?
        {
            return Err(TableError::NotFound);
        }
        match self.games().get_mut(&game) {
            Some(entry) => entry.visibility = visibility,
            None => return Err(TableError::NotFound),
        }
        Ok(())
    }

    /// Closes a game that is still in its lobby, for good. It leaves the registry, so its listing,
    /// links and code reach nothing at once; its record is marked `expired`; every socket of the
    /// game is closed with 4002. Only the host ([`HOST`], as `by`) may, and once the game runs the
    /// answer is [`TableError::Started`].
    ///
    /// The actor decides whether the game began: from its answer on no `Ready` starts it. The
    /// record is written next, and a write that fails takes the answer back, so the lobby goes on
    /// as it was.
    pub async fn close(&self, game: GameId, by: PlayerId) -> Result<(), TableError> {
        let table = self.table(game, |_| true).ok_or(TableError::NotFound)?;
        if by != HOST {
            return Err(TableError::NotAllowed);
        }
        let _turn = table.turn.lock().await;
        // A close that held the turn before this one, or the janitor, may have taken the game.
        if !self.games().contains_key(&game) {
            return Err(TableError::NotFound);
        }
        match table.handle.close_lobby().await {
            Ok(true) => {}
            Ok(false) => return Err(TableError::Started),
            Err(_) => return Err(TableError::NotFound),
        }
        if let Some(records) = &table.records
            && let Err(error) = records.mark_expired(game).await
        {
            // The record still shows the lobby, so the lobby goes on.
            let _ = table.handle.reopen_lobby().await;
            return Err(TableError::Record(error));
        }
        let entry = self.games().remove(&game).ok_or(TableError::NotFound)?;
        // Said before the entry is dropped, which is what closes the sockets.
        entry.alive.send_replace(Removal::ClosedByHost);
        tracing::info!(%game, "lobby closed by the host");
        Ok(())
    }

    /// The parts of a game's entry a change to its table needs, if the game is alive and `shown`
    /// accepts it.
    fn table(&self, game: GameId, shown: impl Fn(&Entry) -> bool) -> Option<Table> {
        let games = self.games();
        let entry = games.get(&game).filter(|entry| shown(entry))?;
        let records = self.shared.records.clone().filter(|_| entry.recorded);
        Some(Table {
            handle: entry.handle.clone(),
            turn: Arc::clone(&entry.turn),
            records,
        })
    }

    /// Gives a seat back in the database after the actor refused it; the claim failed either way.
    async fn undo_claim(&self, table: &Table, game: GameId, seat: PlayerId) {
        if let Some(records) = &table.records
            && let Err(error) = records.release_seat(game, seat).await
        {
            tracing::warn!(%game, seat = seat.get(), %error, "could not take back a claim");
        }
    }
}
