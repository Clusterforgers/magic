//! Goal: piece P1 (selectors): the world holds the names of the checkout's cards and tokens, and each property atom maps
//! to the predicate that means what Forge's atom means or stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, world};

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
