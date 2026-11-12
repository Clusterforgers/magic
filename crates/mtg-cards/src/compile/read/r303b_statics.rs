//! r303b rules of the game read and written whole: "All damage is dealt as though its source had wither.", "All
//! creatures can attack your opponents and planeswalkers your opponents control as though those creatures had haste."

use super::tokens::Reader;
use crate::modify::GameRule;
use crate::refs::{Pred, Ref, Selector};

/// The sentence of `GameRule::DamageAsWither`, without its period.
const R303B_WITHER: &str = "All damage is dealt as though its source had wither";

/// The sentence of `GameRule::AttackAsThoughHasteAt` for your opponents, without its period.
const R303B_HASTE: &str = "All creatures can attack your opponents and planeswalkers your opponents control as though \
                           those creatures had haste";

/// One of the rules this module reads, the whole sentence but its period.
pub(super) fn r303b_rule_read(r: &mut Reader<'_>) -> Option<GameRule> {
    r.words("all damage is dealt as though its source had wither")
        .then_some(GameRule::DamageAsWither)
        .or_else(|| {
            r.words(
                "all creatures can attack your opponents and planeswalkers your opponents control as though those \
                 creatures had haste",
            )
            .then_some(GameRule::AttackAsThoughHasteAt(Ref::Opponents))
        })
}

/// What `r303b_rule_read` reads, written back.
pub(super) fn r303b_rule_texts(rule: &GameRule) -> Vec<String> {
    match rule {
        GameRule::DamageAsWither => vec![R303B_WITHER.to_owned()],
        GameRule::AttackAsThoughHasteAt(Ref::Opponents) => vec![R303B_HASTE.to_owned()],
        _ => Vec::new(),
    }
}

/// "All Walls able to block ~ do so": the creatures of one subtype that must block the card.
pub(super) fn r303b_blocked_by_texts(blockers: &Selector) -> Vec<String> {
    match blockers.0.as_slice() {
        [Pred::Subtype(kind)] => vec![format!("All {}s able to block ~ do so", kind.0)],
        _ => Vec::new(),
    }
}
