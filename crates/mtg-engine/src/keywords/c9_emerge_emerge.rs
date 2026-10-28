//! Emerge (CR 702.119): "You may cast this spell by sacrificing a creature and paying the emerge cost reduced by that
//! creature's mana value."
//!
//! The cost is an alternative cost chosen with the cast (`ways.rs`), open from a hand, whose parts sacrifice one creature
//! the caster picks; the total cost (`announce/total.rs`) takes the mana value of that creature off its generic mana. The
//! spell remembers the way for "if its emerge cost was paid" (`Condition::CastWith`) and the creature for "the sacrificed
//! creature" (`Ref::Paid`).

use super::KeywordRules;

/// The rules of emerge.
#[derive(Debug)]
pub(super) struct Emerge;

impl KeywordRules for Emerge {}
