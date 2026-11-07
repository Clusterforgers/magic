//! Goal: an `Effect`'s delayed trigger reads `EffectSource` as the card whose ability made it (`This`) and stays a gap when its parts name the effect itself (`Self`, a `Count$Card` of it); `Defined$ Convoked` and `Convoked$Amount` read as the creatures that convoked the spell (CR 702.51).
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn creature(lines: &str) -> String {
    format!("Name:Subject\nManaCost:R\nTypes:Creature Elf\nPT:1/1\n{lines}\n")
}

/// The one ability of the card.
fn only(script: &str) -> Value {
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    normal(found[0].clone())
}

const GUARD: &str = "A:AB$ Pump | Cost$ T | NumAtt$ +2 | ValidTgts$ Creature | SubAbility$ DBEffect\nSVar:DBEffect:DB$ Effect | Triggers$ Lost | RememberObjects$ Targeted | ExileOnMoved$ Battlefield\nSVar:Lost:Mode$ ChangesZone | ValidCard$ Card.IsRemembered | Origin$ Battlefield | Destination$ Any | Execute$ Sac | TriggerDescription$ When the targeted creature leaves the battlefield this turn, sacrifice EFFECTSOURCE.";

#[rstest]
#[case::sacrifice_the_source(
    "SVar:Sac:DB$ SacrificeAll | Defined$ EffectSource",
    json!({"Sacrifice": {"filter": [{"Is": "This"}], "who": {"ControllerOf": "This"}}})
)]
#[case::counter_on_the_source(
    "SVar:Sac:DB$ PutCounter | Defined$ EffectSource | CounterType$ P1P1 | CounterNum$ 1",
    json!({"PutCounters": {"counter": "+1/+1", "on": "This"}})
)]
fn the_effect_source_is_the_card(#[case] execute: &str, #[case] effect: Value) {
    let ability = only(&creature(&format!("{GUARD}\n{execute}")));
    let delayed = ability
        .pointer("/Activated/effects/1/DelayedTrigger/effects/0")
        .cloned();
    assert_eq!(delayed, Some(effect), "{ability}");
}

#[rstest]
#[case::the_effect_itself(
    "SVar:Sac:DB$ ChangeZone | Defined$ Self | Origin$ Command | Destination$ Exile"
)]
#[case::a_count_of_the_effect(
    "SVar:Sac:DB$ PutCounter | Defined$ EffectSource | CounterType$ P1P1 | CounterNum$ X\nSVar:X:Count$CardPower"
)]
fn a_trigger_that_names_the_effect_stays_a_gap(#[case] execute: &str) {
    let script = creature(&format!("{GUARD}\n{execute}"));
    assert!(abilities(&script).is_empty(), "{:?}", abilities(&script));
}

#[test]
fn the_creatures_that_convoked_it_get_the_counters() {
    let ability = only(&creature(
        "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Put\nSVar:Put:DB$ PutCounter | Defined$ Convoked | CounterType$ P1P1 | CounterNum$ 1",
    ));
    assert_eq!(
        ability.pointer("/Triggered/effects/0"),
        Some(&json!({"PutCounters": {"counter": "+1/+1", "on": "Convoked"}}))
    );
}

#[rstest]
#[case::the_number("Convoked$Amount", json!({"CountOf": {"of": "Convoked", "filter": []}}))]
#[case::twice("Convoked$Amount/Twice", json!({"Times": {"by": 2, "of": {"CountOf": {"of": "Convoked", "filter": []}}}}))]
fn the_number_that_convoked_it(#[case] svar: &str, #[case] count: Value) {
    let ability = only(&creature(&format!(
        "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ X\nSVar:X:{svar}"
    )));
    let found = ability
        .pointer("/Triggered/effects/0/GainLife/amount")
        .cloned();
    assert_eq!(found, Some(count), "{ability}");
}
