//! Goal: a condition on what the attached permanent is, a group of spells that can't be countered, a prevention for a group of permanents and a prevention during your turn read into their data and write back as the same text; a player in the group, a union with "you", a source, a duration and another verb stay unread.
use super::*;

#[rstest]
#[case::enchanted_permanent_a_creature(
    "As long as enchanted permanent is a creature, it gets +2/+2.",
    r#"{"Matches":{"subject":"Attached","selector":[{"Type":"Creature"}]}}"#
)]
#[case::enchanted_permanent_a_land(
    "As long as enchanted permanent is a land, it has \"{T}: Add two mana of any one color.\"",
    r#"{"Matches":{"subject":"Attached","selector":[{"Type":"Land"}]}}"#
)]
#[case::equipped_a_human(
    "As long as equipped creature is a Human, it has lifelink.",
    r#"{"Matches":{"subject":"Attached","selector":[{"Subtype":"Human"}]}}"#
)]
#[case::enchanted_a_human(
    "As long as enchanted creature is a Human, it has first strike.",
    r#"{"Matches":{"subject":"Attached","selector":[{"Subtype":"Human"}]}}"#
)]
#[case::a_human_or_an_angel(
    "As long as equipped creature is a Human or an Angel, it has vigilance.",
    r#"{"Matches":{"subject":"Attached","selector":[{"AnyOf":[{"Subtype":"Human"},{"Subtype":"Angel"}]}]}}"#
)]
#[case::equipped_a_human_gets(
    "As long as equipped creature is a Human, it gets +1/+1.",
    r#"{"Matches":{"subject":"Attached","selector":[{"Subtype":"Human"}]}}"#
)]
fn c11h_a_condition_on_the_attached_permanent_reads_and_writes_back(
    #[case] text: &str,
    #[case] condition: &str,
) {
    let read = golden(text, ClauseKind::Static, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(condition), "{json}");
    assert!(json.contains(r#""affected":"Attached""#), "{json}");
}

#[rstest]
#[case::creature_spells_you_control(
    "Creature spells you control can't be countered.",
    r#"["Spell",{"Type":"Creature"},{"ControlledBy":"You"}]"#
)]
#[case::creature_spells(
    "Creature spells can't be countered.",
    r#"["Spell",{"Type":"Creature"}]"#
)]
#[case::sliver_spells(
    "Sliver spells can't be countered.",
    r#"["Spell",{"Subtype":"Sliver"}]"#
)]
#[case::green_spells("Green spells you control can't be countered.", r#"{"Color":"Green"}"#)]
#[case::instant_and_sorcery_spells(
    "Instant and sorcery spells you control can't be countered.",
    r#""Spell""#
)]
fn c11h_a_group_of_spells_that_cant_be_countered_reads_and_writes_back(
    #[case] text: &str,
    #[case] part: &str,
) {
    let read = golden(text, ClauseKind::Static, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(r#"{"CantBeCountered":{"Each":"#), "{json}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::other_creatures_noncombat(
    "Prevent all noncombat damage that would be dealt to other creatures you control.",
    r#""to":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"combat":false"#
)]
#[case::attacking_creatures_combat(
    "Prevent all combat damage that would be dealt to attacking creatures you control.",
    r#""combat":true"#
)]
#[case::every_creature(
    "Prevent all damage that would be dealt to creatures.",
    r#""to":{"Each":[{"Type":"Creature"}]}"#
)]
#[case::creatures_you_control(
    "Prevent all damage that would be dealt to creatures you control.",
    r#""to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#
)]
#[case::you_during_your_turn(
    "During your turn, prevent all damage that would be dealt to you.",
    r#""condition":"YourTurn""#
)]
#[case::the_card_during_your_turn(
    "During your turn, prevent all damage that would be dealt to ~.",
    r#""to":"This""#
)]
fn c11h_a_group_prevention_reads_and_writes_back(#[case] text: &str, #[case] part: &str) {
    let read = golden(text, ClauseKind::Static, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(r#""by":"Prevent""#), "{json}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::a_union_with_you(
    "Prevent all noncombat damage that would be dealt to you and creatures you control."
)]
#[case::a_source(
    "Prevent all damage that would be dealt to creatures you control by sources you control."
)]
#[case::a_duration("Prevent all damage that would be dealt to creatures this turn.")]
#[case::dealt_to_and_by(
    "Prevent all combat damage that would be dealt to and dealt by creatures you control."
)]
#[case::spells_of_an_opponent("Creature spells your opponents control can't be countered.")]
#[case::a_spell_in_a_graveyard("Creature cards in your graveyard can't be countered.")]
#[case::a_permanent_group("Creatures you control can't be countered.")]
#[case::attached_has_a_keyword("As long as enchanted creature has vigilance, it gets +1/+1.")]
#[case::attached_isnt_a_kind("As long as enchanted permanent isn't a creature, it gets +1/+1.")]
#[case::another_subject("As long as target creature is a Human, it gets +1/+1.")]
fn c11h_near_misses_stay_unread(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    assert_eq!(read, None, "{text}");
}
