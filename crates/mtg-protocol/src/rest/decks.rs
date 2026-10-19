//! Bodies of the deck endpoints of `mtg-server`: stored decks with their derived marker, the deck
//! check, import of pasted lists and the error codes of the card and deck endpoints.

use mtg_cards::deck::{DeckReport, DeckState, DeckZone, StoredCheck, Summary};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::cards::CardHit;

/// Most decks one account stores.
pub const MAX_DECKS_PER_USER: u32 = 100;
/// Most lines (a card in a zone) in one deck.
pub const MAX_DECK_LINES: usize = 250;
/// Most cards, counting copies, in one deck.
pub const MAX_DECK_CARDS: u32 = 250;
/// Most characters in a deck's name.
pub const MAX_DECK_NAME_CHARS: usize = 100;
/// Most characters in a deck's notes.
pub const MAX_DECK_NOTES_CHARS: usize = 10_000;
/// Most characters in a pasted list for import.
pub const MAX_IMPORT_CHARS: usize = 100_000;
/// Most lines in a pasted list for import.
pub const MAX_IMPORT_LINES: usize = 1_000;

/// One line of a deck as a client sends it: copies of a card in a zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeckLineBody {
    /// The zone.
    pub zone: DeckZone,
    /// The card's oracle id.
    pub oracle_id: String,
    /// The printing whose art the line shows; it must be a printing of the card. Null or absent for the default.
    #[serde(default)]
    pub printing_id: Option<String>,
    /// How many copies, 1 to 999 (at most `MAX_DECK_CARDS` in the deck).
    pub count: u32,
}

/// The body of `POST /api/decks` and `PUT /api/decks/{id}`: the whole deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DeckBody {
    /// The name, 1 to `MAX_DECK_NAME_CHARS` characters without control characters.
    pub name: String,
    /// A format key: `commander`, `freeform` or a constructed format such as `modern`.
    pub format: String,
    /// Notes of the owner, up to `MAX_DECK_NOTES_CHARS` characters.
    #[serde(default)]
    pub notes: String,
    /// The lines in the order the owner keeps them.
    pub cards: Vec<DeckLineBody>,
}

/// One line of a deck as the server shows it: the card as a list shows it, with the chosen art.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct DeckLineView {
    /// The zone.
    pub zone: DeckZone,
    /// How many copies.
    pub count: u32,
    /// The chosen printing; null for the default one.
    pub printing_id: Option<String>,
    /// The card; its pictures are those of the chosen printing, else of the default one.
    pub card: CardHit,
}

/// A stored deck with its derived state, as `GET`, `POST` and `PUT /api/decks` answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct DeckView {
    /// The deck's id.
    pub id: String,
    /// The name.
    pub name: String,
    /// The format key.
    pub format: String,
    /// The notes.
    pub notes: String,
    /// When the deck was created, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
    /// When it was last saved, in the same form.
    pub updated_at: String,
    /// The lines in the owner's order.
    pub cards: Vec<DeckLineView>,
    /// `verified`, `manual_play`, `blocked`, or `unchecked` when no check is stored or the deck or its
    /// cards' rules changed since.
    pub state: DeckState,
    /// True exactly when `state` is `verified`: the stored check is clean, its hashes equal the ones
    /// computed now, and the check of now is clean too.
    pub verified: bool,
    /// A check is stored but the deck, the rules of its cards or what the engine runs changed since it.
    pub changed: bool,
    /// The check stored when the deck was last saved.
    pub check: Option<StoredCheck>,
    /// The deck checked now: legality, and the support of every card by zone.
    pub report: DeckReport,
}

/// A deck in the list of `GET /api/decks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DeckListItem {
    /// The deck's id.
    pub id: String,
    /// The name.
    pub name: String,
    /// The format key.
    pub format: String,
    /// When the deck was created, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub created_at: String,
    /// When it was last saved, in the same form.
    pub updated_at: String,
    /// The derived state, as in [`DeckView`].
    pub state: DeckState,
    /// True exactly when `state` is `verified`.
    pub verified: bool,
    /// A check is stored but the deck changed since it.
    pub changed: bool,
    /// When the stored check ran.
    pub checked_at: Option<String>,
    /// The counts of the deck checked now.
    pub summary: Summary,
}

/// The answer of `GET /api/decks`: the account's decks, the most recently saved first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DeckList {
    /// The decks.
    pub decks: Vec<DeckListItem>,
}

/// The body of `POST /api/decks/check`: a stored deck by id, or an unsaved list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CheckRequest {
    /// The id of a stored deck of the signed-in account; then `format` and `cards` are absent.
    #[serde(default)]
    pub deck: Option<String>,
    /// The format to check an unsaved list in.
    #[serde(default)]
    pub format: Option<String>,
    /// The unsaved list.
    #[serde(default)]
    pub cards: Option<Vec<DeckLineBody>>,
}

/// The kind of list a pasted text is written as; absent in a request means detect it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ImportSource {
    /// `4 Lightning Bolt` lines with optional sections.
    Text,
    /// The Arena export.
    Arena,
    /// MTGO text: names, a `SIDEBOARD:` block, the commander last.
    Mtgo,
    /// The MTGO `.dek` XML.
    MtgoDek,
    /// Moxfield's text export.
    Moxfield,
    /// Archidekt's text export.
    Archidekt,
}

/// The body of `POST /api/decks/import`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    /// The pasted list, up to `MAX_IMPORT_CHARS` characters and `MAX_IMPORT_LINES` lines.
    pub text: String,
    /// How the list is written; detected when absent.
    #[serde(default)]
    pub source: Option<ImportSource>,
    /// The format the deck is for; `commander` makes Moxfield's first line the commander.
    #[serde(default)]
    pub format: Option<String>,
}

/// Why a line of a pasted list could not be tied to one card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedReason {
    /// No card has that name.
    UnknownCard,
    /// Several cards share that name.
    AmbiguousName,
}

/// A card a pasted name might have meant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CardSuggestion {
    /// The card's oracle id.
    pub oracle_id: String,
    /// The card's full name.
    pub name: String,
}

/// A line of a pasted list that no card was found for; it is reported, never guessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UnresolvedLine {
    /// The name as pasted.
    pub name: String,
    /// The copies asked for.
    pub count: u32,
    /// The zone the line sat in.
    pub zone: DeckZone,
    /// Why it stayed unresolved.
    pub reason: UnresolvedReason,
    /// The reason in plain words.
    pub message: String,
    /// Cards with a close or the same name, best first.
    pub suggestions: Vec<CardSuggestion>,
}

/// A line of a pasted list that could not be read as a card entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ListProblem {
    /// The line, counted from 1.
    pub line: u32,
    /// What is wrong with it.
    pub message: String,
}

/// A part of a pasted list that is no part of the deck, such as the maybeboard or Moxfield's stickers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct IgnoredCards {
    /// The part's name as written.
    pub name: String,
    /// How many cards it held.
    pub count: u64,
}

/// The answer of `POST /api/decks/import`: the deck the list resolves to, and what it could not
/// resolve. Nothing is saved; the resolved lines are the cards of a `DeckBody`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ImportResult {
    /// How the list was read, as given or as detected.
    pub source: ImportSource,
    /// The lines tied to one card each, in the order of the list.
    pub cards: Vec<DeckLineView>,
    /// The lines no card was found for, in the order of the list.
    pub unresolved: Vec<UnresolvedLine>,
    /// The lines that were no card entries.
    pub problems: Vec<ListProblem>,
    /// The parts of the list that are not deck cards.
    pub ignored: Vec<IgnoredCards>,
}

/// The `error` code of a failed card or deck request; the body is `{error, message}`.
///
/// `message` is for people and may change, the code is for programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DeckErrorCode {
    /// 400: the body or the query string is not what the endpoint takes.
    InvalidRequest,
    /// 400: the deck breaks a rule of the endpoint (name, notes, format, size, a printing).
    InvalidDeck,
    /// 400: some oracle ids are no deck cards of the database; the message names them.
    UnknownCards,
    /// 404: no such card, deck or route; a deck of another account looks the same.
    NotFound,
    /// 409: the account stores as many decks as allowed.
    DeckLimit,
    /// 422: the deck cannot be written in that format.
    ExportFailed,
    /// 401: no valid session cookie.
    Unauthenticated,
    /// 403: the request comes from another site.
    ForbiddenOrigin,
    /// 413: the body is too large.
    PayloadTooLarge,
    /// 415: the body is not `application/json`.
    UnsupportedMediaType,
    /// 503: the server runs without a database, so it has no decks.
    DecksUnavailable,
    /// 503: the server runs without a database, so it has no cards.
    CardsUnavailable,
    /// 503: the database or the server is not available right now.
    Unavailable,
    /// 500: the server failed; the details stay in its log.
    Internal,
}

impl DeckErrorCode {
    /// Every code, in declaration order.
    pub const ALL: [DeckErrorCode; 14] = [
        DeckErrorCode::InvalidRequest,
        DeckErrorCode::InvalidDeck,
        DeckErrorCode::UnknownCards,
        DeckErrorCode::NotFound,
        DeckErrorCode::DeckLimit,
        DeckErrorCode::ExportFailed,
        DeckErrorCode::Unauthenticated,
        DeckErrorCode::ForbiddenOrigin,
        DeckErrorCode::PayloadTooLarge,
        DeckErrorCode::UnsupportedMediaType,
        DeckErrorCode::DecksUnavailable,
        DeckErrorCode::CardsUnavailable,
        DeckErrorCode::Unavailable,
        DeckErrorCode::Internal,
    ];

    /// The text on the wire.
    pub const fn as_str(self) -> &'static str {
        match self {
            DeckErrorCode::InvalidRequest => "invalid_request",
            DeckErrorCode::InvalidDeck => "invalid_deck",
            DeckErrorCode::UnknownCards => "unknown_cards",
            DeckErrorCode::NotFound => "not_found",
            DeckErrorCode::DeckLimit => "deck_limit",
            DeckErrorCode::ExportFailed => "export_failed",
            DeckErrorCode::Unauthenticated => "unauthenticated",
            DeckErrorCode::ForbiddenOrigin => "forbidden_origin",
            DeckErrorCode::PayloadTooLarge => "payload_too_large",
            DeckErrorCode::UnsupportedMediaType => "unsupported_media_type",
            DeckErrorCode::DecksUnavailable => "decks_unavailable",
            DeckErrorCode::CardsUnavailable => "cards_unavailable",
            DeckErrorCode::Unavailable => "unavailable",
            DeckErrorCode::Internal => "internal",
        }
    }
}
