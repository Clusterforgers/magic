//! Goal: a static ability over a group or an Aura's creature is written part by part as the card words it (a bonus with its zero signed either way, lost keywords and abilities, base power and toughness, a type or subtype in addition to the others, goaded), and another part, order, sign or number is not its text.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

#[rstest]
#[case::gravitational_shift_with_flying(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"HasKeyword":"Flying"}]},"mods":[{"ModifyPT":{"power":2,"toughness":0}}]}}"#,
    "Creatures with flying get +2/+0."
)]
#[case::gravitational_shift_without_flying(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}}]},"mods":[{"ModifyPT":{"power":-2,"toughness":0}}]}}"#,
    "Creatures without flying get -2/-0."
)]
#[case::the_zero_signed_plus(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}}]},"mods":[{"ModifyPT":{"power":-2,"toughness":0}}]}}"#,
    "Creatures without flying get -2/+0."
)]
#[case::a_zero_power_signed_minus(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":[{"ModifyPT":{"power":0,"toughness":-1}}]}}"#,
    "Creatures your opponents control get -0/-1."
)]
#[case::the_flesh_is_weak_artifacts(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasCounter":"+1/+1"}]},"mods":[{"AddType":"Artifact"}]}}"#,
    "Creatures you control with +1/+1 counters on them are artifacts in addition to their other types."
)]
#[case::the_flesh_is_weak_nonartifacts(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"Type":"Artifact"}}]},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}]}}"#,
    "Nonartifact creatures get -1/-1."
)]
#[case::humility(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":["LoseAbilities",{"SetPT":{"power":1,"toughness":1}}]}}"#,
    "All creatures lose all abilities and have base power and toughness 1/1."
)]
#[case::stormtide_leviathan_islands(
    r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":[{"AddSubtype":"Island"}]}}"#,
    "All lands are Islands in addition to their other types."
)]
#[case::graaz_juggernauts(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"mods":[{"SetPT":{"power":5,"toughness":3}},{"AddSubtype":"Juggernaut"}]}}"#,
    "Other creatures you control have base power and toughness 5/3 and are Juggernauts in addition to their other creature types."
)]
#[case::lose_a_keyword(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":[{"RemoveKeyword":"Flying"}]}}"#,
    "Creatures your opponents control lose flying."
)]
#[case::a_bonus_and_a_lost_keyword(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}},{"RemoveKeyword":"Flying"}]}}"#,
    "Creatures your opponents control get -1/-1 and lose flying."
)]
#[case::three_parts(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Flying"},"Goaded"]}}"#,
    "Creatures you control get +1/+1, have flying, and are goaded."
)]
#[case::the_sound_of_drums_goaded_alone(
    r#"{"Static":{"affected":"Attached","mods":["Goaded"]}}"#,
    "Enchanted creature is goaded."
)]
#[case::an_equipped_creature_loses_abilities(
    r#"{"Static":{"affected":"Attached","mods":["LoseAbilities",{"SetPT":{"power":0,"toughness":1}}]}}"#,
    "Equipped creature loses all abilities and has base power and toughness 0/1."
)]
fn parts_of_a_static_are_written_as_the_card_words_them(#[case] json: &str, #[case] text: &str) {
    assert!(
        writes(json, text),
        "{text}: {:?}",
        render_variants(&ability(json))
    );
}

#[rstest]
#[case::another_toughness(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}}]},"mods":[{"ModifyPT":{"power":-2,"toughness":-1}}]}}"#,
    "Creatures without flying get -2/-0."
)]
#[case::another_keyword(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":[{"RemoveKeyword":"Flying"}]}}"#,
    "Creatures your opponents control lose first strike."
)]
#[case::your_creatures_for_theirs(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"RemoveKeyword":"Flying"}]}}"#,
    "Creatures your opponents control lose flying."
)]
#[case::the_parts_in_another_order(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"SetPT":{"power":1,"toughness":1}},"LoseAbilities"]}}"#,
    "All creatures lose all abilities and have base power and toughness 1/1."
)]
#[case::other_base_numbers(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":["LoseAbilities",{"SetPT":{"power":1,"toughness":2}}]}}"#,
    "All creatures lose all abilities and have base power and toughness 1/1."
)]
#[case::another_subtype(
    r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":[{"AddSubtype":"Swamp"}]}}"#,
    "All lands are Islands in addition to their other types."
)]
#[case::another_type(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasCounter":"+1/+1"}]},"mods":[{"AddType":"Enchantment"}]}}"#,
    "Creatures you control with +1/+1 counters on them are artifacts in addition to their other types."
)]
#[case::one_creature_for_a_group(
    r#"{"Static":{"affected":"Attached","mods":["Goaded"]}}"#,
    "Creatures you control are goaded."
)]
fn another_part_order_sign_or_number_is_not_the_text(#[case] json: &str, #[case] text: &str) {
    assert!(!writes(json, text), "{text}");
}
