//! Goal: "with a counter on it", "with counters on them", "for each counter on ~" and "the number of counters on ~" read as a counter of any kind (`HasAnyCounter`, `AnyCounters`) and write back as the card's text; a count of the counters on another object stays unread.
use super::*;

#[rstest]
#[case::a_target(
    "{2}, {T}: Target creature you control with a counter on it gains lifelink until end of turn.",
    ClauseKind::Activated,
    "\"HasAnyCounter\""
)]
#[case::a_group_that_cant_attack(
    "Creatures your opponents control with counters on them can't attack or block.",
    ClauseKind::Static,
    "\"HasAnyCounter\""
)]
#[case::a_group_with_haste(
    "Creatures you control with counters on them have haste.",
    ClauseKind::Static,
    "\"HasAnyCounter\""
)]
#[case::a_pump_for_each(
    "Other Humans you control get +1/+1 for each counter on ~.",
    ClauseKind::Static,
    r#"{"AnyCounters":"This"}"#
)]
#[case::mana_for_each(
    "{T}: Add {U} for each counter on ~. Spend this mana only to cast noncreature spells.",
    ClauseKind::Activated,
    r#"{"AnyCounters":"This"}"#
)]
fn c10_any_counters_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] part: &str,
) {
    let read = golden(text, kind, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(part), "{text}: {json}");
}

#[rstest]
#[case::on_another_object(
    "Other Humans you control get +1/+1 for each counter on enchanted creature."
)]
#[case::a_counter_on_them("Creatures you control with a counter on them have haste.")]
fn c10_any_counters_near_misses_stay_unread(#[case] text: &str) {
    let json =
        clause(text, ClauseKind::Static, false).map(|read| serde_json::to_string(&read).unwrap());
    assert!(
        !json
            .as_deref()
            .is_some_and(|json| json.contains("AnyCounter")),
        "{text}: {json:?}"
    );
}
