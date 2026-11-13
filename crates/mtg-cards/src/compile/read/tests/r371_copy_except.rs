//! Goal: copy exceptions that add to the copy ("a 2/2 black Zombie in addition to its other colors and types", "its starting loyalty is 1", "it's not legendary") read and write back as printed, and a near-miss stays with the table.
use super::*;

const JACE: &str = "When ~ enters, if ~ was kicked, create a token that's a copy of ~, except it's not legendary and its starting loyalty is 1.";
const JACE_JSON: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"If":{"condition":"Kicked","then":[{"CreateToken":{"token":{"CopyOf":{"of":"This","except":[{"RemoveSupertype":"Legendary"},{"SetLoyalty":1}]}}}}]}}]}}"#;
const RATADRABIK: &str = "Whenever another legendary creature you control dies, create a token that's a copy of that creature, except it's not legendary and it's a 2/2 black Zombie in addition to its other colors and types.";
const RATADRABIK_JSON: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"Supertype":"Legendary"},"Other",{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"Triggering","except":[{"RemoveSupertype":"Legendary"},{"SetPT":{"power":2,"toughness":2}},{"AddColors":["Black"]},{"AddSubtype":"Zombie"}]}}}}]}}"#;

fn copy_of_target(except: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"CreateToken":{{"token":{{"CopyOf":{{"of":{{"Target":0}},"except":{except}}}}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::starting_loyalty(JACE, JACE_JSON)]
#[case::colors_and_types_added(RATADRABIK, RATADRABIK_JSON)]
fn r371_copy_exceptions_read_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::two_colors(
    r#"[{"SetPT":{"power":1,"toughness":1}},{"AddColors":["Black","Green"]},{"AddSubtype":"Zombie"},{"AddSubtype":"Fungus"}]"#,
    "Create a token that's a copy of target creature, except it's a 1/1 black and green Zombie Fungus in addition to its other colors and types.",
    true
)]
#[case::loyalty_alone(
    r#"[{"SetLoyalty":3}]"#,
    "Create a token that's a copy of target creature, except its starting loyalty is 3.",
    true
)]
#[case::colors_added_are_not_colors_set(
    r#"[{"SetPT":{"power":2,"toughness":2}},{"AddColors":["Black"]},{"AddSubtype":"Zombie"}]"#,
    "Create a token that's a copy of target creature, except it's a 2/2 black Zombie.",
    false
)]
#[case::colors_set_are_not_colors_added(
    r#"[{"SetPT":{"power":2,"toughness":2}},{"SetColors":["Black"]},{"SetCreatureTypes":["Zombie"]}]"#,
    "Create a token that's a copy of target creature, except it's a 2/2 black Zombie in addition to its other colors and types.",
    false
)]
#[case::a_loyalty_by_amount(
    r#"[{"SetLoyalty":{"Count":[{"Type":"Creature"}]}}]"#,
    "Create a token that's a copy of target creature, except its starting loyalty is X.",
    false
)]
fn r371_copy_exceptions_write_as_printed(
    #[case] except: &str,
    #[case] text: &str,
    #[case] writes: bool,
) {
    let found = written_variants(&ability(&copy_of_target(except)));
    assert_eq!(
        found.iter().any(|w| same_text(w, text)),
        writes,
        "{found:?}"
    );
}

#[rstest]
#[case::a_spell_copy_has_no_starting_loyalty(
    "Copy target instant or sorcery spell, except its starting loyalty is 1."
)]
#[case::colors_without_a_creature_type(
    "Create a token that's a copy of target creature, except it's a 2/2 black in addition to its other colors and types."
)]
#[case::loyalty_as_a_word(
    "Create a token that's a copy of target creature, except its starting loyalty is high."
)]
fn r371_copy_exception_near_misses_stay_by_hand(#[case] text: &str) {
    assert_ne!(
        spell(text)
            .and_then(|read| serde_json::to_string(&read).ok())
            .map(|json| json.contains("SetLoyalty") || json.contains("AddColors")),
        Some(true),
        "{text}"
    );
}
