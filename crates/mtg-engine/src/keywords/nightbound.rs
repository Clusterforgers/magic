//! Nightbound (CR 702.145): the day and night rules live in `day_night.rs`, which reads the keyword on the permanents.

use super::KeywordRules;

/// The rules of nightbound.
#[derive(Debug)]
pub(super) struct Nightbound;

impl KeywordRules for Nightbound {}
