//! Daybound (CR 702.145): the day and night rules live in `day_night.rs`, which reads the keyword on the permanents.

use super::KeywordRules;

/// The rules of daybound.
#[derive(Debug)]
pub(super) struct Daybound;

impl KeywordRules for Daybound {}
