//! Goal: small builders of read hashes, draft keys and drafts for the tests of the read tier.

use mtg_db::{DraftKey, NewDraft};
use mtg_import::ids::OracleId;
use serde_json::json;

/// A read hash made of the number.
pub fn hash(n: u32) -> String {
    format!("{n:032x}")
}

/// The key of the draft of `card` with hash number `n`.
pub fn key(card: &OracleId, n: u32) -> DraftKey {
    DraftKey {
        oracle_id: card.clone(),
        read_hash: hash(n),
    }
}

/// The draft of `card` with hash number `n`; its rules carry the number.
pub fn draft(card: &OracleId, n: u32) -> NewDraft {
    NewDraft {
        oracle_id: card.clone(),
        read_hash: hash(n),
        rules: json!({"name": "A card", "n": n}),
    }
}
