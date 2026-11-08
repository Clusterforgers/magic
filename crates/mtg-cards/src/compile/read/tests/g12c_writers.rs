//! Goal: round 12 pack g12c writes a supertype a static ability gives ("is legendary", "are snow") and one half of base power and toughness ("has base power 0", "have base toughness 1") as cards print them; another supertype, number, half or subject does not read back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn on(affected: &str, mods: &str) -> String {
    format!(r#"{{"Static":{{"affected":{affected},"mods":[{mods}]}}}}"#)
}

const ENCHANTED: &str = r#""Attached""#;
const NONLAND: &str = r#"{"Each":[{"Not":{"Type":"Land"}}]}"#;
const THEIRS: &str = r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}"#;

#[rstest]
#[case::in_bolass_clutches(
    ENCHANTED,
    r#"{"AddSupertype":"Legendary"}"#,
    "Enchanted permanent is legendary.",
    true
)]
#[case::leyline_of_singularity(
    NONLAND,
    r#"{"AddSupertype":"Legendary"}"#,
    "Nonland permanents are legendary.",
    true
)]
#[case::snow(
    ENCHANTED,
    r#"{"AddSupertype":"Snow"}"#,
    "Enchanted permanent is snow.",
    true
)]
#[case::other_supertype(
    ENCHANTED,
    r#"{"AddSupertype":"Snow"}"#,
    "Enchanted permanent is legendary.",
    false
)]
#[case::basic_is_not_written(
    ENCHANTED,
    r#"{"AddSupertype":"Basic"}"#,
    "Enchanted land is basic.",
    false
)]
#[case::withercrown(
    ENCHANTED,
    r#"{"SetPower":0}"#,
    "Enchanted creature has base power 0.",
    true
)]
#[case::maha(
    THEIRS,
    r#"{"SetToughness":1}"#,
    "Creatures your opponents control have base toughness 1.",
    true
)]
#[case::other_half(
    THEIRS,
    r#"{"SetPower":1}"#,
    "Creatures your opponents control have base toughness 1.",
    false
)]
#[case::other_number(
    ENCHANTED,
    r#"{"SetPower":1}"#,
    "Enchanted creature has base power 0.",
    false
)]
#[case::other_subject(
    ENCHANTED,
    r#"{"SetToughness":1}"#,
    "Creatures your opponents control have base toughness 1.",
    false
)]
fn a_given_supertype_and_one_base_half_read_back(
    #[case] affected: &str,
    #[case] modification: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&on(affected, modification), clause), reads);
}
