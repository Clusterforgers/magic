//! Modes with a second step that the plain wording of the mode leaves out: "Return target creature you control and all
//! Auras you control attached to it to their owner's hand." (the Auras move first, then the creature) and "Destroy target
//! creature and you lose life equal to its toughness." (the life lost is the toughness of the target as it last existed).
//! The wording is the plain mode with the second step added to its bullet.

use super::super::variants;
use super::twin_json::{inner_mut, twin};
use crate::ability::Ability;
use serde_json::{Value, json};

/// The second step a mode adds to the plain wording of its bullet.
#[derive(Clone, Copy)]
enum Step {
    /// "and all Auras you control attached to it", before "to their owner's hand".
    AurasToo,
    /// "and you lose life equal to its toughness", at the end.
    LifeToo,
}

/// Adds the wordings of a modal spell whose modes return a permanent with the Auras attached to it, or destroy it and
/// lose life equal to its toughness.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut steps = Vec::new();
    let plain = twin(ability, |value| {
        steps = plain_modes(value)?;
        (!steps.is_empty()).then_some(())
    });
    let Some(plain) = plain else {
        return;
    };
    for text in variants(&plain) {
        if let Some(said) = with_steps(&text, &steps) {
            texts.push(said);
        }
    }
}

/// The modes of the first modal effect of a spell, which hold the effects.
fn modes_of(value: &mut Value) -> Option<&mut Vec<Value>> {
    inner_mut(value)?
        .get_mut("effects")?
        .as_array_mut()?
        .iter_mut()
        .find_map(|effect| effect.get_mut("Modal"))?
        .get_mut("modes")?
        .as_array_mut()
}

/// Takes the second step out of every mode that has one, and lists the steps by the place of the mode.
fn plain_modes(value: &mut Value) -> Option<Vec<(usize, Step)>> {
    let mut steps = Vec::new();
    for (place, mode) in modes_of(value)?.iter_mut().enumerate() {
        let effects = mode.get_mut("effects")?.as_array_mut()?;
        let step = match effects.as_slice() {
            [first, second] if is_auras_attached_to_the_target(first) && is_move_target(second) => {
                effects.remove(0);
                Step::AurasToo
            }
            [first, second] if is_destroy_target(first) && is_life_of_target(second) => {
                effects.remove(1);
                Step::LifeToo
            }
            _ => continue,
        };
        steps.push((place, step));
    }
    Some(steps)
}

/// A move of the Auras the controller controls that are attached to the first target to the hand.
fn is_auras_attached_to_the_target(effect: &Value) -> bool {
    *effect
        == json!({"Move": {
            "what": {"Each": [{"Subtype": "Aura"}, {"ControlledBy": "You"}, {"AttachedTo": {"Target": 0}}]},
            "to": "Hand"
        }})
}

/// A move of the first target to its owner's hand.
fn is_move_target(effect: &Value) -> bool {
    *effect == json!({"Move": {"what": {"Target": 0}, "to": "Hand"}})
}

/// The destruction of the first target.
fn is_destroy_target(effect: &Value) -> bool {
    *effect == json!({"Destroy": {"what": {"Target": 0}}})
}

/// The loss of life, for the controller, equal to the toughness of the first target.
fn is_life_of_target(effect: &Value) -> bool {
    *effect == json!({"LoseLife": {"amount": {"Stat": {"of": {"Target": 0}, "stat": "Toughness"}}}})
}

/// The text with the second step added to the bullet of each mode that has one.
fn with_steps(text: &str, steps: &[(usize, Step)]) -> Option<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    for (place, step) in steps {
        let line = lines.get_mut(place + 1)?;
        *line = match step {
            Step::AurasToo => {
                let head = line.strip_suffix(" to its owner's hand.")?;
                format!("{head} and all Auras you control attached to it to their owner's hand.")
            }
            Step::LifeToo => {
                let head = line.strip_suffix('.')?;
                format!("{head} and you lose life equal to its toughness.")
            }
        };
    }
    Some(lines.join("\n"))
}
