//! Keywords given to the cards of a hand, a graveyard or exile: "Each creature card in your hand has cycling {1}{U}.",
//! "Nonland cards in your hand have miracle {0}.", "Merfolk cards in your graveyard have retrace." A `Static` over the
//! cards a selector names in that zone whose changes are the keywords; the layers hold the cards of the zone like
//! permanents (`docs/engine.md`, Continuous effects).

use super::selectors::{Form, phrase, phrase_text};
use super::statics::statics;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::compile::keywords;
use crate::cost::{Cost, CostItem};
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, ZoneKind};

/// The keywords a card in these zones can be given, as the words of the card: those with a mana cost and retrace.
const GIVEN: [&str; 8] = [
    "cycling",
    "miracle",
    "flashback",
    "madness",
    "unearth",
    "encore",
    "scavenge",
    "retrace",
];

/// Whether the selector names cards of a hand, a graveyard or exile.
pub(super) fn names_cards(preds: &[Pred]) -> bool {
    preds.iter().any(|pred| {
        matches!(
            pred,
            Pred::In(ZoneKind::Hand | ZoneKind::Graveyard | ZoneKind::Exile)
        )
    })
}

/// "Each creature card in your hand has cycling {1}{U}.", "Nonland cards in your hand have miracle {0}."
pub(super) fn ability(r: &mut Reader<'_>) -> Option<Ability> {
    let each = r.word("each");
    let cards = phrase(r, if each { Form::Count } else { Form::All })?;
    names_cards(&cards.0).then_some(())?;
    r.need(if each { "has" } else { "have" })?;
    let keyword = costed(r)?;
    Some(statics(
        Ref::Each(cards),
        vec![Modification::AddKeyword(keyword)],
        None,
    ))
}

/// A keyword of `GIVEN` with its cost: "cycling {1}{U}", "retrace".
fn costed(r: &mut Reader<'_>) -> Option<Keyword> {
    let name = GIVEN.iter().find(|name| r.peek_word() == Some(**name))?;
    r.skip();
    if *name == "retrace" {
        return keywords::read(name);
    }
    let mut mana = String::new();
    while let Some(symbol) = r.attempt(|r| {
        r.symbol()
            .filter(|symbol| !matches!(*symbol, "T" | "Q" | "X"))
    }) {
        mana = format!("{mana}{{{symbol}}}");
    }
    (!mana.is_empty()).then_some(())?;
    keywords::read(&format!("{name} {mana}"))
}

/// Every wording of the ability `ability` reads.
pub(super) fn texts(affected: &Ref, mods: &[Modification]) -> Vec<String> {
    let Ref::Each(selector) = affected else {
        return Vec::new();
    };
    let [Modification::AddKeyword(keyword)] = mods else {
        return Vec::new();
    };
    let Some(given) = given_text(keyword) else {
        return Vec::new();
    };
    let each = phrase_text(selector, Form::Count).map(|cards| format!("Each {cards} has {given}"));
    let all = phrase_text(selector, Form::All)
        .map(|cards| format!("{} have {given}", cards.replace(" from your ", " in your ")));
    each.into_iter().chain(all).collect()
}

/// The keyword as `costed` reads it.
fn given_text(keyword: &Keyword) -> Option<String> {
    let name = keyword.name().to_lowercase();
    if !GIVEN.contains(&name.as_str()) {
        return None;
    }
    match keyword {
        Keyword::Retrace => Some(name),
        Keyword::Cycling(cost)
        | Keyword::Miracle(cost)
        | Keyword::Flashback(cost)
        | Keyword::Madness(cost)
        | Keyword::Unearth(cost)
        | Keyword::Encore(cost)
        | Keyword::Scavenge(cost) => match cost {
            Cost(items) => match items.as_slice() {
                [CostItem::Mana(mana)] => Some(format!("{name} {mana}")),
                _ => None,
            },
        },
        _ => None,
    }
}
