//! Goal: the mana ability wordings of the lands, artifacts and Auras that restrict or condition their mana read into the DSL and write back in the same words: an ability word before a mana ability with an activation condition, mana for the creature type the source chose, and a mana ability an Aura gives; near-misses stay with the table.
use super::*;

const FEROCIOUS: &str = r#"{"Activated":{"cost":["Tap"],"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":4}},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"effects":[{"AddMana":{"mana":{"Fixed":["Green","Green"]}}}]}}"#;

/// The ability word is flavor: the clause with it reads as the clause without it, which is written back.
#[test]
fn the_ability_word_ferocious_is_left_out_of_the_reading() {
    let text = "{T}: Add {G}{G}. Activate only if you control a creature with power 4 or greater.";
    reads_as(text, ClauseKind::Activated, false, FEROCIOUS);
    let with_word = format!("Ferocious \u{2014} {text}");
    assert_eq!(
        clause(&with_word, ClauseKind::Activated, false),
        Some(ability(FEROCIOUS))
    );
}

#[rstest]
#[case::a_creature_of_the_chosen_type(
    "{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"only":[{"Spell":{"types":["Creature"],"chosen_type":true}}]}}]}}"#
)]
#[case::a_spell_or_an_ability_of_the_chosen_type(
    "{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type or activate an ability of a creature source of the chosen type.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"only":[{"Spell":{"types":["Creature"],"chosen_type":true}},{"Ability":{"types":["Creature"],"chosen_type":true}}]}}]}}"#
)]
#[case::an_aura_gives_one_mana_of_any_color(
    "Enchanted land has \"{T}: Add one mana of any color.\"",
    ClauseKind::Static,
    r#"{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}}]}}"#
)]
#[case::an_aura_gives_two_mana_of_one_color(
    "Enchanted land has \"{T}: Add two mana of any one color.\"",
    ClauseKind::Static,
    r#"{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":2}}}]}}}]}}"#
)]
fn the_wordings_of_the_restricted_and_conditioned_mana_abilities_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::another_ability_word_before_an_activated_ability(
    "Boast \u{2014} {1}: Add {G}{G}. Activate only if you control a creature with power 4 or greater.",
    ClauseKind::Activated
)]
#[case::a_threshold_from_above(
    "Ferocious \u{2014} {T}: Add {G}{G}. Activate only if you control a creature with power 4 or less and toughness 4.",
    ClauseKind::Activated
)]
#[case::a_chosen_kind_is_no_chosen_type(
    "{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen kind.",
    ClauseKind::Activated
)]
#[case::the_chosen_type_of_an_ability_without_a_spell(
    "{T}: Add one mana of any color. Spend this mana only to activate an ability of a creature source of the chosen type.",
    ClauseKind::Activated
)]
#[case::an_ability_use_for_a_spell_without_a_chosen_type(
    "{T}: Add one mana of any color. Spend this mana only to cast a creature spell or activate an ability of a creature source of the chosen type.",
    ClauseKind::Activated
)]
#[case::an_aura_that_gives_a_mana_ability_with_text_after_it(
    "Enchanted land has \"{T}: Add one mana of any color.\" and trample",
    ClauseKind::Static
)]
fn near_misses_of_the_restricted_and_conditioned_mana_abilities_stay_with_the_table(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert!(clause(text, kind, false).is_none(), "{text}");
}
