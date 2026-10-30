//! Goal: the wordings of the Auras of the owner's deck that choose a land type as they enter and grant a landwalk of it read into the rules of the hand-written files, write back as the same text, and leave every near-miss to the table.
use super::*;

#[rstest]
#[case::as_it_enters_with_the_name(
    "As ~ enters, choose a land type.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Choose":"LandType"}}}"#
)]
#[case::as_this_aura_enters(
    "As this Aura enters, choose a land type.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"Choose":"LandType"}}}"#
)]
fn a_land_type_chosen_as_it_enters_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::the_host(
    "Enchanted creature has landwalk of the chosen type.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"Landwalk":["ChosenSubtype"]}}]}}"#
)]
#[case::with_another_keyword(
    "Enchanted creature has flying and landwalk of the chosen type.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Flying"},{"AddKeyword":{"Landwalk":["ChosenSubtype"]}}]}}"#
)]
#[case::a_group(
    "Creatures you control have landwalk of the chosen type.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Landwalk":["ChosenSubtype"]}}]}}"#
)]
#[case::a_land_named_by_its_word(
    "Enchanted creature has islandwalk.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"Landwalk":[{"Subtype":"Island"}]}}]}}"#
)]
fn a_landwalk_of_the_chosen_type_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::no_type("As ~ enters, choose a land.")]
#[case::a_type_beside_the_color("As ~ enters, choose a land type other than Island.")]
#[case::two_types("As ~ enters, choose two land types.")]
#[case::a_basic_type("As ~ enters, choose a basic land type.")]
#[case::no_word_of_the_choice("Enchanted creature has landwalk.")]
#[case::the_chosen_color("Enchanted creature has landwalk of the chosen color.")]
#[case::another_chosen_word("Enchanted creature has landwalk of the chosen player.")]
fn a_near_miss_of_the_land_type_is_left_to_the_table(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Static, false).is_none(), "{text}");
}
