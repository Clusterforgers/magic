//! Sentences of the r30-7c card pack that no other writer gives: "that player's life total becomes 1", "~ assigns no
//! combat damage this combat".

use crate::effect::Effect;
use crate::modify::{CombatQuirk, Duration, GameRule};
use crate::refs::Ref;
use crate::value::Amount;

/// The sentence of an effect this pack writes, without its period.
pub(super) fn r307c_text(effect: &Effect) -> Option<String> {
    match effect {
        Effect::SetLife {
            who: Ref::Defending,
            amount: Amount::Fixed(n),
        } => Some(format!("that player's life total becomes {n}")),
        Effect::Rule {
            rule:
                GameRule::Combat {
                    who: Ref::This,
                    quirk: CombatQuirk::AssignsNoCombatDamage,
                },
            duration: Duration::UntilEndOfCombat,
        } => Some("~ assigns no combat damage this combat".to_owned()),
        _ => None,
    }
}
