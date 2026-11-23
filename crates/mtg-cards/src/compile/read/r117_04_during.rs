//! "During your turn" after the event of a trigger ("Whenever an opponent draws a card during your turn"): a condition
//! checked only as the event happens (`Condition::AtTrigger`), never again on resolution; only an "if" clause is an
//! intervening "if" (CR 603.2, 603.4).

use super::tokens::Reader;
use crate::ability::Ability;
use crate::trigger::TriggerPattern;
use crate::value::Condition;

/// The turn after a trigger's event: "during your turn" or "during an opponent's turn".
pub(super) fn r117_04_during(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("during")?;
    if r.words("your turn") {
        return Some(Condition::YourTurn);
    }
    r.need("an opponent's turn")?;
    Some(Condition::Not(Box::new(Condition::YourTurn)))
}

/// The words `r117_04_during` reads for a condition checked as the trigger happens.
pub(super) fn r117_04_during_text(held: &Condition) -> Option<&'static str> {
    match held {
        Condition::YourTurn => Some("during your turn"),
        Condition::Not(inner) if **inner == Condition::YourTurn => {
            Some("during an opponent's turn")
        }
        _ => None,
    }
}

/// "for the first time each turn during your turn" as "for the first time during each of your turns", and "during
/// an opponent's turn" there as "during each opponent's turn": the first event of a turn that is (not) yours.
pub(super) fn r117_04_during_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::Nth { .. },
        intervening_if: Some(Condition::AtTrigger(held)),
        ..
    } = ability
    else {
        return;
    };
    let Some(during) = r117_04_during_text(held) else {
        return;
    };
    let each = if **held == Condition::YourTurn {
        "during each of your turns"
    } else {
        "during each opponent's turn"
    };
    let from = format!(" each turn {during},");
    let to = format!(" {each},");
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
    //! Goal: "during your turn" and "during an opponent's turn" after a trigger's event read as a condition checked as
    //! it triggers and write back as the same text; the same condition as an intervening "if" is worded "if it's your
    //! turn" only, and another turn condition gets no "during" wording.
    #![allow(clippy::unwrap_used)]
    use crate::ability::Ability;
    use crate::compile::read::render::written_variants;
    use rstest::rstest;

    const DRAWS: &str = r#"{"Triggered":{"when":{"Drew":{"who":"Opponents"}},"if":{"AtTrigger":"YourTurn"},"effects":[{"Draw":{}}]}}"#;
    const CASTS: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Type":"Creature"}],"by":"You"}},"if":{"AtTrigger":{"Not":"YourTurn"}},"effects":[{"Draw":{}}]}}"#;

    fn texts(json: &str) -> Vec<String> {
        written_variants(&serde_json::from_str::<Ability>(json).unwrap())
    }

    #[rstest]
    #[case::your_turn(
        DRAWS,
        "Whenever an opponent draws a card during your turn, draw a card."
    )]
    #[case::an_opponents_turn(
        CASTS,
        "Whenever you cast a creature spell during an opponent's turn, draw a card."
    )]
    fn r117_04_a_turn_checked_as_it_triggers_is_worded_during(
        #[case] json: &str,
        #[case] text: &str,
    ) {
        let written = texts(json);
        assert!(written.iter().any(|t| t == text), "{written:#?}");
        let ability =
            crate::compile::read::clause(text, crate::compile::ClauseKind::Triggered, false)
                .unwrap();
        assert_eq!(
            serde_json::to_value(ability).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }

    #[rstest]
    #[case::each_of_your_turns(
        r#"{"Triggered":{"when":{"Nth":{"event":{"LifeGained":{"who":"You"}},"nth":1}},"if":{"AtTrigger":"YourTurn"},"effects":[{"Draw":{}}]}}"#,
        "Whenever you gain life for the first time during each of your turns, draw a card."
    )]
    #[case::each_opponents_turn(
        r#"{"Triggered":{"when":{"Nth":{"event":{"LifeGained":{"who":"You"}},"nth":1}},"if":{"AtTrigger":{"Not":"YourTurn"}},"effects":[{"Draw":{}}]}}"#,
        "Whenever you gain life for the first time during each opponent's turn, draw a card."
    )]
    fn r117_04_the_first_event_of_a_turn_is_worded_during_each(
        #[case] json: &str,
        #[case] text: &str,
    ) {
        let written = texts(json);
        assert!(written.iter().any(|t| t == text), "{written:#?}");
        let plain = texts(&json.replace(r#"{"AtTrigger":"YourTurn"}"#, r#""YourTurn""#));
        assert!(
            !plain.iter().any(|t| t == text) || json.contains("Not"),
            "{plain:#?}"
        );
    }

    #[rstest]
    #[case::intervening_if(&DRAWS.replace(r#"{"AtTrigger":"YourTurn"}"#, r#""YourTurn""#), " during ")]
    #[case::the_main_phase(
        &DRAWS.replace(r#"{"AtTrigger":"YourTurn"}"#, r#"{"AtTrigger":{"Step":"Main1"}}"#),
        " during "
    )]
    #[case::a_damage_batch(
        r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[{"ControlledBy":"You"}],"to":"Opponents","combat":false}}},"if":{"AtTrigger":"YourTurn"},"effects":[{"Draw":{}}]}}"#,
        "one or more permanents you control deal noncombat damage to an opponent during your turn"
    )]
    fn r117_04_another_condition_is_not_worded_during(#[case] json: &str, #[case] words: &str) {
        let written = texts(json);
        assert!(!written.iter().any(|t| t.contains(words)), "{written:#?}");
    }
}
