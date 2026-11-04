//! Drafts of the read tier: whole rules the checker could not prove, which run automatically once a player has read
//! them and found them correct (`docs/cards-dsl.md`, Read tier).

use serde::{Deserialize, Serialize};

use crate::card::Card;
use crate::proposal::partial::PartialProof;
use crate::support::{
    Caps, PrintedCard, Support, SupportRecord, partial_support, read_hash, read_support,
};

/// What the verdicts on one `read_hash` come to: `wrong` when a counting verdict is `wrong`, else `approved` when one
/// is `correct`, else `pending`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ReadState {
    /// No player has judged this draft yet.
    Pending,
    /// A player read it and found it correct, and none found it wrong.
    Approved,
    /// A player read it and found it wrong.
    Wrong,
}

/// A stored draft as the server hands it to a library: the rules of one card, the hash its verdicts are on and what
/// they come to.
#[derive(Debug, Clone, PartialEq)]
pub struct DraftRules {
    /// The oracle id the draft is stored under.
    pub oracle_id: String,
    /// The `read_hash` the verdicts were given on.
    pub read_hash: String,
    /// The rules as the JSON of a `Card`.
    pub rules: serde_json::Value,
    /// What the verdicts on `read_hash` come to.
    pub read: ReadState,
    /// A trusted translation of a Forge script (source `forge`, Oracle text matched): its record is `Forge`, not `Read`.
    pub forge: bool,
    /// For a draft with lines left by hand: the proof that every one of them is safe to leave (Partial translations).
    pub proof: Option<PartialProof>,
}

/// A card's current draft in a library: parsed and checked rules with their read state.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    /// The rules.
    pub card: Card,
    /// The `read_hash` the verdicts were given on.
    pub read_hash: String,
    /// What the verdicts on `read_hash` come to.
    pub read: ReadState,
    /// A trusted translation of a Forge script: its record is `Forge`, not `Read`.
    pub forge: bool,
    /// The proof behind a draft with lines left by hand; `None` for a whole draft.
    pub proof: Option<PartialProof>,
}

impl Draft {
    /// The read state that counts for `printed`: the stored one while its hash is the one this draft and the imported
    /// text give today, `pending` once either changed, since a verdict on another hash counts for nothing.
    pub fn state_for(&self, printed: &PrintedCard) -> ReadState {
        if self.read_hash == read_hash(printed, &self.card) {
            self.read
        } else {
            ReadState::Pending
        }
    }

    /// The record of the card when the draft is approved for `printed`, covers its whole text, and the engine
    /// runs every primitive it uses; `None` otherwise.
    pub fn record(&self, printed: &PrintedCard, caps: &Caps) -> Option<SupportRecord> {
        if self.state_for(printed) != ReadState::Approved {
            return None;
        }
        self.runs(printed, caps)
    }

    /// The record the draft gives the card when the engine can run it, whatever its read state: `Partial` for a proved
    /// partial draft, else `Read` or `Forge`.
    pub fn runs(&self, printed: &PrintedCard, caps: &Caps) -> Option<SupportRecord> {
        if self.proof.is_some() {
            return partial_support(&self.card, printed, caps);
        }
        let tier = if self.forge {
            Support::Forge
        } else {
            Support::Read
        };
        read_support(&self.card, printed, caps, tier)
    }
}
