//! Goal: piece P1 (selectors): the world holds the names of the checkout's cards and tokens, and each property atom maps
//! to the predicate that means what Forge's atom means or stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, translate_script, world};
use crate::forge::script::Script;

#[test]
fn world_names_hold_tokens() {
    assert!(world().names.contains("Soldier Token"));
}

/// The selector `spec` translates to, or its gaps.
fn selected(spec: &str) -> Result<Value, Vec<String>> {
    let script = format!(
        "Name:T\nTypes:Creature Elf\nS:Mode$ Continuous | Affected$ {spec} | AddPower$ 1\nSVar:X:Count$xPaid\n"
    );
    let found = abilities(&script);
    if found.len() != 1 {
        return Err(gaps(&script));
    }
    let each = found[0]["Static"]["affected"]["Each"].clone();
    assert!(serde_json::from_value::<mtg_cards::refs::Selector>(each.clone()).is_ok());
    Ok(each)
}

fn maps(spec: &str, expected: Value) {
    assert_eq!(selected(spec), Ok(expected), "{spec}");
}

fn refuses(spec: &str, gap: &str) {
    assert_eq!(selected(spec), Err(vec![gap.to_owned()]), "{spec}");
}

#[rstest]
#[case::monstrous("Card.IsMonstrous", json!(["Monstrous"]))]
#[case::renowned("Creature.IsRenowned", json!([{"Type": "Creature"}, "Renowned"]))]
#[case::face_down("Creature.faceDown", json!([{"Type": "Creature"}, "FaceDown"]))]
#[case::face_up("Card.faceUp", json!([{"Not": "FaceDown"}]))]
#[case::not_colorless("Permanent.nonColorless", json!([{"Not": "Colorless"}]))]
#[case::unblocked("Creature.attacking+unblocked", json!([{"Type": "Creature"}, "Attacking", "Unblocked"]))]
#[case::paired("Creature.PairedWith", json!([{"Type": "Creature"}, {"PairedWith": "This"}]))]
#[case::milled("Card.milledThisTurn", json!(["MilledThisTurn"]))]
#[case::mono("Creature.MonoColor", json!([{"Type": "Creature"}, {"Compare": {"stat": "Colors", "op": "Eq", "to": 1}}]))]
#[case::colors("Card.numColorsEQ2", json!([{"Compare": {"stat": "Colors", "op": "Eq", "to": 2}}]))]
#[case::base_power("Creature.basePowerEQ1", json!([{"Type": "Creature"}, {"Compare": {"stat": "BasePower", "op": "Eq", "to": 1}}]))]
fn state_atoms_map(#[case] spec: &str, #[case] expected: Value) {
    maps(spec, expected);
}

#[rstest]
#[case::aura_on_creature(
    "Aura.AttachedTo Creature.YouCtrl",
    json!([{"Subtype": "Aura"}, {"AttachedTo": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}])
)]
#[case::creature_with_aura(
    "Creature.EnchantedBy Aura.YouCtrl",
    json!([{"Type": "Creature"}, {"HasAttached": [{"Subtype": "Aura"}, {"ControlledBy": "You"}]}])
)]
#[case::equipped_by_any(
    "Creature.EquippedBy Card.YouCtrl",
    json!([{"Type": "Creature"}, {"HasAttached": [{"ControlledBy": "You"}, {"Subtype": "Equipment"}]}])
)]
#[case::attacking_opponent(
    "Creature.attacking Opponent",
    json!([{"Type": "Creature"}, {"AttackingPlayer": "Opponents"}])
)]
#[case::controlled_by_enchanted_controller(
    "Creature.ControlledBy EnchantedController",
    json!([{"Type": "Creature"}, {"ControlledBy": {"ControllerOf": "Attached"}}])
)]
#[case::owned_by_defender(
    "Creature.OwnedBy DefendingPlayer",
    json!([{"Type": "Creature"}, {"OwnedBy": "Defending"}])
)]
#[case::enchanted_player_ctrl(
    "Creature.EnchantedPlayerCtrl",
    json!([{"Type": "Creature"}, {"ControlledBy": "AttachedPlayer"}])
)]
fn attachment_and_control_words_map(#[case] spec: &str, #[case] expected: Value) {
    maps(spec, expected);
}

#[rstest]
#[case::at_least_one("Card.counters_GE1_P1P1", json!([{"HasCounter": "+1/+1"}]))]
#[case::none("Card.counters_EQ0_TIME", json!([{"Not": {"HasCounter": {"Other": "time"}}}]))]
#[case::fewer_than_one("Card.counters_LT1_P1P1", json!([{"Not": {"HasCounter": "+1/+1"}}]))]
#[case::more_than_zero("Card.counters_GT0_M1M1", json!([{"HasCounter": "-1/-1"}]))]
#[case::three_or_more(
    "Card.counters_GE3_P1P1",
    json!([{"CounterCount": {"kind": "+1/+1", "op": "Ge", "to": 3}}])
)]
#[case::at_most_three_includes_none(
    "Card.counters_LE3_LEVEL",
    json!([{"CounterCount": {"kind": {"Other": "level"}, "op": "Le", "to": 3}}])
)]
#[case::exactly_two(
    "Card.counters_EQ2_CHARGE",
    json!([{"CounterCount": {"kind": "charge", "op": "Eq", "to": 2}}])
)]
fn counter_counts_map(#[case] spec: &str, #[case] expected: Value) {
    maps(spec, expected);
}

#[rstest]
#[case::no_number("Card.counters_GE_M1M1", "valid:counters_GE_M1M1")]
#[case::unknown_operator("Card.counters_XX2_P1P1", "valid:counters_XX2_P1P1")]
#[case::lower_case_kind("Card.counters_GE2_Oil", "valid:counters_GE2_Oil")]
fn counter_counts_that_are_not_numbers_stay_gaps(#[case] spec: &str, #[case] gap: &str) {
    refuses(spec, gap);
}

#[rstest]
#[case::surge("Card.Self+surged", json!(["This", {"CastWith": "Surge"}]))]
#[case::prowl("Card.prowled", json!([{"CastWith": "Prowl"}]))]
#[case::named("Card.namedSoldier Token", json!([{"Named": "Soldier Token"}]))]
#[case::semicolon_alternatives(
    "Creature;Artifact",
    json!([{"AnyOf": [{"Type": "Creature"}, {"Type": "Artifact"}]}])
)]
fn cast_ways_names_and_alternatives_map(#[case] spec: &str, #[case] expected: Value) {
    maps(spec, expected);
}

#[test]
fn each_semicolon_alternative_keeps_its_own_properties() {
    let found = selected("Creature.Other;Artifact")
        .unwrap_or_default()
        .to_string();
    // "another creature or an artifact": the artifact may be the source itself.
    assert!(
        found.contains(r#"{"Type":"Artifact"}"#) && found.contains(r#""Other""#),
        "{found}"
    );
    assert!(
        !found.contains(r#"[{"Type":"Creature"},"Other""#),
        "{found}"
    );
}

#[rstest]
#[case::bare_first_name("Card.namedAjani", "valid:namedAjani")]
#[case::broken_split("Card.namedPage", "valid:namedPage")]
fn an_unknown_name_stays_a_gap(#[case] spec: &str, #[case] gap: &str) {
    refuses(spec, gap);
}

#[test]
fn a_semicolon_in_a_name_is_a_comma() {
    let mut world = world();
    world.names.insert("Nissa, Genesis Mage".to_owned());
    let script = Script::parse(
        "Name:T\nTypes:Creature Elf\nS:Mode$ Continuous | Affected$ Card.namedNissa; Genesis Mage | AddPower$ 1\n",
    );
    let out = translate_script(&world, &script);
    let face = &out.faces[0];
    assert_eq!(
        face.abilities[0]["Static"]["affected"]["Each"],
        json!([{"Named": "Nissa, Genesis Mage"}])
    );
}

#[rstest]
#[case::noted_player(
    "Creature.ControlledBy Player.NotedForWar",
    "valid:ControlledBy Player.NotedForWar"
)]
#[case::unknown_attached("Aura.AttachedTo Bogus.Thing", "defined:Bogus.Thing")]
fn unknown_words_stay_gaps(#[case] spec: &str, #[case] gap: &str) {
    refuses(spec, gap);
}

#[rstest]
#[case::blocked_is_not_unblocked("Creature.blocked", "valid:blocked")]
#[case::colors_bound_unknown("Card.numColorsEQX", "valid:numColorsEQX")]
#[case::kicked_option("Card.kicked 1", "valid:kicked 1")]
fn state_atoms_that_differ_stay_gaps(#[case] spec: &str, #[case] gap: &str) {
    refuses(spec, gap);
}
