//! Sentences of the r30-7c card pack that no other writer gives: "that player's life total becomes 1", "~ assigns no
//! combat damage this combat", "surveil X, where X is the number of opponents being attacked", "1 life for each card type
//! among those cards".

use crate::effect::Effect;
use crate::modify::{CombatQuirk, Duration, GameRule};
use crate::refs::{Pred, Ref, ZoneKind};
use crate::value::{Amount, Condition};

/// Whether the amount is the number of opponents being attacked: those attacked in the combat in progress (CR 506.2).
fn r307c_opponents_attacked(amount: &Amount) -> bool {
    let Amount::Players(Ref::PlayersWhere { who, test }) = amount else {
        return false;
    };
    **who == Ref::Opponents && **test == Condition::AttackedThisCombat(Ref::You)
}

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
        Effect::Surveil {
            who: Ref::You,
            count,
            ..
        } if r307c_opponents_attacked(count) => {
            Some("surveil X, where X is the number of opponents being attacked".to_owned())
        }
        _ => None,
    }
}

/// The life of a gain or loss this pack words: "1 life for each card type among those cards" (the cards of the event, in a
/// graveyard).
pub(super) fn r307c_life(amount: &Amount) -> Option<String> {
    match amount {
        Amount::CardTypes(among)
            if among.0 == [Pred::In(ZoneKind::Graveyard), Pred::Is(Ref::Triggering)] =>
        {
            Some("1 life for each card type among those cards".to_owned())
        }
        _ => None,
    }
}
