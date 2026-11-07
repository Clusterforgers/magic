//! Goal: a target only a kicked line declares is chosen only when kicked, "activated abilities cost more" without
//! `Activated.!ManaAbility` covers mana abilities (`Every`), and "for each of those permanents" after a destroy reads
//! the targets while "this way" keeps what was destroyed; every near-miss keeps its old reading.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn card(types: &str, lines: &str, oracle: &str) -> String {
    format!("Name:T\nManaCost:R\nTypes:{types}\n{lines}\nOracle:{oracle}\n")
}

fn read(text: &str) -> String {
    assert!(gaps(text).is_empty(), "{:?}", gaps(text));
    Value::Array(abilities(text)).to_string()
}

fn kicked() -> String {
    r#""min":{"When":{"condition":"Kicked","otherwise":0,"then":1}}"#.to_owned()
}

#[rstest]
#[case::kicked_only(
    "A:SP$ Effect | StaticAbilities$ ST | SubAbility$ DBDamage | SpellDescription$ x\nSVar:ST:Mode$ CantPreventDamage | Description$ x\nSVar:DBDamage:DB$ DealDamage | ValidTgts$ Player,Planeswalker | NumDmg$ 5 | NoPrevention$ True | Condition$ Kicked",
    true
)]
#[case::second_target_kicked(
    "A:SP$ ChangeZone | Origin$ Battlefield | Destination$ Hand | ValidTgts$ Creature | SubAbility$ DBDamage | SpellDescription$ x\nSVar:DBDamage:DB$ DealDamage | ValidTgts$ Creature.Other | NumDmg$ 2 | Condition$ Kicked",
    true
)]
#[case::unconditional_target(
    "A:SP$ DealDamage | ValidTgts$ Player,Planeswalker | NumDmg$ 5 | SpellDescription$ x",
    false
)]
#[case::kicked_without_a_target(
    "A:SP$ DealDamage | ValidTgts$ Creature | NumDmg$ 2 | SubAbility$ DBDraw | SpellDescription$ x\nSVar:DBDraw:DB$ Draw | NumCards$ 1 | Condition$ Kicked",
    false
)]
fn fix14_a_kicked_line_declares_its_target_only_when_kicked(
    #[case] lines: &str,
    #[case] expected: bool,
) {
    let text = card(
        "Instant",
        &format!("K:Kicker:3 R\n{lines}"),
        "Kicker {3}{R}\nx",
    );
    assert_eq!(read(&text).contains(&kicked()), expected, "{}", read(&text));
}

#[rstest]
#[case::every_ability(
    "S:Mode$ RaiseCost | ValidCard$ Creature.AttachedBy | Type$ Ability | Amount$ 3 | AffectedZone$ Battlefield",
    "Every"
)]
#[case::not_mana_abilities(
    "S:Mode$ RaiseCost | ValidCard$ Creature.AttachedBy | Type$ Ability | ValidSpell$ Activated.!ManaAbility | Amount$ 3 | AffectedZone$ Battlefield",
    "Any"
)]
fn fix14_activated_abilities_include_mana_abilities(#[case] line: &str, #[case] kind: &str) {
    let text = card(
        "Enchantment Aura",
        &format!("K:Enchant:Creature\n{line}"),
        "Enchant creature\nActivated abilities of enchanted creature cost {3} more to activate.",
    );
    let wanted = json!({ "activating": kind }).to_string();
    let wanted = wanted.trim_start_matches('{').trim_end_matches('}');
    assert!(read(&text).contains(wanted), "{}", read(&text));
}

const SOUL: &str = "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigDestroy | TriggerDescription$ x\nSVar:TrigDestroy:DB$ Destroy | TargetMin$ 0 | TargetMax$ 3 | ValidTgts$ Permanent.nonLand | RememberTargets$ True | SubAbility$ MakeTokens\nSVar:MakeTokens:DB$ RepeatEach | RepeatSubAbility$ DBToken | DefinedCards$ Remembered | UseImprinted$ True | SubAbility$ DBCleanup\nSVar:DBToken:DB$ Draw | Defined$ ImprintedController | NumCards$ 1\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True";

#[rstest]
#[case::those_permanents(
    "When T enters, destroy up to three target nonland permanents. For each of those permanents, its controller draws a card.",
    true
)]
#[case::destroyed_this_way(
    "When T enters, destroy up to three target nonland permanents. For each permanent destroyed this way, its controller draws a card.",
    false
)]
fn fix14_each_of_those_permanents_reads_the_targets(#[case] oracle: &str, #[case] targets: bool) {
    let text = card("Creature Avatar\nPT:5/7", SOUL, oracle);
    let read = read(&text);
    let over_targets = read.contains(&json!({"over": {"Target": 0}}).to_string()[1..14]);
    assert_eq!(over_targets, targets, "{read}");
    assert_eq!(read.contains("\"destroyed\""), !targets, "{read}");
}
