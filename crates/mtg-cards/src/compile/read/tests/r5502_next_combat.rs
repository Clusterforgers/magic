//! Goal: "target creature (an opponent controls) attacks during its controller's next combat phase if able" reads into a `MustAttack` rule that lasts until that creature's controller's next combat phase ends, in a spell and in a triggered ability, writes back as the card's own text, and the near-misses ("this combat", "each combat", no target, "blocks") stay with the table.
use super::*;

const TRENCH: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Rule":{"rule":{"MustAttack":{"Target":0}},"duration":"UntilItsControllersNextCombatEnds"}}]}}"#;

#[rstest]
#[case::landfall(
    "Whenever a land you control enters, target creature an opponent controls attacks during its controller's next combat phase if able.",
    ClauseKind::Triggered,
    TRENCH
)]
#[case::a_spell(
    "Target creature attacks during its controller's next combat phase if able.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Rule":{"rule":{"MustAttack":{"Target":0}},"duration":"UntilItsControllersNextCombatEnds"}}]}}"#
)]
fn r5502_the_requirement_reads_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::this_combat("Target creature attacks this combat if able.")]
#[case::each_combat("Target creature attacks during its controller's next combat phase.")]
#[case::not_a_target("Each creature attacks during its controller's next combat phase if able.")]
#[case::blocks("Target creature blocks during its controller's next combat phase if able.")]
fn r5502_near_misses_do_not_last_until_the_next_combat(#[case] text: &str) {
    let read = clause(text, ClauseKind::Spell, false);
    let lasting = serde_json::to_string(&read).unwrap_or_default();
    assert!(
        !lasting.contains("UntilItsControllersNextCombatEnds"),
        "{text}: {read:?}"
    );
}
