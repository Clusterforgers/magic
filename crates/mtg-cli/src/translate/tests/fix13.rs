//! Goal: a permanent's "other target" leaves the source out where the script forgets it, "each other creature with the same name" names creatures, a damage trigger the Oracle text says is to an opponent reads opponents, and a delayed "sacrifice it" is the trigger controller's own (CR 701.21a, 603.7d) while "its controller sacrifices it" stays the object's controller; every near-miss keeps its old reading.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn card(types: &str, lines: &str, oracle: &str) -> String {
    format!("Name:T\nManaCost:2 U\nTypes:{types}\n{lines}\nOracle:{oracle}\n")
}

fn first(text: &str) -> Value {
    assert!(gaps(text).is_empty(), "{:?}", gaps(text));
    abilities(text).into_iter().next().unwrap_or(Value::Null)
}

fn has(value: &Value, part: &Value) -> bool {
    value.to_string().contains(&part.to_string())
}

const DESTROY: &str =
    "A:AB$ Destroy | Cost$ T | ValidTgts$ Permanent.nonLand | SpellDescription$ x";

#[rstest]
#[case::other_on_a_permanent("Creature", "{T}: Destroy another target nonland permanent.", true)]
#[case::other_with_reminder(
    "Creature",
    "{T}: Destroy up to one other target nonland permanent. (Reminder target permanent.)",
    true
)]
#[case::plain_target("Creature", "{T}: Destroy target nonland permanent.", false)]
#[case::another_after_a_target(
    "Creature",
    "{T}: Tap target creature. Destroy another target nonland permanent.",
    false
)]
#[case::on_a_spell("Instant", "Destroy another target nonland permanent.", false)]
fn fix13_other_target_leaves_the_source_out(
    #[case] types: &str,
    #[case] oracle: &str,
    #[case] other: bool,
) {
    let found = first(&card(types, DESTROY, oracle));
    let what = &found["Activated"]["targets"][0]["what"]["Object"];
    assert_eq!(
        what.as_array()
            .is_some_and(|preds| preds.contains(&json!("Other"))),
        other,
        "{found}"
    );
}

#[test]
fn fix13_a_script_that_says_other_keeps_one_other() {
    let lines =
        "A:AB$ Destroy | Cost$ T | ValidTgts$ Permanent.nonLand+Other | SpellDescription$ x";
    let found = first(&card(
        "Creature",
        lines,
        "{T}: Destroy another target nonland permanent.",
    ));
    let what = &found["Activated"]["targets"][0]["what"]["Object"];
    assert_eq!(
        what,
        &json!([{"Not": {"Type": "Land"}}, "Other"]),
        "{found}"
    );
}

const SAME_NAME: &str = "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 4 | SubAbility$ DamageSame | SpellDescription$ x\nSVar:DamageSame:DB$ DamageAll | ValidCards$ Targeted.sameName+Other | NumDmg$ 4";

#[rstest]
#[case::creatures(
    "~ deals 4 damage to target creature and each other creature with the same name as that creature.",
    true
)]
#[case::permanents(
    "~ deals 4 damage to target creature and each other permanent with the same name as that creature.",
    false
)]
fn fix13_the_same_name_group_has_the_type_the_text_names(
    #[case] oracle: &str,
    #[case] creature: bool,
) {
    let found = first(&card("Instant", SAME_NAME, oracle));
    let group = json!([{"SharesName": {"Target": 0}}, {"Not": {"Is": {"Target": 0}}}, {"Type": "Creature"}]);
    assert_eq!(has(&found, &group), creature, "{found}");
    assert!(
        has(&found, &json!({"SharesName": {"Target": 0}})),
        "{found}"
    );
}

fn damage_trigger(description: &str) -> String {
    format!(
        "T:Mode$ DamageDone | ValidSource$ Creature.YouCtrl | ValidTarget$ Player | CombatDamage$ True | TriggerZones$ Battlefield | Execute$ Draw | TriggerDescription$ {description}\nSVar:Draw:DB$ Draw | NumCards$ 1"
    )
}

#[rstest]
#[case::both_say_opponent(
    "Whenever a creature you control deals combat damage to an opponent, draw a card.",
    "Whenever a creature you control deals combat damage to an opponent, draw a card.",
    "Opponents"
)]
#[case::the_oracle_says_a_player(
    "Whenever a creature you control deals combat damage to an opponent, draw a card.",
    "Whenever a creature you control deals combat damage to a player, draw a card.",
    "EachPlayer"
)]
#[case::the_description_says_a_player(
    "Whenever a creature you control deals combat damage to a player, draw a card.",
    "Whenever a creature you control deals combat damage to an opponent, draw a card.",
    "EachPlayer"
)]
fn fix13_damage_to_an_opponent_reads_opponents(
    #[case] description: &str,
    #[case] oracle: &str,
    #[case] to: &str,
) {
    let found = first(&card("Creature", &damage_trigger(description), oracle));
    assert_eq!(
        found["Triggered"]["when"]["DealsDamage"]["to"],
        json!(to),
        "{found}"
    );
}

const REANIMATE: &str = "A:SP$ ChangeZone | Origin$ Graveyard | Destination$ Battlefield | ValidTgts$ Creature.YouCtrl | GainControl$ True | AtEOT$ {word} | SpellDescription$ x";

#[rstest]
#[case::sacrifice_it("Sacrifice", None)]
#[case::its_controller_sacrifices_it("SacrificeCtrl", Some(json!({"ControllerOf": {"Bound": "moved"}})))]
fn fix13_a_delayed_sacrifice_is_its_controllers(#[case] word: &str, #[case] who: Option<Value>) {
    let found = first(&card(
        "Sorcery",
        &REANIMATE.replace("{word}", word),
        "Return target creature card from your graveyard to the battlefield. Sacrifice it at the beginning of the next end step.",
    ));
    let sacrifice = &found["Spell"]["effects"][1]["DelayedTrigger"]["effects"][0]["Sacrifice"];
    assert_eq!(
        sacrifice["filter"],
        json!([{"Is": {"Bound": "moved"}}]),
        "{found}"
    );
    assert_eq!(sacrifice.get("who").cloned(), who, "{found}");
}
