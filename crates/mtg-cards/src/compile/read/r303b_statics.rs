//! r303b rules of the game read and written whole: "All damage is dealt as though its source had wither.", "All
//! creatures can attack your opponents and planeswalkers your opponents control as though those creatures had haste.",
//! "Damage isn't removed from this creature during cleanup steps."

use super::tokens::Reader;
use crate::modify::GameRule;
use crate::refs::{Pred, Ref, Selector};

/// The sentence of `GameRule::DamageAsWither`, without its period.
const R303B_WITHER: &str = "All damage is dealt as though its source had wither";

/// The sentence of `GameRule::AttackAsThoughHasteAt` for your opponents, without its period.
const R303B_HASTE: &str = "All creatures can attack your opponents and planeswalkers your opponents control as though \
                           those creatures had haste";

/// The sentence of `GameRule::ManaBurn`, without its period.
const R303B_BURN: &str = "A player losing unspent mana causes that player to lose that much life";

/// One of the rules this module reads, the whole sentence but its period.
pub(super) fn r303b_rule_read(r: &mut Reader<'_>) -> Option<GameRule> {
    r.words("all damage is dealt as though its source had wither")
        .then_some(GameRule::DamageAsWither)
        .or_else(|| {
            r.words("a player losing unspent mana causes that player to lose that much life")
                .then_some(GameRule::ManaBurn)
        })
        .or_else(|| {
            (r.words("damage isn't removed from") && r.source() && r.words("during cleanup steps"))
                .then_some(GameRule::KeepsDamage)
        })
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
        GameRule::ManaBurn => vec![R303B_BURN.to_owned()],
        GameRule::KeepsDamage => ["~", "this creature"]
            .map(|it| format!("Damage isn't removed from {it} during cleanup steps"))
            .to_vec(),
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
