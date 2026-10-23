//! Goal: "Spend this mana only to cast Dragon spells or activate abilities of Dragons" reads for a creature type or other subtype, alone or with the abilities of the same type in the plural, and writes back as the same words; a second type, another type for the abilities and other nouns stay with the table.
use super::*;

#[rstest]
#[case::spells_of_a_type(
    "{T}: Add {G}. Spend this mana only to cast Elf spells.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"only":[{"Spell":{"subtypes":["Elf"]}}]}}]}}"#
)]
#[case::spells_and_abilities_of_a_type(
    "{T}: Add one mana of any color. Spend this mana only to cast Elemental spells or activate abilities of Elementals.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"only":[{"Spell":{"subtypes":["Elemental"]}},{"Ability":{"subtypes":["Elemental"]}}]}}]}}"#
)]
#[case::an_irregular_plural(
    "{T}: Add {G}. Spend this mana only to cast Elf spells or activate abilities of Elves.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"only":[{"Spell":{"subtypes":["Elf"]}},{"Ability":{"subtypes":["Elf"]}}]}}]}}"#
)]
#[case::the_storage_land_of_dragons(
    "{T}, Remove X storage counters from this land: Add X mana in any combination of colors. Spend this mana only to cast Dragon spells or activate abilities of Dragons.",
    r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"counter":{"Other":"storage"},"count":"X"}}],"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":"X"}},"only":[{"Spell":{"subtypes":["Dragon"]}},{"Ability":{"subtypes":["Dragon"]}}]}}]}}"#
)]
fn restrictions_by_a_subtype_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[test]
fn a_spell_of_a_type_is_also_written_in_the_singular() {
    let plural = "{T}: Add {G}. Spend this mana only to cast Elf spells.";
    let singular = "{T}: Add {G}. Spend this mana only to cast an Elf spell.";
    assert_eq!(
        clause(singular, ClauseKind::Activated, false),
        clause(plural, ClauseKind::Activated, false)
    );
}

#[rstest]
#[case::abilities_of_another_type(
    "{T}: Add {G}. Spend this mana only to cast Elf spells or activate abilities of Dragons."
)]
#[case::sources_instead_of_the_plural(
    "{T}: Add {G}. Spend this mana only to cast Elemental spells or activate abilities of Elemental sources."
)]
#[case::a_list_of_types(
    "{T}: Add {G}. Spend this mana only to cast Vampire, Cleric, and/or Warrior spells."
)]
#[case::a_word_that_is_no_type("{T}: Add {G}. Spend this mana only to cast Cheese spells.")]
fn near_misses_of_restrictions_by_a_subtype_stay_with_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Activated, false).is_none(),
        "{text}"
    );
}
