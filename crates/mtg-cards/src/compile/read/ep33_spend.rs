//! Restricted mana and votes of ep33: mana for your commander ("Spend this mana only to cast your commander"), mana that can't be
//! spent on spells ("This mana can't be spent to cast spells"), and mana for special actions alone or beside spells
//! ("Spend this mana only to turn permanents face up", "... to foretell a card from your hand or cast an instant or
//! sorcery spell", "... to cast an enchantment spell, unlock a door, or turn a permanent face up").

use mtg_core::card::CardType;

use super::selectors::type_name;
use crate::spend::{SpecialAction, SpendFact, SpendFilter, SpendOn};

/// The sentence of the uses, with its capital and no period; `None` for uses this module does not word.
pub(super) fn ep33_spend_text(only: &[SpendOn]) -> Option<String> {
    let commander = SpendFilter {
        facts: vec![SpendFact::YourCommander],
        ..SpendFilter::default()
    };
    match only {
        [SpendOn::Spell(filter)] if *filter == commander => {
            return Some("Spend this mana only to cast your commander".to_owned());
        }
        [SpendOn::Ability(filter), SpendOn::OtherCosts] if *filter == SpendFilter::default() => {
            return Some("This mana can't be spent to cast spells".to_owned());
        }
        [SpendOn::Special(SpecialAction::TurnFaceUp)] => {
            return Some("Spend this mana only to turn permanents face up".to_owned());
        }
        [
            SpendOn::Special(SpecialAction::Foretell),
            SpendOn::Spell(filter),
        ] if *filter == foretold_spells() => {
            return Some(
                "Spend this mana only to foretell cards or cast spells that have foretell"
                    .to_owned(),
            );
        }
        _ => {}
    }
    if !only.iter().any(|on| matches!(on, SpendOn::Special(_))) {
        return None;
    }
    let mut phrases: Vec<String> = Vec::new();
    let mut types: Vec<CardType> = Vec::new();
    for on in only {
        match on {
            SpendOn::Special(action) => {
                flush(&mut types, &mut phrases);
                phrases.push(special_phrase(*action).to_owned());
            }
            // Spells of one card type each, listed one after the other, are one spell of either type.
            SpendOn::Spell(filter) => types.push(one_type(filter)?),
            _ => return None,
        }
    }
    flush(&mut types, &mut phrases);
    let list = match phrases.as_slice() {
        [first, second] => format!("{first} or {second}"),
        [init @ .., last] if !init.is_empty() => format!("{}, or {last}", init.join(", ")),
        _ => return None,
    };
    Some(format!("Spend this mana only to {list}"))
}

/// Spells that have foretell, and nothing else.
fn foretold_spells() -> SpendFilter {
    SpendFilter {
        facts: vec![SpendFact::Foretell],
        ..SpendFilter::default()
    }
}

/// The card type of a filter of spells of exactly one card type and nothing else.
fn one_type(filter: &SpendFilter) -> Option<CardType> {
    match filter.types.as_slice() {
        [one] if *filter == SpendFilter::of_type(*one) => Some(*one),
        _ => None,
    }
}

/// "cast an instant or sorcery spell" for the types gathered so far, which it empties.
fn flush(types: &mut Vec<CardType>, phrases: &mut Vec<String>) {
    if types.is_empty() {
        return;
    }
    let names: Vec<&str> = types.iter().map(|kind| type_name(*kind)).collect();
    let kind = names.join(" or ");
    phrases.push(format!(
        "cast {} {kind} spell",
        super::triggers::article(&kind)
    ));
    types.clear();
}

/// The sentence of a rule about your votes: "While voting, you may vote an additional time" (`VoteAgain`) or "While
/// voting, you get an additional vote" (`ExtraVote`).
pub(super) fn ep33_vote_text(rule: &crate::modify::GameRule) -> String {
    let more = if matches!(rule, crate::modify::GameRule::ExtraVote(_)) {
        "you get an additional vote"
    } else {
        "you may vote an additional time"
    };
    format!("While voting, {more}")
}

/// The special action as one use of a list: "turn a permanent face up".
const fn special_phrase(action: SpecialAction) -> &'static str {
    match action {
        SpecialAction::TurnFaceUp => "turn a permanent face up",
        SpecialAction::Unlock => "unlock a door",
        SpecialAction::Foretell => "foretell a card from your hand",
    }
}
