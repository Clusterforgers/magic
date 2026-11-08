//! Goal: round 14 pack e14d writes protection with an exception as White Ward, Spectra Ward and Benevolent Blessing print it, and recruit as one word; another quality, another exception or no exception does not read back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn sparing(from: &str, spares: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"Attached","mods":[{{"AddKeyword":{{"ProtectionSparing":{{"from":{from},"spares":{spares}}}}}}}]}}}}"#
    )
}

const THIS: &str = r#"["This"]"#;
const AURAS: &str = r#"[{"Subtype":"Aura"}]"#;
const YOURS: &str =
    r#"[{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"}]},{"ControlledBy":"You"}]"#;

#[rstest]
#[case::white_ward(
    sparing(r#"[{"Color":"White"}]"#, THIS),
    "Enchanted creature has protection from white. This effect doesn't remove ~.",
    true
)]
#[case::tattoo_ward(
    sparing(r#"[{"Type":"Enchantment"}]"#, THIS),
    "Enchanted creature has protection from enchantments. This effect doesn't remove ~.",
    true
)]
#[case::chosen_color(
    sparing(r#"["ChosenColor"]"#, THIS),
    "Enchanted creature has protection from the chosen color. This effect doesn't remove ~.",
    true
)]
#[case::benevolent_blessing(
    sparing(r#"["ChosenColor"]"#, YOURS),
    "Enchanted creature has protection from the chosen color. This effect doesn't remove Auras and Equipment you control that are already attached to it.",
    true
)]
#[case::another_color(
    sparing(r#"[{"Color":"Blue"}]"#, THIS),
    "Enchanted creature has protection from white. This effect doesn't remove ~.",
    false
)]
#[case::auras_written_as_this(
    sparing(r#"[{"Color":"White"}]"#, AURAS),
    "Enchanted creature has protection from white. This effect doesn't remove ~.",
    false
)]
#[case::this_written_as_auras(
    sparing(r#"[{"Color":"White"}]"#, THIS),
    "Enchanted creature has protection from white. This effect doesn't remove Auras.",
    false
)]
#[case::no_exception_written(
    sparing(r#"[{"Color":"White"}]"#, THIS),
    "Enchanted creature has protection from white.",
    false
)]
#[case::an_unknown_exception(
    sparing(r#"[{"Color":"White"}]"#, r#"[{"Subtype":"Equipment"}]"#),
    "Enchanted creature has protection from white. This effect doesn't remove Equipment.",
    false
)]
fn protection_with_an_exception_reads_back(
    #[case] json: String,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(written(&json, clause), same, "{clause}");
}

#[rstest]
#[case::recruit("Recruit.", true)]
#[case::only_the_loot("Draw a card, then discard a card.", false)]
fn recruit_reads_back_as_one_word(#[case] clause: &str, #[case] same: bool) {
    let steps = crate::macros::action_effects("recruit", 0, &crate::refs::Ref::You).unwrap();
    let json = format!(
        r#"{{"Spell":{{"effects":{}}}}}"#,
        serde_json::to_string(&steps).unwrap()
    );
    assert_eq!(written(&json, clause), same, "{clause}");
}

const ENTERS: &str = r#"{"ZoneChange":{"to":"Battlefield","what":["This"]}}"#;

#[rstest]
#[case::barbed_servitor(
    format!(r#"{{"Triggered":{{"when":{ENTERS},"effects":[{{"Suspect":{{"what":"This"}}}}]}}}}"#),
    "When ~ enters, suspect it.",
    true
)]
#[case::person_of_interest(
    format!(r#"{{"Triggered":{{"when":{ENTERS},"effects":[{{"Suspect":{{"what":"Attached"}}}}]}}}}"#),
    "When ~ enters, suspect enchanted creature.",
    true
)]
#[case::target(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Suspect":{"what":{"Target":0}}}]}}"#.to_owned(),
    "Suspect target creature.",
    true
)]
#[case::goad_is_not_suspect(
    format!(r#"{{"Triggered":{{"when":{ENTERS},"effects":[{{"Goad":{{"what":"This"}}}}]}}}}"#),
    "When ~ enters, suspect it.",
    false
)]
#[case::another_creature(
    format!(r#"{{"Triggered":{{"when":{ENTERS},"effects":[{{"Suspect":{{"what":"Attached"}}}}]}}}}"#),
    "When ~ enters, suspect it.",
    false
)]
fn suspect_reads_back(#[case] json: String, #[case] clause: &str, #[case] same: bool) {
    let found = written_variants(&ability(&json));
    assert_eq!(
        found.iter().any(|wording| same_text(wording, clause)),
        same,
        "{clause}: {found:?}"
    );
}

#[test]
fn plain_protection_does_not_read_as_one_with_an_exception() {
    let plain = r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"Protection":[{"Color":"White"}]}}]}}"#;
    assert!(!written(
        plain,
        "Enchanted creature has protection from white. This effect doesn't remove ~."
    ));
}
