//! A trigger condition checked only as the event happens (`Condition::AtTrigger`) is worded "while" after the event:
//! each wording of the same ability under an intervening "if" ("<event>, if <condition>, <effect>") becomes "<event>
//! while <condition>, <effect>"; "no other Dinosaurs" is also "another Dinosaur" there.

use super::subtypes::plural_subtype;
use crate::ability::Ability;
use crate::refs::{Pred, Ref};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition};

/// Whether a condition checked as the trigger happens is worded "while" after its event: an event that is no step
/// beginning ("At end of combat, if ..."), and a condition that is not about the turn or step ("during your turn").
pub(super) fn r116_06_worded_while(when: &TriggerPattern, held: &Condition) -> bool {
    super::render::while_held(when)
        && !matches!(when, TriggerPattern::StepBegins { .. })
        && !of_the_turn(held)
}

/// Whether the condition asks whose turn, which step or whether it is combat.
fn of_the_turn(held: &Condition) -> bool {
    match held {
        Condition::YourTurn | Condition::Step(_) | Condition::InCombat => true,
        Condition::Not(inner) => of_the_turn(inner),
        Condition::All(parts) | Condition::Any(parts) => parts.iter().any(of_the_turn),
        _ => false,
    }
}

/// Adds the "while" wordings of a trigger whose condition is checked only as it triggers.
pub(super) fn r116_06_while_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ability::Triggered {
        when,
        intervening_if: Some(Condition::AtTrigger(held)),
        ..
    } = ability
    else {
        return;
    };
    if !r116_06_worded_while(when, held) {
        return;
    }
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = Some((**held).clone());
    }
    for text in written(&plain) {
        let Some((event, rest)) = text.split_once(", if ") else {
            continue;
        };
        let Some((condition, effect)) = rest.split_once(", ") else {
            continue;
        };
        let worded = format!("{event} while {condition}, {effect}");
        if !texts.contains(&worded) {
            texts.push(worded);
        }
    }
    none_other(held, texts);
}

/// "while you control no other Dinosaurs" also as "while you don't control another Dinosaur".
fn none_other(held: &Condition, texts: &mut Vec<String>) {
    let Condition::Compare {
        left: Amount::Count(selector),
        op: Cmp::Eq,
        right: Amount::Fixed(0),
    } = held
    else {
        return;
    };
    let [
        Pred::Subtype(kind),
        Pred::Other,
        Pred::ControlledBy(Ref::You),
    ] = selector.0.as_slice()
    else {
        return;
    };
    let from = format!(" while you control no other {}, ", plural_subtype(&kind.0));
    let to = format!(" while you don't control another {}, ", kind.0);
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(&from))
        .map(|text| text.replacen(&from, &to, 1))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

#[cfg(test)]
mod tests {
    //! Goal: a trigger condition checked as it triggers is worded "while" after the event in each wording the same
    //! condition has as an intervening "if", and an intervening "if" or another number is never worded "while".
    #![allow(clippy::unwrap_used)]
    use crate::ability::Ability;
    use crate::compile::read::render::written_variants;
    use rstest::rstest;

    const ATTACK: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"if":{"AtTrigger":{"Compare":{"left":{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}},"op":"Ge","right":8}}},"effects":[{"Draw":{}}]}}"#;
    const ALONE: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"AtTrigger":{"Compare":{"left":{"Count":[{"Subtype":"Dinosaur"},"Other",{"ControlledBy":"You"}]},"op":"Eq","right":0}}},"effects":[{"Draw":{}}]}}"#;
    const MONARCH: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"AtTrigger":{"Monarch":"You"}},"effects":[{"Draw":{}}]}}"#;

    fn plain(json: &str) -> String {
        let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
        let held = value["Triggered"]["if"]["AtTrigger"].clone();
        value["Triggered"]["if"] = held;
        value.to_string()
    }

    #[test]
    fn r116_06_another_number_is_not_worded_while() {
        let nine: Ability =
            serde_json::from_str(&ATTACK.replace(r#""right":8"#, r#""right":9"#)).unwrap();
        let text = "Whenever you attack while creatures you control have total power 8 or greater, draw a card.";
        assert!(!written_variants(&nine).iter().any(|t| t == text));
    }

    #[rstest]
    #[case::during_your_turn(r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"if":{"AtTrigger":"YourTurn"},"effects":[{"Draw":{}}]}}"#)]
    #[case::a_main_phase(r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"if":{"AtTrigger":{"All":["YourTurn",{"Step":"Main1"}]}},"effects":[{"Draw":{}}]}}"#)]
    #[case::a_step_begins(r#"{"Triggered":{"when":{"StepBegins":{"step":"EndCombat"}},"if":{"AtTrigger":{"Monarch":"You"}},"effects":[{"Draw":{}}]}}"#)]
    fn r116_06_the_turn_and_a_step_beginning_are_never_worded_while(#[case] json: &str) {
        let ability: Ability = serde_json::from_str(json).unwrap();
        let texts = written_variants(&ability);
        assert!(!texts.iter().any(|t| t.contains(" while ")), "{texts:#?}");
    }

    #[rstest]
    #[case::you_attack(
        ATTACK,
        "Whenever you attack while creatures you control have total power 8 or greater, draw a card."
    )]
    #[case::no_other_dinosaur(
        ALONE,
        "Whenever ~ attacks while you don't control another Dinosaur, draw a card."
    )]
    #[case::the_monarch(MONARCH, "Whenever ~ attacks while you're the monarch, draw a card.")]
    fn r116_06_a_trigger_time_condition_is_worded_while(#[case] json: &str, #[case] text: &str) {
        let at_trigger: Ability = serde_json::from_str(json).unwrap();
        let texts = written_variants(&at_trigger);
        assert!(texts.iter().any(|t| t == text), "{texts:#?}");
        let intervening: Ability = serde_json::from_str(&plain(json)).unwrap();
        let texts = written_variants(&intervening);
        assert!(!texts.iter().any(|t| t == text), "{texts:#?}");
    }
}
