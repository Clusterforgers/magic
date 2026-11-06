//! Goal: round 3 piece D reads the object a batch of damage hurt, the void and dungeon facts, and "has been attacked this combat" as the engine's history facts, one rstest table per rule.
#![allow(clippy::unwrap_used)]

use rstest::rstest;
use serde_json::Value;

use super::{abilities, gaps};

/// The abilities of a creature with these lines as compact JSON, and its gaps.
fn creature(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Creature Elf\n{lines}\n");
    (
        serde_json::to_string(&Value::Array(abilities(&text))).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::lki_copy_of_the_creature_hurt(
    "Creature.EnchantedBy",
    "DB$ Destroy | Defined$ TriggeredTargetLKICopy",
    r#"{"Destroy":{"what":"Triggering"}}"#
)]
#[case::the_creature_hurt(
    "Creature.OppCtrl",
    "DB$ Destroy | Defined$ TriggeredTarget",
    r#"{"Destroy":{"what":"Triggering"}}"#
)]
#[case::lands_of_the_player_hurt(
    "Player",
    "DB$ TapAll | ValidCards$ Land.ControlledBy TriggeredTarget",
    r#"{"ControlledBy":"Triggering"}"#
)]
fn a_batch_of_damage_names_what_it_hurt(
    #[case] target: &str,
    #[case] effect: &str,
    #[case] has: &str,
) {
    let lines = format!(
        "T:Mode$ DamageDoneOnce | ValidTarget$ {target} | Execute$ Go | TriggerDescription$ Test.\nSVar:Go:{effect}"
    );
    let (json, gaps) = creature(&lines);
    assert!(gaps.is_empty(), "gaps: {gaps:?}");
    assert!(json.contains(has), "{has} missing in {json}");
    assert!(json.contains(r#""Batch""#), "{json}");
}

#[rstest]
#[case::void_is_a_nonland_permanent_leaving_or_a_warped_spell(
    "Count$Void.3.2",
    &[r#""NonlandPermanentsLeft""#, r#""SpellsWarped""#, r#""Any""#]
)]
#[case::void_branches_swap(
    "Count$Void.0.1",
    &[r#""NonlandPermanentsLeft""#, r#""then":0"#]
)]
#[case::dungeons_completed("DungeonsCompleted$Amount", &[r#""DungeonsCompleted""#, r#""who":"You""#])]
#[case::been_attacked_this_combat(
    "PlayerCountPropertyYou$HasPropertyBeenAttackedThisCombat",
    &[r#""Attacked""#, r#""combat":true"#, r#""to":"You""#]
)]
fn a_history_fact_is_read_as_its_tally(#[case] expression: &str, #[case] has: &[&str]) {
    let lines = format!(
        "A:SP$ DealDamage | ValidTgts$ Any | NumDmg$ X | SpellDescription$ Test.\nSVar:X:{expression}"
    );
    let text = format!("Name:Subject\nTypes:Instant\n{lines}\n");
    let json = serde_json::to_string(&Value::Array(abilities(&text))).unwrap();
    assert!(gaps(&text).is_empty(), "gaps: {:?}", gaps(&text));
    for word in has {
        assert!(json.contains(word), "{word} missing in {json}");
    }
}

#[test]
fn only_if_you_were_attacked_this_step_is_a_cast_restriction() {
    let text = "Name:Subject\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature | NumAtt$ +1 | CheckSVar$ X | ActivationPhases$ Declare Attackers | SpellDescription$ Test.\nSVar:X:PlayerCountPropertyYou$HasPropertyBeenAttackedThisCombat\n";
    assert!(gaps(text).is_empty(), "gaps: {:?}", gaps(text));
    let json = serde_json::to_string(&Value::Array(abilities(text))).unwrap();
    assert!(
        json.contains(r#""CastOnly""#) && json.contains("DeclareAttackers"),
        "{json}"
    );
}

#[rstest]
#[case::while_you_control_it("LeavesPlay,LoseControl", r#""duration":"WhileYouControlSource""#)]
#[case::while_it_remains("LeavesPlay", r#""duration":"WhileSourceOnBattlefield""#)]
#[case::while_it_stays_tapped("Untap,LeavesPlay", r#""duration":"WhileSourceTapped""#)]
fn control_lasts_as_long_as_the_line_says(#[case] lose: &str, #[case] has: &str) {
    let line = format!(
        "A:AB$ GainControl | Cost$ T | ValidTgts$ Creature | LoseControl$ {lose} | SpellDescription$ Test."
    );
    let (json, gaps) = creature(&line);
    assert!(gaps.is_empty(), "gaps: {gaps:?}");
    assert!(json.contains(has), "{has} missing in {json}");
}

#[rstest]
#[case::colorless("SetColor$ Colorless", r#""SetColors":[]"#)]
#[case::all_colors(
    "SetColor$ All",
    r#""SetColors":["White","Blue","Black","Red","Green"]"#
)]
#[case::every_creature_type("AddAllCreatureTypes$ True", r#""AddAllCreatureTypes""#)]
#[case::more_types("AddType$ Elf & Wizard", r#""AddSubtype":"Wizard""#)]
fn a_characteristic_defining_line_defines_colors_and_types(#[case] mod_: &str, #[case] has: &str) {
    let line = format!(
        "S:Mode$ Continuous | Affected$ Card.Self | CharacteristicDefining$ True | {mod_} | Description$ Test."
    );
    let (json, gaps) = creature(&line);
    assert!(gaps.is_empty(), "gaps: {gaps:?}");
    assert!(
        json.contains(has) && json.contains(r#""defining":true"#),
        "{json}"
    );
}

#[test]
fn a_conditional_characteristic_defining_line_stays_a_gap() {
    let line = "S:Mode$ Continuous | CharacteristicDefining$ True | Affected$ Card.Self | SetColor$ All | IsPresent$ Creature.YouCtrl | Description$ Test.";
    let (_, gaps) = creature(line);
    assert_eq!(gaps, ["key:Static.Continuous.defining-with-condition"]);
}
