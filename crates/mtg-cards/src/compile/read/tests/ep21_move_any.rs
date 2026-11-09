//! Goal: a move of counters of any kind ("move a counter", "move all counters") reads into `MoveCounters` without a
//! kind, with one counter or every counter on where they leave, and is written back as the same text; a count that is
//! neither one nor all of them, or every counter of another object, reads `Differs`.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const NESTING: &str =
    "Move a counter from target permanent you control onto a second target permanent.";

#[rstest]
#[case::a_counter_between_targets(NESTING, ClauseKind::Spell)]
#[case::a_counter_of_the_card("Move a counter from ~ onto target creature.", ClauseKind::Spell)]
#[case::all_counters_of_the_card(
    "Move all counters from ~ onto target creature.",
    ClauseKind::Spell
)]
fn counters_of_any_kind_read_and_write_back(#[case] text: &str, #[case] kind: ClauseKind) {
    let read = clause(text, kind, false).unwrap_or_else(|| panic!("not read: {text}"));
    let found = serde_json::to_string(&read).unwrap();
    assert!(
        found.contains("MoveCounters") && !found.contains("counter\":"),
        "{found}"
    );
    let wordings = render_variants(&read);
    assert!(
        wordings.iter().any(|wording| same_text(wording, text)),
        "{text}: {wordings:?}"
    );
}

const ALL: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"MoveCounters":{"from":"This","to":{"Target":0},"count":{"AnyCounters":"This"}}}]}}"#;

#[rstest]
#[case::all(ALL, true)]
#[case::two_counters(&ALL.replace(r#"{"AnyCounters":"This"}"#, "2"), false)]
#[case::all_of_the_target(&ALL.replace(r#"{"AnyCounters":"This"}"#, r#"{"AnyCounters":{"Target":0}}"#), false)]
#[case::one_kind(&ALL.replace(r#""count""#, r#""counter":"+1/+1","count""#), false)]
fn near_misses_read_differs(#[case] json: &str, #[case] same: bool) {
    assert_eq!(
        written(json, "Move all counters from ~ onto target creature."),
        same,
        "{json}"
    );
}

const TRANSFER: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"MoveCounters":{"from":{"Target":0},"to":{"Target":1},"count":{"AnyCounters":{"Target":0}}}}]}}"#;

/// "Onto another target creature" is the second target of "onto a second target creature".
#[rstest]
#[case::another(
    "Move all counters from target creature onto another target creature.",
    true
)]
#[case::a_second(
    "Move all counters from target creature onto a second target creature.",
    true
)]
#[case::the_same("Move all counters from target creature onto target creature.", false)]
fn all_counters_onto_another_target(#[case] text: &str, #[case] same: bool) {
    assert_eq!(written(TRANSFER, text), same, "{text}");
}
