//! Goal: `Animate` lines that give or take away abilities and `Effect` objects of rules map to the nodes that carry
//! them, and every shape the nodes cannot carry stays a gap.
use rstest::rstest;
use serde_json::Value;

use super::{abilities, gaps};

/// The abilities of a script as one string, so a case names the fragments its result must hold.
fn shown(script: &str) -> String {
    let found = abilities(script);
    assert!(!found.is_empty(), "{:?}", gaps(script));
    Value::Array(found).to_string()
}

#[rstest]
#[case::prevent_one(
    "Name:T\nTypes:Creature Bird\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidSource$ Spell | ValidTarget$ Permanent,Player | ReplaceWith$ DBReplace | PreventionEffect$ True\nSVar:DBReplace:DB$ ReplaceDamage | Amount$ 1\n",
    &["PreventUpTo\":1"], 1
)]
#[case::prevent_and_lose_a_counter(
    "Name:T\nTypes:Creature Spirit\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ Card.Self | ReplaceWith$ Rm | PreventionEffect$ True | AlwaysReplace$ True\nSVar:Rm:DB$ RemoveCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1\n",
    &["PreventRemovingCounter\":\"+1/+1\""], 1
)]
#[case::prevent_and_gain_life(
    "Name:T\nTypes:Enchantment\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ You | ReplaceWith$ Gain | PreventionEffect$ True | IsCombat$ False\nSVar:Gain:DB$ GainLife | Defined$ ReplacedTarget | LifeAmount$ X\nSVar:X:ReplaceCount$DamageAmount\n",
    &["PreventAnd", "GainLife", "\"amount\":\"Triggering\"", "\"combat\":false"], 1
)]
#[case::prevent_and_put_counters(
    "Name:T\nTypes:Creature Hydra\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ Card.Self | ReplaceWith$ C | PreventionEffect$ True\nSVar:C:DB$ PutCounter | Defined$ ReplacedTarget | CounterType$ M1M1 | CounterNum$ X\nSVar:X:ReplaceCount$DamageAmount\n",
    &["PreventAnd", "PutCounters", "-1/-1", "\"count\":\"Triggering\""], 1
)]
#[case::counters_instead(
    "Name:T\nTypes:Creature Hydra\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ Card.Self | ReplaceWith$ C\nSVar:C:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ X\nSVar:X:ReplaceCount$DamageAmount\n",
    &["Instead", "PutCounters", "+1/+1"], 1
)]
#[case::redirect_to_the_enchanted_creature(
    "Name:T\nTypes:Enchantment Aura\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ You | ReplaceWith$ D | DamageTarget$ Enchanted\nSVar:D:DB$ ReplaceEffect | VarName$ Affected | VarValue$ Enchanted | VarType$ Card\n",
    &["Instead", "DealDamage", "Dealer", "\"to\":\"Attached\""], 1
)]
#[case::you_and_your_permanents_are_two_replacements(
    "Name:T\nTypes:Enchantment\nR:Event$ DamageDone | ActiveZones$ Battlefield | ValidTarget$ You,Planeswalker.YouCtrl | Prevent$ True\n",
    &["Prevent", "Planeswalker"], 2
)]
fn damage_replacements_map(#[case] script: &str, #[case] holds: &[&str], #[case] count: usize) {
    let found = abilities(script);
    assert_eq!(found.len(), count, "{:?}", gaps(script));
    let text = shown(script);
    for part in holds {
        assert!(text.contains(part), "{part} missing in {text}");
    }
}

#[rstest]
#[case::two_players_in_a_list("ValidTarget$ You,Opponent | Prevent$ True")]
#[case::redirect_to_another_target("ValidTarget$ You | ReplaceWith$ D | DamageTarget$ Self\nSVar:D:DB$ ReplaceEffect | VarName$ Affected | VarValue$ Enchanted | VarType$ Card")]
#[case::counters_by_a_variable("ValidTarget$ Card.Self | ReplaceWith$ D | PreventionEffect$ True\nSVar:D:DB$ RemoveCounter | Defined$ ReplacedTarget | CounterType$ P1P1 | CounterNum$ Y")]
fn damage_replacements_beyond_the_nodes_stay_gaps(#[case] line: &str) {
    let script = format!("Name:T\nTypes:Enchantment\nR:Event$ DamageDone | ActiveZones$ Battlefield | {line}\n");
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert_ne!(gaps(&script), Vec::<String>::new());
}

#[rstest]
#[case::granted_trigger(
    "Name:T\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Creature | NumAtt$ +2 | SubAbility$ DBA\nSVar:DBA:DB$ Animate | Triggers$ Dies | Defined$ ParentTarget\nSVar:Dies:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ 1\n",
    &["GrantAbility", "Triggered", "GainLife"]
)]
#[case::loses_all_abilities(
    "Name:T\nTypes:Sorcery\nA:SP$ Animate | ValidTgts$ Creature | RemoveAllAbilities$ True | Duration$ Permanent\n",
    &["LoseAbilities", "Forever"]
)]
#[case::flash_for_creatures(
    "Name:T\nTypes:Enchantment\nS:Mode$ CastWithFlash | ValidCard$ Creature | ValidSA$ Spell | Caster$ You | Description$ x\n",
    &["MayCast", "\"flash\":true", "Creature"]
)]
#[case::flash_for_any_player(
    "Name:T\nTypes:Enchantment\nS:Mode$ CastWithFlash | ValidCard$ Creature,Enchantment | ValidSA$ Spell | Caster$ Player | Description$ x\n",
    &["MayCast", "EachPlayer", "Enchantment"]
)]
#[case::opponents_cast_nothing_on_your_turn(
    "Name:T\nTypes:Enchantment\nS:Mode$ CantBeCast | ValidCard$ Card | Condition$ PlayerTurn | Caster$ Opponent | Description$ x\n",
    &["CantCastSpells", "Opponents", "YourTurn"]
)]
#[case::an_effect_stops_all_casting(
    "Name:T\nTypes:Sorcery\nA:SP$ Effect | StaticAbilities$ S\nSVar:S:Mode$ CantBeCast | ValidCard$ Card | Caster$ Player | Description$ x\n",
    &["CantCastSpells", "EachPlayer", "UntilEndOfTurn"]
)]
fn animate_and_casting_statics_map(#[case] script: &str, #[case] holds: &[&str]) {
    let text = shown(script);
    for part in holds {
        assert!(text.contains(part), "{part} missing in {text}");
    }
}

#[rstest]
#[case::only_some_cards("Mode$ CantBeCast | ValidCard$ Card.nonCreature | Caster$ You")]
#[case::a_limit_each_turn("Mode$ CantBeCast | ValidCard$ Card | Caster$ Player | NumLimitEachTurn$ 1")]
#[case::loyalty_abilities("Mode$ CastWithFlash | ValidCard$ Card.Self | ValidSA$ Activated.Loyalty | Caster$ You")]
#[case::the_spell_itself("Mode$ CastWithFlash | ValidCard$ Card.Self | ValidSA$ Spell | Caster$ You")]
fn casting_statics_beyond_the_nodes_stay_gaps(#[case] line: &str) {
    let script = format!("Name:T\nTypes:Sorcery\nA:SP$ Effect | StaticAbilities$ S\nSVar:S:{line}\n");
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert_ne!(gaps(&script), Vec::<String>::new());
}
