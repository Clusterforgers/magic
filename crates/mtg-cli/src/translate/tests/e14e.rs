//! Goal: round 14 pack e14e reads a copy of the ability an activation put on the stack, casts during a resolution and
//! permissions that pay another cost rather than the mana cost, delayed triggers that last until the controller's next
//! turn, and leaves the neighbouring shapes a gap by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given type line, script lines and Oracle text, as compact JSON, and its gaps.
fn card(types: &str, lines: &str, oracle: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\nPT:2/2\n{lines}\nOracle:{oracle}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const RINGS: &str = "T:Mode$ AbilityCast | ValidActivatingPlayer$ You | ValidSA$ SpellAbility.!ManaAbility | TriggerZones$ Battlefield | Execute$ TrigCopy | OptionalDecider$ You | TriggerDescription$ x\nSVar:TrigCopy:AB$ CopySpellAbility | Cost$ 2 | Defined$ TriggeredSpellAbility | MayChooseTarget$ True";

#[rstest]
#[case::paid_for(RINGS, r#"{"CopyAbility":{"new_targets":true,"what":"Dealer"}}"#)]
#[case::of_equipped_creature(
    "T:Mode$ AbilityCast | ValidCard$ Creature.EquippedBy | ValidSA$ SpellAbility.!ManaAbility | TriggerZones$ Battlefield | Execute$ TrigCopy | TriggerDescription$ x\nSVar:TrigCopy:DB$ CopySpellAbility | Defined$ TriggeredSpellAbility",
    r#"{"CopyAbility":{"what":"Dealer"}}"#
)]
fn an_activated_ability_is_copied(#[case] lines: &str, #[case] part: &str) {
    let (json, gaps) = card(
        "Artifact",
        lines,
        "Whenever you activate an ability, if it isn't a mana ability, you may pay {2}. If you do, copy that ability. You may choose new targets for the copy.",
    );
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::another_controller("Controller$ Opponent", "key:CopySpellAbility.Controller")]
#[case::optional("Optional$ True", "key:CopySpellAbility.Optional-of-an-ability")]
fn the_neighbours_of_a_copied_ability_stay_gaps(#[case] key: &str, #[case] gap: &str) {
    let lines = format!(
        "T:Mode$ AbilityCast | ValidActivatingPlayer$ You | ValidSA$ SpellAbility.!ManaAbility | TriggerZones$ Battlefield | Execute$ TrigCopy | TriggerDescription$ x\nSVar:TrigCopy:DB$ CopySpellAbility | Defined$ TriggeredSpellAbility | {key}"
    );
    let (_, gaps) = card(
        "Artifact",
        &lines,
        "Whenever you activate an ability, copy that ability.",
    );
    assert!(gaps.iter().any(|g| g.starts_with(gap)), "{gaps:?}");
}
