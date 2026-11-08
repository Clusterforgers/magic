//! Goal: a target that depends on the targets chosen before it is read: cards "from their graveyard" of a target player,
//! a creature "that player" (the earlier creature's controller) controls, "another target" after a target that may be a
//! player, and a total mana value over the chosen set; a parent that is not one player or one object, and a total over
//! players, stay gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(lines: &[&str]) -> String {
    use std::fmt::Write;
    let mut script = format!("Name:T\nTypes:Sorcery\nA:SP$ {}\n", lines[0]);
    for (at, line) in lines.iter().enumerate().skip(1) {
        let _ = writeln!(script, "SVar:S{at}:DB$ {line}");
    }
    script
}

fn targets(script: &str) -> Value {
    abilities(script)[0]["Spell"]["targets"].clone()
}

#[rstest]
#[case::their_graveyard(
    "Pump | ValidTgts$ Player | IsCurse$ True | SubAbility$ S1",
    "ChangeZone | Origin$ Graveyard | Destination$ Library | Shuffle$ True | TargetMin$ 0 | TargetMax$ 3 | TargetsWithDefinedController$ ParentTarget | ValidTgts$ Card",
    json!({"OwnedBy": {"Target": 0}})
)]
#[case::that_creatures_controller(
    "Pump | ValidTgts$ Creature.OppCtrl | SubAbility$ S1",
    "Destroy | ValidTgts$ Creature | TargetUnique$ True | TargetsWithDefinedController$ ParentTargetedController",
    json!({"ControlledBy": {"ControllerOf": {"Target": 0}}})
)]
#[case::that_players_land(
    "Pump | ValidTgts$ Player | IsCurse$ True | SubAbility$ S1",
    "Destroy | ValidTgts$ Land.ControlledBy ParentTarget",
    json!({"ControlledBy": {"Target": 0}})
)]
fn a_later_target_reads_the_earlier_one(
    #[case] first: &str,
    #[case] second: &str,
    #[case] pred: Value,
) {
    let script = spell(&[first, second]);
    let found = targets(&script);
    let preds = found[1]["what"]["Object"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(preds.contains(&pred), "{found} {:?}", gaps(&script));
}

#[rstest]
#[case::a_player_parent_for_a_controller(
    "Pump | ValidTgts$ Player | IsCurse$ True | SubAbility$ S1",
    "Destroy | ValidTgts$ Creature | TargetsWithDefinedController$ ParentTargetedController",
    "key:TargetsWithDefinedController=ParentTargetedController"
)]
#[case::an_object_parent_for_a_player(
    "Pump | ValidTgts$ Creature | SubAbility$ S1",
    "ChangeZone | Origin$ Graveyard | Destination$ Hand | ValidTgts$ Card | TargetsWithDefinedController$ ParentTarget",
    "key:TargetsWithDefinedController=ParentTarget"
)]
#[case::several_parents(
    "Pump | ValidTgts$ Player | TargetMin$ 0 | TargetMax$ 2 | IsCurse$ True | SubAbility$ S1",
    "ChangeZone | Origin$ Graveyard | Destination$ Hand | ValidTgts$ Card | TargetsWithDefinedController$ ParentTarget",
    "key:TargetsWithDefinedController=ParentTarget"
)]
#[case::a_total_over_players(
    "DealDamage | ValidTgts$ Player | TargetMin$ 0 | TargetMax$ 2 | MaxTotalTargetCMC$ 3 | NumDmg$ 1",
    "",
    "key:MaxTotalTargetCMC-not-objects"
)]
fn a_parent_that_is_not_one_of_its_kind_stays_a_gap(
    #[case] first: &str,
    #[case] second: &str,
    #[case] gap: &str,
) {
    let lines: Vec<&str> = [first, second]
        .into_iter()
        .filter(|l| !l.is_empty())
        .collect();
    let script = spell(&lines);
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::any_other_target("Any", json!({"Another": {"what": "AnyTarget", "earlier": [0]}}))]
#[case::another_player("Player", json!({"Another": {"what": "Player", "earlier": [0]}}))]
fn another_target_after_a_player_leaves_out_the_earlier_target(
    #[case] second: &str,
    #[case] what: Value,
) {
    let script = spell(&[
        "DealDamage | ValidTgts$ Any | NumDmg$ 2 | SubAbility$ S1",
        &format!("DealDamage | ValidTgts$ {second} | TargetUnique$ True | NumDmg$ 1"),
    ]);
    let found = targets(&script);
    assert_eq!(found[1]["what"], what, "{:?}", gaps(&script));
}

#[test]
fn a_target_only_another_target_leaves_out_is_still_unused() {
    let script = spell(&[
        "Pump | ValidTgts$ Creature.YouCtrl | SubAbility$ S1",
        "DealDamage | ValidTgts$ Creature.YouCtrl | TargetUnique$ True | NumDmg$ 1",
    ]);
    assert!(
        gaps(&script).iter().any(|found| found == "targets:unused"),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::fixed("3", json!(3))]
#[case::paid("X", json!("X"))]
fn a_total_mana_value_bounds_the_chosen_set(#[case] most: &str, #[case] amount: Value) {
    let script = format!(
        "{}SVar:X:Count$xPaid\n",
        spell(&[&format!(
            "ChangeZone | Origin$ Graveyard | Destination$ Battlefield | TargetMin$ 0 | TargetMax$ 3 | MaxTotalTargetCMC$ {most} | ValidTgts$ Creature.YouOwn"
        )])
    );
    let found = targets(&script);
    assert_eq!(
        found[0]["what"]["Together"]["rule"],
        json!({"TotalManaValueAtMost": amount}),
        "{found} {:?}",
        gaps(&script)
    );
    assert_eq!(found[0]["bounds"], json!({"min": 0, "max": 3}));
}
