//! Goal: "can't be countered" lines become `CantBeCountered` for the spell itself or the group they name, and lines about
//! abilities, conditions or another effect stay named gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn card(line: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\nR:{line}\n")
}

#[rstest]
#[case::itself(
    "Event$ Counter | ValidCard$ Card.Self | ValidSA$ Spell | Layer$ CantHappen | Description$ This spell can't be countered.",
    json!({"Rule": {"rule": {"CantBeCountered": "This"}, "from": "Stack"}})
)]
#[case::yours(
    "Event$ Counter | ValidSA$ Spell.YouCtrl | Layer$ CantHappen | ActiveZones$ Battlefield | Description$ Spells you control can't be countered.",
    json!({"Rule": {"rule": {"CantBeCountered": {"Each": ["Spell", {"ControlledBy": "You"}]}}}})
)]
#[case::creature_spells(
    "Event$ Counter | ValidSA$ Spell.Creature+YouCtrl | Layer$ CantHappen | ActiveZones$ Battlefield | Description$ x",
    json!({"Rule": {"rule": {"CantBeCountered": {"Each": ["Spell", {"Type": "Creature"}, {"ControlledBy": "You"}]}}}})
)]
fn a_spell_that_cannot_be_countered(#[case] line: &str, #[case] want: Value) {
    let script = card(line);
    let mut found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(normal(found.remove(0)), normal(want));
}

#[rstest]
#[case::abilities(
    "Event$ Counter | ValidSA$ Spell,Activated,Triggered | Layer$ CantHappen | ActiveZones$ Battlefield",
    "replacement:Counter-abilities"
)]
#[case::condition(
    "Event$ Counter | ValidCard$ Card.Self | ValidSA$ Spell | Layer$ CantHappen | CheckSVar$ X | SVarCompare$ GE5",
    "key:Replacement.Counter.CheckSVar"
)]
#[case::exile_instead(
    "Event$ Counter | ActiveZones$ Battlefield | ValidSA$ Spell | ValidCause$ SpellAbility.YouCtrl | ReplaceWith$ DBRemove",
    "key:Replacement.Counter.ValidCause"
)]
#[case::itself_from_battlefield(
    "Event$ Counter | ValidCard$ Card.Self | ValidSA$ Spell | Layer$ CantHappen | ActiveZones$ Battlefield",
    "replacement:Counter-zone"
)]
fn a_shape_that_does_not_map_is_a_named_gap(#[case] line: &str, #[case] gap: &str) {
    let script = card(line);
    assert_eq!(abilities(&script).len(), 0);
    assert_eq!(gaps(&script), [gap]);
}
