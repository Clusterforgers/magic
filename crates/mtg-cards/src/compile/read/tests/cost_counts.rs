//! Goal: a cost change that counts ("for each creature card in your graveyard", "{X} less to cast, where X is the greatest power among creatures you control") and one that names where a spell is cast from ("from anywhere other than your hand") read into a `CostModifier`, write back as the same text, and leave every near-miss to the table.
use super::*;

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

/// A cost reduction of the card itself that counts.
fn itself(per: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["This"],"change":"Reduce","mana":"{{1}}","per":{per}}}}}"#
    )
}

const CREATURE_CARDS: &str =
    r#"{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}"#;

#[rstest]
#[case::creature_cards_in_your_graveyard(
    "This spell costs {1} less to cast for each creature card in your graveyard.",
    CREATURE_CARDS
)]
#[case::land_cards(
    "This spell costs {1} less to cast for each land card in your graveyard.",
    r#"{"Count":[{"Type":"Land"},{"In":"Graveyard"},{"OwnedBy":"You"}]}"#
)]
#[case::creatures_on_the_battlefield(
    "This spell costs {1} less to cast for each creature on the battlefield.",
    r#"{"Count":[{"Type":"Creature"}]}"#
)]
#[case::creatures_you_control(
    "This spell costs {1} less to cast for each creature you control.",
    r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#
)]
#[case::creatures_opponents_control(
    "This spell costs {1} less to cast for each creature your opponents control.",
    r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}"#
)]
#[case::attacking_creatures(
    "This spell costs {1} less to cast for each attacking creature.",
    r#"{"Count":[{"Type":"Creature"},"Attacking"]}"#
)]
#[case::an_opponent(
    "This spell costs {1} less to cast for each opponent.",
    r#"{"Players":"Opponents"}"#
)]
fn a_reduction_for_each_thing_counted(#[case] text: &str, #[case] per: &str) {
    reads_as(text, ClauseKind::Static, false, &itself(per));
}

#[rstest]
#[case::two_mana(
    "This spell costs {2} less to cast for each creature card in your graveyard.",
    r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{2}","per":{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}}"#
)]
#[case::colored_mana(
    "This spell costs {G} less to cast for each creature you control.",
    r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{G}","per":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#
)]
#[case::an_increase(
    "This spell costs {1} more to cast for each creature you control.",
    r#"{"CostModifier":{"applies_to":["This"],"change":"Increase","mana":"{1}","per":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#
)]
#[case::the_cards_name(
    "~ costs {1} less to cast for each attacking creature you control.",
    r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{1}","per":{"Count":[{"Type":"Creature"},"Attacking",{"ControlledBy":"You"}]}}}"#
)]
#[case::a_kind_of_spell_you_cast(
    "Creature spells you cast cost {1} less to cast for each creature you control.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Type":"Creature"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}","per":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#
)]
#[case::a_subtype_of_spell_and_creature(
    "Dragon spells you cast cost {1} less to cast for each Dragon you control.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Subtype":"Dragon"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}","per":{"Count":[{"Subtype":"Dragon"},{"ControlledBy":"You"}]}}}"#
)]
#[case::creatures_in_your_party(
    "This spell costs {1} less to cast for each creature in your party.",
    r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{1}","per":{"Party":"You"}}}"#
)]
#[case::counters_on_the_card(
    "Creature spells you cast cost {1} less to cast for each +1/+1 counter on ~.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Type":"Creature"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}","per":{"Counters":{"on":"This","counter":"+1/+1"}}}}"#
)]
fn the_mana_and_the_spells_vary(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

fn x_less(amount: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["This"],"change":"Reduce","mana":"{{1}}","per":{amount}}}}}"#
    )
}

#[rstest]
#[case::greatest_power(
    "This spell costs {X} less to cast, where X is the greatest power among creatures you control.",
    r#"{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}"#
)]
#[case::greatest_toughness(
    "This spell costs {X} less to cast, where X is the greatest toughness among creatures you control.",
    r#"{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Toughness"}}"#
)]
#[case::total_power(
    "This spell costs {X} less to cast, where X is the total power of creatures you control.",
    r#"{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}"#
)]
#[case::total_toughness(
    "This spell costs {X} less to cast, where X is the total toughness of creatures you control.",
    r#"{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Toughness"}}"#
)]
#[case::total_mana_value_of_a_subtype(
    "This spell costs {X} less to cast, where X is the total mana value of Dragons you control.",
    r#"{"Total":{"of":[{"Subtype":"Dragon"},{"ControlledBy":"You"}],"stat":"ManaValue"}}"#
)]
fn a_reduction_where_x_is_a_number(#[case] text: &str, #[case] amount: &str) {
    reads_as(text, ClauseKind::Static, false, &x_less(amount));
}

#[rstest]
#[case::an_increase_of_spells_you_cast(
    "Spells you cast from anywhere other than your hand cost {1} more to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"You"},{"Not":{"CastFrom":"Hand"}}],"change":"Increase","mana":"{1}"}}"#
)]
#[case::a_reduction_of_two(
    "Spells you cast from anywhere other than your hand cost {2} less to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"You"},{"Not":{"CastFrom":"Hand"}}],"change":"Reduce","mana":"{2}"}}"#
)]
#[case::a_kind_of_spell(
    "Creature spells you cast from anywhere other than your hand cost {1} less to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Type":"Creature"},{"ControlledBy":"You"},{"Not":{"CastFrom":"Hand"}}],"change":"Reduce","mana":"{1}"}}"#
)]
fn spells_cast_from_anywhere_other_than_the_hand(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[test]
fn a_subtype_of_cards_in_a_graveyard_is_counted() {
    reads_as(
        "This spell costs {1} less to cast for each Elf card in your graveyard.",
        ClauseKind::Static,
        false,
        &itself(r#"{"Count":[{"Subtype":"Elf"},{"In":"Graveyard"},{"OwnedBy":"You"}]}"#),
    );
}

#[rstest]
#[case::the_number_of_with_x(
    "This spell costs {X} less to cast, where X is the number of artifacts you control."
)]
#[case::where_x_without_x(
    "This spell costs {1} less to cast, where X is the greatest power among creatures you control."
)]
#[case::x_without_where("This spell costs {X} less to cast.")]
#[case::a_party_of_another_player(
    "This spell costs {1} less to cast for each creature in their party."
)]
#[case::counted_by_a_target("This spell costs {1} less to cast for each target.")]
#[case::another_player(
    "This spell costs {1} less to cast for each creature card in their graveyard."
)]
#[case::from_elsewhere_without_you(
    "Spells cast from anywhere other than your hand cost {1} less to cast."
)]
#[case::from_the_graveyard("Spells you cast from your graveyard cost {1} less to cast.")]
#[case::a_sum_of_two_counts(
    "This spell costs {1} less to cast for each Cave you control and each Cave card in your graveyard."
)]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
