//! A spell cast from the hand for free: "You may cast a spell with mana value X or less from your hand without paying its
//! mana cost."

use mtg_core::card::CardType;

use super::Effect;
use crate::refs::{Pred, Ref, ZoneKind};
use crate::value::{Amount, Cmp, StatKind};

/// The limit on the mana value of a spell, as the words after "a spell".
fn limit(amount: &Amount) -> Option<String> {
    match amount {
        Amount::X => Some(" with mana value X or less".to_owned()),
        Amount::Fixed(number) if *number >= 0 => Some(format!(" with mana value {number} or less")),
        _ => None,
    }
}

/// "you may cast a spell with mana value X or less from your hand without paying its mana cost": a choice of at most one
/// nonland card in the hand and its free cast; the number of effects it takes.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(selector),
            count,
            bind,
            rest_as: None,
        },
        Effect::CastFromZone {
            who: Ref::You,
            what: Ref::Bound(cast),
            free: true,
            play: false,
            way: None,
            transformed: false,
            bind: None,
            otherwise,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    (bind == cast && otherwise.is_empty() && count.min == Amount::Fixed(0)).then_some(())?;
    (count.max == Some(Amount::Fixed(1))).then_some(())?;
    let mut value = String::new();
    let (mut hand, mut yours, mut spells) = (false, false, false);
    for pred in &selector.0 {
        match pred {
            Pred::In(ZoneKind::Hand) => hand = true,
            Pred::OwnedBy(Ref::You) => yours = true,
            Pred::Not(inner) if **inner == Pred::Type(CardType::Land) => spells = true,
            Pred::Compare {
                stat: StatKind::ManaValue,
                op: Cmp::Le,
                to,
            } if value.is_empty() => value = limit(to)?,
            _ => return None,
        }
    }
    (hand && yours && spells).then_some(())?;
    Some((
        format!("you may cast a spell{value} from your hand without paying its mana cost"),
        2,
    ))
}
