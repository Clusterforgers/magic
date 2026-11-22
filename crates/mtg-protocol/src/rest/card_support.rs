//! Bodies of the card support endpoints of `mtg-server`: `GET /api/card-support` (the newest coverage
//! snapshot, the trend over earlier ones, the newest refresh run and the feed of events) and
//! `GET /api/card-support/missing` (the cards that do not play automatically). The errors use
//! `DeckErrorCode`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How many snapshots the trend holds at most.
pub const MAX_SUPPORT_HISTORY: u32 = 120;

/// How many events the feed holds at most.
pub const MAX_FEED_EVENTS: usize = 40;

/// How many card names one feed event lists at most; its count stays whole.
pub const MAX_FEED_NAMES: usize = 30;

/// The answer of `GET /api/card-support`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CardSupport {
    /// The newest snapshot; none before the first one.
    pub snapshot: Option<SupportSnapshot>,
    /// The snapshots of the trend, the oldest first.
    pub history: Vec<SupportPoint>,
    /// The newest refresh run; none before the first one.
    pub last_check: Option<RefreshCheck>,
    /// What changed, the newest first.
    pub feed: Vec<FeedEvent>,
}

/// How many Commander-legal cards play automatically at one time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SupportSnapshot {
    /// The snapshot's id; a new id means the missing cards changed too.
    pub id: u32,
    /// When it was taken, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub taken_at: String,
    /// The release that took it (`v0.2.131`), when known.
    pub release: Option<String>,
    /// Commander-legal cards judged.
    pub cards: u32,
    /// Of them, the cards that play automatically: verified, read and unverified.
    pub automatic: u32,
    /// The cards per state.
    pub states: SupportStates,
    /// The cards among the most played, by rank.
    pub buckets: Vec<SupportBucket>,
    /// How many cards do not play automatically.
    pub missing: u32,
}

/// How many cards are in each support state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SupportStates {
    /// Verified by the checker.
    pub verified: u32,
    /// Rules a player read and approved, or a trusted Forge translation.
    pub read: u32,
    /// Whole rules that play while they wait for a player's read.
    pub unverified: u32,
    /// Partial rules waiting for a read; played by hand until then.
    pub waiting: u32,
    /// Rules except some clauses, which the table plays by hand.
    pub partial: u32,
    /// No rules the engine runs: played by hand.
    pub manual: u32,
    /// Stale or unplayable.
    pub other: u32,
}

/// The cards among the most played up to one rank.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SupportBucket {
    /// The bucket: `top 100`, `top 500`, ..., `all ranked` or `no rank`.
    pub label: String,
    /// Cards in it.
    pub cards: u32,
    /// Verified cards in it.
    pub verified: u32,
    /// Cards in it that play automatically.
    pub automatic: u32,
}

/// One point of the trend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SupportPoint {
    /// When the snapshot was taken, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub taken_at: String,
    /// The release that took it, when known.
    pub release: Option<String>,
    /// Commander-legal cards judged.
    pub cards: u32,
    /// Of them, the cards that play automatically.
    pub automatic: u32,
}

/// How a refresh run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RefreshState {
    /// Still running.
    Running,
    /// Checked, loaded what changed and counted again.
    Succeeded,
    /// Checked, but loading waits for a time with no game running.
    Deferred,
    /// A step failed.
    Failed,
}

/// The newest run of the daily check for new cards and precons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RefreshCheck {
    /// When it started, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub started_at: String,
    /// When it ended, in the same form; none while it runs.
    pub finished_at: Option<String>,
    /// How it ended.
    pub state: RefreshState,
    /// When Scryfall last updated its card data, as Scryfall writes it.
    pub scryfall_updated_at: Option<String>,
    /// The MTGJSON build of the precon list (`5.3.0+20261022`).
    pub mtgjson_build: Option<String>,
}

/// One event of the feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FeedEvent {
    /// The daily check found new cards.
    NewCards {
        /// When, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
        at: String,
        /// How many.
        count: u32,
        /// Some of their names.
        names: Vec<String>,
    },
    /// The daily check found new precons of one set.
    NewPrecons {
        /// When, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
        at: String,
        /// The set code.
        code: String,
        /// The set's name, when known.
        set_name: Option<String>,
        /// The decks' names.
        decks: Vec<String>,
    },
    /// More cards play automatically, or a new release counted them.
    NowAutomatic {
        /// When, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
        at: String,
        /// The release that counted them, when known.
        release: Option<String>,
        /// Whether that release is new since the snapshot before.
        new_release: bool,
        /// How many cards play automatically that did not before.
        count: u32,
        /// Some of their names.
        names: Vec<String>,
    },
    /// The daily check failed.
    CheckFailed {
        /// When, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
        at: String,
        /// What failed.
        failures: Vec<String>,
    },
}

/// Why a card does not play automatically, as a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MissingState {
    /// Partial rules wait for a player's read.
    Waiting,
    /// Some clauses are played by hand.
    Partial,
    /// The whole card is played by hand.
    Manual,
    /// Stale or unplayable rules.
    Other,
}

/// A card that does not play automatically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MissingCardView {
    /// The oracle id.
    pub oracle_id: String,
    /// The card's name.
    pub name: String,
    /// Its state.
    pub state: MissingState,
    /// What holds it back first, in plain words.
    pub blocker: String,
    /// The same blocker as the coverage report labels it.
    pub detail: String,
    /// How many blockers hold it back.
    pub blockers: u32,
    /// Its EDHREC rank (1 is the most played), when it has one.
    pub rank: Option<u32>,
}

/// The answer of `GET /api/card-support/missing`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MissingCards {
    /// The snapshot the list belongs to; none before the first one.
    pub snapshot_id: Option<u32>,
    /// The cards, the most played first.
    pub cards: Vec<MissingCardView>,
}
