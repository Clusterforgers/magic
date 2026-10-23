//! Replacement effects that change an event: damage dealt, tokens created, counters put, a card drawn, a card moved
//! to a graveyard or from the battlefield. Written from the data only ("If a source would deal damage to a permanent
//! or player, it deals double that damage to that permanent or player instead."); no template reads them.

use super::amounts::count_word;
use super::counters::kind_text;
use super::triggers::subject_text;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// Whether the event is one this module writes.
pub(super) fn handles(when: &ReplaceWhen) -> bool {
    matches!(
        when,
        ReplaceWhen::Damage { .. }
            | ReplaceWhen::TokenCreation { .. }
            | ReplaceWhen::CountersPut { .. }
            | ReplaceWhen::Draw { .. }
            | ReplaceWhen::ZoneChange { .. }
    )
}

/// The wordings of a replacement of one of the events above, each without its capital and closing period.
pub(super) fn sentences(when: &ReplaceWhen, by: &ReplaceBy) -> Vec<String> {
    match when {
        ReplaceWhen::Damage { to, source, combat } => damage(to.as_ref(), source, *combat, by),
        ReplaceWhen::TokenCreation { controller } => tokens(controller, by),
        ReplaceWhen::CountersPut { on, counter } => counters(on.as_ref(), counter.as_ref(), by),
        ReplaceWhen::Draw { who: Ref::You } => draw(by),
        ReplaceWhen::ZoneChange { what, from, to } => zone_change(what, *from, *to, by),
        _ => Vec::new(),
    }
}

/// A number as a card words it, in digits and in words.
fn numbers(number: u32) -> Vec<String> {
    let mut found = vec![number.to_string()];
    found.extend(count_word(number).map(str::to_owned));
    found
}

/// The recipient of damage: how it is named first, and how "that" names it again.
fn recipient(to: Option<&Ref>) -> Option<(String, &'static str)> {
    Some(match to {
        None => (
            "a permanent or player".to_owned(),
            "that permanent or player",
        ),
        Some(Ref::You) => ("you".to_owned(), "you"),
        Some(Ref::Opponents) => ("an opponent".to_owned(), "that player"),
        Some(Ref::EachPlayer) => ("a player".to_owned(), "that player"),
        Some(Ref::This) => ("~".to_owned(), "it"),
        Some(Ref::Each(what)) => (subject_text(what)?, "it"),
        Some(_) => return None,
    })
}

/// The source of damage as a subject, and whether it is a "source" that "that source" may name again.
fn dealer(source: &Selector) -> Option<(String, bool)> {
    Some(match source.0.as_slice() {
        [] => ("a source".to_owned(), true),
        [Pred::ControlledBy(Ref::You)] => ("a source you control".to_owned(), true),
        [Pred::ControlledBy(Ref::Opponents)] => ("a source an opponent controls".to_owned(), true),
        [Pred::Other, Pred::ControlledBy(Ref::You)] => {
            ("another source you control".to_owned(), true)
        }
        [Pred::Is(Ref::Attached)] => ("enchanted creature".to_owned(), false),
        _ => (subject_text(source)?, false),
    })
}

/// The damage replaced: "damage", "combat damage" or "noncombat damage".
const fn kind(combat: Option<bool>) -> &'static str {
    match combat {
        Some(true) => "combat damage",
        Some(false) => "noncombat damage",
        None => "damage",
    }
}

/// "If a source would deal damage to an opponent, it deals double that damage to that player instead."
fn damage(
    to: Option<&Ref>,
    source: &Selector,
    combat: Option<bool>,
    by: &ReplaceBy,
) -> Vec<String> {
    let (Some((whom, again)), Some((from, sourced))) = (recipient(to), dealer(source)) else {
        return Vec::new();
    };
    let kind = kind(combat);
    let head = format!("if {from} would deal {kind} to {whom}");
    // The replaced event says who deals it, so a prevention may name the damage alone.
    let passive =
        (source.is_empty() && to.is_some()).then(|| format!("if {kind} would be dealt to {whom}"));
    let mut found = Vec::new();
    let mut deals = vec!["it deals".to_owned()];
    if sourced {
        deals.push("that source deals".to_owned());
    }
    match by {
        ReplaceBy::Multiply(2) => {
            for deal in &deals {
                found.push(format!(
                    "{head}, {deal} double that damage to {again} instead"
                ));
                found.push(format!("{head}, {deal} double that damage instead"));
            }
        }
        ReplaceBy::Add(more) if *more > 0 => {
            for deal in &deals {
                for number in numbers(*more) {
                    found.push(format!(
                        "{head}, {deal} that much damage plus {number} instead"
                    ));
                }
            }
        }
        ReplaceBy::Prevent => {
            found.push(format!("{head}, prevent that damage"));
            found.extend(passive.map(|passive| format!("{passive}, prevent that damage")));
        }
        ReplaceBy::PreventUpTo(most) if *most > 0 => {
            for number in numbers(*most) {
                found.push(format!("{head}, prevent {number} of that damage"));
                found.extend(
                    passive
                        .iter()
                        .map(|passive| format!("{passive}, prevent {number} of that damage")),
                );
            }
        }
        _ => {}
    }
    found
}

/// "If an effect would create one or more tokens under your control, it creates twice that many of those tokens
/// instead."
fn tokens(controller: &Ref, by: &ReplaceBy) -> Vec<String> {
    let whose = match controller {
        Ref::You => "your",
        Ref::Opponents => "an opponent's",
        _ => return Vec::new(),
    };
    if *by != ReplaceBy::Multiply(2) {
        return Vec::new();
    }
    vec![
        format!(
            "if an effect would create one or more tokens under {whose} control, it creates twice that many of those tokens instead"
        ),
        format!(
            "if one or more tokens would be created under {whose} control, twice that many of those tokens are created instead"
        ),
    ]
}

/// "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on
/// it instead."
fn counters(
    on: Option<&Ref>,
    counter: Option<&crate::value::CounterKind>,
    by: &ReplaceBy,
) -> Vec<String> {
    let place = match on {
        Some(Ref::Each(what)) => subject_text(what),
        Some(Ref::This) => Some("~".to_owned()),
        _ => None,
    };
    let kind = match counter {
        None => Some(String::new()),
        Some(counter) => kind_text(counter).map(|kind| format!("{kind} ")),
    };
    let (Some(place), Some(kind)) = (place, kind) else {
        return Vec::new();
    };
    // A kind that is not named is "those counters" when the sentence says them again.
    let again = if kind.is_empty() {
        "of those counters".to_owned()
    } else {
        format!("{kind}counters")
    };
    let passive = format!("if one or more {kind}counters would be put on {place}");
    let active = format!("if an effect would put one or more {kind}counters on {place}");
    let mut found = Vec::new();
    let again_on = anaphors(&place);
    match by {
        ReplaceBy::Multiply(2) => {
            for that in &again_on {
                found.push(format!(
                    "{passive}, twice that many {again} are put on {that} instead"
                ));
                found.push(format!(
                    "{active}, it puts twice that many {again} on {that} instead"
                ));
            }
        }
        ReplaceBy::Add(more) if *more > 0 => {
            for (number, that) in numbers(*more)
                .iter()
                .flat_map(|n| again_on.iter().map(move |t| (n, t)))
            {
                found.push(format!(
                    "{passive}, that many plus {number} {kind}counters are put on {that} instead"
                ));
            }
        }
        _ => {}
    }
    found
}

/// How a sentence names the permanent again: "it", "that permanent", "that creature".
fn anaphors(place: &str) -> Vec<String> {
    let noun = place
        .strip_prefix("another ")
        .or_else(|| place.strip_prefix("an "))
        .or_else(|| place.strip_prefix("a "))
        .unwrap_or(place);
    let noun = [
        " you control",
        " an opponent controls",
        " your opponents control",
    ]
    .iter()
    .find_map(|tail| noun.strip_suffix(tail))
    .unwrap_or(noun);
    let noun = if noun.contains(" or ") {
        "permanent"
    } else {
        noun
    };
    vec!["it".to_owned(), format!("that {noun}")]
}

/// "If you would draw a card, draw two cards instead."
fn draw(by: &ReplaceBy) -> Vec<String> {
    match by {
        ReplaceBy::Multiply(2) => {
            vec!["if you would draw a card, draw two cards instead".to_owned()]
        }
        _ => Vec::new(),
    }
}

/// What happens to the object instead of the move.
fn instead(by: &ReplaceBy) -> Vec<&'static str> {
    match by {
        ReplaceBy::MoveTo(ZoneKind::Exile) => vec!["exile it instead", "exile that card instead"],
        ReplaceBy::MoveTo(ZoneKind::Hand) => vec!["return it to its owner's hand instead"],
        ReplaceBy::MoveTo(ZoneKind::Graveyard) => vec!["put it into its owner's graveyard instead"],
        _ => Vec::new(),
    }
}

/// "If a creature you control would die, exile it instead."
fn zone_change(
    what: &Selector,
    from: Option<ZoneKind>,
    to: Option<ZoneKind>,
    by: &ReplaceBy,
) -> Vec<String> {
    let (subject, owner) = moved(what, to);
    let Some(subject) = subject else {
        return Vec::new();
    };
    let events: Vec<String> = match (from, to) {
        (None, Some(ZoneKind::Graveyard)) => {
            vec![format!("be put into {owner} from anywhere")]
        }
        (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard)) => {
            vec![
                "die".to_owned(),
                format!("be put into {owner} from the battlefield"),
            ]
        }
        (Some(ZoneKind::Battlefield), None) => vec!["leave the battlefield".to_owned()],
        (None, Some(ZoneKind::Exile)) => vec!["be exiled".to_owned()],
        _ => Vec::new(),
    };
    events
        .iter()
        .flat_map(|event| {
            instead(by)
                .into_iter()
                .map(move |then| (event.clone(), then))
        })
        .map(|(event, then)| format!("if {subject} would {event}, {then}"))
        .collect()
}

/// The subject of a move and the graveyard it goes to: a selector that is only the owner of a card is "a card" going to
/// "your graveyard" or "an opponent's graveyard", an empty one is "a card or token".
fn moved(what: &Selector, to: Option<ZoneKind>) -> (Option<String>, String) {
    let graveyard = "a graveyard".to_owned();
    if to != Some(ZoneKind::Graveyard) {
        return (subject_text(what), graveyard);
    }
    match what.0.as_slice() {
        [] => (Some("a card or token".to_owned()), graveyard),
        [Pred::OwnedBy(Ref::You)] => (Some("a card".to_owned()), "your graveyard".to_owned()),
        [Pred::OwnedBy(Ref::Opponents)] => (
            Some("a card".to_owned()),
            "an opponent's graveyard".to_owned(),
        ),
        _ => (subject_text(what), graveyard),
    }
}
