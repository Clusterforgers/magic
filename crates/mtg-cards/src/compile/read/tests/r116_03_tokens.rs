//! Goal: a noncreature token without a size reads and writes back with its name before its abilities and no colour word
//! for a land ("a tapped Planet land token named New Planet with "{T}: ..."", CR 111.4), the tokens named after cards
//! read from their files, and a creature token without a size or a token without a card type stays with the table.
use super::*;

const PLANET: &str = r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"New Planet","types":["Land"],"subtypes":["Planet"],"abilities":[{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}]}},"tapped":true}}]}}"#;

#[rstest]
#[case::planet(
    "Create a tapped Planet land token named New Planet with \"{T}: Add one mana of any color.\"",
    PLANET
)]
#[case::equipment(
    "Create a colorless Equipment artifact token named Neuroflux Heirloom.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Neuroflux Heirloom","types":["Artifact"],"subtypes":["Equipment"]}}}}]}}"#
)]
#[case::black_lotus(
    "Create a Black Lotus token.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"black_lotus"}}}]}}"#
)]
#[case::scryb_sprites(
    "Create ten Scryb Sprites tokens.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"scryb_sprites"},"count":10}}]}}"#
)]
fn r116_a_sizeless_or_card_named_token_reads(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case(
    "Create a tapped Planet land token named New Planet with \"{T}: Add one mana of any color.\""
)]
#[case("Create a Planet land token named New Planet.")]
#[case("Create a colorless Equipment artifact token named Neuroflux Heirloom.")]
#[case("Create a Black Lotus token.")]
#[case("Create a Giant Badger token.")]
#[case("Create a Lord of the Pit token.")]
#[case("Create ten Scryb Sprites tokens.")]
fn r116_a_sizeless_or_card_named_token_writes_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::creature_without_size("Create a tapped Planet creature token named New Planet.")]
#[case::no_card_type("Create a tapped Planet token named New Planet.")]
#[case::creature_land_without_size("Create a Dryad land creature token named Grove.")]
fn r116_a_near_miss_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}
