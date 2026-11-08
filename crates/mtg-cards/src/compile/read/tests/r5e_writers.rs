//! Goal: "Do this only once each turn." after a first step that is a choice reads as the limit spent when the choice is done and writes back only from it; the limit spent as the ability triggers reads as "This ability triggers only once each turn." after a choice, and either sentence after a step that is always done.
use super::*;
use crate::ability::Limit;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn trigger(limit: &str, effects: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"LifeGained":{{"who":"You"}}}},"limit":"{limit}","effects":{effects}}}}}"#
    )
}

const MAY_DRAW: &str = r#"[{"May":{"effects":[{"Draw":{}}]}}]"#;
const DRAW: &str = r#"[{"Draw":{}}]"#;
const PAY_DRAW: &str = r#"[{"Pay":{"cost":"{1}","paid":[{"Draw":{}}]}}]"#;

const DO_THIS: &str = "Whenever you gain life, you may draw a card. Do this only once each turn.";
const TRIGGERS: &str =
    "Whenever you gain life, you may draw a card. This ability triggers only once each turn.";
const ALWAYS: &str = "Whenever you gain life, draw a card. Do this only once each turn.";

#[rstest]
#[case::done_once_of_a_choice("OncePerTurnDone", MAY_DRAW, DO_THIS, true)]
#[case::spent_on_trigger_is_another_rule("OncePerTurn", MAY_DRAW, DO_THIS, false)]
#[case::spent_on_trigger_says_it_triggers("OncePerTurn", MAY_DRAW, TRIGGERS, true)]
#[case::done_once_never_says_it_triggers("OncePerTurnDone", MAY_DRAW, TRIGGERS, false)]
#[case::a_step_always_done("OncePerTurn", DRAW, ALWAYS, true)]
#[case::the_choice_is_missing("OncePerTurnDone", DRAW, DO_THIS, false)]
fn do_this_only_once_follows_the_choice(
    #[case] limit: &str,
    #[case] effects: &str,
    #[case] clause: &str,
    #[case] accepted: bool,
) {
    assert_eq!(
        written(&trigger(limit, effects), clause),
        accepted,
        "{clause}"
    );
}

#[rstest]
#[case::a_may(DO_THIS, "OncePerTurnDone")]
#[case::it_triggers(TRIGGERS, "OncePerTurn")]
fn the_reader_takes_the_limit_from_the_first_step(#[case] text: &str, #[case] limit: &str) {
    let read = clause(text, ClauseKind::Triggered, false).unwrap_or_else(|| panic!("{text}"));
    let Ability::Triggered {
        limit: Some(found), ..
    } = read
    else {
        panic!("{read:?}");
    };
    assert_eq!(
        serde_json::to_value(found).unwrap(),
        serde_json::json!(limit)
    );
}

#[test]
fn a_payment_first_counts_as_a_choice() {
    assert_eq!(
        Limit::done_once(&serde_json::from_str::<Vec<crate::effect::Effect>>(PAY_DRAW).unwrap()),
        Limit::OncePerTurnDone
    );
}
