//! Two replacements in one clause of r28-4: a choice made as the permanent enters, then the counters it enters with,
//! counted with that choice ("As this artifact enters, choose a creature type. This artifact enters with a fellowship
//! counter on it for each creature you control of the chosen type.", "As this creature enters, choose an opponent. This
//! creature enters with a -1/-1 counter on it for each creature that player controls.").

use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

/// What replaces the card's own entering, with no condition.
fn own_entering(ability: &Ability) -> Option<&ReplaceBy> {
    match ability {
        Ability::Replacement {
            when: ReplaceWhen::Enters { what },
            by,
            condition: None,
            from: ZoneKind::Battlefield,
        } if *what == Selector::this() => Some(by),
        _ => None,
    }
}

/// The counters counted over the permanents the chosen player controls, written as over those the opponents control
/// (the writers have no phrase for the chosen player), and whether it was; the ability as it is otherwise.
fn counted_for_opponents(ability: &Ability) -> (Ability, bool) {
    let mut twin = ability.clone();
    let mut changed = false;
    if let Ability::Replacement {
        by:
            ReplaceBy::WithCounters {
                count: Amount::Count(selector),
                ..
            },
        ..
    } = &mut twin
    {
        for pred in &mut selector.0 {
            if *pred == Pred::ControlledBy(Ref::Chosen) {
                *pred = Pred::ControlledBy(Ref::Opponents);
                changed = true;
            }
        }
    }
    (twin, changed)
}

/// The wordings of the choice and the counters as one clause; empty for any other two abilities.
pub(super) fn r284_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (Some(ReplaceBy::Choose(_)), Some(ReplaceBy::WithCounters { tapped: false, .. })) =
        (own_entering(first), own_entering(second))
    else {
        return Vec::new();
    };
    let heads: Vec<String> = super::render::written_variants(first)
        .into_iter()
        .filter(|text| text.starts_with("As ~ enters, choose "))
        .collect();
    let (written, chosen_player) = counted_for_opponents(second);
    let tails: Vec<String> = super::render::written_variants(&written)
        .into_iter()
        .filter(|text| text.starts_with("~ enters with "))
        // The player chosen in the sentence before is "that player".
        .filter_map(|text| {
            if chosen_player {
                text.contains(" your opponents control")
                    .then(|| text.replace(" your opponents control", " that player controls"))
            } else {
                Some(text)
            }
        })
        .collect();
    heads
        .iter()
        .flat_map(|head| tails.iter().map(move |tail| format!("{head} {tail}")))
        .collect()
}
