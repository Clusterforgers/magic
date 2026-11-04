//! Goal: round 3 piece P6 reads every trigger mode's filters through the selector, one rstest table per rule.
#![allow(clippy::unwrap_used)]

use rstest::rstest;
use serde_json::Value;

use super::{abilities, gaps};

/// The first ability of a creature with the given trigger line (and `SVar` lines) as compact JSON, and its gaps.
fn creature(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Creature Elf\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text).first().unwrap_or(&Value::Null)).unwrap(),
        gaps(&text),
    )
}

/// A trigger line that draws a card.
fn trigger(mode: &str) -> String {
    format!(
        "T:Mode$ {mode} | Execute$ Go | TriggerDescription$ Test.\nSVar:Go:DB$ Draw | NumCards$ 1"
    )
}

#[rstest]
#[case::player_or_planeswalker(
    "DamageDone | ValidSource$ Card.Self | ValidTarget$ Player,Planeswalker | CombatDamage$ True",
    &[r#""AnyOf":[{"DealsDamage":{"combat":true,"source":["This"],"to":"EachPlayer"}}"#,
      r#"{"DealsDamage":{"combat":true,"source":["This"],"to":{"Each":[{"Type":"Planeswalker"}]}}}"#]
)]
#[case::player_or_battle(
    "DamageDone | ValidSource$ Card.Self | ValidTarget$ Player,Battle",
    &[r#""to":"EachPlayer""#, r#"{"Type":"Battle"}"#]
)]
#[case::you_or_your_permanent(
    "DamageDone | ValidSource$ Creature | ValidTarget$ You,Permanent.YouCtrl",
    &[r#""to":"You""#, r#"{"ControlledBy":"You"}"#]
)]
fn damage_to_either_of_two_recipients(#[case] mode: &str, #[case] has: &[&str]) {
    let (json, gaps) = creature(&trigger(mode));
    assert!(gaps.is_empty(), "gaps: {gaps:?}");
    for word in has {
        assert!(json.contains(word), "{word} missing in {json}");
    }
}

#[test]
fn one_or_more_damage_to_either_stays_a_gap() {
    let (_, gaps) = creature(&trigger(
        "DamageDoneOnce | ValidSource$ Card.Self | ValidTarget$ Player,Planeswalker",
    ));
    assert_eq!(gaps, ["key:Trigger.DamageDoneOnce.ValidTarget-or"]);
}

#[rstest]
#[case::no_islands(
    "Always | TriggerZones$ Battlefield | IsPresent$ Island.YouCtrl | PresentCompare$ EQ0",
    r#""when":{"State":{"condition":{"Compare":"#
)]
#[case::big_power(
    "Always | TriggerZones$ Battlefield | IsPresent$ Creature.Self+powerGE7",
    r#""State""#
)]
fn a_state_trigger_holds_the_condition(#[case] mode: &str, #[case] has: &str) {
    let (json, gaps) = creature(&trigger(mode));
    assert!(gaps.is_empty(), "gaps: {gaps:?}");
    assert!(json.contains(has), "{has} missing in {json}");
    assert!(!json.contains(r#""if""#), "{json}");
}

#[test]
fn a_state_trigger_without_a_condition_is_a_gap() {
    let (_, gaps) = creature(&trigger("Always"));
    assert_eq!(gaps, ["trigger:Always-without-condition"]);
}

/// Each `ValidSA` value with the words of the spell it keeps.
#[rstest]
#[case::plain_spell("Spell", &[r#""Spell""#], &["SingleTarget"])]
#[case::single_target("Spell.singleTarget", &["SingleTarget"], &[])]
#[case::instant_or_sorcery_single(
    "Instant.singleTarget,Sorcery.singleTarget",
    &["SingleTarget", "Instant", "Sorcery"],
    &[]
)]
fn a_cast_trigger_reads_the_spell_ability(
    #[case] valid: &str,
    #[case] has: &[&str],
    #[case] lacks: &[&str],
) {
    let line = trigger(&format!(
        "SpellCast | ValidSA$ {valid} | ValidActivatingPlayer$ You"
    ));
    let (json, gaps) = creature(&line);
    assert!(gaps.is_empty(), "gaps: {gaps:?}");
    for word in has {
        assert!(json.contains(word), "{word} missing in {json}");
    }
    for word in lacks {
        assert!(!json.contains(word), "{word} present in {json}");
    }
}

#[rstest]
#[case::single_target("IsSingleTarget$ True", &["SingleTarget"])]
#[case::targets_only_it("IsSingleTarget$ True | TargetsValid$ Card.Self", &["SingleTarget", "Targeting"])]
fn a_cast_trigger_names_single_targets(#[case] words: &str, #[case] has: &[&str]) {
    let (json, gaps) = creature(&trigger(&format!(
        "SpellCast | ValidActivatingPlayer$ You | {words}"
    )));
    assert!(gaps.is_empty(), "gaps: {gaps:?}");
    for word in has {
        assert!(json.contains(word), "{word} missing in {json}");
    }
}

#[test]
fn an_unknown_spell_ability_word_stays_a_gap() {
    let (_, gaps) = creature(&trigger("SpellCast | ValidSA$ Spell.Warp"));
    assert!(gaps.iter().any(|gap| gap.contains("Warp")), "{gaps:?}");
}

#[rstest]
#[case::blocks_a_creature("ValidCard$ Creature | ValidBlocker$ Card.Self", true)]
#[case::enchanted_blocker("ValidCard$ Creature | ValidBlocker$ Creature.EnchantedBy", true)]
#[case::any_creature_blocks("ValidCard$ Card.Self | ValidBlocker$ Creature", false)]
#[case::white_creature_blocks("ValidCard$ Card.Self | ValidBlocker$ Creature.White", false)]
fn a_blocked_trigger_reads_a_single_blocker_only(#[case] words: &str, #[case] reads: bool) {
    let (json, gaps) = creature(&trigger(&format!("AttackerBlocked | {words}")));
    if reads {
        assert!(gaps.is_empty(), "gaps: {gaps:?}");
        assert!(json.contains(r#""by":"#), "{json}");
    } else {
        assert_eq!(gaps, ["key:Trigger.BecomesBlocked.ValidBlocker"]);
    }
}

#[rstest]
#[case::declare_attackers("Declare Attackers", r#"{"Step":"DeclareAttackers"}"#)]
#[case::combat("BeginCombat->EndCombat", r#""InCombat""#)]
#[case::main("Main1,Main2", r#"{"Any":[{"Step":"Main1"},{"Step":"Main2"}]}"#)]
fn entering_during_a_step_is_a_condition_of_the_event(#[case] phase: &str, #[case] has: &str) {
    let (json, gaps) = creature(&trigger(&format!(
        "ChangesZone | Phase$ {phase} | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self"
    )));
    assert!(gaps.is_empty(), "gaps: {gaps:?}");
    assert!(
        json.contains(&format!(r#""if":{{"AtTrigger":{has}}}"#)),
        "{json}"
    );
}

#[test]
fn an_unknown_phase_of_a_zone_change_stays_a_gap() {
    let (_, gaps) = creature(&trigger(
        "ChangesZone | Phase$ Upkeep | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self",
    ));
    assert_eq!(gaps, ["key:Trigger.ChangesZone.Phase=Upkeep"]);
}

#[rstest]
#[case::dies("Battlefield", "Graveyard", true)]
#[case::from_the_library("Library", "Graveyard", false)]
fn the_controller_of_a_death_trigger_is_the_controller_as_it_left(
    #[case] from: &str,
    #[case] to: &str,
    #[case] reads: bool,
) {
    let (_, gaps) = creature(&trigger(&format!(
        "ChangesZone | Origin$ {from} | Destination$ {to} | ValidCard$ Card.Self | TriggerController$ TriggeredCardController"
    )));
    if reads {
        assert!(gaps.is_empty(), "gaps: {gaps:?}");
    } else {
        assert_eq!(gaps, ["key:Trigger.ChangesZone.TriggerController"]);
    }
}

#[rstest]
#[case::second("2", Some(2))]
#[case::first("1", Some(1))]
#[case::zero("0", None)]
#[case::word("X", None)]
fn drawing_the_nth_card_of_a_turn(#[case] number: &str, #[case] nth: Option<u32>) {
    let (json, gaps) = creature(&trigger(&format!(
        "Drawn | ValidCard$ Card.YouCtrl | Number$ {number}"
    )));
    match nth {
        Some(nth) => {
            assert!(gaps.is_empty(), "gaps: {gaps:?}");
            assert!(
                json.contains(&format!(
                    r#""Nth":{{"event":{{"Drew":{{"who":"You"}}}},"nth":{nth}}}"#
                )),
                "{json}"
            );
        }
        None => assert_eq!(gaps, ["key:Trigger.Drawn.Number"]),
    }
}

#[test]
fn revolt_gates_the_end_step_trigger() {
    let (json, gaps) = creature(&trigger(
        "Phase | Phase$ End of Turn | ValidPlayer$ You | Revolt$ True",
    ));
    assert!(gaps.is_empty(), "gaps: {gaps:?}");
    assert!(json.contains("PermanentsLeftUnder"), "{json}");
}

#[rstest]
#[case::any_land(
    "ValidCard$ Land | Activator$ You | Static$ True",
    "Produced$ Any",
    true
)]
#[case::a_forest(
    "ValidCard$ Land.Green | Activator$ You | Static$ True",
    "Produced$ G",
    true
)]
#[case::not_a_mana_shape("ValidCard$ Land.Green | Activator$ You | Static$ True", "", false)]
fn a_triggered_mana_ability_must_fold_into_the_tap(
    #[case] words: &str,
    #[case] mana: &str,
    #[case] folds: bool,
) {
    let body = if folds {
        format!("DB$ Mana | {mana}")
    } else {
        "DB$ Draw | NumCards$ 1".to_owned()
    };
    let line = format!(
        "T:Mode$ TapsForMana | {words} | Execute$ Go | TriggerDescription$ Test.\nSVar:Go:{body}"
    );
    let (_, gaps) = creature(&line);
    if folds {
        assert!(gaps.is_empty(), "gaps: {gaps:?}");
    } else {
        assert_eq!(gaps, ["key:Trigger.TapsForMana.Static"]);
    }
}
