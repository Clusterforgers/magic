//! Phasing (CR 702.26a): the untap step reads the keyword (`phasing::w7e_keyword_out_events`); it adds no ability.

use super::KeywordRules;

/// The rules of phasing.
#[derive(Debug)]
pub(super) struct Phasing;

impl KeywordRules for Phasing {}
