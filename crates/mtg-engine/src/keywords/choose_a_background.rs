//! Choose a Background (CR 702.124): a deck-building rule. A commander with it pairs with a Background enchantment, which
//! then is a commander too; `CardDefinition::partners_with` and `can_be_commander_beside` read it when the format checks
//! a seat (`format/commander.rs`), so the keyword has no hook in play.

use super::KeywordRules;

/// The rules of Choose a Background.
#[derive(Debug)]
pub(super) struct ChooseABackground;

impl KeywordRules for ChooseABackground {}
