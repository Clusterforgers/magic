//! Suspend (CR 702.62): the rules are a stub until the keyword task (`docs/cards-dsl.md`, Keywords).

use super::KeywordRules;

/// The rules of suspend.
#[derive(Debug)]
pub(super) struct Suspend;

impl KeywordRules for Suspend {}
