//! Casting during a resolution: "Discover 4.", and the sentences of an exile from the top of a library until a card is
//! found, the free cast of it and the move of the cards exiled to the bottom, each read from its wording and written
//! back the same way.

use mtg_core::card::CardType;

use super::amounts::{points, points_text};
use super::tokens::Reader;
use crate::effect::{Destination, EXILED, Effect, FOUND, Order, Place, discover};
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, StatKind};

/// "Discover 4.": the exiling until a nonland card of that value or less, the cast for nothing or the move to the
/// hand, and the move of the cards exiled to the bottom (CR 701.57a).
pub fn discover_sentence(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("discover")?;
    Some(discover(points(r)?))
}

/// "Exile cards from the top of your library until you exile a nonland card with mana value 3 or less."
pub fn exile_until(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need(
        "exile cards from the top of your library until you exile a nonland card with mana value",
    )?;
    let count = points(r)?;
    r.need("or less")?;
    Some(vec![Effect::ExileUntil {
        who: Ref::You,
        filter: nonland_up_to(count),
        bind: FOUND.to_owned(),
        exiled_as: EXILED.to_owned(),
    }])
}

/// The sentences that follow an exiling until a card is found and refer to what it exiled: "You may cast it without paying
/// its mana cost." and "Put all cards exiled this way that weren't cast on the bottom of your library in a random order."
pub fn after_exiling(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let last = effects.last()?;
    let next = match last {
        Effect::ExileUntil { bind, .. } if bind == FOUND => {
            r.need("you may cast")?;
            (r.word("it") || r.words("that card")).then_some(())?;
            r.need("without paying its mana cost")?;
            Effect::CastFromZone {
                who: Ref::You,
                what: Ref::Bound(FOUND.to_owned()),
                free: true,
                otherwise: Vec::new(),
            }
        }
        Effect::CastFromZone { .. } => {
            r.need("put all cards exiled this way that weren't cast on the bottom of your library in a random order")?;
            bottom_in_a_random_order()
        }
        _ => return None,
    };
    r.sentence_end().then(|| effects.push(next))
}

/// "a nonland card with mana value N or less".
fn nonland_up_to(count: Amount) -> Selector {
    Selector(vec![
        Pred::Not(Box::new(Pred::Type(CardType::Land))),
        Pred::Compare {
            stat: StatKind::ManaValue,
            op: Cmp::Le,
            to: count,
        },
    ])
}

/// The cards exiled that are still in exile go to the bottom of the library in a random order.
fn bottom_in_a_random_order() -> Effect {
    Effect::Move {
        what: Ref::Bound(EXILED.to_owned()),
        to: Destination::Library {
            place: Place::Bottom,
            order: Some(Order::Random),
        },
    }
}

/// The sentence the effects at the front of the list are when they are discover, and how many effects it takes.
pub fn discover_text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::ExileUntil {
            filter: Selector(preds),
            ..
        },
        ..,
    ] = effects
    else {
        return None;
    };
    let [
        _,
        Pred::Compare {
            op: Cmp::Le,
            to: count,
            ..
        },
    ] = preds.as_slice()
    else {
        return None;
    };
    let text = format!("discover {}", points_text(count)?);
    (effects.get(..3)? == discover(count.clone())).then_some((text, 3))
}

/// The wording of the effects of the sentences above, one effect each.
pub fn text(effect: &Effect) -> Option<String> {
    Some(match effect {
        Effect::ExileUntil {
            who: Ref::You,
            filter: Selector(preds),
            bind,
            exiled_as,
        } if bind == FOUND && exiled_as == EXILED => {
            let [
                _,
                Pred::Compare {
                    op: Cmp::Le,
                    to: count,
                    ..
                },
            ] = preds.as_slice()
            else {
                return None;
            };
            format!(
                "exile cards from the top of your library until you exile a nonland card with mana value {} or less",
                points_text(count)?
            )
        }
        Effect::CastFromZone {
            who: Ref::You,
            what: Ref::Bound(name),
            free: true,
            otherwise,
        } if name == FOUND && otherwise.is_empty() => {
            "you may cast it without paying its mana cost".to_owned()
        }
        moved if *moved == bottom_in_a_random_order() => {
            "put all cards exiled this way that weren't cast on the bottom of your library in a random order".to_owned()
        }
        _ => return None,
    })
}
