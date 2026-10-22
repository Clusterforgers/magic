//! Partner with a text (CR 702.124: `Partner—Friends forever`, `Partner—Character select`, `Partner—Survivors`): a
//! deck-building rule. Two commanders that have the same text pair; `CardDefinition::partners_with` reads it when the
//! format checks a seat (`format/commander.rs`), so the keyword has no hook in play.

use super::KeywordRules;

/// The rules of a partner group.
#[derive(Debug)]
pub(super) struct PartnerGroup;

impl KeywordRules for PartnerGroup {}
