//! Goal: the trigger wordings of the r55-22 pack read into their patterns and write back as the card's text, and a near
//! miss reads otherwise.
use super::*;
use crate::trigger::TriggerPattern;

#[rstest]
#[case::valkmira(
    "Whenever you or another permanent you control becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {1}.",
    r#"{"AnyOf":[{"PlayerBecomesTarget":{"who":"You","by":[{"ControlledBy":"Opponents"}]}},{"BecomesTarget":{"what":["Other",{"ControlledBy":"You"}],"by":[{"ControlledBy":"Opponents"}]}}]}"#,
    r#"[{"Pay":{"who":"Triggering","cost":"{1}","unpaid":[{"Counter":{"what":"Triggering"}}]}}]"#
)]
fn r5522_triggers_read_and_write_back(
    #[case] text: &str,
    #[case] when: &str,
    #[case] effects: &str,
) {
    let json = format!(r#"{{"Triggered":{{"when":{when},"effects":{effects}}}}}"#);
    reads_as(text, ClauseKind::Triggered, false, &json);
}

#[rstest]
#[case::another_creature(
    "Whenever you or another creature you control becomes the target of a spell or ability an opponent controls, draw a card."
)]
#[case::another_you_control_not(
    "Whenever you or another permanent becomes the target of a spell or ability an opponent controls, draw a card."
)]
fn r5522_trigger_near_misses_read_otherwise(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false);
    let ours = |when: &TriggerPattern| {
        match when {
        TriggerPattern::AnyOf(found) => found.iter().any(|one| {
            matches!(one, TriggerPattern::BecomesTarget { what, .. } if what.0.contains(&crate::refs::Pred::Other))
        }),
        _ => false,
    }
    };
    assert!(
        !matches!(&read, Some(Ability::Triggered { when, .. }) if ours(when)),
        "{text}: {read:?}"
    );
}
