//! Bodies of the read endpoints of `mtg-server`: `GET /api/reads?deck=` or `?precon=`, `GET /api/reads/{oracle_id}`
//! and `PUT /api/reads/{oracle_id}`. A player reads a card's draft beside its printed text and gives a verdict; the
//! errors use `DeckErrorCode`.

use mtg_cards::draft::ReadState;
use mtg_cards::read::ReadSheet;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a player says of a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReadVerdict {
    /// The draft does what the card says.
    Correct,
    /// The draft does not do what the card says; the note says what.
    Wrong,
}

/// The body of `PUT /api/reads/{oracle_id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReadBody {
    /// The `read_hash` of the draft the page showed; another than the current one is 409 `stale_read`.
    pub read_hash: String,
    /// The verdict.
    pub verdict: ReadVerdict,
    /// Why the draft is wrong: required for `wrong`, at most `MAX_FIX_NOTE_CHARS` characters without control
    /// characters other than line breaks; ignored for `correct`.
    #[serde(default)]
    pub note: Option<String>,
}

/// One card of a deck that its reader has left to read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ReadQueueCard {
    /// The card's oracle id.
    pub oracle_id: String,
    /// The card's name.
    pub name: String,
    /// The `read_hash` of its current draft.
    pub read_hash: String,
    /// Where the draft stands: `pending` waits for a first read, `approved` runs and takes more reads.
    pub state: ReadState,
    /// How many clauses read back as other text than the card (`ReportCard.differs`).
    pub differs: u32,
}

/// The answer of `GET /api/reads`: what the account has left to read in one deck, pending drafts first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ReadQueue {
    /// The deck's or the precon's name.
    pub title: String,
    /// The cards, in deck order within each state.
    pub cards: Vec<ReadQueueCard>,
}

/// The account's own verdict on a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MyRead {
    /// What it said.
    pub verdict: ReadVerdict,
    /// Why, for `wrong`.
    pub note: Option<String>,
    /// When, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
    /// Whether it is on the current draft; false means the draft changed since and waits for a new read.
    pub current: bool,
}

/// The answer of `GET` and `PUT /api/reads/{oracle_id}`: a card's current draft with its read sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ReadCard {
    /// The card's oracle id.
    pub oracle_id: String,
    /// The card's name.
    pub name: String,
    /// The hash a verdict is given on; send it back in `ReadBody.read_hash`.
    pub read_hash: String,
    /// Where the draft stands.
    pub state: ReadState,
    /// Accounts whose verdict on this hash is `correct`.
    pub correct: u32,
    /// Verdicts on this hash that are `wrong`, the server's included.
    pub wrong: u32,
    /// A trusted translation of a Forge script: approved without a read until a `wrong` counts.
    pub forge: bool,
    /// Every printed clause beside the engine's reading of it.
    pub sheet: ReadSheet,
    /// The account's verdict, if it gave one.
    pub mine: Option<MyRead>,
}

/// A card's current draft as the card endpoint shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CardDraft {
    /// The hash verdicts are given on.
    pub read_hash: String,
    /// Where the draft stands for the card's text today.
    pub state: ReadState,
}
