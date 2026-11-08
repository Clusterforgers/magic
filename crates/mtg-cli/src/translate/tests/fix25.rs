//! Goal: a control change of a group read by its controller names the group once, so "untap them" and "they gain haste" act on the permanents taken, while a group read otherwise or a change without later steps stays as it was; a spell that copies itself and names no target makes copies without new targets, while one that names a target keeps the choice.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// The effects of the first ability of the card.
fn effects(script: &str) -> Value {
    assert_eq!(gaps(script), Vec::<String>::new(), "{script}");
    let first = abilities(script).into_iter().next().unwrap_or_default();
    let body = first
        .as_object()
        .and_then(|map| map.values().next().cloned());
    body.and_then(|body| body.get("effects").cloned())
        .unwrap_or_default()
}

fn takeover(valid: &str, rest: &str) -> String {
    format!(
        "Name:Test Takeover\nTypes:Sorcery\nA:SP$ GainControl | AllValid$ {valid} | LoseControl$ EOT{rest}\n"
    )
}

#[rstest]
#[case::opponents_artifacts_untapped_with_haste(
    "Artifact.OppCtrl",
    " | Untap$ True | AddKWs$ Haste",
    true
)]
#[case::opponents_creatures_untapped("Creature.OppCtrl", " | Untap$ True", true)]
#[case::all_creatures_untapped_with_haste("Creature", " | Untap$ True | AddKWs$ Haste", false)]
#[case::opponents_artifacts_only_taken("Artifact.OppCtrl", "", false)]
fn a_group_read_by_its_controller_is_named_once(
    #[case] valid: &str,
    #[case] rest: &str,
    #[case] named: bool,
) {
    let effects = effects(&takeover(valid, rest));
    let list = effects.as_array().cloned().unwrap_or_default();
    assert_eq!(list[0].get("Bind").is_some(), named, "{effects}");
    let affected = if named {
        json!({"Bound": "taken"})
    } else {
        list[0]["Continuous"]["affected"].clone()
    };
    for step in &list[usize::from(named)..] {
        let what = step
            .pointer("/Continuous/affected")
            .or_else(|| step.pointer("/Untap/what"));
        assert_eq!(what, Some(&affected), "{effects}");
    }
}

fn storm(oracle: &str) -> String {
    format!(
        "Name:Test Storm\nTypes:Sorcery\nA:SP$ Draw\nT:Mode$ SpellCast | ValidCard$ Card.Self | Execute$ TrigCopy | TriggerDescription$ When you cast this spell, copy it.\nSVar:TrigCopy:DB$ CopySpellAbility | Defined$ TriggeredSpellAbility | Amount$ 2 | MayChooseTarget$ True\nOracle:{oracle}\n"
    )
}

#[rstest]
#[case::no_target("When you cast this spell, copy it twice.\\nDraw a card.", false)]
#[case::a_target(
    "When you cast this spell, copy it twice. You may choose new targets for the copies.\\nTarget player draws a card.",
    true
)]
fn a_self_copy_without_targets_chooses_none(#[case] oracle: &str, #[case] new_targets: bool) {
    let script = storm(oracle);
    assert_eq!(gaps(&script), Vec::<String>::new(), "{script}");
    let copy = abilities(&script)
        .iter()
        .find_map(|ability| ability.pointer("/Triggered/effects/0/CopySpell").cloned())
        .unwrap_or_default();
    assert_eq!(
        copy.get("new_targets") == Some(&json!(true)),
        new_targets,
        "{copy}"
    );
}
