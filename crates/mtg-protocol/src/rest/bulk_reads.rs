//! Bodies of the bulk read endpoints of `mtg-server`: `GET /api/reads/bulk` lists the drafts whose clauses read back as
//! other text only in wording (`mtg_cards::wording`), and `PUT /api/reads/bulk` records one `correct` verdict per card
//! the player approved.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One equivalence a listed clause needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BulkRule {
    /// Its name (`self_name`, `duration_order`, ...).
    pub name: String,
    /// Its heading, in a few words.
    pub title: String,
    /// What a reader checks, in one sentence.
    pub summary: String,
}

/// One clause of a listed card: the printed words beside the engine's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BulkClause {
    /// The clause as printed (the card's name as `~`).
    pub printed: String,
    /// The engine's words for it.
    pub engine: String,
    /// The names of the equivalences that make the two the same; never empty.
    pub rules: Vec<String>,
}

/// A card whose every differing clause matches under the equivalences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BulkCard {
    /// The card's oracle id.
    pub oracle_id: String,
    /// The card's name.
    pub name: String,
    /// The `read_hash` of its current draft, which an approval names.
    pub read_hash: String,
    /// The clause it groups under: the first rule of its first clause.
    pub rule: String,
    /// Every clause that reads back as other text.
    pub clauses: Vec<BulkClause>,
}

/// The answer of `GET /api/reads/bulk`: the cards the account may approve in bulk, by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BulkReads {
    /// The equivalences any listed card needs, in their fixed order.
    pub rules: Vec<BulkRule>,
    /// The cards, pending drafts the account has not judged.
    pub cards: Vec<BulkCard>,
}

/// One card of an approval: the draft the page showed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BulkApproval {
    /// The card's oracle id.
    pub oracle_id: String,
    /// The `read_hash` the page showed.
    pub read_hash: String,
}

/// The body of `PUT /api/reads/bulk`: the cards the player approved, at most `MAX_BULK_APPROVALS`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BulkApprovalBody {
    /// The approved cards.
    pub cards: Vec<BulkApproval>,
}

/// The most cards one bulk approval may name.
pub const MAX_BULK_APPROVALS: usize = 2000;

/// The answer of `PUT /api/reads/bulk`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BulkApprovalResult {
    /// The oracle ids that got a `correct` verdict.
    pub approved: Vec<String>,
    /// The oracle ids left alone: the draft changed since the page loaded it, or the card is no longer a candidate.
    pub stale: Vec<String>,
}
