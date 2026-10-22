//! Per-player state: life, status, mana pool and mulligan progress.

use std::collections::BTreeMap;

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use crate::mana::ManaPool;

/// Why a player lost the game.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum LossReason {
    /// Life total of 0 or less (CR 704.5a).
    ZeroLife,
    /// Attempted to draw from an empty library (CR 704.5b).
    EmptyLibrary,
    /// Ten or more poison counters (CR 704.5c).
    Poison,
    /// The player gave up (CR 104.3a).
    Conceded,
    /// A reason in words: a format's own condition, such as commander damage, or an opponent who won by an effect.
    Format(String),
    /// Another player reached the life total that wins the game, a house rule.
    OpponentReachedLife,
    /// Dealt 21 or more combat damage by one commander (CR 903.10a).
    CommanderDamage,
}

/// Whether a player is still in the game.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlayerStatus {
    /// Still playing.
    InGame,
    /// Eliminated.
    Lost(LossReason),
}

/// Progress of one player through the opening-hand procedure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum MulliganStatus {
    /// A new hand must be dealt.
    NeedsHand,
    /// Waiting for the keep-or-mulligan decision.
    Deciding,
    /// Chose to mulligan; waiting for the rest of the table to decide.
    Mulliganing,
    /// Kept; this many cards still go to the bottom.
    Bottoming(usize),
    /// Kept after a mulligan in a style that scries; the top card of the library is looked at before the hand is final.
    Scrying,
    /// Hand is final.
    Kept,
}

/// A player seat.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Player {
    pub(crate) id: PlayerId,
    pub(crate) life: i32,
    pub(crate) status: PlayerStatus,
    pub(crate) mana: ManaPool,
    pub(crate) failed_draws: u32,
    pub(crate) mulligans: u32,
    pub(crate) mulligan: MulliganStatus,
    pub(crate) keywords: Vec<Keyword>,
    pub(crate) counters: BTreeMap<CounterKind, u32>,
}

impl Player {
    pub(crate) fn new(
        id: PlayerId,
        life: i32,
        mulligan: MulliganStatus,
        keywords: Vec<Keyword>,
    ) -> Self {
        Player {
            id,
            life,
            status: PlayerStatus::InGame,
            mana: ManaPool::default(),
            failed_draws: 0,
            mulligans: 0,
            mulligan,
            keywords,
            counters: BTreeMap::new(),
        }
    }

    /// The seat id.
    pub const fn id(&self) -> PlayerId {
        self.id
    }

    /// Current life total.
    pub const fn life(&self) -> i32 {
        self.life
    }

    /// Whether the player is in the game or has lost.
    pub const fn status(&self) -> &PlayerStatus {
        &self.status
    }

    /// Whether the player is still in the game.
    pub const fn is_in_game(&self) -> bool {
        matches!(self.status, PlayerStatus::InGame)
    }

    /// Mana currently floating.
    pub const fn mana(&self) -> &ManaPool {
        &self.mana
    }

    /// Mulligans taken so far.
    pub const fn mulligans(&self) -> u32 {
        self.mulligans
    }

    /// Opening-hand progress.
    pub const fn mulligan_status(&self) -> MulliganStatus {
        self.mulligan
    }

    /// The counters on the player, by kind in a fixed order; a kind they have none of is not listed.
    pub fn counters(&self) -> impl Iterator<Item = (&CounterKind, u32)> {
        self.counters.iter().map(|(kind, count)| (kind, *count))
    }

    /// How many counters of `kind` are on the player.
    pub fn counters_of(&self, kind: &CounterKind) -> u32 {
        self.counters.get(kind).copied().unwrap_or(0)
    }

    /// The keywords the player's setup gives them (hexproof, CR 702.11d); `keywords::player_keywords` is the reader.
    pub fn keywords(&self) -> &[Keyword] {
        &self.keywords
    }
}
