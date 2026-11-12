//! Two steps a sentence joins to one target: "Return target creature you control and all Auras you control attached
//! to it to their owner's hand." (the Auras move first) and "Destroy target creature and you lose life equal to its
//! toughness." The sentence is the one of the step on the target with the other step added.

use serde_json::{Value, json};

use super::alternatives;
use crate::effect::Effect;
use crate::refs::TargetSpec;

/// The sentences of the two joined steps, or none for other effects.
pub(super) fn ce29_joined(targets: &[TargetSpec], effects: &[Effect]) -> Option<Vec<Vec<String>>> {
    let [first, second] = effects else {
        return None;
    };
    let (first_value, second_value) = (
        serde_json::to_value(first).ok()?,
        serde_json::to_value(second).ok()?,
    );
    let (step, tail): (&Effect, fn(&str) -> Option<String>) = if first_value
        == auras_of_the_target()
        && second_value == target_to_hand()
    {
        (second, |sentence| {
            let head = sentence.strip_suffix(" to its owner's hand")?;
            Some(format!(
                "{head} and all Auras you control attached to it to their owner's hand"
            ))
        })
    } else if first_value == json!({"Destroy": {"what": {"Target": 0}}})
        && second_value
            == json!({"LoseLife": {"amount": {"Stat": {"of": {"Target": 0}, "stat": "Toughness"}}}})
    {
        (first, |sentence| {
            Some(format!(
                "{sentence} and you lose life equal to its toughness"
            ))
        })
    } else {
        return None;
    };
    let found: Vec<Vec<String>> = alternatives(targets, std::slice::from_ref(step))
        .into_iter()
        .filter_map(|sentences| match sentences.as_slice() {
            [one] => tail(one).map(|text| vec![text]),
            _ => None,
        })
        .collect();
    (!found.is_empty()).then_some(found)
}

/// The Auras the controller controls that are attached to the first target, moved to the hand.
fn auras_of_the_target() -> Value {
    json!({"Move": {
        "what": {"Each": [{"Subtype": "Aura"}, {"ControlledBy": "You"}, {"AttachedTo": {"Target": 0}}]},
        "to": "Hand"
    }})
}

/// The first target moved to its owner's hand.
fn target_to_hand() -> Value {
    json!({"Move": {"what": {"Target": 0}, "to": "Hand"}})
}

#[cfg(test)]
mod tests {
    //! Goal: the two joined steps are written as the one sentence the card prints, and a lone step as usual.
    use rstest::rstest;

    use crate::ability::Ability;
    use crate::compile::read::render::variants;

    #[rstest]
    #[case::auras(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Move":{"what":{"Each":[{"Subtype":"Aura"},{"ControlledBy":"You"},{"AttachedTo":{"Target":0}}]},"to":"Hand"}},{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#,
        "Return target creature you control and all Auras you control attached to it to their owner's hand."
    )]
    #[case::life(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"LoseLife":{"amount":{"Stat":{"of":{"Target":0},"stat":"Toughness"}}}}]}}"#,
        "Destroy target creature and you lose life equal to its toughness."
    )]
    fn the_joined_steps_are_one_sentence(#[case] json: &str, #[case] text: &str) {
        let ability: Ability = serde_json::from_str(json).unwrap();
        let written = variants(&ability);
        assert!(written.iter().any(|w| w == text), "{written:?}");
    }
}
