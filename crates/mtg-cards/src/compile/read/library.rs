//! The library: searching it for cards, looking at its top cards and putting cards back, read from
//! the wordings of the cards and written back the same way. A search, the move of what it finds and
//! the shuffle are one sentence on a card and three effects in the data.

use std::fmt::Write;

use mtg_core::card::{CardType, Subtype};

use super::amounts::{count_word, number_word};
use super::defining;
use super::effects::target_object;
use super::pairs;
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

/// The name the cards `who` finds are bound to: another player's search is bound apart from yours, as a name is bound
/// once in a resolution ("That land's controller may search ... You may search ...").
fn bound_as(who: &Ref, filter: &Selector) -> String {
    if *who == Ref::You {
        found_as(filter).to_owned()
    } else {
        format!("their_{}", found_as(filter))
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

/// A card type word.
fn one_type(r: &mut Reader<'_>) -> Option<CardType> {
    let found = r.peek_word().and_then(card_type)?;
    r.skip();
    Some(found)
}

/// The items of a list as a card words it: "Plains", "Plains or Island", "Forest, Plains, or Island" (the comma
/// before "or" only in a list of three or more).
fn listed<T>(
    r: &mut Reader<'_>,
    mut item: impl FnMut(&mut Reader<'_>) -> Option<T>,
) -> Option<Vec<T>> {
    let mut found = vec![item(r)?];
    loop {
        let more = r.attempt(|r| {
            let comma = r.punct(',');
            let or = r.word("or");
            (comma || or).then_some(())?;
            Some((comma, or, item(r)?))
        });
        let Some((comma, or, next)) = more else {
            return (found.len() == 1).then_some(found);
        };
        found.push(next);
        if or {
            return (comma == (found.len() > 2)).then_some(found);
        }
    }
}

/// "basic land", "Plains or Island", "Forest, Plains, or Island", "legendary Dragon creature", "artifact or creature",
/// then "card" or "cards", then what the card has: "with mana value 3 or less".
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
    if let Some(names) = r.attempt(|r| listed(r, subtype)) {
        preds.push(one_of(
            names
                .into_iter()
                .map(|name| Pred::Subtype(Subtype(name)))
                .collect(),
        ));
    }
    let mut types: Vec<Pred> = r
        .attempt(|r| listed(r, one_type))
        .unwrap_or_default()
        .into_iter()
        .map(Pred::Type)
        .collect();
    if types.is_empty() && r.word("permanent") {
        types = PERMANENTS.iter().map(|kind| Pred::Type(*kind)).collect();
    }
    if !types.is_empty() {
        preds.push(one_of(types));
    }
    r.word(if plural { "cards" } else { "card" })
        .then_some(())?;
    if let Some(extra) = r.attempt(|r| {
        suffix(r, Form::Subject)
            .filter(|pred| matches!(pred, Pred::Compare { .. } | Pred::HasKeyword(_)))
    }) {
        preds.push(extra);
    }
    Some(Selector(preds))
}

/// "a basic land card", "up to two Forest cards", "any number of Dragon creature cards".
pub(super) fn what(r: &mut Reader<'_>) -> Option<(Selector, Bounds)> {
    // "up to X basic land cards, where X is the greatest power among creatures you control": the count follows the cards.
    let counted = r.words("up to x");
    let (mut bounds, plural) = if counted {
        (Bounds::up_to(Some(Amount::X)), true)
    } else if r.words("up to") {
        let number = number_word(r.take_word()?).filter(|number| *number > 1)?;
        let max = Some(Amount::Fixed(i32::try_from(number).ok()?));
        (Bounds::up_to(max), true)
    } else if r.words("any number of") {
        (Bounds::up_to(None), true)
    } else if r.word("a") || r.word("an") {
        (Bounds::default(), false)
    } else {
        return None;
    };
    let found = filter(r, plural)?;
    if counted {
        r.punct(',').then_some(())?;
        r.need("where x is")?;
        bounds.max = Some(defining::value(r)?);
        // The sentence's where clause is read: none is left for `where_x::bind` to find.
        r.where_x = false;
    }
    Some((found, bounds))
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
pub(super) fn found_destination(r: &mut Reader<'_>) -> Option<Destination> {
    if r.words("onto the battlefield") {
        let tapped = r.word("tapped");
        return Some(Destination::Battlefield {
            tapped,
            under: None,
            transformed: false,
            counters: Vec::new(),
        });
    }
    if r.words("into your graveyard") {
        return Some(Destination::Graveyard);
    }
    r.words("into your hand").then_some(Destination::Hand)
}

/// "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
/// "... for a card, reveal it, put it into your hand, then shuffle.", "... for a card, then shuffle
/// and put that card on top.", each also after "you may".
pub fn search(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let optional = r.words("you may");
    searched_by(r, Ref::You, optional)
}

/// "Its controller may search their library for a basic land card, put it onto the battlefield tapped, then
/// shuffle.": the same search for the controller of a permanent the sentence before named.
pub fn search_by_controller(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let who = super::effects::player(r).filter(|who| matches!(who, Ref::ControllerOf(_)))?;
    r.need("may")?;
    searched_by(r, who, true)
}

/// The search of `who` and the move of what they find: "search your library for ..." for you, "search their library
/// for ..." for another player.
fn searched_by(r: &mut Reader<'_>, who: Ref, optional: bool) -> Option<Vec<Effect>> {
    r.need(if who == Ref::You {
        "search your library for"
    } else {
        "search their library for"
    })?;
    let (filter, count) = what(r)?;
    let counted = matches!(&count.max, Some(amount) if !matches!(amount, Amount::Fixed(_)));
    let bind = bound_as(&who, &filter);
    let pair = r.attempt(|r| pairs::read(r, &filter, &count, &bind));
    let plural = !count.is_one() || pair.is_some();
    let reveal = r.attempt(|r| {
        r.punct(',').then_some(())?;
        r.need("reveal")?;
        pronoun(r, plural)
    });
    let (count, second) = match pair {
        Some((first, second)) => (first, Some(second)),
        None => (count, None),
    };
    let (mut searches, found) =
        pairs::searches(&who, (filter, count, bind), second, reveal.is_some());
    let shuffle = Effect::Shuffle { who: who.clone() };
    // A where clause may end the sentence: "..., where X is ... . Put those cards onto the battlefield, then shuffle."
    if !(counted && r.punct('.')) {
        r.punct(',').then_some(())?;
    }
    let effects = if r.need("then shuffle and put").is_some() {
        (!plural).then_some(())?;
        pronoun(r, false)?;
        r.need("on top")?;
        let to = Destination::Library {
            place: Place::Top,
            order: None,
        };
        searches.extend([
            shuffle,
            Effect::Move {
                what: found[0].clone(),
                to,
            },
        ]);
        searches
    } else {
        r.need("put")?;
        pronoun(r, plural)?;
        let to = found_destination(r)?;
        r.punct(',').then_some(())?;
        r.need("then shuffle")?;
        searches.extend(found.into_iter().map(|what| Effect::Move {
            what,
            to: to.clone(),
        }));
        searches.push(shuffle);
        searches
    };
    Some(if optional {
        vec![Effect::May { who, effects }]
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

/// Where the cards of the top that are not kept go: "into your graveyard", "on the bottom of your library in any order",
/// "on the bottom of your library in a random order".
pub(super) fn rest_destination(r: &mut Reader<'_>) -> Option<Destination> {
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
}

/// The wording of the destination `rest_destination` reads.
pub(super) fn rest_text(to: &Destination) -> Option<&'static str> {
    match to {
        Destination::Graveyard => Some("into your graveyard"),
        Destination::Library {
            place: Place::Bottom,
            order: Some(Order::Chosen),
        } => Some("on the bottom of your library in any order"),
        Destination::Library {
            place: Place::Bottom,
            order: Some(Order::Random),
        } => Some("on the bottom of your library in a random order"),
        _ => None,
    }
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
        rest_destination(r)
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

/// Two or more items as a card words them: "Plains or Island", "Forest, Plains, or Island".
fn list_text(words: &[String]) -> String {
    match words {
        [first, last] => format!("{first} or {last}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
        [] => String::new(),
    }
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
            Some((slot, list_text(&words)))
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
pub(super) fn what_text(filter: &Selector, count: &Bounds) -> Option<String> {
    if count.is_one() {
        let text = filter_text(filter, false)?;
        return Some(format!("{} {text}", article(&text)));
    }
    let text = filter_text(filter, true)?;
    match (count.fixed_min()?, &count.max) {
        (0, None) => Some(format!("any number of {text}")),
        (0, Some(Amount::Fixed(most))) if *most > 1 => Some(format!(
            "up to {} {text}",
            count_word(u32::try_from(*most).ok()?)?
        )),
        (0, Some(amount)) if !matches!(amount, Amount::Fixed(_) | Amount::X) => Some(format!(
            "up to X {text}, where X is {}",
            defining::value_text(amount)?
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
pub(super) fn destination_text(to: &Destination) -> Option<String> {
    match to {
        Destination::Battlefield {
            tapped,
            under: None,
            transformed: false,
            counters,
        } if counters.is_empty() => Some(format!(
            "onto the battlefield{}",
            if *tapped { " tapped" } else { "" }
        )),
        Destination::Hand => Some("into your hand".to_owned()),
        Destination::Graveyard => Some("into your graveyard".to_owned()),
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
        [Effect::Search { .. }, ..] => search_sequence(effects, &Ref::You),
        [Effect::RevealUntil { .. }, ..] => super::reveal_until::sequence(effects),
        [Effect::LookAtTop { .. }, ..] => {
            look_sequence(effects).or_else(|| super::among::sequence(effects))
        }
        _ => None,
    }
}

/// The other wordings of a search: "put that card onto the battlefield" for "put it onto the battlefield", and
/// "That land's controller" for "Its controller" when another player searches.
pub fn other_wordings(texts: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for text in texts.iter().filter(|text| text.contains(" library for ")) {
        let that = text.replacen(
            ", put it onto the battlefield",
            ", put that card onto the battlefield",
            1,
        );
        if that != *text {
            found.push(that.clone());
        }
        let buried = text.replacen(
            ", put it into your graveyard",
            ", put that card into your graveyard",
            1,
        );
        if buried != *text {
            found.push(buried);
        }
        // "put those cards", or the put in a sentence of its own, after a count that a where clause names.
        if text.contains(", where X is ") {
            found.push(text.replacen(", put them ", ", put those cards ", 1));
            found.push(text.replacen(", put them ", ". Put those cards ", 1));
        }
        for owner in ["That creature's controller", "That land's controller"] {
            if text.contains("Its controller may search") {
                found.push(text.replacen("Its controller", owner, 1));
                found.push(that.replacen("Its controller", owner, 1));
            }
        }
    }
    found
}

/// What another player may do with their library: "may search their library for ..." after `owner` ("its controller").
pub fn may_search_text(owner: &str, who: &Ref, inner: &[Effect]) -> Option<String> {
    let (text, used) = search_sequence(inner, who)?;
    (who != &Ref::You && used == inner.len()).then(|| format!("{owner} may {text}"))
}

/// A search by `who`, the move of what it finds and the shuffle, or the shuffle and then the move to the top.
fn search_sequence(effects: &[Effect], who: &Ref) -> Option<(String, usize)> {
    let [
        Effect::Search {
            who: searcher,
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
    // The name a single search binds is bookkeeping: a proposal may call the card by its type. A pair needs the
    // conventional name, from which the second is derived.
    let pair = matches!(rest, [Effect::Search { .. }, ..]);
    if (pair && *bind != bound_as(who, filter)) || searcher != who {
        return None;
    }
    // A pair: a second search that leaves out the first card, and a move for each.
    let (looks_for, count_of) = match rest {
        [
            Effect::Search {
                who: second,
                zone: ZoneKind::Library,
                filter: other,
                count: other_count,
                reveal: other_reveal,
                bind: other_bind,
            },
            ..,
        ] if second == who && other_reveal == reveal => (
            pairs::text((filter, count, bind), (other, other_count, other_bind))?,
            2,
        ),
        _ => (what_text(filter, count)?, 1),
    };
    let plural = count_of == 2 || !count.is_one();
    let library = if *who == Ref::You { "your" } else { "their" };
    let mut text = format!("search {library} library for {looks_for}");
    if *reveal {
        text.push_str(if plural {
            ", reveal them"
        } else {
            ", reveal it"
        });
    }
    let found = Ref::Bound(bind.clone());
    let rest = &effects[count_of..];
    match rest {
        [
            Effect::Move { what, to },
            Effect::Shuffle { who: shuffler },
            ..,
        ] if count_of == 1 && *what == found && shuffler == who => {
            let pron = if plural { "them" } else { "it" };
            let _ = write!(text, ", put {pron} {}, then shuffle", destination_text(to)?);
        }
        [
            Effect::Move { what, to },
            Effect::Move {
                what: other,
                to: other_to,
            },
            Effect::Shuffle { who: shuffler },
            ..,
        ] if count_of == 2
            && *what == found
            && *other == Ref::Bound(pairs::second_bind(bind))
            && to == other_to
            && shuffler == who =>
        {
            let _ = write!(text, ", put them {}, then shuffle", destination_text(to)?);
        }
        [
            Effect::Shuffle { who: shuffler },
            Effect::Move {
                what,
                to:
                    Destination::Library {
                        place: Place::Top,
                        order: None,
                    },
            },
            ..,
        ] if count_of == 1 && *what == found && !plural && shuffler == who => {
            text.push_str(", then shuffle and put that card on top");
        }
        _ => return None,
    }
    Some((text, count_of * 2 + 1))
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
            let tail = rest_text(left_to)?;
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
