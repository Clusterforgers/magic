//! Pronouns and names that the renderers write one way and a card writes another: "you may have it deal", "Tap it.",
//! "That creature gains trample", "That creature's controller draws", "that player" for the controller of the creature
//! of an event, "If it was an artifact" after a return, "your commander" and "Do this only once each turn."

use crate::ability::{Ability, Limit};
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Targetable};
use crate::trigger::TriggerPattern;

/// What an ability must have for a swap to apply.
type Guard = fn(&Ability) -> bool;

/// The swaps: where they apply, the renderers' words and the card's.
const SWAPS: [(Guard, &str, &str); 11] = [
    (
        damaging_trigger,
        ", you may have ~ deal ",
        ", you may have it deal ",
    ),
    (taps_itself, ". Tap ~.", ". Tap it."),
    (creature_target, ". It gains ", ". That creature gains "),
    (creature_target, ". It gets ", ". That creature gets "),
    (
        controller_of_target,
        ". Its controller ",
        ". That creature's controller ",
    ),
    (
        controller_of_target,
        ". Then its controller ",
        ". That creature's controller ",
    ),
    (
        controller_of_event,
        "that permanent's controller",
        "that player",
    ),
    (returns_target, "If it's a", "If it was a"),
    (
        once_each_turn,
        ". Then this ability triggers only once each turn.",
        ". Do this only once each turn.",
    ),
    (
        sacrifice_unless,
        "sacrifice ~ unless",
        "sacrifice it unless",
    ),
    (taps_itself, ". Then tap ~.", ". Then tap it."),
];

/// Adds the wordings with each swap that applies, one swap at a time.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut found = Vec::new();
    for (applies, from, to) in SWAPS {
        if applies(ability) {
            found.extend(
                texts
                    .iter()
                    .filter(|text| text.contains(from))
                    .map(|text| text.replacen(from, to, 1)),
            );
        }
    }
    texts.extend(found);
}

/// The rules as JSON, which the guards search for the shape they want.
fn json(ability: &Ability) -> String {
    serde_json::to_string(ability).unwrap_or_default()
}

fn damaging_trigger(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::DealsDamage { .. },
            ..
        }
    )
}

fn taps_itself(ability: &Ability) -> bool {
    ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::Tap {
                what: Ref::This,
                ..
            }
        )
    })
}

/// A spell with one creature target that a later step changes.
fn creature_target(ability: &Ability) -> bool {
    let Ability::Spell { targets, effects } = ability else {
        return false;
    };
    let [spec] = targets.as_slice() else {
        return false;
    };
    matches!(&spec.what, Targetable::Object(selector) if selector.0.contains(&Pred::Type(mtg_core::card::CardType::Creature)))
        && effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Continuous {
                    affected: Ref::Target(0),
                    ..
                }
            )
        })
}

fn controller_of_target(ability: &Ability) -> bool {
    json(ability).contains(r#""who":{"ControllerOf":{"Target":0}}"#)
}

fn controller_of_event(ability: &Ability) -> bool {
    matches!(ability, Ability::Triggered { .. })
        && json(ability).contains(r#"{"ControllerOf":"Triggering"}"#)
}

/// A condition on a target that an earlier step returned to a hand.
fn returns_target(ability: &Ability) -> bool {
    let text = json(ability);
    text.contains(r#""Matches":{"subject":{"Target":0}"#)
        && text.contains(r#""Move":{"what":{"Target":0}"#)
}

fn once_each_turn(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Triggered {
            limit: Some(Limit::OncePerTurn),
            ..
        }
    )
}

fn sacrifice_unless(ability: &Ability) -> bool {
    json(ability).contains(r#""unpaid":[{"Sacrifice""#)
}
