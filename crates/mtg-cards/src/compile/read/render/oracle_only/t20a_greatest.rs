//! An intervening "if" that the controller has the best of a kind, worded as the oracle does: "At the beginning of your
//! upkeep, if you control the artifact with the greatest mana value or tied for the greatest mana value, draw a card."
//! The rules compare the greatest value among what the controller controls with the greatest value among all of them.

use super::super::{pattern_text, variants};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, Condition, StatKind};
use mtg_core::card::CardType;

/// Adds the wordings of a trigger whose condition is that its controller controls the best object of a kind.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        intervening_if: Some(held),
        limit: None,
        ..
    } = ability
    else {
        return;
    };
    let (Some(phrase), Some(event)) = (best(held), pattern_text(when)) else {
        return;
    };
    let mut bare = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut bare {
        *intervening_if = None;
    }
    let opening = format!("{event}, ");
    for text in variants(&bare) {
        if let Some(rest) = text.strip_prefix(&opening) {
            texts.push(format!("{opening}if {phrase}, {rest}"));
        }
    }
}

/// "you control the artifact with the greatest mana value or tied for the greatest mana value".
fn best(held: &Condition) -> Option<String> {
    let Condition::Compare {
        left: Amount::Greatest {
            of: own,
            stat: own_stat,
        },
        op: Cmp::Ge,
        right: Amount::Greatest { of: all, stat },
    } = held
    else {
        return None;
    };
    let [Pred::Type(kind)] = all.0.as_slice() else {
        return None;
    };
    let yours = Pred::ControlledBy(Ref::You);
    let own_kind = Selector(vec![Pred::Type(*kind), yours.clone()]);
    let own_kind_first = Selector(vec![yours, Pred::Type(*kind)]);
    if own_stat != stat || (*own != own_kind && *own != own_kind_first) {
        return None;
    }
    let stat = stat_words(*stat)?;
    Some(format!(
        "you control the {} with the greatest {stat} or tied for the greatest {stat}",
        kind_word(*kind)?
    ))
}

/// The characteristic as a count of the cards of a kind says it.
fn stat_words(stat: StatKind) -> Option<&'static str> {
    match stat {
        StatKind::Power => Some("power"),
        StatKind::Toughness => Some("toughness"),
        StatKind::ManaValue => Some("mana value"),
        _ => None,
    }
}

/// The card types a player can control the best of.
fn kind_word(kind: CardType) -> Option<&'static str> {
    match kind {
        CardType::Artifact => Some("artifact"),
        CardType::Creature => Some("creature"),
        CardType::Enchantment => Some("enchantment"),
        CardType::Land => Some("land"),
        _ => None,
    }
}
