//! Goal: a `CantBlockBy` line with a defender that has counters maps to the unblockable static with a condition on the
//! defending player, and any other defender, attacker or blocker filter stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps, normal};

fn script(line: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\nS:{line}\n")
}

#[rstest]
#[case::rad("RAD", json!({"Other": "rad"}))]
#[case::poison("POISON", json!("poison"))]
fn unblockable_while_the_defender_has_counters(
    #[case] word: &str,
    #[case] kind: serde_json::Value,
) {
    let line = format!(
        "Mode$ CantBlockBy | ValidAttacker$ Card.Self | ValidDefender$ Player.Condition Counters.{word}"
    );
    assert_eq!(
        abilities(&script(&line)),
        vec![normal(json!({"Static": {
            "affected": "This",
            "mods": ["CantBeBlocked"],
            "condition": {"Compare": {"left": {"Counters": {"on": "Defending", "counter": kind}}, "op": "Ge", "right": 1}},
        }}))]
    );
}

#[rstest]
#[case::another_defender(
    "Mode$ CantBlockBy | ValidAttacker$ Card.Self | ValidDefender$ Player.Opponent"
)]
#[case::another_attacker(
    "Mode$ CantBlockBy | ValidAttacker$ Creature.Other | ValidDefender$ Player.Condition Counters.RAD"
)]
#[case::a_blocker(
    "Mode$ CantBlockBy | ValidAttacker$ Card.Self | ValidBlocker$ Creature.Red | ValidDefender$ Player.Condition Counters.RAD"
)]
fn other_shapes_stay_gaps(#[case] line: &str) {
    assert_eq!(
        gaps(&script(line)),
        vec!["key:Static.CantBlockBy.ValidDefender".to_owned()]
    );
}

#[test]
fn an_etb_counter_line_with_an_empty_condition_field_has_no_condition() {
    let script = "Name:T\nTypes:Creature Elf\nK:etbCounter:P1P1:X::Enters with counters.\nSVar:X:Count$TotalCommanderCastFromCommandZone/Twice\n";
    let found = abilities(script);
    let text = serde_json::to_string(&found).unwrap_or_default();
    assert!(text.contains(r#""WithCounters""#) && !text.contains("condition"), "{text}");
}

#[test]
fn an_etb_counter_line_with_a_condition_stays_a_gap() {
    let script = "Name:T\nTypes:Creature Elf\nK:etbCounter:P1P1:2:Foo$ Bar\n";
    assert_eq!(gaps(script), vec!["keyword:etbCounter-condition".to_owned()]);
}

#[rstest]
#[case::player_and_permanent("Player,Permanent", true)]
#[case::permanent_and_player("Permanent,Player", true)]
#[case::opponent_and_permanent("Opponent,Permanent", false)]
fn damage_once_to_players_and_permanents_names_no_recipient(
    #[case] valid: &str,
    #[case] anywhere: bool,
) {
    let script = format!(
        "Name:T\nTypes:Creature Elf\nT:Mode$ DamageDoneOnce | ValidSource$ Card.RedSource+YouCtrl | ValidTarget$ {valid} | TriggerZones$ Battlefield | Execute$ Go\nSVar:Go:DB$ Draw | NumCards$ 1\n"
    );
    let found = abilities(&script);
    let text = serde_json::to_string(&found).unwrap_or_default();
    assert_eq!(text.contains(r#""Batch""#) && !text.contains(r#""to""#), anywhere, "{text}");
    if !anywhere {
        assert_eq!(
            gaps(&script),
            vec!["key:Trigger.DamageDoneOnce.ValidTarget-or".to_owned()]
        );
    }
}

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Instant\nA:SP$ Sacrifice | {line}\n")
}

#[test]
fn the_creature_with_the_greatest_power_of_the_targeted_player_is_sacrificed() {
    let found = abilities(&spell(
        "ValidTgts$ Opponent | SacValid$ Creature.greatestPowerControlledByTargeted",
    ));
    let text = serde_json::to_string(&found).unwrap_or_default();
    assert!(
        text.contains(r#"{"ControlledBy":{"Target":0}}"#) && text.contains("GreatestAmong"),
        "{text}"
    );
}

#[test]
fn a_targeted_word_without_a_player_target_stays_a_gap() {
    let found = gaps(&spell(
        "Defined$ You | SacValid$ Creature.greatestPowerControlledByTargeted",
    ));
    assert_eq!(
        found,
        vec!["valid:greatestPowerControlledByTargeted".to_owned()]
    );
}

#[rstest]
#[case::you("You", true)]
#[case::opponent("Opponent", false)]
fn creatures_that_dealt_combat_damage_to_you_this_turn(#[case] who: &str, #[case] read: bool) {
    let line =
        format!("Defined$ Player.Opponent | SacValid$ Creature.dealtCombatDamageThisTurn {who}");
    let found = abilities(&spell(&line));
    let text = serde_json::to_string(&found).unwrap_or_default();
    assert_eq!(
        text.contains(r#""event":"Damaged""#) && text.contains(r#""combat":true"#),
        read,
        "{text}"
    );
}
