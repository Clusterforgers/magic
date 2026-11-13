//! Goal: "two mana of any one color and two mana of any other color" reads to the ten lists of two distinct colors each
//! twice and is written back as the cards word it; two different colors, one pair, or a list missing a pair is not its
//! text. Mana restricted to cumulative upkeep costs reads and writes back, and other uses do not.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

/// A tap ability that adds `mana`.
fn tap_for(mana: &crate::effect::Produce) -> String {
    let mana = serde_json::to_string(mana).unwrap();
    format!(r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{mana}}}}}]}}}}"#)
}

const PAIRS: &str = "{T}: Add two mana of any one color and two mana of any other color.";

#[test]
fn two_pairs_of_colors_reads_and_writes_back() {
    let pairs = crate::effect::Produce::two_pairs_of_colors();
    let crate::effect::Produce::OneOfLists(lists) = &pairs else {
        panic!("lists");
    };
    assert_eq!(lists.len(), 10);
    assert!(lists.iter().all(|list| list.len() == 4
        && list[0] == list[1]
        && list[2] == list[3]
        && list[0] != list[2]));
    reads_as(PAIRS, ClauseKind::Activated, false, &tap_for(&pairs));
}

#[rstest]
#[case::different_colors("{T}: Add two mana of different colors.")]
#[case::one_color("{T}: Add four mana of any one color.")]
#[case::any_combination("{T}: Add four mana in any combination of colors.")]
fn another_kind_of_mana_is_not_two_pairs(#[case] text: &str) {
    assert!(
        !writes(
            &tap_for(&crate::effect::Produce::two_pairs_of_colors()),
            text
        ),
        "{text}"
    );
}

#[rstest]
#[case::two_different(crate::effect::Produce::two_different_colors())]
#[case::missing_a_pair({
    let crate::effect::Produce::OneOfLists(mut lists) = crate::effect::Produce::two_pairs_of_colors() else {
        panic!("lists");
    };
    lists.pop();
    crate::effect::Produce::OneOfLists(lists)
})]
fn other_lists_are_not_two_pairs(#[case] mana: crate::effect::Produce) {
    assert!(!writes(&tap_for(&mana), PAIRS));
}

const UNICORN: &str =
    "{T}: Add {U} or {C}{U}. Spend this mana only to pay cumulative upkeep costs.";

/// Adarkar Unicorn's ability with the uses given.
fn unicorn(only: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"OneOfLists":[["Blue"],["Colorless","Blue"]]}},"only":[{only}]}}}}]}}}}"#
    )
}

#[test]
fn mana_for_cumulative_upkeep_reads_and_writes_back() {
    reads_as(
        UNICORN,
        ClauseKind::Activated,
        false,
        &unicorn(r#""CumulativeUpkeep""#),
    );
}

#[rstest]
#[case::other_costs(r#""OtherCosts""#)]
#[case::abilities(r#"{"Ability":{"types":[]}}"#)]
#[case::upkeep_and_other_costs(r#""CumulativeUpkeep","OtherCosts""#)]
fn other_uses_are_not_cumulative_upkeep(#[case] only: &str) {
    assert!(!writes(&unicorn(only), UNICORN), "{only}");
}
