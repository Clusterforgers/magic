//! Goal: an enchantment that becomes an Aura with enchant creature on its own entering, manifests and attaches itself to the manifested card reads as the three effects and writes back as the card's text; the same words in a trigger of another object, or with another enchant, stay with the table.
use super::*;

const AURA: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Continuous":{"affected":"This","mods":[{"AddSubtype":"Aura"},{"AddKeyword":{"Enchant":[{"Type":"Creature"}]}}],"duration":"Forever"}},{"Manifest":{"as":"manifested"}},{"Attach":{"what":"This","to":{"Bound":"manifested"}}}]}}"#;

#[rstest]
#[case::enchantment(
    "When this enchantment enters, it becomes an Aura with enchant creature. Manifest the top card of your library and attach this enchantment to it."
)]
#[case::named(
    "When ~ enters, it becomes an Aura with enchant creature. Manifest the top card of your library and attach ~ to it."
)]
fn the_card_becomes_an_aura_on_a_manifested_card(#[case] text: &str) {
    reads_as(text, ClauseKind::Triggered, false, AURA);
}

#[rstest]
#[case::another_creature(
    "Whenever another creature enters, it becomes an Aura with enchant creature. Manifest the top card of your library and attach this enchantment to it."
)]
#[case::enchant_land(
    "When this enchantment enters, it becomes an Aura with enchant land. Manifest the top card of your library and attach this enchantment to it."
)]
#[case::no_attach(
    "When this enchantment enters, it becomes an Aura with enchant creature. Manifest the top card of your library."
)]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
