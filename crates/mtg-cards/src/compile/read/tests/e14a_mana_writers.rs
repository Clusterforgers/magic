//! Goal: "two mana of different colors" reads to the ten lists of two distinct colors and is written back as the cards
//! word it; another amount or kind of mana is not its text.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

/// A tap ability with `cost` that adds two mana of different colors.
fn different(cost: &str) -> String {
    let mana = serde_json::to_string(&crate::effect::Produce::two_different_colors()).unwrap();
    format!(r#"{{"Activated":{{"cost":{cost},"effects":[{{"AddMana":{{"mana":{mana}}}}}]}}}}"#)
}

#[rstest]
#[case::firemind_vessel(r#"["Tap"]"#, "{T}: Add two mana of different colors.")]
#[case::guild_globe(
    r#"[{"Mana":"{2}"},"Tap","SacrificeSelf"]"#,
    "{2}, {T}, Sacrifice this artifact: Add two mana of different colors."
)]
fn two_mana_of_different_colors_reads_and_writes_back(#[case] cost: &str, #[case] text: &str) {
    reads_as(text, ClauseKind::Activated, false, &different(cost));
}

#[rstest]
#[case::any_one_color("{T}: Add two mana of any one color.")]
#[case::any_combination("{T}: Add two mana in any combination of colors.")]
#[case::three("{T}: Add three mana of different colors.")]
#[case::one("{T}: Add one mana of any color.")]
fn another_kind_of_mana_is_not_two_of_different_colors(#[case] text: &str) {
    assert!(!writes(&different(r#"["Tap"]"#), text), "{text}");
}

#[test]
fn a_list_missing_a_pair_is_not_two_mana_of_different_colors() {
    let crate::effect::Produce::OneOfLists(mut lists) =
        crate::effect::Produce::two_different_colors()
    else {
        panic!("lists");
    };
    lists.pop();
    let mana = serde_json::to_string(&crate::effect::Produce::OneOfLists(lists)).unwrap();
    let json = format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{mana}}}}}]}}}}"#
    );
    assert!(!writes(&json, "{T}: Add two mana of different colors."));
}
