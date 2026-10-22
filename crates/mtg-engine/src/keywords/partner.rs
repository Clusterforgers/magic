//! Partner (CR 702.124): a deck-building rule. Two commanders that both have it pair; `CardDefinition::partners_with`
//! reads it when the format checks a seat (`format/commander.rs`), so the keyword has no hook in play.

use super::KeywordRules;

/// The rules of partner.
#[derive(Debug)]
pub(super) struct Partner;

impl KeywordRules for Partner {}
