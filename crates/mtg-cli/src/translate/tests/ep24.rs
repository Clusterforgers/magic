//! Goal: the pieces of ep24 translate as the card prints them: a change of one target to the source (`DefinedMagnet$ Self` with `ChangeSingleTarget$`) over spells and abilities or instant and sorcery spells with a single target; a magnet without a single change, another magnet or a magnet beside a chooser stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn activated(line: &str) -> String {
    format!("Name:T\nTypes:Artifact Creature Horror\nPT:0/4\nA:AB$ {line}\n")
}

fn effects(script: &str) -> Value {
    abilities(script)
        .first()
        .and_then(|found| found.get("Activated"))
        .and_then(|body| body.get("effects"))
        .cloned()
        .unwrap_or_default()
}

fn targets(script: &str) -> Value {
    abilities(script)
        .first()
        .and_then(|found| found.get("Activated"))
        .and_then(|body| body.get("targets"))
        .cloned()
        .unwrap_or_default()
}

#[rstest]
#[case::spell_or_ability(
    "ChangeTargets | Cost$ U | TargetType$ Spell,Activated,Triggered | ValidTgts$ Card,Emblem | DefinedMagnet$ Self | ChangeSingleTarget$ True",
    json!([{"AnyOf": ["Spell", {"Ability": "Activated"}, {"Ability": "Triggered"}]}])
)]
#[case::instant_or_sorcery_with_one_target(
    "ChangeTargets | Cost$ U | TargetType$ Instant.singleTarget,Sorcery.singleTarget | ValidTgts$ Card | DefinedMagnet$ Self | ChangeSingleTarget$ True",
    json!(["Spell", {"AnyOf": [{"Type": "Instant"}, {"Type": "Sorcery"}]}, "SingleTarget"])
)]
fn ep24_a_target_changes_to_the_source(#[case] line: &str, #[case] selector: Value) {
    let script = activated(line);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{line}");
    assert_eq!(
        effects(&script),
        json!([{"ChangeTargets": {"what": {"Target": 0}, "to": "One", "onto": "This"}}])
    );
    assert_eq!(targets(&script), json!([{"what": {"Object": selector}}]));
}

#[rstest]
#[case::magnet_without_single_change(
    "ChangeTargets | Cost$ U | TargetType$ Spell | ValidTgts$ Card | DefinedMagnet$ Self"
)]
#[case::another_magnet(
    "ChangeTargets | Cost$ U | TargetType$ Spell | ValidTgts$ Card | DefinedMagnet$ Enchanted | ChangeSingleTarget$ True"
)]
#[case::magnet_beside_a_chooser(
    "ChangeTargets | Cost$ U | TargetType$ Spell | ValidTgts$ Card | DefinedMagnet$ Self | ChangeSingleTarget$ True | Optional$ True"
)]
fn ep24_other_magnets_stay_gaps(#[case] line: &str) {
    let found = gaps(&activated(line));
    assert!(
        found.iter().any(|gap| gap.contains("ChangeTargets")),
        "{line}: {found:?}"
    );
}
