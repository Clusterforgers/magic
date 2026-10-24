//! The condition after "as long as" in a static ability: what the permanent itself is ("it's untapped", "~ is
//! monstrous"), the counters on it, a count of cards in a graveyard or a hand ("there are seven or more cards in your
//! graveyard", "there are four or more card types among cards in your graveyard", "you have no cards in hand"), the
//! devotion of its controller, and what they control. Read from the wording and written back the same way.

use mtg_core::card::Supertype;
use mtg_core::color::Color;

use super::amounts::{count_word, number_word};
use super::counters;
use super::facts;
use super::selectors::{Form, color, color_name, phrase, phrase_text};
use super::statics::{holds, holds_texts};
use super::tokens::Reader;
use super::triggers;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition, CounterKind};

/// What a permanent can be that a condition names: "untapped", "tapped", "attacking", "blocking", "monstrous",
/// "legendary" (a color is another).
const STATES: [(&str, Pred); 6] = [
    ("untapped", Pred::Untapped),
    ("tapped", Pred::Tapped),
    ("attacking", Pred::Attacking),
    ("blocking", Pred::Blocking),
    ("monstrous", Pred::Monstrous),
    ("legendary", Pred::Supertype(Supertype::Legendary)),
];

/// "as long as ..." and the condition it states. `it` is what the pronoun "it" stands for, when the wording follows the
/// subject it is about.
pub(super) fn read(r: &mut Reader<'_>, it: Option<&Ref>) -> Option<Condition> {
    r.need("as long as")?;
    if r.words("it's your turn") {
        return Some(Condition::YourTurn);
    }
    body(r, it)
}

/// The conditions of "as long as" and of "if" that state how the game stands, after the words that introduce them.
pub(super) fn body(r: &mut Reader<'_>, it: Option<&Ref>) -> Option<Condition> {
    triggers::monarch_condition(r)
        .or_else(|| r.attempt(|r| state(r, it)))
        .or_else(|| r.attempt(|r| counters_held(r, it)))
        .or_else(|| r.attempt(devotion))
        .or_else(|| r.attempt(cards_counted))
        .or_else(|| r.attempt(controls_none))
        .or_else(|| holds(r))
        .or_else(|| r.attempt(triggers::condition))
}

/// What a condition is about when it names the card itself or the creature it is on, which "it" then names: "As long as
/// ~ is monstrous, it has menace.", "As long as equipped creature is legendary, it has hexproof."
pub(super) fn subject_of(condition: &Condition) -> Option<Ref> {
    let named = match condition {
        Condition::Matches { subject, .. } => subject,
        Condition::Compare {
            left: Amount::Counters { on, .. },
            ..
        } => on,
        _ => return None,
    };
    matches!(named, Ref::This | Ref::Attached).then(|| named.clone())
}

/// A number written as digits or as a word.
fn quantity(r: &mut Reader<'_>) -> Option<u32> {
    r.number()
        .or_else(|| r.attempt(|r| number_word(r.take_word()?)))
}

/// A number the way a card writes it: a word up to ten, else digits.
fn quantity_text(number: u32) -> String {
    count_word(number)
        .filter(|_| number <= 10)
        .map_or_else(|| number.to_string(), str::to_owned)
}

/// The thing a condition is about: the card ("~"), the creature an Aura or Equipment is on, or "it".
fn subject(r: &mut Reader<'_>, it: Option<&Ref>) -> Option<Ref> {
    if r.source() {
        Some(Ref::This)
    } else if r.words("equipped creature") || r.words("enchanted creature") {
        Some(Ref::Attached)
    } else if r.word("it") {
        it.cloned()
    } else {
        None
    }
}

/// "~ is untapped", "it's attacking", "equipped creature is legendary", "it's red".
fn state(r: &mut Reader<'_>, it: Option<&Ref>) -> Option<Condition> {
    let subject = if r.words("it's") {
        it.cloned()?
    } else {
        let subject = subject(r, it)?;
        r.need("is")?;
        subject
    };
    let word = r.take_word()?;
    let pred = STATES
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, pred)| pred.clone())
        .or_else(|| color(word).map(Pred::Color))?;
    Some(Condition::Matches {
        subject,
        selector: Selector(vec![pred]),
    })
}

/// Whether the counters have a block of their own that writes the condition (`stations.rs`, `levels.rs`).
fn written_elsewhere(counter: &CounterKind) -> bool {
    *counter == CounterKind::Charge || counter.name() == "level"
}

/// "~ has five or more quest counters on it", "it has a slumber counter on it".
fn counters_held(r: &mut Reader<'_>, it: Option<&Ref>) -> Option<Condition> {
    let on = subject(r, it)?;
    r.need("has")?;
    let number = if r.word("a") || r.word("an") {
        1
    } else {
        let number = quantity(r)?;
        r.need("or more")?;
        number
    };
    // The charge counters of a Spacecraft and the level counters of a leveler are read with their blocks.
    let counter = counters::kind(r).filter(|counter| !written_elsewhere(counter))?;
    if !(r.words("counters on it") || r.words("counter on it")) {
        return None;
    }
    Some(Condition::Compare {
        left: Amount::Counters { on, counter },
        op: Cmp::Ge,
        right: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// "your devotion to white is less than five", "your devotion to green and white is less than seven".
fn devotion(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("your devotion to")?;
    let mut colors = vec![color(r.take_word()?)?];
    while r.word("and") {
        colors.push(color(r.take_word()?)?);
    }
    r.need("is less than")?;
    let number = quantity(r)?;
    Some(Condition::Compare {
        left: Amount::Devotion(colors),
        op: Cmp::Lt,
        right: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// The cards in a hand: "cards in hand" are the cards in the hand of the player "you" names.
fn hand() -> Selector {
    Selector(vec![Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)])
}

/// How many cards there are, or how many types among them: "there are seven or more cards in your graveyard", "there
/// are fewer than eight cards in your graveyard", "there are four or more card types among cards in your graveyard",
/// "you have no cards in hand", "you have one or fewer cards in hand".
fn cards_counted(r: &mut Reader<'_>) -> Option<Condition> {
    if r.words("you have") {
        return in_hand(r);
    }
    r.need("there are")?;
    if r.words("fewer than") {
        let number = quantity(r)?;
        return Some(compare(Amount::Count(graveyard(r)?), Cmp::Lt, number));
    }
    let number = quantity(r)?;
    r.need("or more")?;
    if r.words("card types among") {
        return Some(compare(Amount::CardTypes(graveyard(r)?), Cmp::Ge, number));
    }
    Some(compare(Amount::Count(graveyard(r)?), Cmp::Ge, number))
}

/// "no cards in hand", "seven or more cards in hand", "one or fewer cards in hand" after "you have".
fn in_hand(r: &mut Reader<'_>) -> Option<Condition> {
    if r.words("no cards in hand") {
        return Some(compare(Amount::Count(hand()), Cmp::Eq, 0));
    }
    let number = quantity(r)?;
    let op = if r.words("or more") {
        Cmp::Ge
    } else if r.words("or fewer") {
        Cmp::Le
    } else {
        return None;
    };
    r.need("cards in hand")?;
    Some(compare(Amount::Count(hand()), op, number))
}

/// The cards in the graveyard of the player "you" names.
fn graveyard_cards() -> Selector {
    Selector(vec![Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)])
}

/// "cards in your graveyard".
fn graveyard(r: &mut Reader<'_>) -> Option<Selector> {
    r.words("cards in your graveyard").then(graveyard_cards)
}

/// A count of the amount compared with a number.
fn compare(left: Amount, op: Cmp, number: u32) -> Condition {
    Condition::Compare {
        left,
        op,
        right: Amount::Fixed(i32::try_from(number).unwrap_or(i32::MAX)),
    }
}

/// "you control no other creatures": nothing of a kind is controlled.
fn controls_none(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("you control no")?;
    let mut preds = phrase(r, Form::All)?.0;
    if preds
        .iter()
        .any(|pred| matches!(pred, Pred::ControlledBy(_)))
    {
        return None;
    }
    preds.push(Pred::ControlledBy(Ref::You));
    Some(compare(Amount::Count(Selector(preds)), Cmp::Eq, 0))
}

/// What `read` reads, written back after "as long as", one text for each wording.
pub(super) fn texts(condition: &Condition) -> Vec<String> {
    if *condition == Condition::YourTurn {
        return vec!["it's your turn".to_owned()];
    }
    let own = [
        state_texts(condition),
        counters_texts(condition),
        devotion_texts(condition),
        counted_texts(condition),
        none_texts(condition),
        facts::texts(condition),
    ]
    .concat();
    if !own.is_empty() {
        own
    } else if let Some(written) = triggers::condition_text(condition) {
        vec![written]
    } else {
        holds_texts(condition)
    }
}

/// The wordings of the thing a condition is about, for the verb ("is", "has"): "~ is", "it's" for the card itself.
fn subject_texts(subject: &Ref, verb: &str) -> Vec<String> {
    let contracted = |name: &str| {
        if verb == "is" {
            format!("{name}'s")
        } else {
            format!("{name} {verb}")
        }
    };
    match subject {
        Ref::This => vec![format!("~ {verb}"), contracted("it")],
        Ref::Attached => vec![
            format!("equipped creature {verb}"),
            format!("enchanted creature {verb}"),
            contracted("it"),
        ],
        _ => Vec::new(),
    }
}

/// "~ is untapped", "it's legendary", "equipped creature is red".
fn state_texts(condition: &Condition) -> Vec<String> {
    let Condition::Matches { subject, selector } = condition else {
        return Vec::new();
    };
    let [pred] = selector.0.as_slice() else {
        return Vec::new();
    };
    let name = match pred {
        Pred::Color(shade) => color_name(*shade).to_owned(),
        other => match STATES.iter().find(|(_, state)| state == other) {
            Some((name, _)) => (*name).to_owned(),
            None => return Vec::new(),
        },
    };
    subject_texts(subject, "is")
        .into_iter()
        .map(|start| format!("{start} {name}"))
        .collect()
}

/// "~ has five or more quest counters on it", "it has a slumber counter on it".
fn counters_texts(condition: &Condition) -> Vec<String> {
    let Condition::Compare {
        left: Amount::Counters { on, counter },
        op: Cmp::Ge,
        right: Amount::Fixed(number),
    } = condition
    else {
        return Vec::new();
    };
    let (Some(kind), Ok(number)) = (counters::kind_text(counter), u32::try_from(*number)) else {
        return Vec::new();
    };
    if written_elsewhere(counter) {
        return Vec::new();
    }
    // A player holds counters without "on it": "defending player has a rad counter".
    if *on == Ref::Defending {
        let held = if number == 1 {
            format!("{} {kind} counter", super::triggers::article(kind))
        } else {
            format!("{} or more {kind} counters", quantity_text(number))
        };
        return vec![format!("defending player has {held}")];
    }
    let held = if number == 1 {
        format!("{} {kind} counter on it", super::triggers::article(kind))
    } else {
        format!("{} or more {kind} counters on it", quantity_text(number))
    };
    subject_texts(on, "has")
        .into_iter()
        .map(|start| format!("{start} {held}"))
        .collect()
}

/// "your devotion to white and black is less than seven".
fn devotion_texts(condition: &Condition) -> Vec<String> {
    let Condition::Compare {
        left: Amount::Devotion(colors),
        op: Cmp::Lt,
        right: Amount::Fixed(number),
    } = condition
    else {
        return Vec::new();
    };
    let Ok(number) = u32::try_from(*number) else {
        return Vec::new();
    };
    let names: Vec<&str> = colors
        .iter()
        .map(|shade: &Color| color_name(*shade))
        .collect();
    vec![format!(
        "your devotion to {} is less than {}",
        names.join(" and "),
        quantity_text(number)
    )]
}

/// The counts of cards in a graveyard or a hand, with the number as a word and as digits.
fn counted_texts(condition: &Condition) -> Vec<String> {
    let Condition::Compare {
        right: Amount::Fixed(number),
        ..
    } = condition
    else {
        return Vec::new();
    };
    let Ok(number) = u32::try_from(*number) else {
        return Vec::new();
    };
    facts::quantity_texts(number)
        .iter()
        .flat_map(|words| counted_text(condition, number, words))
        .collect()
}

/// The counts of cards in a graveyard or a hand, for one wording `words` of `number`.
fn counted_text(condition: &Condition, number: u32, words: &str) -> Vec<String> {
    let Condition::Compare { left, op, .. } = condition else {
        return Vec::new();
    };
    match (left, op) {
        (Amount::CardTypes(of), Cmp::Ge) if *of == graveyard_cards() => vec![format!(
            "there are {words} or more card types among cards in your graveyard"
        )],
        (Amount::Count(of), Cmp::Ge) if *of == graveyard_cards() => {
            vec![format!("there are {words} or more cards in your graveyard")]
        }
        (Amount::Count(of), Cmp::Lt) if *of == graveyard_cards() => {
            vec![format!(
                "there are fewer than {words} cards in your graveyard"
            )]
        }
        (Amount::Count(of), _) if *of == hand() => match op {
            Cmp::Eq if number == 0 => vec!["you have no cards in hand".to_owned()],
            Cmp::Ge => vec![format!("you have {words} or more cards in hand")],
            Cmp::Le => vec![format!("you have {words} or fewer cards in hand")],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// "you control no other creatures".
fn none_texts(condition: &Condition) -> Vec<String> {
    let Condition::Compare {
        left: Amount::Count(Selector(preds)),
        op: Cmp::Eq,
        right: Amount::Fixed(0),
    } = condition
    else {
        return Vec::new();
    };
    let mut rest = preds.clone();
    let Some(at) = rest
        .iter()
        .position(|pred| *pred == Pred::ControlledBy(Ref::You))
    else {
        return Vec::new();
    };
    rest.remove(at);
    phrase_text(&Selector(rest), Form::All)
        .map(|group| format!("you control no {group}"))
        .into_iter()
        .collect()
}
