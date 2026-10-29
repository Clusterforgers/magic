//! The top cards of the library looked at or revealed, and every card a player takes "from among them": one or several
//! takes ("You may put a land card from among them onto the battlefield tapped. You may put a permanent card from among
//! them into your hand."), the top counted by X, the cards named by a mixed list ("a Human or artifact card"), by
//! "historic", "non-Human" or "that shares a creature type with that creature", the rest sent to the hand, the
//! graveyard, the bottom or shuffled into the library. Read from the wording and written back the same way; the other
//! wordings of a sentence are listed by [`w20a_wordings`].

use std::fmt::Write as _;

use mtg_core::card::Subtype;

use super::amounts::count_word;
use super::defining;
use super::library::{KEPT, TOP, filter, filter_text, rest_destination, rest_text};
use super::selectors::{Form, adjective, adjective_text, card_type, subtype, type_name};
use super::tokens::Reader;
use super::triggers::article;
use crate::effect::{Destination, Effect, Order, Place};
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, CounterKind, StatKind};

/// The cards looked at.
fn w20a_top() -> Ref {
    Ref::Bound(TOP.to_owned())
}

/// The name the cards of the take number `index` are bound to.
fn w20a_bind(index: usize) -> String {
    if index == 0 {
        KEPT.to_owned()
    } else {
        format!("{KEPT}_{}", index + 1)
    }
}

/// The cards of the library named `TOP` that match a filter.
fn w20a_among(filter: Selector) -> Selector {
    let mut preds = vec![Pred::In(ZoneKind::Library), Pred::Is(w20a_top())];
    preds.extend(filter.0);
    Selector(preds)
}

/// The filter of a selector made by `w20a_among`.
fn w20a_filter_of(selector: &Selector) -> Option<Selector> {
    match selector.0.as_slice() {
        [
            Pred::In(ZoneKind::Library),
            Pred::Is(Ref::Bound(name)),
            rest @ ..,
        ] if name == TOP => Some(Selector(rest.to_vec())),
        _ => None,
    }
}

/// "the top six cards" or "the top X cards".
fn w20a_top_cards(r: &mut Reader<'_>) -> Option<Amount> {
    let counted = r.attempt(|r| {
        r.need("the top")?;
        r.x().then_some(())?;
        r.word("cards").then_some(())
    });
    if counted.is_some() {
        return Some(Amount::X);
    }
    super::library::top_cards(r)
}

/// The words of `w20a_top_cards`.
fn w20a_top_text(count: &Amount) -> Option<String> {
    if *count == Amount::X {
        return Some("the top X cards".to_owned());
    }
    super::library::top_text(count)
}

/// An adjective before the card type that the common reader does not know: "historic" and "non-Human".
fn w20a_adjective(r: &mut Reader<'_>) -> Option<Pred> {
    let word = r.peek_word()?;
    let pred = if word == "historic" {
        adjective(word, Form::Each)?
    } else {
        let name = word.strip_prefix("non-")?;
        let mut chars = name.chars();
        let name = format!("{}{}", chars.next()?.to_uppercase(), chars.as_str());
        r.is_subtype(&name)
            .then(|| Pred::Not(Box::new(Pred::Subtype(Subtype(name)))))?
    };
    r.skip();
    Some(pred)
}

/// "Human or artifact", "Knight, Equipment, or artifact": subtypes and card types in one list.
fn w20a_mixed(r: &mut Reader<'_>) -> Option<Pred> {
    fn item(r: &mut Reader<'_>) -> Option<Pred> {
        if let Some(kind) = r.peek_word().and_then(card_type) {
            r.skip();
            return Some(Pred::Type(kind));
        }
        subtype(r).map(|name| Pred::Subtype(Subtype(name)))
    }
    let mut preds = vec![item(r)?];
    loop {
        let more = r.attempt(|r| {
            let comma = r.punct(',');
            r.word("or").then_some(())?;
            item(r).map(|pred| (comma, pred))
        });
        let Some((comma, pred)) = more else { break };
        preds.push(pred);
        if comma != (preds.len() > 2) {
            return None;
        }
    }
    let types = preds.iter().filter(|pred| matches!(pred, Pred::Type(_)));
    let names = preds.iter().filter(|pred| matches!(pred, Pred::Subtype(_)));
    (preds.len() > 1 && types.count() > 0 && names.count() > 0).then_some(Pred::AnyOf(preds))
}

/// What follows the noun: "with mana value less than or equal to the number of lands you control" and "that shares a
/// creature type with that creature".
fn w20a_suffix(r: &mut Reader<'_>) -> Option<Pred> {
    if r.words("with mana value less than or equal to") {
        let to = defining::value(r)?;
        return Some(Pred::Compare {
            stat: StatKind::ManaValue,
            op: Cmp::Le,
            to,
        });
    }
    // The creature of the event, so only a trigger has one to name.
    r.triggered.then_some(())?;
    r.need("that shares a creature type with that creature")?;
    Some(Pred::SharesSubtype(Ref::Triggering))
}

/// The cards a take names, as a filter: the common one, or with the additions of this module.
fn w20a_card_filter(r: &mut Reader<'_>, plural: bool) -> Option<Selector> {
    let mut preds = Vec::new();
    while let Some(pred) = w20a_adjective(r) {
        preds.push(pred);
    }
    let base = r.attempt(|r| filter(r, plural)).or_else(|| {
        let mixed = r.attempt(w20a_mixed)?;
        r.word(if plural { "cards" } else { "card" })
            .then_some(Selector(vec![mixed]))
    })?;
    preds.extend(base.0);
    if let Some(extra) = r.attempt(w20a_suffix) {
        preds.push(extra);
    }
    Some(Selector(preds))
}

/// "a land card", "up to one Elf card", "up to two land cards", "any number of permanent cards".
fn w20a_what(r: &mut Reader<'_>) -> Option<(Selector, Bounds)> {
    let (bounds, plural) = if r.words("up to one") {
        (Bounds::up_to(Some(Amount::one())), false)
    } else if r.words("up to") {
        let number = super::amounts::number_word(r.take_word()?).filter(|number| *number > 1)?;
        (
            Bounds::up_to(Some(Amount::Fixed(i32::try_from(number).ok()?))),
            true,
        )
    } else if r.words("any number of") {
        (Bounds::up_to(None), true)
    } else if r.word("a") || r.word("an") {
        (Bounds::default(), false)
    } else {
        return None;
    };
    Some((w20a_card_filter(r, plural)?, bounds))
}

/// Whether the bounds name one card at most.
fn w20a_single(bounds: &Bounds) -> bool {
    bounds.is_one() || *bounds == Bounds::up_to(Some(Amount::one()))
}

/// The filter as a card words it: the common writer, with "historic", "non-Human", a mixed list and the suffixes of this
/// module around it.
fn w20a_filter_text(filter: &Selector, plural: bool) -> Option<String> {
    let (mut front, mut back, mut kept, mut mixed) = (Vec::new(), Vec::new(), Vec::new(), None);
    for pred in &filter.0 {
        match pred {
            Pred::Not(inner) => match &**inner {
                Pred::Subtype(Subtype(name)) => front.push(format!("non-{name}")),
                _ => kept.push(pred.clone()),
            },
            Pred::Compare {
                stat: StatKind::ManaValue,
                op: Cmp::Le,
                to,
            } if !matches!(to, Amount::Fixed(_) | Amount::X) => back.push(format!(
                "with mana value less than or equal to {}",
                defining::value_text(to)?
            )),
            Pred::SharesSubtype(Ref::Triggering) => {
                back.push("that shares a creature type with that creature".to_owned());
            }
            Pred::AnyOf(list) if w20a_is_mixed(list) => mixed = Some(w20a_mixed_text(list)?),
            other if adjective_text(other, Form::Each).as_deref() == Some("historic") => {
                front.push("historic".to_owned());
            }
            other => kept.push(other.clone()),
        }
    }
    let base = match mixed {
        Some(text) => {
            kept.is_empty().then_some(())?;
            format!("{text} {}", if plural { "cards" } else { "card" })
        }
        None => filter_text(&Selector(kept), plural)?,
    };
    front.push(base);
    front.extend(back);
    Some(front.join(" "))
}

/// Whether a list of predicates holds both subtypes and card types.
fn w20a_is_mixed(list: &[Pred]) -> bool {
    list.iter().any(|pred| matches!(pred, Pred::Type(_)))
        && list.iter().any(|pred| matches!(pred, Pred::Subtype(_)))
        && list
            .iter()
            .all(|pred| matches!(pred, Pred::Type(_) | Pred::Subtype(_)))
}

/// "Human or artifact", "Knight, Equipment, or artifact".
fn w20a_mixed_text(list: &[Pred]) -> Option<String> {
    let names: Option<Vec<String>> = list
        .iter()
        .map(|pred| match pred {
            Pred::Type(kind) => Some(type_name(*kind).to_owned()),
            Pred::Subtype(Subtype(name)) => Some(name.clone()),
            _ => None,
        })
        .collect();
    Some(match names?.as_slice() {
        [one, two] => format!("{one} or {two}"),
        [front @ .., end] if !front.is_empty() => format!("{}, or {end}", front.join(", ")),
        _ => return None,
    })
}

/// "into your hand", "onto the battlefield", "onto the battlefield tapped", "onto the battlefield with an indestructible
/// counter on it".
fn w20a_destination(r: &mut Reader<'_>) -> Option<Destination> {
    if r.words("into your hand") {
        return Some(Destination::Hand);
    }
    r.need("onto the battlefield")?;
    let tapped = r.word("tapped");
    let counter = r.attempt(|r| {
        (r.words("with a") || r.words("with an")).then_some(())?;
        let name = r.take_word()?.to_owned();
        r.need("counter on it")?;
        Some((CounterKind::Other(name), 1))
    });
    Some(Destination::Battlefield {
        tapped,
        under: None,
        transformed: false,
        counters: counter.into_iter().collect(),
    })
}

/// The words of `w20a_destination`.
fn w20a_destination_text(to: &Destination) -> Option<String> {
    match to {
        Destination::Hand => Some("into your hand".to_owned()),
        Destination::Battlefield {
            tapped,
            under: None,
            transformed: false,
            counters,
        } => {
            let mut text = "onto the battlefield".to_owned();
            if *tapped {
                text.push_str(" tapped");
            }
            match counters.as_slice() {
                [] => {}
                [(CounterKind::Other(name), 1)] => {
                    let _ = write!(text, " with {} {name} counter on it", article(name));
                }
                _ => return None,
            }
            Some(text)
        }
        _ => None,
    }
}

/// Where the rest goes: the common destinations, the hand and the bottom worded without "of your library".
fn w20a_rest(r: &mut Reader<'_>) -> Option<Destination> {
    if r.words("into your hand") {
        return Some(Destination::Hand);
    }
    if r.words("on the bottom in a random order") {
        return Some(Destination::Library {
            place: Place::Bottom,
            order: Some(Order::Random),
        });
    }
    rest_destination(r)
}

/// The words of `w20a_rest`.
fn w20a_rest_text(to: &Destination) -> Option<String> {
    match to {
        Destination::Hand => Some("into your hand".to_owned()),
        other => rest_text(other).map(str::to_owned),
    }
}

/// The sentence that sends the rest away: "Put the rest on the bottom of your library in a random order", "Then put the
/// rest of the cards revealed this way ...", "Then put all cards revealed this way that weren't put onto the battlefield
/// into your graveyard" and "Then shuffle the rest into your library".
fn w20a_rest_sentence(r: &mut Reader<'_>) -> Option<Destination> {
    r.word("then");
    if r.words("shuffle the rest into your library") {
        return Some(Destination::Library {
            place: Place::Shuffled,
            order: None,
        });
    }
    r.need("put")?;
    if r.words("all cards revealed this way that weren't put onto the battlefield") {
        r.need("into your graveyard")?;
        return Some(Destination::Graveyard);
    }
    r.need("the rest")?;
    let _ = r.words("of the cards revealed this way") || r.words("of those cards");
    w20a_rest(r)
}

/// One take: the filter, the count, whether the cards are revealed, and where they go.
struct W20aTake {
    reveal: bool,
    filter: Selector,
    bounds: Bounds,
    to: Destination,
}

/// "it" or "that card" for one card, "them" or "those cards" for several.
fn w20a_pronoun(r: &mut Reader<'_>, plural: bool) -> Option<()> {
    let found = if plural {
        r.word("them") || r.words("those cards")
    } else {
        r.word("it") || r.words("that card")
    };
    found.then_some(())
}

/// One take of a sentence: "reveal a creature card from among them and put it into your hand", "put a land card from
/// among them onto the battlefield tapped", and after "and" "up to one Elf card from among them into your hand".
fn w20a_phrase(r: &mut Reader<'_>, may: bool, first: bool) -> Option<W20aTake> {
    let reveal = first && may && r.word("reveal");
    if first && !reveal {
        r.need("put")?;
    }
    let (filter, bounds) = w20a_what(r)?;
    r.need("from among them")?;
    let to = if reveal {
        r.need("and put")?;
        w20a_pronoun(r, !w20a_single(&bounds))?;
        w20a_destination(r)?
    } else {
        w20a_destination(r)?
    };
    let bounds = if may {
        Bounds {
            min: Amount::Fixed(0),
            ..bounds
        }
    } else {
        bounds
    };
    Some(W20aTake {
        reveal,
        filter,
        bounds,
        to,
    })
}

/// The effects of the takes of one sentence, numbered from `taken`.
fn w20a_effects(items: Vec<W20aTake>, before: usize) -> Vec<Effect> {
    let mut effects = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let bind = w20a_bind(before + index);
        effects.push(Effect::Choose {
            who: Ref::You,
            from: Ref::Each(w20a_among(item.filter)),
            count: item.bounds,
            bind: bind.clone(),
            rest_as: None,
        });
        if item.reveal {
            effects.push(Effect::Reveal {
                what: Ref::Bound(bind.clone()),
            });
        }
        effects.push(Effect::Move {
            what: Ref::Bound(bind),
            to: item.to,
        });
    }
    effects
}

/// A sentence of takes, with the destination of the rest when the sentence names it ("... and the rest into your hand").
fn w20a_sentence(
    r: &mut Reader<'_>,
    before: usize,
) -> Option<(Vec<Effect>, usize, Option<Destination>)> {
    let may = r.words("you may");
    let mut items = vec![w20a_phrase(r, may, true)?];
    while let Some(next) = r.attempt(|r| {
        r.need("and")?;
        w20a_phrase(r, may, false)
    }) {
        items.push(next);
    }
    let rest = r.attempt(|r| {
        r.need("and the rest")?;
        w20a_rest(r)
    });
    if rest.is_none() {
        r.punct('.').then_some(())?;
    }
    let count = items.len();
    Some((w20a_effects(items, before), count, rest))
}

/// "Look at the top six cards of your library. You may reveal a Human or artifact card from among them and put it into your
/// hand. Put the rest on the bottom of your library in a random order.", and the other shapes of the module.
pub fn w20a_look_pick(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let looked = r.words("look at");
    if !looked {
        r.need("reveal")?;
    }
    let count = w20a_top_cards(r)?;
    r.need("of your library")?;
    r.punct('.').then_some(())?;
    let mut effects = vec![Effect::LookAtTop {
        who: Ref::You,
        count,
        bind: TOP.to_owned(),
    }];
    if !looked {
        effects.push(Effect::Reveal { what: w20a_top() });
    }
    let mut taken = 0;
    let mut rest = None;
    while rest.is_none() {
        let Some((made, count, named)) = r.attempt(|r| w20a_sentence(r, taken)) else {
            break;
        };
        effects.extend(made);
        taken += count;
        rest = named;
    }
    (taken > 0).then_some(())?;
    let to = match rest {
        Some(to) => to,
        None => w20a_rest_sentence(r)?,
    };
    effects.push(Effect::Move {
        what: w20a_top(),
        to,
    });
    Some(effects)
}

/// The words of one take: "You may reveal a creature card from among them and put it into your hand".
fn w20a_take_text(
    bounds: &Bounds,
    filter: &Selector,
    to: &Destination,
    revealed: bool,
    joined: bool,
) -> Option<String> {
    let may = bounds.fixed_min() == Some(0);
    let single = w20a_single(bounds);
    let plural = !single;
    let cards = if single && !bounds.is_one() && joined {
        let text = w20a_filter_text(filter, false)?;
        format!("up to one {text}")
    } else if single {
        let text = w20a_filter_text(filter, false)?;
        format!("{} {text}", article(&text))
    } else {
        let text = w20a_filter_text(filter, true)?;
        match (&bounds.min, &bounds.max) {
            (Amount::Fixed(0), None) => format!("any number of {text}"),
            (Amount::Fixed(0), Some(Amount::Fixed(most))) if *most > 1 => {
                format!("up to {} {text}", count_word(u32::try_from(*most).ok()?)?)
            }
            _ => return None,
        }
    };
    if revealed {
        let pronoun = if plural { "them" } else { "it" };
        return Some(format!(
            "You may reveal {cards} from among them and put {pronoun} {}",
            w20a_destination_text(to)?
        ));
    }
    let lead = if may && single && !joined {
        "You may put"
    } else {
        "Put"
    };
    Some(format!(
        "{lead} {cards} from among them {}",
        w20a_destination_text(to)?
    ))
}

/// The sentences the effects at the front of the list are when they are one of the shapes of the module, and how many
/// effects they take.
pub fn w20a_sequence(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::LookAtTop {
            who: Ref::You,
            count,
            bind,
        },
        tail @ ..,
    ] = effects
    else {
        return None;
    };
    (bind == TOP).then_some(())?;
    let (verb, mut tail, mut used) = match tail {
        [Effect::Reveal { what }, tail @ ..] if *what == w20a_top() => ("reveal", tail, 2),
        _ => ("look at", tail, 1),
    };
    let mut text = format!("{verb} {} of your library", w20a_top_text(count)?);
    let mut taken = 0;
    while let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(selector),
            count: bounds,
            bind,
            rest_as: None,
        },
        after @ ..,
    ] = tail
    {
        (*bind == w20a_bind(taken)).then_some(())?;
        let (revealed, after) = match after {
            [Effect::Reveal { what }, after @ ..] if *what == Ref::Bound(bind.clone()) => {
                (true, after)
            }
            _ => (false, after),
        };
        let [
            Effect::Move {
                what: Ref::Bound(moved),
                to,
            },
            after @ ..,
        ] = after
        else {
            return None;
        };
        (moved == bind).then_some(())?;
        let sentence = w20a_take_text(bounds, &w20a_filter_of(selector)?, to, revealed, false)?;
        text.push_str(". ");
        text.push_str(&sentence);
        used += 2 + usize::from(revealed);
        taken += 1;
        tail = after;
    }
    (taken > 0).then_some(())?;
    let [
        Effect::Move {
            what: Ref::Bound(left),
            to,
        },
        ..,
    ] = tail
    else {
        return None;
    };
    (left == TOP).then_some(())?;
    let rest = match to {
        Destination::Library {
            place: Place::Shuffled,
            order: None,
        } => "Then shuffle the rest into your library".to_owned(),
        other => format!("Put the rest {}", w20a_rest_text(other)?),
    };
    Some((format!("{text}. {rest}"), used + 1))
}

/// "You may put up to one land card from among them" for "You may put a land card from among them".
fn w20a_up_to_one(text: &str) -> Option<String> {
    let at = text.find("You may put a")?;
    let after = &text[at + "You may put a".len()..];
    let after = after.strip_prefix('n').unwrap_or(after);
    Some(format!("{}You may put up to one{after}", &text[..at]))
}

/// The last two takes in one sentence: "..., and up to one Elf card from among them into your hand".
fn w20a_joined(text: &str) -> Option<String> {
    const LEAD: &str = ". You may put ";
    (text.matches(LEAD).count() >= 2).then_some(())?;
    let at = text.rfind(LEAD)?;
    let after = &text[at + LEAD.len()..];
    let after = after
        .strip_prefix("an ")
        .or_else(|| after.strip_prefix("a "))?;
    Some(format!("{} and up to one {after}", &text[..at]))
}

/// The other wordings of the sentences of this module:the rest worded "of the cards revealed this way" or "of those
/// cards", "on the bottom" without "of your library", the cards that were not put onto the battlefield, the rest in the
/// sentence of the last take, "any number" under "you may", and two takes in one sentence.
pub fn w20a_wordings(texts: &[String]) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    for text in texts
        .iter()
        .filter(|text| text.contains(" from among them "))
    {
        let mut variants = vec![text.clone()];
        let steps: [fn(&str) -> Option<String>; 8] = [
            |t| {
                Some(t.replacen(
                    ". Put the rest ",
                    ". Then put the rest of the cards revealed this way ",
                    1,
                ))
            },
            |t| Some(t.replacen(". Put the rest ", ". Put the rest of those cards ", 1)),
            |t| {
                Some(t.replacen(
                    " on the bottom of your library in ",
                    " on the bottom in ",
                    1,
                ))
            },
            |t| {
                t.contains("onto the battlefield").then(|| {
                    t.replacen(
                        ". Put the rest into your graveyard",
                        ". Then put all cards revealed this way that weren't put onto the battlefield into your graveyard",
                        1,
                    )
                })
            },
            |t| Some(t.replacen(". Put the rest ", " and the rest ", 1)),
            |t| Some(t.replacen(". Put any number of ", ". You may put any number of ", 1)),
            w20a_up_to_one,
            w20a_joined,
        ];
        for step in steps {
            let made: Vec<String> = variants
                .iter()
                .filter_map(|variant| step(variant))
                .filter(|made| !variants.contains(made))
                .collect();
            variants.extend(made);
        }
        all.extend(variants.into_iter().skip(1));
    }
    all
}
