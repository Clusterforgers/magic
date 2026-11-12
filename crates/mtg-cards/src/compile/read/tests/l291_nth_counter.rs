//! Goal: "When the Nth <kind> counter is put on ~" reads into `NthCounterPut` and writes back as the card's text, for any ordinal and counter kind; a counter of no number, another permanent and an ordinal past tenth stay elsewhere.
use super::*;
use crate::trigger::TriggerPattern;

#[rstest]
#[case::robot_domination(
    "When the third plan counter is put on ~, sacrifice it. Create three 2/2 colorless Robot Villain artifact creature tokens.",
    r#"{"Triggered":{"when":{"NthCounterPut":{"counter":{"Other":"plan"},"number":3}},"effects":[{"Sacrifice":{"filter":["This"]}},{"CreateToken":{"token":{"Custom":{"name":"Robot Villain","types":["Artifact","Creature"],"subtypes":["Robot","Villain"],"power":2,"toughness":2}},"count":3}}]}}"#
)]
#[case::rewrite_history(
    "When the fourth plan counter is put on ~, sacrifice it. When you do, return up to two target instant and/or sorcery cards from your graveyard to your hand.",
    r#"{"Triggered":{"when":{"NthCounterPut":{"counter":{"Other":"plan"},"number":4}},"effects":[{"Sacrifice":{"filter":["This"]}},{"Reflexive":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":2}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}]}}"#
)]
#[case::first_charge(
    "When the first charge counter is put on ~, draw a card.",
    r#"{"Triggered":{"when":{"NthCounterPut":{"counter":"charge","number":1}},"effects":[{"Draw":{}}]}}"#
)]
#[case::tenth_plus_one(
    "When the tenth +1/+1 counter is put on ~, you gain 10 life.",
    r#"{"Triggered":{"when":{"NthCounterPut":{"counter":"+1/+1","number":10}},"effects":[{"GainLife":{"amount":10}}]}}"#
)]
fn l291_nth_counter_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::any_plan_counter("When a plan counter is put on ~, draw a card.")]
#[case::another_permanent("When the third plan counter is put on target creature, draw a card.")]
#[case::eleventh("When the eleventh plan counter is put on ~, draw a card.")]
#[case::removed("When the third plan counter is removed from ~, draw a card.")]
fn l291_nth_counter_near_misses_are_not_nth(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false);
    assert!(
        !matches!(
            read,
            Some(Ability::Triggered {
                when: TriggerPattern::NthCounterPut { .. },
                ..
            })
        ),
        "{text}: {read:?}"
    );
}

#[rstest]
#[case::zero(r#"{"Triggered":{"when":{"NthCounterPut":{"counter":"charge","number":0}},"effects":[{"Draw":{}}]}}"#)]
fn l291_nth_counter_from_zero_writes_nothing(#[case] json: &str) {
    assert!(render_variants(&ability(json)).is_empty(), "{json}");
}
