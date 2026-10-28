//! Goal: the counted amounts of what the resolving effect did ("for each creature destroyed this way", "the number of creatures you controlled that were destroyed this way") read in every amount position, write back as the card's text, and leave each near-miss unread.
use super::*;

#[rstest]
#[case::fumigate(
    "Destroy all creatures. You gain 1 life for each creature destroyed this way.",
    r#"{"Destroyed":[{"Type":"Creature"}]}"#
)]
#[case::decree_of_pain(
    "Destroy all creatures. They can't be regenerated. Draw a card for each creature destroyed this way.",
    r#"{"Destroyed":[{"Type":"Creature"}]}"#
)]
#[case::kayas_wrath(
    "Destroy all creatures. You gain life equal to the number of creatures you controlled that were destroyed this way.",
    r#"{"Destroyed":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#
)]
#[case::each_you_controlled(
    "Destroy all artifacts. Draw a card for each artifact you controlled that was destroyed this way.",
    r#"{"Destroyed":[{"Type":"Artifact"},{"ControlledBy":"You"}]}"#
)]
fn w2c_destroyed_this_way_reads_and_writes_back(#[case] text: &str, #[case] part: &str) {
    let json = serde_json::to_string(&golden(text, ClauseKind::Spell, false)).unwrap();
    assert!(json.contains(part), "{text}: {json}");
}

#[rstest]
#[case::bane_of_progress(
    "When this creature enters, destroy all artifacts and enchantments. Put a +1/+1 counter on this creature for each permanent destroyed this way."
)]
#[case::whiptongue_hydra(
    "When this creature enters, destroy all creatures with flying. Put a +1/+1 counter on this creature for each creature destroyed this way."
)]
#[case::ob_nixilis(
    "When ~ enters, destroy all tapped creatures your opponents control. You gain 2 life for each creature destroyed this way."
)]
fn w2c_destroyed_this_way_reads_in_triggers(#[case] text: &str) {
    let json = serde_json::to_string(&golden(text, ClauseKind::Triggered, false)).unwrap();
    assert!(json.contains("Destroyed"), "{text}: {json}");
}

#[rstest]
#[case::controlled_now(
    "Destroy all creatures. You gain 1 life for each creature you control destroyed this way."
)]
#[case::exiled("Destroy all creatures. You gain 1 life for each creature sacrificed this way.")]
#[case::wrong_verb(
    "Destroy all creatures. You gain life equal to the number of creatures you controlled that was destroyed this way."
)]
#[case::opponents_controlled(
    "Destroy all creatures. You gain life equal to the number of creatures your opponents controlled that were destroyed this way."
)]
fn w2c_destroyed_near_misses_stay_unread(#[case] text: &str) {
    assert!(spell(text).is_none(), "{text}");
}
