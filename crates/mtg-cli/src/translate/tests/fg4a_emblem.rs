//! Goal: Forge's permanent `Effect` objects named as emblems map to `CreateEmblem` with the abilities they carry, and
//! an `Effect` that remembers objects, is unnamed or carries nothing stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

#[rstest]
#[case::static_emblem(
    "Name:T\nTypes:Planeswalker Elf\nA:AB$ Effect | Cost$ SubCounter<2/LOYALTY> | Name$ Emblem - T | StaticAbilities$ Pump | Planeswalker$ True | Ultimate$ True | Duration$ Permanent\nSVar:Pump:Mode$ Continuous | EffectZone$ Command | Affected$ Creature.YouCtrl | AddPower$ 2 | AddToughness$ 2\n",
    json!({"Loyalty": {"cost": {"Minus": 2}, "effects": [{"CreateEmblem": {"abilities": [
        {"Static": {"affected": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]},
            "mods": [{"ModifyPT": {"power": 2, "toughness": 2}}]}}]}}]}})
)]
#[case::triggered_emblem(
    "Name:T\nTypes:Planeswalker Elf\nA:AB$ Effect | Cost$ SubCounter<2/LOYALTY> | Name$ Emblem - T | Triggers$ Draws | Planeswalker$ True | Ultimate$ True | Duration$ Permanent\nSVar:Draws:Mode$ Drawn | ValidCard$ Card.YouCtrl | TriggerZones$ Command | Execute$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ 1\n",
    json!({"Loyalty": {"cost": {"Minus": 2}, "effects": [{"CreateEmblem": {"abilities": [
        {"Triggered": {"when": {"Drew": {"who": "You"}}, "effects": [{"GainLife": {"amount": 1}}]}}]}}]}})
)]
fn emblems_map(#[case] script: &str, #[case] expected: Value) {
    let mut found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert_eq!(normal(found.remove(0)), normal(expected));
}

#[rstest]
#[case::named_but_remembering(
    "Name$ Emblem - T | RememberObjects$ Targeted | ValidTgts$ Creature | StaticAbilities$ Pump | Duration$ Permanent"
)]
#[case::emblem_without_abilities("Name$ Emblem - T | Duration$ Permanent")]
fn effects_that_are_no_emblem_stay_gaps(#[case] line: &str) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Effect | {line}\nSVar:Pump:Mode$ Continuous | Affected$ Creature.YouCtrl | AddPower$ 2\nSVar:Draws:Mode$ Drawn | ValidCard$ Card.YouCtrl | Execute$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ 1\n"
    );
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert_ne!(gaps(&script), Vec::<String>::new());
}
