//! Harmonize (CR 702.180): "You may cast this card from your graveyard for its harmonize cost. You may tap a creature you
//! control to reduce that cost by an amount of generic mana equal to its power. Then exile this spell."
//!
//! The cost is an alternative cost chosen with the cast (`ways.rs`), open from its owner's graveyard, offered once as it
//! is and once with the tap of an untapped creature the caster picks; the total cost (`announce/total.rs`) takes the power
//! of that creature off its generic mana. An `{X}` in the cost is announced as for the printed cost, and the spell is
//! exiled when it leaves the stack (`CastWay::exiles_the_spell`).

use super::KeywordRules;

/// The rules of harmonize.
#[derive(Debug)]
pub(super) struct Harmonize;

impl KeywordRules for Harmonize {}
