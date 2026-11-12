//! Offering (CR 702.48): "You may cast this spell any time you could cast an instant by sacrificing a Fox and paying the
//! difference in mana costs between this and the sacrificed Fox."
//!
//! The way is chosen with the cast (`ways.rs`), open from a hand at instant speed, whose part sacrifices one permanent of
//! the quality; the total cost (`announce/total.rs`) takes that permanent's mana cost off, colors included
//! (`r301_offering.rs`).

use super::KeywordRules;

/// The rules of offering.
#[derive(Debug)]
pub(super) struct Offering;

impl KeywordRules for Offering {}
