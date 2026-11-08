//! The tax of the X paid for an ability, for this turn, written back: "this turn, creatures can't block unless their
//! controller pays {X} for each blocking creature they control" and "this turn, creatures can't attack unless their
//! controller pays {X} for each attacking creature they control". Read from `Effect::Rule` of a `BlockTax` or an
//! `AttackTax` on every creature that pays {1} as many times as the X.

use crate::effect::Effect;
use crate::modify::{Attacked, Duration, GameRule};
use crate::refs::Ref;
use crate::value::Amount;

/// The sentence of the rule, without a capital and a period; none for any other effect.
pub fn g12f_x_tax_text(effect: &Effect) -> Option<String> {
    let Effect::Rule {
        rule,
        duration: Duration::UntilEndOfTurn,
    } = effect
    else {
        return None;
    };
    let one = |mana: &crate::mana::ManaCostX, life: u32, per: &Amount| {
        mana.to_string() == "{1}" && life == 0 && *per == Amount::X
    };
    match rule {
        GameRule::BlockTax {
            blockers,
            attackers,
            mana,
            life,
            per,
        } if blockers.is_empty() && attackers.is_empty() && one(mana, *life, per) => Some(
            "this turn, creatures can't block unless their controller pays {X} for each blocking creature they control"
                .to_owned(),
        ),
        GameRule::AttackTax {
            who: Ref::EachPlayer,
            attacked: Attacked::Both,
            attackers,
            mana,
            life,
            per,
        } if attackers.is_empty() && one(mana, *life, per) => Some(
            "this turn, creatures can't attack unless their controller pays {X} for each attacking creature they control"
                .to_owned(),
        ),
        _ => None,
    }
}
