//! Goal: "Add N mana in any combination of colors" (also of two or three listed types) reads whole in an activated mana ability, a triggered one and a spell, with its cost (mana, energy), and writes back as the same text; a count of one, one listed type and unknown words stay with the table.
use super::*;
use crate::mana_trigger::mana_trigger;

#[rstest]
#[case::two_of_any_color(
    "{T}: Add two mana in any combination of colors.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":2}}}}]}}"#
)]
#[case::four_for_mana(
    "{4}, {T}: Add four mana in any combination of colors.",
    r#"{"Activated":{"cost":[{"Mana":"{4}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":4}}}}]}}"#
)]
#[case::three_for_energy(
    "{T}, Pay {E}{E}{E}: Add three mana in any combination of colors.",
    r#"{"Activated":{"cost":["Tap",{"PayEnergy":3}],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":3}}}}]}}"#
)]
#[case::two_listed_types(
    "{1}, {T}: Add two mana in any combination of {R} and/or {G}.",
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":2,"among":["Red","Green"]}}}}]}}"#
)]
#[case::three_listed_types(
    "{T}: Add two mana in any combination of {U}, {B}, and/or {R}.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":2,"among":["Blue","Black","Red"]}}}}]}}"#
)]
fn the_wordings_of_mana_in_any_combination_read_and_write_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[test]
fn a_triggered_mana_ability_adds_a_combination() {
    let text = "Whenever enchanted land is tapped for mana, its controller adds an additional two mana in any combination of colors.";
    let read = golden(text, ClauseKind::Triggered, false);
    assert_eq!(
        read,
        ability(
            r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Is":"Attached"},{"Type":"Land"}]}},"effects":[{"AddMana":{"who":"Triggering","mana":{"AnyCombination":{"count":2}}}}]}}"#
        )
    );
    assert!(mana_trigger(&read).is_some());
}

#[test]
fn a_spell_adds_a_combination_with_what_else_it_does() {
    let text = "Add two mana in any combination of colors. Draw a card.";
    let read = golden(text, ClauseKind::Spell, false);
    assert_eq!(
        read,
        ability(
            r#"{"Spell":{"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":2}}}},{"Draw":{}}]}}"#
        )
    );
}

#[rstest]
#[case::one_mana("{T}: Add one mana in any combination of colors.")]
#[case::one_listed_type("{T}: Add two mana in any combination of {R}.")]
#[case::a_type_that_is_not_mana("{T}: Add two mana in any combination of {W} and/or {T}.")]
#[case::the_variable_count("{T}: Add X mana in any combination of colors, where X is its power.")]
#[case::another_word("{T}: Add two mana in any combination of lands.")]
fn near_misses_of_mana_in_any_combination_stay_with_the_table(#[case] text: &str) {
    let read = clause(text, ClauseKind::Activated, false);
    let combination = format!("{read:?}").contains("AnyCombination");
    assert!(!combination, "{text}: {read:?}");
}
