//! Goal: the land types a static ability gives a whole group read into data and write back as the same text (the land types set to one basic type, every basic land type added to the others), and a wording with another type, a count or a possessive that does not fit, or a set of types the text cannot say is left to the table.
use super::*;

/// The five basic land types as five additions, in the order the rules list them.
const EVERY_BASIC: &str = r#"[{"AddSubtype":"Plains"},{"AddSubtype":"Island"},{"AddSubtype":"Swamp"},{"AddSubtype":"Mountain"},{"AddSubtype":"Forest"}]"#;

fn group(selector: &str, mods: &str) -> String {
    format!(r#"{{"Static":{{"affected":{{"Each":[{selector}]}},"mods":{mods}}}}}"#)
}

const NONBASIC: &str = r#"{"Type":"Land"},{"Not":{"Supertype":"Basic"}}"#;
const YOURS: &str = r#"{"Type":"Land"},{"ControlledBy":"You"}"#;

#[rstest]
#[case::blood_moon(
    "Nonbasic lands are Mountains.",
    NONBASIC,
    r#"[{"SetLandTypes":["Mountain"]}]"#
)]
#[case::harbinger_of_the_seas(
    "Nonbasic lands are Islands.",
    NONBASIC,
    r#"[{"SetLandTypes":["Island"]}]"#
)]
#[case::plains_say_the_same_in_the_plural(
    "Nonbasic lands are Plains.",
    NONBASIC,
    r#"[{"SetLandTypes":["Plains"]}]"#
)]
#[case::swamps(
    "Nonbasic lands are Swamps.",
    NONBASIC,
    r#"[{"SetLandTypes":["Swamp"]}]"#
)]
#[case::forests(
    "Nonbasic lands are Forests.",
    NONBASIC,
    r#"[{"SetLandTypes":["Forest"]}]"#
)]
#[case::a_group_of_yours(
    "Lands you control are Forests.",
    YOURS,
    r#"[{"SetLandTypes":["Forest"]}]"#
)]
#[case::dryad_of_the_ilysian_grove(
    "Lands you control are every basic land type in addition to their other types.",
    YOURS,
    EVERY_BASIC
)]
#[case::each_land_of_yours(
    "Each land you control is every basic land type in addition to its other types.",
    YOURS,
    EVERY_BASIC
)]
#[case::every_nonbasic_land(
    "Nonbasic lands are every basic land type in addition to their other types.",
    NONBASIC,
    EVERY_BASIC
)]
fn the_land_types_of_a_group_read_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] selector: &str,
    #[case] mods: &str,
) {
    reads_as(text, ClauseKind::Static, false, &group(selector, mods));
}

#[rstest]
#[case::the_singular("Nonbasic lands are Mountain.")]
#[case::not_a_basic_type("Nonbasic lands are Elves.")]
#[case::not_a_land_type_at_all("Nonbasic lands are Zombies.")]
#[case::kept_other_types("Nonbasic lands are Mountains in addition to their other land types.")]
#[case::kept_other_types_said_short(
    "Nonbasic lands are Mountains in addition to their other types."
)]
#[case::two_types("Nonbasic lands are Mountains and Islands.")]
#[case::every_type_alone("Nonbasic lands are every basic land type.")]
#[case::every_type_of_a_land(
    "Lands you control are every land type in addition to their other types."
)]
#[case::its_for_a_group(
    "Lands you control are every basic land type in addition to its other types."
)]
#[case::creature_types(
    "Lands you control are every basic land type in addition to their other creature types."
)]
#[case::the_singular_group("Each nonbasic land is a Mountain.")]
#[case::the_card("~ is a Mountain.")]
fn a_wording_the_data_cannot_say_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

/// Whether one of the wordings of the rules is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

#[rstest]
#[case::set_to_a_mountain(&group(NONBASIC, r#"[{"SetLandTypes":["Mountain"]}]"#), "Nonbasic lands are Mountains.")]
#[case::set_for_lands_you_control(&group(YOURS, r#"[{"SetLandTypes":["Island"]}]"#), "Lands you control are Islands.")]
#[case::every_type_of_your_lands(
    &group(YOURS, EVERY_BASIC),
    "Lands you control are every basic land type in addition to their other types."
)]
#[case::every_type_of_all_lands(
    &group(r#"{"Type":"Land"}"#, EVERY_BASIC),
    "All lands are every basic land type in addition to their other types."
)]
fn the_land_types_of_a_group_are_written(#[case] json: &str, #[case] text: &str) {
    assert!(
        writes(json, text),
        "{text}: {:?}",
        render_variants(&ability(json))
    );
}

#[rstest]
#[case::two_types_set(&group(NONBASIC, r#"[{"SetLandTypes":["Island","Mountain"]}]"#))]
#[case::no_type_set(&group(NONBASIC, r#"[{"SetLandTypes":[]}]"#))]
#[case::a_creature_type_set(&group(NONBASIC, r#"[{"SetLandTypes":["Elf"]}]"#))]
fn a_set_of_types_the_text_cannot_say_is_not_written(#[case] json: &str) {
    assert_eq!(render_variants(&ability(json)), Vec::<String>::new());
}

#[rstest]
#[case::four_types_added(&group(YOURS, r#"[{"AddSubtype":"Plains"},{"AddSubtype":"Island"},{"AddSubtype":"Swamp"},{"AddSubtype":"Mountain"}]"#))]
#[case::the_types_out_of_order(&group(YOURS, r#"[{"AddSubtype":"Island"},{"AddSubtype":"Plains"},{"AddSubtype":"Swamp"},{"AddSubtype":"Mountain"},{"AddSubtype":"Forest"}]"#))]
fn a_part_of_the_types_is_not_every_type(#[case] json: &str) {
    for written in render_variants(&ability(json)) {
        assert!(!written.contains("every basic land type"), "{written}");
    }
}
