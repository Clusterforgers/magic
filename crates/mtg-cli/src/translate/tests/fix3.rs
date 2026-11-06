//! Goal: a dig that takes every card leaves no rest to move, a choice titled "to reveal" is shown to the table, and the mana of a group attack counts the attackers.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn spell(body: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {body}\n")
}

#[rstest]
#[case::every_card(
    "Dig | DigNum$ 1 | ChangeNum$ All | ChangeValid$ Card | DestinationZone$ Hand",
    false
)]
#[case::one_creature(
    "Dig | DigNum$ 3 | ChangeNum$ 1 | ChangeValid$ Creature | DestinationZone$ Hand",
    true
)]
#[case::every_land(
    "Dig | DigNum$ 3 | ChangeNum$ All | ChangeValid$ Land | DestinationZone$ Hand",
    true
)]
fn the_rest_of_a_dig_moves_only_when_cards_are_left(#[case] line: &str, #[case] rest_moves: bool) {
    let script = spell(line);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let effects = abilities(&script)[0]["Spell"]["effects"].clone();
    let to_library = effects
        .as_array()
        .into_iter()
        .flatten()
        .filter(|effect| effect["Move"]["to"].get("Library").is_some())
        .count();
    assert_eq!(to_library, usize::from(rest_moves));
}

#[rstest]
#[case::titled(
    "ChooseCard | Choices$ Creature | Amount$ 2 | ChoiceTitle$ Choose up to two creature cards to reveal",
    true
)]
#[case::flagged("ChooseCard | Choices$ Creature | Amount$ 2 | Reveal$ True", true)]
#[case::plain(
    "ChooseCard | Choices$ Creature | Amount$ 2 | ChoiceTitle$ Choose up to two creature cards",
    false
)]
fn a_choice_titled_to_reveal_is_revealed(#[case] line: &str, #[case] revealed: bool) {
    let script = spell(line);
    let effects = abilities(&script)[0]["Spell"]["effects"].clone();
    let shown = effects
        .as_array()
        .into_iter()
        .flatten()
        .any(|effect| effect.get("Reveal").is_some());
    assert_eq!(shown, revealed);
}

#[test]
fn the_mana_of_a_group_attack_is_the_number_of_attackers() {
    let script = "Name:T\nTypes:Creature Elf\nPT:1/1\nT:Mode$ AttackersDeclared | AttackingPlayer$ You | Execute$ Mana\nSVar:Mana:DB$ Mana | Produced$ Combo R G | Amount$ X\nSVar:X:TriggerObjectsAttackers$Amount\n";
    assert_eq!(gaps(script), Vec::<String>::new());
    let mana = abilities(script)[0]["Triggered"]["effects"][0]["AddMana"]["mana"].clone();
    assert_eq!(
        mana,
        json!({"AnyCombination": {"among": ["Red", "Green"], "count": "Triggering"}})
    );
}
