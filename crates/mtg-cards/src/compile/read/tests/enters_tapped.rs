//! Goal: a group of permanents that enters tapped ("Creatures your opponents control enter tapped.") reads into a replacement of the entering of what the group names, writes back as the same text, and leaves every spell, zone or missing group to the table.
use super::*;

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

#[rstest]
#[case::creatures_of_opponents(
    "Creatures your opponents control enter tapped.",
    r#"[{"Type":"Creature"},{"ControlledBy":"Opponents"}]"#
)]
#[case::artifacts_of_opponents(
    "Artifacts your opponents control enter tapped.",
    r#"[{"Type":"Artifact"},{"ControlledBy":"Opponents"}]"#
)]
#[case::two_types(
    "Artifacts and creatures your opponents control enter tapped.",
    r#"[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]},{"ControlledBy":"Opponents"}]"#
)]
#[case::your_own_creatures(
    "Creatures you control enter tapped.",
    r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#
)]
#[case::every_creature("Creatures enter tapped.", r#"[{"Type":"Creature"}]"#)]
fn a_group_that_enters_tapped(#[case] text: &str, #[case] what: &str) {
    let json =
        format!(r#"{{"Replacement":{{"when":{{"Enters":{{"what":{what}}}}},"by":"Tapped"}}}}"#);
    reads_as(text, ClauseKind::Static, false, &json);
}

#[rstest]
#[case::spells("Spells your opponents cast enter tapped.")]
#[case::cards_in_a_zone("Creature cards in your graveyard enter tapped.")]
#[case::no_group("Enter tapped.")]
#[case::a_condition("Creatures your opponents control enter tapped unless you control a Forest.")]
fn what_is_no_group_of_permanents_stays_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
