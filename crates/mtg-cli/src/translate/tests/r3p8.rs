//! Goal: replacement and mana lines of round 3 map to the nodes that carry them (a depletion land that stays tapped, a half prevention) and the shapes the nodes cannot carry stay gaps.
use mtg_cards::ability::Ability;
use rstest::rstest;

use super::{abilities, gaps};

#[rstest]
#[case::a_depletion_land_with_counters_stays_tapped(
    "Name:T\nTypes:Land\nR:Event$ Untap | ActiveZones$ Battlefield | ValidCard$ Card.Self+counters_GE1_DEPLETION | ValidStepTurnToController$ You | Layer$ CantHappen | Description$ x\n",
    "DoesntUntap"
)]
#[case::half_the_damage_to_you(
    "Name:T\nTypes:Creature Angel\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ You | ReplaceWith$ D | PreventionEffect$ True\nSVar:D:DB$ ReplaceDamage | Amount$ S\nSVar:S:ReplaceCount$DamageAmount/HalfUp\n",
    "PreventHalf"
)]
fn lines_map_to_nodes_that_parse(#[case] script: &str, #[case] node: &str) {
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert!(found[0].to_string().contains(node), "{}", found[0]);
    assert!(serde_json::from_value::<Ability>(found[0].clone()).is_ok());
}

#[rstest]
#[case::half_rounded_down(
    "Name:T\nTypes:Creature Angel\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ You | ReplaceWith$ D | PreventionEffect$ True\nSVar:D:DB$ ReplaceDamage | Amount$ S\nSVar:S:ReplaceCount$DamageAmount/HalfDown\n"
)]
#[case::mana_that_adds_a_counter(
    "Name:T\nTypes:Land\nA:AB$ Mana | Cost$ T | Produced$ Any | AddsCounters$ Creature.nonHuman_P1P1_1\n"
)]
#[case::a_nocounter_value_it_does_not_know(
    "Name:T\nTypes:Land\nA:AB$ Mana | Cost$ T | Produced$ Any | AddsNoCounter$ Spell.Creature\n"
)]
fn shapes_the_nodes_cannot_carry_stay_gaps(#[case] script: &str) {
    assert!(abilities(script).is_empty(), "{:?}", abilities(script));
    assert_ne!(gaps(script).len(), 0);
}

#[rstest]
#[case::the_chosen_color(
    "Name:T\nTypes:Land\nA:AB$ Mana | Cost$ T | Produced$ Chosen | SpellDescription$ x\n",
    r#""ChosenColor":1"#
)]
#[case::the_chosen_color_twice(
    "Name:T\nTypes:Land\nA:AB$ Mana | Cost$ T | Produced$ ChosenColor | Amount$ 2\n",
    r#""ChosenColor":2"#
)]
#[case::a_color_or_the_chosen_one(
    "Name:T\nTypes:Land\nA:AB$ Mana | Cost$ T | Produced$ Combo W Chosen\n",
    r#""OrChosenColor":"White""#
)]
#[case::the_commanders_identity(
    "Name:T\nTypes:Land\nA:AB$ Mana | Cost$ T | Produced$ Combo ColorIdentity\n",
    r#""CommanderIdentity":1"#
)]
#[case::a_type_a_land_could_produce(
    "Name:T\nTypes:Artifact\nA:AB$ ManaReflected | Cost$ T | ColorOrType$ Type | Valid$ Land.YouCtrl | ReflectProperty$ Produce\n",
    r#""colorless":true"#
)]
#[case::a_color_a_land_could_produce(
    "Name:T\nTypes:Land\nA:AB$ ManaReflected | Cost$ T | ColorOrType$ Color | Valid$ Land.OppCtrl | ReflectProperty$ Produce\n",
    r#""count":1}}}}"#
)]
fn mana_of_a_chosen_or_reflected_kind(#[case] script: &str, #[case] holds: &str) {
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert!(found[0].to_string().contains(holds), "{}", found[0]);
    assert!(serde_json::from_value::<Ability>(found[0].clone()).is_ok());
}

#[rstest]
#[case::colors_among_permanents(
    "Name:T\nTypes:Land\nA:AB$ ManaReflected | Cost$ T | ColorOrType$ Color | Valid$ Permanent.Legendary+YouCtrl | ReflectProperty$ Is\n"
)]
#[case::chosen_color_of_an_unwritten_amount(
    "Name:T\nTypes:Land\nA:AB$ Mana | Cost$ T | Produced$ Chosen | Amount$ X\nSVar:X:Count$Valid Creature.YouCtrl\n"
)]
fn reflections_and_amounts_that_are_not_carried_stay_gaps(#[case] script: &str) {
    assert!(abilities(script).is_empty(), "{:?}", abilities(script));
    assert_ne!(gaps(script).len(), 0);
}
