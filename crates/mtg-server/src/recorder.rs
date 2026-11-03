//! What a persistent game's actor writes to its records as the game moves on.
//!
//! The records never decide anything: a failed write is logged and the game goes on, because the
//! event log is the truth and boot repairs a status that lags behind it.

use std::collections::BTreeMap;

use mtg_core::ids::PlayerId;
use mtg_db::{DraftKey, MAX_NOTE_CHARS, ReadRepo, Verdict};
use mtg_engine::state::GameResult;
use mtg_import::ids::OracleId;

use crate::game_id::GameId;
use crate::images::Warmer;
use crate::recipe::CardArt;
use crate::recipe::GameRecipe;
use crate::records::{GameRecords, RecordError};

/// The actor's hook into the `games` and `seats` tables.
#[derive(Debug)]
pub(crate) struct Recorder {
    records: GameRecords,
    game: GameId,
    /// Seats already noted as joined, so each is written once.
    joined: Vec<bool>,
    /// Whether the records showed the table as playing with manual cards when the recorder was made.
    manual_play: bool,
    /// Fetches the pictures of the cards a game starts with; none without a picture cache.
    warmer: Option<Warmer>,
    /// The cards of the decks the game was made with, for a game whose seats do not choose.
    art: Vec<CardArt>,
    /// Where the server's own verdicts go; none without a card database.
    verdicts: Option<ReadRepo>,
    /// The draft each card that runs on a read plays on in this game, by oracle id.
    reads: BTreeMap<String, String>,
}

impl Recorder {
    /// A recorder for `game`; `joined` says which seats the records already show as joined and
    /// `manual_play` whether they show the table as playing with manual cards.
    pub(crate) fn new(
        records: GameRecords,
        game: GameId,
        joined: Vec<bool>,
        manual_play: bool,
    ) -> Recorder {
        Recorder {
            records,
            game,
            joined,
            manual_play,
            warmer: None,
            art: Vec::new(),
            verdicts: None,
            reads: BTreeMap::new(),
        }
    }

    /// The same recorder that starts the fetching of a game's pictures with `warmer` when the game
    /// starts; `art` are the cards of the decks it was made with.
    #[must_use]
    pub(crate) fn with_warmer(mut self, warmer: Option<Warmer>, art: Vec<CardArt>) -> Recorder {
        self.warmer = warmer;
        self.art = art;
        self
    }

    /// The same recorder that writes the server's `wrong` verdict on the draft a card plays on (`reads`, by oracle id)
    /// when one of its abilities is left to the table.
    #[must_use]
    pub(crate) fn with_reads(
        mut self,
        verdicts: Option<ReadRepo>,
        reads: BTreeMap<String, String>,
    ) -> Recorder {
        self.verdicts = verdicts;
        self.reads = reads;
        self
    }

    /// Replaces the pinned drafts, for a game whose seats chose other decks than it was made with.
    pub(crate) fn set_reads(&mut self, reads: BTreeMap<String, String>) {
        self.reads = reads;
    }

    /// Starts fetching the pictures of the cards in the background: those of the decks the seats
    /// `chose`, else those of the decks the game was made with.
    pub(crate) fn warm_images(&self, chosen: Option<Vec<CardArt>>) {
        if let Some(warmer) = &self.warmer {
            warmer.warm(chosen.unwrap_or_else(|| self.art.clone()));
        }
    }

    /// Whether the records showed the table as playing with manual cards when the recorder was made.
    pub(crate) fn manual_play(&self) -> bool {
        self.manual_play
    }

    /// Records the server's own `wrong` verdict on the draft `oracle_id` plays on in this game, with the words of the
    /// ability the game could not run as the note, so the card is played by hand from the next game. A card that
    /// is pinned to no draft, or a failed write, is only logged.
    pub(crate) async fn left_to_table(&self, oracle_id: &str, text: &str) {
        let (Some(repo), Some(hash)) = (&self.verdicts, self.reads.get(oracle_id)) else {
            return;
        };
        let note: String = text.trim().chars().take(MAX_NOTE_CHARS).collect();
        let note = if note.is_empty() {
            "an ability could not be run in a game"
        } else {
            note.as_str()
        };
        let draft = DraftKey {
            oracle_id: OracleId(oracle_id.to_owned()),
            read_hash: hash.clone(),
        };
        if let Err(error) = repo.append(None, &draft, Verdict::Wrong, Some(note)).await {
            tracing::warn!(game = %self.game, %error, "could not record a read that failed");
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

    /// Stores what the game starts with: the recipe when the seats chose other decks than it names,
    /// and whether the table plays with manual cards. Unlike the notes below, the game must not
    /// start without it: a stored log replays under the recipe and a resumed game keeps the flag, so
    /// the caller refuses the start when this fails.
    pub(crate) async fn start_with(
        &self,
        recipe: Option<&GameRecipe>,
        manual_play: bool,
    ) -> Result<(), RecordError> {
        self.records.set_start(self.game, recipe, manual_play).await
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
