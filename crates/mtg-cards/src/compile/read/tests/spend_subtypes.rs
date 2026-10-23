//! Goal: "Spend this mana only to cast Dragon spells or activate abilities of Dragons" reads for creature types, other subtypes and card types, alone, in a list or with the abilities of the same kind, and writes back as the same words; another type for the abilities and other nouns stay with the table.
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
#[case::sources_instead_of_the_plural(
    "{T}: Add one mana of any color. Spend this mana only to cast Elemental spells or activate abilities of Elemental sources.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"only":[{"Spell":{"subtypes":["Elemental"]}},{"Ability":{"subtypes":["Elemental"]}}]}}]}}"#
)]
#[case::an_ability_of_a_source_of_a_type(
    "{T}: Add one mana of any color. Spend this mana only to cast a Dinosaur spell or activate an ability of a Dinosaur source.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"only":[{"Spell":{"subtypes":["Dinosaur"]}},{"Ability":{"subtypes":["Dinosaur"]}}]}}]}}"#
)]
#[case::a_list_of_types(
    "{T}: Add {G}. Spend this mana only to cast Vampire, Cleric, and/or Warrior spells.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"only":[{"Spell":{"subtypes":["Vampire","Cleric","Warrior"]}}]}}]}}"#
)]
#[case::a_card_type_and_its_abilities(
    "{T}: Add two mana of any one color. Spend this mana only to cast artifact spells or activate abilities of artifacts.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":2},"only":[{"Spell":{"types":["Artifact"]}},{"Ability":{"types":["Artifact"]}}]}}]}}"#
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
#[case::a_word_that_is_no_type("{T}: Add {G}. Spend this mana only to cast Cheese spells.")]
fn near_misses_of_restrictions_by_a_subtype_stay_with_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Activated, false).is_none(),
        "{text}"
    );
}
