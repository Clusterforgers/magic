//! Doctor's companion (CR 702.124): a deck-building rule. A commander with it pairs with a legendary Time Lord Doctor;
//! `CardDefinition::partners_with` reads it when the format checks a seat (`format/commander.rs`), so the keyword has
//! no hook in play.

use super::KeywordRules;

/// The rules of Doctor's companion.
#[derive(Debug)]
pub(super) struct DoctorsCompanion;

impl KeywordRules for DoctorsCompanion {}
