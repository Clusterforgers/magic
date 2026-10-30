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

#[rstest]
#[case::the_loss_then_the_types(
    "Enchanted creature loses all abilities and is a Toy artifact creature with base power and toughness 0/2 in addition to its other types.",
    r#"{"Static":{"affected":"Attached","mods":["LoseAbilities",{"AddType":"Artifact"},{"AddType":"Creature"},{"AddSubtype":"Toy"},{"SetPT":{"power":0,"toughness":2}}]}}"#
)]
#[case::no_loss(
    "Enchanted creature is a Toy artifact creature with base power and toughness 0/2 in addition to its other types.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddType":"Artifact"},{"AddType":"Creature"},{"AddSubtype":"Toy"},{"SetPT":{"power":0,"toughness":2}}]}}"#
)]
#[case::no_artifact(
    "Enchanted creature loses all abilities and is a Treefolk creature with base power and toughness 0/4 in addition to its other types.",
    r#"{"Static":{"affected":"Attached","mods":["LoseAbilities",{"AddType":"Creature"},{"AddSubtype":"Treefolk"},{"SetPT":{"power":0,"toughness":4}}]}}"#
)]
#[case::two_types(
    "Enchanted creature is a Human Wizard artifact creature with base power and toughness 1/1 in addition to its other types.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddType":"Artifact"},{"AddType":"Creature"},{"AddSubtype":"Human"},{"AddSubtype":"Wizard"},{"SetPT":{"power":1,"toughness":1}}]}}"#
)]
fn an_aura_that_adds_types_and_a_size_reads_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::as_long_as_it_is_face_down(
    "As long as enchanted creature is face down, it can't be turned face up.",
    r#"{"Static":{"affected":"Attached","mods":["CantBeTurnedFaceUp"],"condition":{"Matches":{"subject":"Attached","selector":["FaceDown"]}}}}"#
)]
fn a_face_down_creature_that_stays_so_reads_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::a_color_would_replace_the_others(
    "Enchanted creature is a green Toy artifact creature with base power and toughness 0/2 in addition to its other types."
)]
#[case::no_creature_type(
    "Enchanted creature is an artifact creature with base power and toughness 0/2 in addition to its other types."
)]
#[case::no_creature(
    "Enchanted creature is a Toy artifact with base power and toughness 0/2 in addition to its other types."
)]
#[case::no_size("Enchanted creature is a Toy artifact creature in addition to its other types.")]
#[case::the_words_of_the_end_changed(
    "Enchanted creature is a Toy artifact creature with base power and toughness 0/2 in addition to its colors."
)]
#[case::a_group(
    "Creatures you control are Toy artifact creatures with base power and toughness 0/2 in addition to their other types."
)]
#[case::face_up("As long as enchanted creature is face up, it can't be turned face up.")]
#[case::turned_down("As long as enchanted creature is face down, it can't be turned face down.")]
#[case::another_restriction_after(
    "As long as enchanted creature is face down, it can't be turned face up and can't attack."
)]
#[case::a_group_of_creatures(
    "As long as enchanted creature is face down, creatures you control can't be turned face up."
)]
fn near_misses_of_the_toy_and_the_face_down_creature_are_left_to_the_table(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Static, false).is_none(), "{text}");
}

/// Whether some wording the checker accepts for the rules is the clause.
fn da_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const DA_MY_AURAS: &str = r#"{"Move":{"what":{"Each":[{"Subtype":"Aura"},{"ControlledBy":"You"},{"AttachedTo":{"Target":0}}]},"to":"Hand"}}"#;
const DA_THE_TARGET_TO_HAND: &str = r#"{"Move":{"what":{"Target":0},"to":"Hand"}}"#;
const DA_DESTROY: &str = r#"{"Destroy":{"what":{"Target":0}}}"#;
const DA_TOUGHNESS: &str =
    r#"{"LoseLife":{"amount":{"Stat":{"of":{"Target":0},"stat":"Toughness"}}}}"#;
const DA_BACK: &str = r#"{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Le","to":1}},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}"#;

/// The modal spell of Orzhov Charm with the effects given for its first two modes.
fn da_charm(first: &str, second: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Modal":{{"modes":[{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}}],"effects":[{first}]}},{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{second}]}},{DA_BACK}]}}}}]}}}}"#
    )
}

const DA_CHARM_TEXT: &str = "Choose one \u{2014}\n\u{2022} Return target creature you control and all Auras you control attached to it to their owner's hand.\n\u{2022} Destroy target creature and you lose life equal to its toughness.\n\u{2022} Return target creature card with mana value 1 or less from your graveyard to the battlefield.";

#[test]
fn orzhov_charm_is_written_back_as_the_oracle_wording() {
    let rules = da_charm(
        &format!("{DA_MY_AURAS},{DA_THE_TARGET_TO_HAND}"),
        &format!("{DA_DESTROY},{DA_TOUGHNESS}"),
    );
    assert!(da_written(&rules, DA_CHARM_TEXT));
}

#[rstest]
#[case::the_creature_before_its_auras(
    format!("{DA_THE_TARGET_TO_HAND},{DA_MY_AURAS}"),
    format!("{DA_DESTROY},{DA_TOUGHNESS}")
)]
#[case::the_creature_alone(
    DA_THE_TARGET_TO_HAND.to_owned(),
    format!("{DA_DESTROY},{DA_TOUGHNESS}")
)]
#[case::auras_of_any_controller(
    format!(r#"{{"Move":{{"what":{{"Each":[{{"Subtype":"Aura"}},{{"AttachedTo":{{"Target":0}}}}]}},"to":"Hand"}}}},{DA_THE_TARGET_TO_HAND}"#),
    format!("{DA_DESTROY},{DA_TOUGHNESS}")
)]
#[case::equipment_in_place_of_auras(
    format!("{},{DA_THE_TARGET_TO_HAND}", DA_MY_AURAS.replace("Aura", "Equipment")),
    format!("{DA_DESTROY},{DA_TOUGHNESS}")
)]
#[case::auras_sent_to_the_graveyard(
    format!("{},{DA_THE_TARGET_TO_HAND}", DA_MY_AURAS.replace(r#""Hand""#, r#""Graveyard""#)),
    format!("{DA_DESTROY},{DA_TOUGHNESS}")
)]
#[case::the_life_lost_before_the_destruction(
    format!("{DA_MY_AURAS},{DA_THE_TARGET_TO_HAND}"),
    format!("{DA_TOUGHNESS},{DA_DESTROY}")
)]
#[case::the_life_of_the_power(
    format!("{DA_MY_AURAS},{DA_THE_TARGET_TO_HAND}"),
    format!("{DA_DESTROY},{}", DA_TOUGHNESS.replace("Toughness", "Power"))
)]
#[case::the_life_of_an_opponent(
    format!("{DA_MY_AURAS},{DA_THE_TARGET_TO_HAND}"),
    format!(r#"{DA_DESTROY},{{"LoseLife":{{"who":{{"ControllerOf":{{"Target":0}}}},"amount":{{"Stat":{{"of":{{"Target":0}},"stat":"Toughness"}}}}}}}}"#)
)]
#[case::no_life_lost(
    format!("{DA_MY_AURAS},{DA_THE_TARGET_TO_HAND}"),
    DA_DESTROY.to_owned()
)]
fn a_changed_orzhov_charm_is_not_written_as_the_oracle_wording(
    #[case] first: String,
    #[case] second: String,
) {
    assert!(!da_written(&da_charm(&first, &second), DA_CHARM_TEXT));
}
