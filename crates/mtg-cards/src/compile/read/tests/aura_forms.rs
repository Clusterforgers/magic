//! Goal: the Auras that make the creature they enchant a creature type with a base size (and take its abilities, give it keywords, make it goaded) read into one list of changes in the order the layers need, write back in the order each card words them, and leave a near miss to the table.
use super::*;

#[rstest]
#[case::a_type_and_a_size_then_the_loss(
    "Enchanted creature is a Treefolk with base power and toughness 0/4 and loses all abilities.",
    r#"{"Static":{"affected":"Attached","mods":[{"SetCreatureTypes":["Treefolk"]},{"SetPT":{"power":0,"toughness":4}},"LoseAbilities"]}}"#
)]
#[case::the_loss_first_then_a_card_type(
    "Enchanted creature loses all abilities and is a green Elk creature with base power and toughness 3/3.",
    r#"{"Static":{"affected":"Attached","mods":[{"SetColors":["Green"]},{"SetTypes":["Creature"]},{"SetCreatureTypes":["Elk"]},{"SetPT":{"power":3,"toughness":3}},"LoseAbilities"]}}"#
)]
#[case::a_keyword_after_the_loss_of_the_others(
    "Enchanted creature is an Insect artifact creature with base power and toughness 0/1 and has indestructible, and it loses all other abilities, card types, and creature types.",
    r#"{"Static":{"affected":"Attached","mods":[{"SetTypes":["Artifact","Creature"]},{"SetCreatureTypes":["Insect"]},{"SetPT":{"power":0,"toughness":1}},"LoseAbilities",{"AddKeyword":"Indestructible"}]}}"#
)]
#[case::keywords_and_goaded(
    "Enchanted creature is a black Dragon with base power and toughness 4/2, has flying and deathtouch, and is goaded.",
    r#"{"Static":{"affected":"Attached","mods":[{"SetColors":["Black"]},{"SetCreatureTypes":["Dragon"]},{"SetPT":{"power":4,"toughness":2}},{"AddKeyword":"Flying"},{"AddKeyword":"Deathtouch"},"Goaded"]}}"#
)]
fn an_aura_that_remakes_its_creature_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::no_size("Enchanted creature is a Treefolk and loses all abilities.")]
#[case::no_type("Enchanted creature is a green creature with base power and toughness 3/3.")]
#[case::the_other_losses_without_a_card_type(
    "Enchanted creature is a Dragon with base power and toughness 4/2 and has flying, and it loses all other abilities, card types, and creature types."
)]
#[case::a_size_that_is_no_number(
    "Enchanted creature is a Treefolk with base power and toughness X/4."
)]
#[case::a_group(
    "Creatures you control are Treefolk with base power and toughness 0/4 and lose all abilities."
)]
#[case::the_card_itself(
    "This creature is a Treefolk with base power and toughness 0/4 and loses all abilities."
)]
fn near_misses_are_left_to_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
