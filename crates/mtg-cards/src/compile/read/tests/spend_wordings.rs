//! Goal: restrictions by a mana value, an X in the mana cost, who owns the spell, the zone it is cast from and a second kind of spell read and write back as the card words them; a use of another shape stays with the table.
use super::*;

#[rstest]
#[case::mana_value_or_x(
    "{T}: Add {G}{U}. Spend this mana only to cast spells with mana value 5 or greater or spells with {X} in their mana costs.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green","Blue"]},"only":[{"Spell":{"min_value":5}},{"Spell":{"facts":["XInCost"]}}]}}]}}"#
)]
#[case::spells_you_dont_own(
    "{T}, Pay 1 life: Add two mana of any one color. Spend this mana only to cast spells you don't own.",
    r#"{"Activated":{"cost":["Tap",{"PayLife":1}],"effects":[{"AddMana":{"mana":{"AnyColor":2},"only":[{"Spell":{"facts":["NotOwned"]}}]}}]}}"#
)]
#[case::a_spell_from_your_graveyard(
    "Pay 1 life: Add {C}. Spend this mana only to cast a spell from your graveyard.",
    r#"{"Activated":{"cost":[{"PayLife":1}],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"only":[{"Spell":{"facts":["FromGraveyard"]}}]}}]}}"#
)]
#[case::chosen_type_or_legendary(
    "{T}: Add {G}. Spend this mana only to cast a creature spell of the chosen type or a legendary creature spell.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"only":[{"Spell":{"types":["Creature"],"chosen_type":true}},{"Spell":{"types":["Creature"],"supertypes":["Legendary"]}}]}}]}}"#
)]
#[case::instant_and_or_sorcery(
    "{T}: Add {U}{B}{R}. Spend this mana only to cast instant and/or sorcery spells.",
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Blue","Black","Red"]},"only":[{"Spell":{"types":["Instant","Sorcery"]}}]}}]}}"#
)]
fn restrictions_by_value_cost_owner_zone_and_a_second_kind_read(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::a_value_without_the_comparison(
    "{T}: Add {G}. Spend this mana only to cast spells with mana value 5."
)]
#[case::a_second_kind_of_ability(
    "{T}: Add {G}. Spend this mana only to cast spells you don't own or activate abilities of artifacts."
)]
#[case::an_unknown_zone("{T}: Add {G}. Spend this mana only to cast a spell from your hand.")]
fn near_misses_of_restrictions_by_value_cost_owner_and_zone_stay_with_the_table(
    #[case] text: &str,
) {
    assert!(
        clause(text, ClauseKind::Activated, false).is_none(),
        "{text}"
    );
}
