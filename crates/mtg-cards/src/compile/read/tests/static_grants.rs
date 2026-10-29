//! Goal: the statics that give keywords, sizes and bonuses to a group or a host (landwalk, fear, exalted, extort, evolve, fabricate and annihilator by number, ward, commander creatures, a subtype taken out, "during your turn") read into the shapes of the hand-written files and write back as the card's own text, and the ward reminder is not mistaken for a counter.
use super::*;
use crate::compile::normalize::reminder_mentions_counter;

#[rstest]
#[case::a_subtype_written_the_same_in_the_plural(
    "Other Merfolk you control get +1/+1.",
    r#"{"Static":{"affected":{"Each":[{"Subtype":"Merfolk"},"Other",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#
)]
#[case::islandwalk_beside_a_bonus(
    "Other Merfolk creatures you control get +1/+1 and have islandwalk.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Subtype":"Merfolk"},"Other",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":{"Landwalk":[{"Subtype":"Island"}]}}]}}"#
)]
#[case::swampwalk_for_every_other_zombie(
    "Other Zombie creatures have swampwalk.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Subtype":"Zombie"},"Other"]},"mods":[{"AddKeyword":{"Landwalk":[{"Subtype":"Swamp"}]}}]}}"#
)]
#[case::fear_for_the_equipped_creature(
    "Equipped creature has fear.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Fear"}]}}"#
)]
#[case::shroud_for_an_enchanted_land(
    "Enchanted land has shroud.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Shroud"}]}}"#
)]
#[case::exalted_for_the_other_creatures(
    "Other creatures you control have exalted.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Exalted"}]}}"#
)]
#[case::extort_for_the_creatures(
    "Creatures you control have extort.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Extort"}]}}"#
)]
#[case::evolve_for_a_subtype(
    "Clues you control have evolve.",
    r#"{"Static":{"affected":{"Each":[{"Subtype":"Clue"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Evolve"}]}}"#
)]
#[case::fabricate_by_number(
    "Other nontoken creatures you control have fabricate 1.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":"Token"},"Other",{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Fabricate":1}}]}}"#
)]
#[case::annihilator_beside_trample(
    "Enchanted creature gets +10/+10 and has trample and annihilator 2.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":10,"toughness":10}},{"AddKeyword":"Trample"},{"AddKeyword":{"Annihilator":2}}]}}"#
)]
#[case::ward_for_the_aura_host(
    "Enchanted creature has ward {2}.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"Ward":"{2}"}}]}}"#
)]
#[case::ward_for_a_kind_of_permanent(
    "Artifacts you control have ward {1}.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Ward":"{1}"}}]}}"#
)]
#[case::commander_creatures_get_a_bonus(
    "Commander creatures you control get +2/+2.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Commander",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":2,"toughness":2}}]}}"#
)]
#[case::commander_creatures_get_a_bonus_and_a_keyword(
    "Commander creatures you control get +2/+2 and have indestructible.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Commander",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Indestructible"}]}}"#
)]
#[case::a_subtype_taken_out_of_a_group(
    "Non-Archon creatures have base power and toughness 3/3.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"Subtype":"Archon"}}]},"mods":[{"SetPT":{"power":3,"toughness":3}}]}}"#
)]
#[case::during_your_turn_for_a_group(
    "During your turn, creature tokens you control get +1/+4.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":4}}],"condition":"YourTurn"}}"#
)]
#[case::during_your_turn_for_the_card(
    "During your turn, ~ has flying.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"condition":"YourTurn"}}"#
)]
#[case::forests_from_the_graveyard(
    "You may play Forests from your graveyard.",
    r#"{"Rule":{"rule":{"MayCast":{"what":[{"Subtype":"Forest"}],"from":"Graveyard","play":true}}}}"#
)]
#[case::lands_from_the_graveyard(
    "You may play lands from your graveyard.",
    r#"{"Rule":{"rule":{"MayCast":{"what":[{"Type":"Land"}],"from":"Graveyard","play":true}}}}"#
)]
#[case::forests_from_the_top_of_the_library(
    "You may play Forests from the top of your library.",
    r#"{"Rule":{"rule":{"MayCast":{"what":[{"Subtype":"Forest"}],"from":"Library","play":true}}}}"#
)]
fn a_static_grant_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::a_subtype_that_is_no_noun("Other Gizmos you control get +1/+1.")]
#[case::a_number_missing("Other creatures you control have fabricate.")]
#[case::a_number_that_is_no_number("Other creatures you control have annihilator two.")]
#[case::a_landwalk_of_no_land("Other creatures you control have cowwalk.")]
#[case::commander_without_a_noun("Commander you control get +2/+2.")]
#[case::elves_from_the_graveyard("You may play Elves from your graveyard.")]
#[case::creatures_from_the_graveyard("You may play creatures from your graveyard.")]
#[case::during_the_opponent_s_turn("During an opponent's turn, creatures you control get +1/+1.")]
fn near_misses_are_left_to_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[rstest]
#[case::the_ward_reminder(
    "Enchanted creature has ward {2}. (Whenever it becomes the target of a spell or ability an opponent controls, counter it unless that player pays {2}.)",
    false
)]
#[case::the_modified_reminder(
    "Modified creatures you control have haste. (Equipment, Auras you control, and counters are modifications.)",
    false
)]
#[case::a_counter_the_clause_puts(
    "Put a shield counter on target creature. (If it would be dealt damage or destroyed, remove a shield counter instead.)",
    true
)]
fn the_ward_reminder_is_not_the_reminder_of_a_counter(#[case] text: &str, #[case] counter: bool) {
    assert_eq!(reminder_mentions_counter(text), counter, "{text}");
}
