//! Goal: any target with a fact of the turn (`ValidTgts$ Any.wasDealtDamageThisTurn`) translates as the creatures,
//! planeswalkers and battles with the property or a player the same fact holds for, never as a permanent alone; a
//! property no player can have stays a gap, one no player has (`Any.!EnchantedBy`) takes every player, and a target
//! player dealt damage this turn is a `PlayerAmong`.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn needle(valid: &str) -> String {
    format!(
        "Name:T\nManaCost:R\nTypes:Instant\nA:SP$ DealDamage | ValidTgts$ {valid} | NumDmg$ 1 | SpellDescription$ CARDNAME deals 1 damage to any target that was dealt damage this turn.\n"
    )
}

#[test]
fn dk26_any_target_dealt_damage_this_turn() {
    let script = needle("Any.wasDealtDamageThisTurn");
    assert_eq!(gaps(&script), Vec::<String>::new());
    let what = &abilities(&script)[0]["Spell"]["targets"][0]["what"];
    let kinds = what["Either"].as_array().expect("either");
    assert_eq!(kinds.len(), 2, "{what}");
    let objects = kinds[0]["Object"].to_string();
    for word in ["Creature", "Planeswalker", "Battle", "DamagedThisTurn"] {
        assert!(objects.contains(word), "{word} in {objects}");
    }
    let test = json!({"Compare": {"left": {"Tallied": {"who": "You", "what": "DamageTaken"}}, "op": "Ge", "right": 1}});
    assert_eq!(
        kinds[1],
        json!({"PlayerAmong": {"PlayersWhere": {"who": "EachPlayer", "test": test}}})
    );
}

#[rstest]
#[case::a_permanent_property("Any.tapped")]
#[case::by_the_source("Any.wasDealtDamageThisTurnBySource")]
#[case::another_negation("Any.!Opponent")]
fn dk26_any_target_of_a_property_no_player_has_stays_a_gap(#[case] valid: &str) {
    assert!(!gaps(&needle(valid)).is_empty(), "{valid}");
}

#[rstest]
#[case::not_the_enchanted_creature("Any.!EnchantedBy", "Attached")]
#[case::not_a_commander("Any.!IsCommander", "Commander")]
fn dk26_any_target_without_a_card_property_takes_every_player(
    #[case] valid: &str,
    #[case] word: &str,
) {
    let script = needle(valid);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let what = &abilities(&script)[0]["Spell"]["targets"][0]["what"];
    let kinds = what["Either"].as_array().expect("either");
    assert!(kinds[0]["Object"].to_string().contains(word), "{what}");
    assert_eq!(kinds[1], json!("Player"));
}

#[test]
fn dk26_target_player_dealt_damage_this_turn() {
    let script = needle("Player.wasDealtDamageThisTurn");
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert!(
        abilities(&script)[0]["Spell"]["targets"][0]["what"]
            .get("PlayerAmong")
            .is_some()
    );
}

const EXOSKELETON: &str = "Name:T\nTypes:Artifact Equipment\nK:Equip:2\nT:Mode$ Unattached | ValidAttachment$ Card.Self | ValidObject$ Permanent | Execute$ TrigSac | TriggerDescription$ Whenever CARDNAME becomes unattached from a permanent, sacrifice that permanent.\nSVar:TrigSac:DB$ SacrificeAll | Defined$ TriggeredObjectLKICopy | Controller$ You\n";

#[test]
fn dk26_unattached_from_a_permanent_sacrifices_it() {
    assert_eq!(gaps(EXOSKELETON), Vec::<String>::new());
    let trigger = abilities(EXOSKELETON)
        .into_iter()
        .find(|ability| ability.get("Triggered").is_some())
        .expect("a trigger");
    assert_eq!(
        trigger,
        json!({"Triggered": {"when": {"Unattached": {"what": ["This"]}}, "effects": [{"Sacrifice": {"filter": [{"Is": "Triggering"}]}}]}})
    );
}

#[rstest]
#[case::another_key(EXOSKELETON.replace("ValidObject$ Permanent", "ValidObject$ Permanent | Other$ True"))]
#[case::the_object_of_another_mode(EXOSKELETON.replace("Mode$ Unattached", "Mode$ Taps"))]
fn dk26_a_near_miss_of_the_unattachment_stays_a_gap(#[case] script: String) {
    assert!(!gaps(&script).is_empty(), "{script}");
}
