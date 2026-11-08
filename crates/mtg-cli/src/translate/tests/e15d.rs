//! Goal: round 15 pack e15d keeps the gaps that dropping a needed parameter causes out of the gaps behind the first, and
//! reads a chain line's `ConditionDefined$` with nothing that counts it, and a default `ConditionCompare$` beside a
//! `ConditionCheckSVar$`, as no condition and the plain count.
#![allow(clippy::unwrap_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, world};
use crate::forge::script::Script;
use crate::translate::allgaps::others;

fn behind(text: &str) -> Vec<String> {
    others(&world(), &Script::parse(text)).into_iter().collect()
}

#[rstest]
#[case::a_static_without_its_affected(
    "Name:T\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ You | AddKeyword$ Protection:Player.Opponent:each of your opponents | Description$ You have protection from each of your opponents.\n",
    "static-affected:none"
)]
#[case::a_check_without_its_count(
    "Name:T\nTypes:Creature Goblin\nPT:2/2\nS:Mode$ CantAttack | ValidCard$ Card.Self | CheckSVar$ X | SVarCompare$ LT1 | Description$ CARDNAME can't attack unless an opponent has been dealt damage this turn.\nSVar:X:PlayerCountPropertyYou$DamageToOppsThisTurn\n",
    "key:Condition.SVarCompareWithoutCheckSVar"
)]
#[case::a_cost_change_without_its_type(
    "Name:T\nTypes:Creature Human\nPT:2/1\nS:Mode$ ReduceCost | ValidCard$ Instant.YouCtrl,Sorcery.YouCtrl | Relative$ True | Type$ Spell | Amount$ ReduceCost | EffectZone$ All | Description$ Each instant and sorcery spell you cast costs {1} less to cast for each creature it targets.\n",
    "static:CostModifier-Type"
)]
#[case::a_target_whose_reader_is_dropped(
    "Name:T\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Any | IsCurse$ True | SubAbility$ P | SpellDescription$ Prevent.\nSVar:P:DB$ PreventDamage | ValidTgts$ Creature | Amount$ 2 | PreventionSubAbility$ R | ShieldEffectTarget$ Targeted\nSVar:R:DB$ DealDamage | Defined$ ShieldEffectTarget | NumDmg$ PreventedDamage\n",
    "targets:unused"
)]
fn a_gap_a_drop_causes_is_not_listed_behind(#[case] script: &str, #[case] artifact: &str) {
    let first = gaps(script);
    assert_ne!(first.len(), 0);
    assert!(
        !behind(script).iter().any(|name| name == artifact),
        "{:?}",
        behind(script)
    );
}

/// The effects of the first ability of a one-ability script.
fn effects(script: &str) -> Value {
    let json = serde_json::to_value(abilities(script)).unwrap();
    let ability = json[0]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    ability["effects"].clone()
}

#[rstest]
#[case::untap_it(
    "Name:T\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature | NumAtt$ +1 | SubAbility$ U | SpellDescription$ Target creature gets +1/+0 until end of turn. Untap it.\nSVar:U:DB$ Untap | Defined$ Targeted | ConditionDefined$ Targeted\n",
    json!({"Untap": {"what": {"Target": 0}}})
)]
#[case::then_sacrifice_it(
    "Name:T\nTypes:Artifact\nA:AB$ Draw | Cost$ T | SubAbility$ S | SpellDescription$ Draw a card. Then sacrifice CARDNAME.\nSVar:S:DB$ Sacrifice | ConditionDefined$ Self\n",
    json!({"Sacrifice": {"filter": ["This"]}})
)]
fn a_defined_word_with_nothing_counted_is_no_condition(#[case] script: &str, #[case] last: Value) {
    assert_eq!(gaps(script), Vec::<String>::new());
    let effects = effects(script);
    let found = effects.as_array().unwrap().last().unwrap();
    assert_eq!(found, &last, "{effects}");
}

#[rstest]
#[case::a_default_compare_is_the_count("GE1", None)]
#[case::another_compare_stays_a_gap("GE2", Some("key:Condition.PresentWithoutIsPresent"))]
fn a_compare_beside_a_count(#[case] compare: &str, #[case] gap: Option<&str>) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Discard | Defined$ You | NumCards$ 1 | Mode$ Random | RememberDiscarded$ True | SubAbility$ D | SpellDescription$ Discard a card at random. If you do, draw a card.\nSVar:D:DB$ Draw | ConditionCheckSVar$ Y | ConditionCompare$ {compare} | SubAbility$ C\nSVar:C:DB$ Cleanup | ClearRemembered$ True\nSVar:Y:Remembered$Amount\n"
    );
    let found = gaps(&script);
    match gap {
        None => assert!(
            !found.iter().any(|name| name.contains("Condition")),
            "{found:?}"
        ),
        Some(name) => assert!(found.iter().any(|g| g == name), "{found:?}"),
    }
}

#[rstest]
#[case::a_defined_word_counted_stays_a_condition(
    "Name:T\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Creature | NumAtt$ +1 | SubAbility$ U | SpellDescription$ Target creature gets +1/+0. If it's a Goblin, untap it.\nSVar:U:DB$ Untap | Defined$ Targeted | ConditionDefined$ Targeted | ConditionPresent$ Goblin | ConditionCompare$ GE1\n"
)]
fn a_counted_defined_word_keeps_its_condition(#[case] script: &str) {
    let effects = effects(script);
    assert!(effects.to_string().contains("Goblin"), "{effects}");
}
