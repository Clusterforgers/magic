//! Goal: a token's `PumpKeywords$` grants the tokens just made their keywords for the stated time, and a group line aimed at a player ("creatures target player controls") acts on that player's objects only.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

fn first_effects(text: &str) -> Vec<Value> {
    let found = abilities(text);
    assert!(!found.is_empty(), "{:?}", gaps(text));
    found[0]["Spell"]["effects"].as_array().unwrap().clone()
}

#[rstest]
#[case::haste("PumpKeywords$ Haste | PumpDuration$ EOT", json!([{"AddKeyword": "Haste"}]), "UntilEndOfTurn")]
#[case::two("PumpKeywords$ Menace & Haste | PumpDuration$ EOT", json!([{"AddKeyword": "Menace"}, {"AddKeyword": "Haste"}]), "UntilEndOfTurn")]
fn pumped_tokens_gain_the_keywords(#[case] extra: &str, #[case] mods: Value, #[case] until: &str) {
    let effects = first_effects(&spell(&format!(
        "Token | TokenScript$ w_1_1_soldier | TokenOwner$ You | {extra}"
    )));
    assert_eq!(effects.len(), 2, "{effects:?}");
    let name = effects[0]["CreateToken"]["as"].clone();
    assert_eq!(
        effects[1],
        json!({"Continuous": {"affected": {"Bound": name}, "mods": mods, "duration": until}})
    );
}

#[rstest]
#[case::unknown_duration("PumpKeywords$ Haste | PumpDuration$ Whenever", "duration:Whenever")]
#[case::duration_alone("PumpDuration$ EOT", "key:Token.PumpDuration")]
fn an_unread_pump_stays_a_gap(#[case] extra: &str, #[case] gap: &str) {
    let text = spell(&format!("Token | TokenScript$ w_1_1_soldier | {extra}"));
    assert!(gaps(&text).iter().any(|g| g == gap), "{:?}", gaps(&text));
}

#[rstest]
#[case::pump(
    "PumpAll | ValidTgts$ Player | ValidCards$ Creature | NumAtt$ -2 | NumDef$ -2 | IsCurse$ True"
)]
#[case::opponent("PumpAll | ValidTgts$ Opponent | ValidCards$ Creature | NumAtt$ 1")]
fn a_group_of_a_players_creatures_is_limited_to_that_player(#[case] line: &str) {
    let effects = first_effects(&spell(line));
    let each = &effects[0]["Continuous"]["affected"]["Each"];
    assert!(
        each.as_array()
            .unwrap()
            .contains(&json!({"ControlledBy": {"Target": 0}})),
        "{effects:?}"
    );
}

#[test]
fn control_of_all_of_a_players_creatures() {
    let text = spell(
        "GainControl | ValidTgts$ Opponent | AllValid$ Creature.TargetedPlayerCtrl | NewController$ You",
    );
    let effects = first_effects(&text);
    let each = effects[0]["Continuous"]["affected"]["Each"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(
        each.iter()
            .filter(|p| p.get("ControlledBy").is_some())
            .count(),
        1,
        "{effects:?}"
    );
}

#[rstest]
#[case::creature_target("PumpAll | ValidTgts$ Creature | ValidCards$ Creature | NumAtt$ 1")]
fn a_group_aimed_at_something_else_is_a_gap(#[case] line: &str) {
    assert!(
        gaps(&spell(line))
            .iter()
            .any(|g| g == "key:Group.ValidTgts-not-a-player")
    );
}

#[rstest]
#[case::targeted("Mana | ValidTgts$ Player | Defined$ Targeted | Produced$ C | Amount$ 2", Some(json!({"Target": 0})))]
#[case::you("Mana | Produced$ C | Amount$ 2", None)]
fn the_mana_goes_to_the_named_player(#[case] line: &str, #[case] who: Option<Value>) {
    let effects = first_effects(&spell(line));
    assert_eq!(
        effects[0]["AddMana"].get("who").cloned(),
        who,
        "{effects:?}"
    );
}

#[rstest]
#[case::remembered(
    "Mana | Defined$ Remembered | Produced$ C",
    "key:Mana.Defined=Remembered"
)]
fn an_unread_pool_owner_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert!(gaps(&spell(line)).iter().any(|g| g == gap));
}

#[rstest]
#[case::default("", None)]
#[case::you("Controller$ You", None)]
#[case::target_controller("Controller$ TargetedController", Some(json!({"ControllerOf": {"Target": 0}})))]
fn the_copy_is_made_for_the_named_player(#[case] extra: &str, #[case] who: Option<Value>) {
    let effects = first_effects(&spell(&format!(
        "CopyPermanent | ValidTgts$ Creature | {extra}"
    )));
    assert_eq!(
        effects[0]["CreateToken"].get("who").cloned(),
        who,
        "{effects:?}"
    );
}

#[test]
fn an_unread_copy_owner_is_a_gap() {
    let text = spell("CopyPermanent | ValidTgts$ Creature | Controller$ Opponent");
    assert!(
        gaps(&text)
            .iter()
            .any(|g| g == "key:CopyPermanent.Controller=Opponent")
    );
}

fn copier(controller: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Wizard\nPT:1/1\nT:Mode$ SpellCast | ValidCard$ Instant | Execute$ Copy | TriggerZones$ Battlefield | TriggerDescription$ x\nSVar:Copy:DB$ CopySpellAbility | Defined$ TriggeredSpellAbility | Controller$ {controller}\n"
    )
}

#[rstest]
#[case::caster("TriggeredCardController", Some(json!("TriggeringPlayer")))]
#[case::you("You", None)]
fn the_copy_is_controlled_by_the_named_player(#[case] who: &str, #[case] expected: Option<Value>) {
    let found = abilities(&copier(who));
    assert_eq!(found.len(), 1, "{:?}", gaps(&copier(who)));
    let copy = &found[0]["Triggered"]["effects"][0]["CopySpell"];
    assert_eq!(copy.get("who").cloned(), expected, "{copy}");
}

#[test]
fn another_copy_controller_is_a_gap() {
    assert!(
        gaps(&copier("TargetedOrController"))
            .iter()
            .any(|g| g == "key:CopySpellAbility.Controller")
    );
}
