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

const ROOTS: &str = r#"{"Activated":{"cost":[{"PutCounters":{"counter":{"Other":"-0/-1"}}}],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}],"limit":"OncePerTurn"}}"#;
const EXERT: &str =
    r#"{"Activated":{"cost":["Tap","Exert"],"effects":[{"AddMana":{"mana":{"AnyColor":2}}}]}}"#;
const CINDER: &str = r#"{"Activated":{"cost":"{1}","effects":[{"AddMana":{"mana":{"OneOf":["Black","Red"]}}}],"limit":{"TimesPerTurn":3}}}"#;

#[rstest]
#[case::wall_of_roots(
    ROOTS,
    "Put a -0/-1 counter on this creature: Add {G}. Activate only once each turn."
)]
#[case::oasis_ritualist(EXERT, "{T}, Exert this creature: Add two mana of any one color.")]
#[case::manaforge_cinder(
    CINDER,
    "{1}: Add {B} or {R}. Activate no more than three times each turn."
)]
fn a_cost_on_the_source_and_a_limit_are_written_as_the_card_words_them(
    #[case] json: &str,
    #[case] text: &str,
) {
    assert!(
        writes(json, text),
        "{text}: {:?}",
        render_variants(&ability(json))
    );
}

#[rstest]
#[case::wall_of_roots(
    ROOTS,
    "Put a -0/-1 counter on this creature: Add {G}. Activate only once each turn."
)]
#[case::oasis_ritualist(EXERT, "{T}, Exert this creature: Add two mana of any one color.")]
fn a_cost_on_the_source_reads_back(#[case] json: &str, #[case] text: &str) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::another_counter(
    ROOTS,
    "Put a -1/-1 counter on this creature: Add {G}. Activate only once each turn."
)]
#[case::no_limit(ROOTS, "Put a -0/-1 counter on this creature: Add {G}.")]
#[case::no_exert(EXERT, "{T}: Add two mana of any one color.")]
#[case::twice(CINDER, "{1}: Add {B} or {R}. Activate no more than twice each turn.")]
#[case::digits(
    CINDER,
    "{1}: Add {B} or {R}. Activate no more than 3 times each turn."
)]
fn another_cost_or_limit_is_not_the_text(#[case] json: &str, #[case] text: &str) {
    assert!(!writes(json, text), "{text}");
}
