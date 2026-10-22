//! The library: searching it for cards, looking at its top cards and putting cards back, read from
//! the wordings of the cards and written back the same way. A search, the move of what it finds and
//! the shuffle are one sentence on a card and three effects in the data.

use std::fmt::Write;

use mtg_core::card::{CardType, Subtype};

use super::amounts::{count_word, number_word};
use super::effects::target_object;
use super::selectors::{
    Form, adjective, adjective_text, card_type, subtype, suffix, suffix_text, type_name,
};
use super::tokens::Reader;
use super::triggers::article;
use crate::effect::{Destination, Effect, Order, Place};
use crate::refs::{Bounds, Pred, Ref, Selector, Targetable, ZoneKind};
use crate::value::Amount;

/// The types of a permanent, which "permanent card" stands for.
const PERMANENTS: [CardType; 6] = [
    CardType::Artifact,
    CardType::Creature,
    CardType::Enchantment,
    CardType::Land,
    CardType::Planeswalker,
    CardType::Battle,
];

/// The land types of the basic lands.
const BASIC_LANDS: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

/// The name the cards a player looks at are bound to.
pub const TOP: &str = "top";

/// The name the card a player keeps of those they looked at is bound to.
pub const KEPT: &str = "kept";

/// The name the cards a player does not keep are bound to.
pub const REST: &str = "rest";

/// The name the cards a search finds are bound to: lands are lands, anything else a card.
fn found_as(filter: &Selector) -> &'static str {
    let land_type = |pred: &Pred| {
        match pred {
        Pred::Type(CardType::Land) => true,
        Pred::Subtype(Subtype(name)) => BASIC_LANDS.contains(&name.as_str()),
        Pred::AnyOf(list) => list.iter().all(|pred| {
            matches!(pred, Pred::Subtype(Subtype(name)) if BASIC_LANDS.contains(&name.as_str()))
        }),
        _ => false,
    }
    };
    if filter.0.iter().any(land_type) {
        "land"
    } else {
        "card"
    }
}

/// An adjective a card in a library can have.
fn is_card_adjective(pred: &Pred) -> bool {
    match pred {
        Pred::Supertype(_) | Pred::Color(_) | Pred::Colorless => true,
        Pred::Not(inner) => matches!(
            **inner,
            Pred::Supertype(_) | Pred::Color(_) | Pred::Type(_) | Pred::Token
        ),
        _ => false,
    }
}

/// One predicate or an `AnyOf` of several.
fn one_of(mut preds: Vec<Pred>) -> Pred {
    if preds.len() == 1 {
        preds.remove(0)
    } else {
        Pred::AnyOf(preds)
    }
}

/// "basic land", "Plains or Island", "legendary Dragon creature", "artifact or creature", then
/// "card" or "cards", then what the card has: "with mana value 3 or less".
pub(super) fn filter(r: &mut Reader<'_>, plural: bool) -> Option<Selector> {
    let mut preds = Vec::new();
    while let Some(pred) = r
        .peek_word()
        .and_then(|word| adjective(word, Form::Each))
        .filter(is_card_adjective)
    {
        r.skip();
        preds.push(pred);
    }
    if let Some(first) = subtype(r) {
        let mut names = vec![first];
        while let Some(next) = r.attempt(|r| {
            r.need("or")?;
            subtype(r)
        }) {
            names.push(next);
        }
        preds.push(one_of(
            names
                .into_iter()
                .map(|name| Pred::Subtype(Subtype(name)))
                .collect(),
        ));
    }
    let mut types = Vec::new();
    while let Some(found) = r.peek_word().and_then(card_type) {
        r.skip();
        types.push(Pred::Type(found));
        if r.word_at(0) == Some("or") && r.word_at(1).and_then(card_type).is_some() {
            r.skip();
        } else {
            break;
        }
    }
    if types.is_empty() && r.word("permanent") {
        types = PERMANENTS.iter().map(|kind| Pred::Type(*kind)).collect();
    }
    if !types.is_empty() {
        preds.push(one_of(types));
    }
    r.word(if plural { "cards" } else { "card" })
        .then_some(())?;
    if let Some(extra) = r
        .attempt(|r| suffix(r, Form::Subject))
        .filter(|pred| matches!(pred, Pred::Compare { .. } | Pred::HasKeyword(_)))
    {
        preds.push(extra);
    }
    Some(Selector(preds))
}

/// "a basic land card", "up to two Forest cards", "any number of Dragon creature cards".
fn what(r: &mut Reader<'_>) -> Option<(Selector, Bounds)> {
    let (bounds, plural) = if r.words("up to") {
        let number = number_word(r.take_word()?).filter(|number| *number > 1)?;
        let max = Some(Amount::Fixed(i32::try_from(number).ok()?));
        (Bounds { min: 0, max }, true)
    } else if r.words("any number of") {
        (Bounds { min: 0, max: None }, true)
    } else if r.word("a") || r.word("an") {
        (Bounds::default(), false)
    } else {
        return None;
    };
    Some((filter(r, plural)?, bounds))
}

/// "it" or "that card" for one card, "them" or "those cards" for several.
fn pronoun(r: &mut Reader<'_>, plural: bool) -> Option<()> {
    let found = if plural {
        r.word("them") || r.words("those cards")
    } else {
        r.word("it") || r.words("that card")
    };
    found.then_some(())
}

/// Where a search puts what it finds: "onto the battlefield", "onto the battlefield tapped", "into your hand".
fn found_destination(r: &mut Reader<'_>) -> Option<Destination> {
    if r.words("onto the battlefield") {
        let tapped = r.word("tapped");
        return Some(Destination::Battlefield {
            tapped,
            under: None,
        });
    }
    r.words("into your hand").then_some(Destination::Hand)
}

/// "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
/// "... for a card, reveal it, put it into your hand, then shuffle.", "... for a card, then shuffle
/// and put that card on top.", each also after "you may".
pub fn search(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let optional = r.words("you may");
    r.need("search your library for")?;
    let (filter, count) = what(r)?;
    let plural = !count.is_one();
    let reveal = r.attempt(|r| {
        r.punct(',').then_some(())?;
        r.need("reveal")?;
        pronoun(r, plural)
    });
    let bind = found_as(&filter).to_owned();
    let found = Ref::Bound(bind.clone());
    let search = Effect::Search {
        who: Ref::You,
        zone: ZoneKind::Library,
        filter,
        count,
        reveal: reveal.is_some(),
        bind,
    };
    let shuffle = Effect::Shuffle { who: Ref::You };
    r.punct(',').then_some(())?;
    let effects = if r.need("then shuffle and put").is_some() {
        (!plural).then_some(())?;
        pronoun(r, false)?;
        r.need("on top")?;
        let to = Destination::Library {
            place: Place::Top,
            order: None,
        };
        vec![search, shuffle, Effect::Move { what: found, to }]
    } else {
        r.need("put")?;
        pronoun(r, plural)?;
        let to = found_destination(r)?;
        r.punct(',').then_some(())?;
        r.need("then shuffle")?;
        vec![search, Effect::Move { what: found, to }, shuffle]
    };
    Some(if optional {
        vec![Effect::May {
            who: Ref::You,
            effects,
        }]
    } else {
        effects
    })
}

/// "the top card", "the top three cards": how many cards of the top of a library.
pub(super) fn top_cards(r: &mut Reader<'_>) -> Option<Amount> {
    r.need("the top")?;
    if r.word("card") {
        return Some(Amount::one());
    }
    let number = number_word(r.take_word()?).filter(|number| *number > 1)?;
    r.word("cards").then_some(())?;
    Some(Amount::Fixed(i32::try_from(number).ok()?))
}

/// "Look at the top three cards of your library.", and after it "Put one of them into your hand and
/// the rest on the bottom of your library in any order." or "... into your graveyard."
pub fn look(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("look at")?;
    let count = top_cards(r)?;
    r.need("of your library")?;
    let looked = Effect::LookAtTop {
        who: Ref::You,
        count,
        bind: TOP.to_owned(),
    };
    let rest = r.attempt(|r| {
        r.punct('.').then_some(())?;
        r.need("put one of them into your hand and the rest")?;
        if r.words("into your graveyard") {
            return Some(Destination::Graveyard);
        }
        r.need("on the bottom of your library in")?;
        let order = if r.words("any order") {
            Order::Chosen
        } else {
            r.need("a random order")?;
            Order::Random
        };
        Some(Destination::Library {
            place: Place::Bottom,
            order: Some(order),
        })
    });
    let Some(rest) = rest else {
        return Some(vec![looked]);
    };
    Some(vec![
        looked,
        Effect::Choose {
            who: Ref::You,
            from: Ref::Bound(TOP.to_owned()),
            count: Bounds::default(),
            bind: KEPT.to_owned(),
            rest_as: Some(REST.to_owned()),
        },
        Effect::Move {
            what: Ref::Bound(KEPT.to_owned()),
            to: Destination::Hand,
        },
        Effect::Move {
            what: Ref::Bound(REST.to_owned()),
            to: rest,
        },
    ])
}

/// "Put target card from your graveyard on the bottom of your library.", "Put target creature on top
/// of its owner's library."
pub fn put_on_library(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("put")?;
    let what = target_object(r)?;
    let Some(Targetable::Object(selector)) = r.targets.first().map(|spec| &spec.what) else {
        return None;
    };
    let yours = selector.0.contains(&Pred::In(ZoneKind::Graveyard))
        && selector.0.contains(&Pred::OwnedBy(Ref::You));
    let permanent = !selector
        .0
        .iter()
        .any(|pred| matches!(pred, Pred::In(_) | Pred::Spell));
    let place = if r.words("on top of") {
        Place::Top
    } else {
        r.need("on the bottom of")?;
        Place::Bottom
    };
    if yours {
        r.need("your library")?;
    } else if permanent {
        r.need("its owner's library")?;
        r.it = Some(what.clone());
    } else {
        return None;
    }
    Some(vec![Effect::Move {
        what,
        to: Destination::Library { place, order: None },
    }])
}

/// The card types and subtypes of a predicate, as the words before "card".
fn names_text(pred: &Pred) -> Option<(u8, String)> {
    match pred {
        Pred::Subtype(Subtype(name)) => Some((1, name.clone())),
        Pred::Type(kind) => Some((2, type_name(*kind).to_owned())),
        Pred::AnyOf(list) => {
            let permanents: Vec<Pred> = PERMANENTS.iter().map(|kind| Pred::Type(*kind)).collect();
            if *list == permanents {
                return Some((2, "permanent".to_owned()));
            }
            let names: Option<Vec<(u8, String)>> = list.iter().map(names_text).collect();
            let names = names?;
            let slot = names.first()?.0;
            if names.len() < 2 || names.iter().any(|(other, _)| *other != slot) {
                return None;
            }
            let words: Vec<String> = names.into_iter().map(|(_, name)| name).collect();
            Some((slot, words.join(" or ")))
        }
        _ => None,
    }
}

/// The filter of a search as the card words it, without its count: "basic land card",
/// "Plains or Island cards", "creature card with mana value 3 or less".
pub(super) fn filter_text(filter: &Selector, plural: bool) -> Option<String> {
    let (mut head, mut tail) = (Vec::new(), Vec::new());
    let mut stage = 0;
    for pred in &filter.0 {
        if is_card_adjective(pred) {
            if stage > 0 {
                return None;
            }
            head.push(adjective_text(pred, Form::Each)?);
        } else if let Some((slot, text)) = names_text(pred) {
            // Subtypes come before card types, each once.
            if slot <= stage {
                return None;
            }
            stage = slot;
            head.push(text);
        } else if matches!(pred, Pred::Compare { .. } | Pred::HasKeyword(_)) {
            tail.push(suffix_text(pred, Form::Subject)?);
        } else {
            return None;
        }
    }
    head.push(if plural { "cards" } else { "card" }.to_owned());
    head.extend(tail);
    Some(head.join(" "))
}

/// "a basic land card", "up to two Forest cards", "any number of Dragon creature cards".
fn what_text(filter: &Selector, count: &Bounds) -> Option<String> {
    if count.is_one() {
        let text = filter_text(filter, false)?;
        return Some(format!("{} {text}", article(&text)));
    }
    let text = filter_text(filter, true)?;
    match (count.min, &count.max) {
        (0, None) => Some(format!("any number of {text}")),
        (0, Some(Amount::Fixed(most))) if *most > 1 => Some(format!(
            "up to {} {text}",
            count_word(u32::try_from(*most).ok()?)?
        )),
        _ => None,
    }
}

/// How many cards of the top: "the top card", "the top three cards".
pub(super) fn top_text(count: &Amount) -> Option<String> {
    match count {
        Amount::Fixed(1) => Some("the top card".to_owned()),
        Amount::Fixed(number) if *number > 1 => Some(format!(
            "the top {} cards",
            count_word(u32::try_from(*number).ok()?)?
        )),
        _ => None,
    }
}

/// Where a search puts what it finds, as the card words it.
fn destination_text(to: &Destination) -> Option<String> {
    match to {
        Destination::Battlefield {
            tapped,
            under: None,
        } => Some(format!(
            "onto the battlefield{}",
            if *tapped { " tapped" } else { "" }
        )),
        Destination::Hand => Some("into your hand".to_owned()),
        _ => None,
    }
}

/// The sentence the effects at the front of the list are, and how many effects it takes: a search
/// with the move of what it finds and the shuffle, a look at the top cards and the choice from them,
/// each also under "you may". `None` when the list does not begin with one.
pub fn sequence(effects: &[Effect]) -> Option<(String, usize)> {
    match effects {
        [
            Effect::May {
                who: Ref::You,
                effects: inner,
            },
            ..,
        ] => {
            let (text, used) = sequence(inner)?;
            (used == inner.len() && matches!(inner.first(), Some(Effect::Search { .. })))
                .then(|| (format!("you may {text}"), 1))
        }
        [Effect::Search { .. }, ..] => search_sequence(effects),
        [Effect::LookAtTop { .. }, ..] => look_sequence(effects),
        _ => None,
    }
}

/// A search, the move of what it finds and the shuffle, or the shuffle and then the move to the top.
fn search_sequence(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Search {
            who: Ref::You,
            zone: ZoneKind::Library,
            filter,
            count,
            reveal,
            bind,
        },
        rest @ ..,
    ] = effects
    else {
        return None;
    };
    if bind != found_as(filter) {
        return None;
    }
    let plural = !count.is_one();
    let mut text = format!("search your library for {}", what_text(filter, count)?);
    if *reveal {
        text.push_str(if plural {
            ", reveal them"
        } else {
            ", reveal it"
        });
    }
    let found = Ref::Bound(bind.clone());
    match rest {
        [
            Effect::Move { what, to },
            Effect::Shuffle { who: Ref::You },
            ..,
        ] if *what == found => {
            let pron = if plural { "them" } else { "it" };
            let _ = write!(text, ", put {pron} {}, then shuffle", destination_text(to)?);
        }
        [
            Effect::Shuffle { who: Ref::You },
            Effect::Move {
                what,
                to:
                    Destination::Library {
                        place: Place::Top,
                        order: None,
                    },
            },
            ..,
        ] if *what == found && !plural => {
            text.push_str(", then shuffle and put that card on top");
        }
        _ => return None,
    }
    Some((text, 3))
}

/// A look at the top cards, the choice of one of them for the hand and where the rest go.
fn look_sequence(effects: &[Effect]) -> Option<(String, usize)> {
    match effects {
        [
            Effect::LookAtTop {
                who: Ref::You,
                count,
                bind,
            },
            Effect::Choose {
                who: Ref::You,
                from: Ref::Bound(from),
                count: chosen,
                bind: kept,
                rest_as: Some(rest),
            },
            Effect::Move {
                what: Ref::Bound(moved),
                to: Destination::Hand,
            },
            Effect::Move {
                what: Ref::Bound(left),
                to: left_to,
            },
            ..,
        ] if bind == TOP
            && from == TOP
            && kept == KEPT
            && rest == REST
            && moved == KEPT
            && left == REST
            && chosen.is_one() =>
        {
            let tail = match left_to {
                Destination::Graveyard => "into your graveyard",
                Destination::Library {
                    place: Place::Bottom,
                    order: Some(Order::Chosen),
                } => "on the bottom of your library in any order",
                Destination::Library {
                    place: Place::Bottom,
                    order: Some(Order::Random),
                } => "on the bottom of your library in a random order",
                _ => return None,
            };
            Some((
                format!(
                    "look at {} of your library. Put one of them into your hand and the rest {tail}",
                    top_text(count)?
                ),
                4,
            ))
        }
        _ => None,
    }
}

/// A look at the top cards of the library and nothing after it: "look at the top card of your library".
pub fn look_text(count: &Amount, bind: &str) -> Option<String> {
    (bind == TOP).then_some(())?;
    let cards = top_text(count)?;
    Some(format!("look at {cards} of your library"))
}

/// A card put on a library: "on top of", "on the bottom of".
pub fn place_text(place: Place) -> Option<&'static str> {
    match place {
        Place::Top => Some("on top of"),
        Place::Bottom => Some("on the bottom of"),
        Place::Shuffled => None,
    }
}
