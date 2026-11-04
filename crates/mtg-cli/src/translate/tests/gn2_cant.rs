//! Goal: the static modes with a subject beyond a `Ref` (casts, activations, triggers, spell targets, attack and block pairs) become `GameRule::Cant` and statics the engine has, and every shape the DSL cannot say stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn card(line: &str) -> String {
    format!("Name:T\nTypes:Artifact\nS:{line}\n")
}

fn rule(cant: &Value) -> Value {
    normal(json!({"Rule": {"rule": {"Cant": cant}}}))
}

#[rstest]
#[case::once_each_turn(
    "Mode$ CantBeCast | ValidCard$ Card | Caster$ Player | NumLimitEachTurn$ 1",
    json!({"who": "EachPlayer", "action": {"Cast": {"more_than": 1}}})
)]
#[case::opponents_nonartifact(
    "Mode$ CantBeCast | ValidCard$ Card.nonArtifact | Caster$ Opponent | NumLimitEachTurn$ 1",
    json!({"who": "Opponents", "action": {"Cast": {"filter": [{"Not": {"Type": "Artifact"}}], "more_than": 1}}})
)]
#[case::creatures_never(
    "Mode$ CantBeCast | ValidCard$ Creature | Caster$ You",
    json!({"who": "You", "action": {"Cast": {"filter": [{"Type": "Creature"}]}}})
)]
#[case::activations_of_artifacts(
    "Mode$ CantBeActivated | ValidCard$ Artifact | ValidSA$ Activated.!ManaAbility",
    json!({"who": {"Each": [{"Type": "Artifact"}]}, "action": {"Activate": []}})
)]
#[case::activations_by_opponents(
    "Mode$ CantBeActivated | ValidCard$ Creature | ValidSA$ Activated.!ManaAbility | Activator$ Opponent",
    json!({"who": "Opponents", "action": {"Activate": [{"Type": "Creature"}]}})
)]
#[case::entering_creatures(
    "Mode$ DisableTriggers | ValidCause$ Creature | ValidMode$ ChangesZone,ChangesZoneAll | Destination$ Battlefield",
    json!({"who": "EachPlayer", "action": {"Trigger": {"ZoneChange": {"what": [{"Type": "Creature"}], "to": "Battlefield"}}}})
)]
#[case::dying_creatures(
    "Mode$ DisableTriggers | ValidCause$ Creature | ValidMode$ ChangesZone,ChangesZoneAll | Origin$ Battlefield | Destination$ Graveyard",
    json!({"who": "EachPlayer", "action": {"Trigger": {"ZoneChange": {"what": [{"Type": "Creature"}], "from": "Battlefield", "to": "Graveyard"}}}})
)]
#[case::spells_only(
    "Mode$ CantTarget | ValidTarget$ Creature | ValidSA$ Spell",
    json!({"who": {"Each": [{"Type": "Creature"}]}, "action": {"BeTargeted": ["Spell"]}})
)]
fn cant_rules(#[case] line: &str, #[case] cant: Value) {
    assert_eq!(abilities(&card(line)), vec![rule(&cant)]);
}

#[rstest]
#[case::plain_activated(
    "Mode$ CantBeActivated | ValidCard$ Artifact | ValidSA$ Activated",
    "key:Static.CantBeActivated.ValidSA"
)]
#[case::other_zone(
    "Mode$ CantBeActivated | ValidCard$ Artifact | ValidSA$ Activated.!ManaAbility | AffectedZone$ Graveyard",
    "key:Static.CantBeActivated.AffectedZone"
)]
#[case::phase_limit(
    "Mode$ CantBeCast | ValidCard$ Card | Phases$ Upkeep",
    "key:Static.CantBeCast.Phases"
)]
#[case::ability_source(
    "Mode$ CantTarget | ValidTarget$ Creature | ValidSA$ Activated",
    "key:Static.CantTarget.ValidSA"
)]
fn what_the_dsl_cannot_say_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert!(abilities(&card(line)).is_empty(), "{line}");
    assert!(gaps(&card(line)).iter().any(|g| g == gap), "{line}");
}

#[rstest]
#[case::pair("Creature.EnchantedBy", json!("Attached"))]
#[case::this("Card.Self", json!("This"))]
fn attack_and_block_pairs(#[case] valid: &str, #[case] affected: Value) {
    let line = format!("Mode$ CantAttack,CantBlock | ValidCard$ {valid}");
    let expected =
        normal(json!({"Static": {"affected": affected, "mods": ["CantAttack", "CantBlock"]}}));
    assert_eq!(abilities(&card(&line)), vec![expected]);
}
