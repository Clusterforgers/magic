//! Casting during a resolution: "Discover 4.", and the sentences of an exile from the top of a library until a card is
//! found, the free cast of it and the move of the cards exiled to the bottom, each read from its wording and written
//! back the same way; and the impulse draw, "Exile the top card of your library. You may play that card this turn."

use mtg_core::card::CardType;

use super::amounts::{points, points_text};
use super::library::{top_cards, top_text};
use super::tokens::Reader;
use crate::effect::{Destination, EXILED, Effect, FOUND, IMPULSE, Order, Place, discover, impulse};
use crate::modify::Duration;
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

/// "Exile the top card of your library.", "Exile the top two cards of your library.": the cards are exiled face up and
/// named for the sentence that lets the player play them.
pub fn exile_top(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("exile")?;
    let count = top_cards(r)?;
    r.need("of your library")?;
    Some(vec![Effect::ExileTop {
        who: Ref::You,
        count,
        bind: IMPULSE.to_owned(),
    }])
}

/// "You may play that card this turn.", "Until end of turn, you may play that card.", "Until the end of your next turn,
/// you may play those cards.": the permission to play the cards an exile from the top of a library named.
fn may_play(r: &mut Reader<'_>, count: &Amount) -> Option<Duration> {
    let leading = r.attempt(|r| {
        let duration = if r.words("until end of turn") {
            Duration::UntilEndOfTurn
        } else {
            r.need("until the end of your next turn")?;
            Duration::UntilEndOfYourNextTurn
        };
        r.punct(',').then_some(duration)
    });
    r.need("you may play")?;
    if count.is_one() {
        r.need("that card")?;
    } else if !(r.words("those cards") || r.word("them")) {
        return None;
    }
    match leading {
        Some(duration) => Some(duration),
        None => r.need("this turn").map(|()| Duration::UntilEndOfTurn),
    }
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
                play: false,
                way: None,
                otherwise: Vec::new(),
            }
        }
        Effect::ExileTop { count, bind, .. } if bind == IMPULSE => {
            let duration = may_play(r, count)?;
            impulse(count.clone(), duration).remove(1)
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

/// The two sentences the effects at the front of the list are when they are an impulse draw, and how many effects they take.
pub fn impulse_text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::ExileTop { count, .. },
        Effect::Rule { duration, .. },
        ..,
    ] = effects
    else {
        return None;
    };
    if effects.get(..2)? != impulse(count.clone(), *duration) {
        return None;
    }
    let cards = if count.is_one() {
        "that card"
    } else {
        "those cards"
    };
    let may = match duration {
        Duration::UntilEndOfTurn => format!("You may play {cards} this turn"),
        Duration::UntilEndOfYourNextTurn => {
            format!("Until the end of your next turn, you may play {cards}")
        }
        _ => return None,
    };
    Some((
        format!("exile {} of your library. {may}", top_text(count)?),
        2,
    ))
}

/// The other wordings of an impulse draw in the text of an ability: the duration may come first and the cards may be
/// "them".
pub fn impulse_wordings(text: String) -> Vec<String> {
    let mut found = vec![text];
    for (from, to) in [
        (
            "You may play that card this turn",
            "Until end of turn, you may play that card",
        ),
        (
            "You may play those cards this turn",
            "Until end of turn, you may play those cards",
        ),
        ("You may play those cards", "You may play them"),
        ("you may play those cards", "you may play them"),
    ] {
        let more: Vec<String> = found
            .iter()
            .filter(|text| text.contains(from))
            .map(|text| text.replace(from, to))
            .collect();
        found.extend(more);
    }
    found
}

/// The wording of the effects of the sentences above, one effect each.
pub fn text(effect: &Effect) -> Option<String> {
    Some(match effect {
        Effect::ExileTop {
            who: Ref::You,
            count,
            bind,
        } if bind == IMPULSE => format!("exile {} of your library", top_text(count)?),
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
            play: false,
            way: None,
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
