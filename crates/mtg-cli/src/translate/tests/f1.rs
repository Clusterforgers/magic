//! Goal: the attack target and the two sides of a block are table rows: `TriggeredDefender` and `TriggeredAttackedTarget` read what the event went against, a blocker is the dealer of a block and its attacker what it went against, a group of attackers is the event object, and a word read in the wrong kind of trigger stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn creature(body: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\nPT:1/1\n{body}\n")
}

#[rstest]
#[case::hellrider_defender(
    "Mode$ Attacks | ValidCard$ Creature.YouCtrl",
    "TriggeredDefender.Opponent & Valid Planeswalker.TriggeredDefender",
    json!("Against")
)]
#[case::defender(
    "Mode$ Attacks | ValidCard$ Creature.YouCtrl",
    "TriggeredDefender",
    json!("Against")
)]
#[case::attacked_target_of_a_group(
    "Mode$ AttackersDeclaredOneTarget | ValidAttackers$ Creature.YouCtrl | AttackedTarget$ Opponent",
    "TriggeredAttackedTarget",
    json!("Against")
)]
#[case::attackers_of_a_group(
    "Mode$ AttackersDeclared | AttackingPlayer$ You | ValidAttackers$ Creature",
    "TriggeredAttackers",
    json!("Triggering")
)]
#[case::attacking_player_of_a_group(
    "Mode$ AttackersDeclared | AttackingPlayer$ Player.Other | ValidAttackers$ Creature",
    "TriggeredAttackingPlayer",
    json!({"ControllerOf": "Triggering"})
)]
#[case::blocker_of_a_block(
    "Mode$ Blocks | ValidCard$ Creature.YouCtrl",
    "TriggeredBlockerLKICopy",
    json!("Dealer")
)]
#[case::blocker_controller_of_a_block(
    "Mode$ Blocks | ValidCard$ Creature",
    "TriggeredBlockerController",
    json!({"ControllerOf": "Dealer"})
)]
#[case::attacker_of_a_block(
    "Mode$ Blocks | ValidCard$ Creature.YouCtrl",
    "TriggeredAttacker",
    json!("Against")
)]
fn a_word_of_an_attack_or_a_block_names_what_the_event_holds(
    #[case] trigger: &str,
    #[case] word: &str,
    #[case] who: Value,
) {
    let script = creature(&format!(
        "T:{trigger} | Execute$ Hit\nSVar:Hit:DB$ DealDamage | Defined$ {word} | NumDmg$ 1"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script)[0]["Triggered"]["effects"][0]["DealDamage"]["to"],
        who
    );
}

#[rstest]
#[case::defender_of_a_block(
    "Mode$ Blocks | ValidCard$ Creature",
    "TriggeredDefender",
    "defined:TriggeredDefender-in-Blocks"
)]
#[case::attacked_target_of_one_attacker(
    "Mode$ Attacks | ValidCard$ Creature",
    "TriggeredAttackedTarget",
    "defined:TriggeredAttackedTarget-in-Attacks"
)]
#[case::attackers_of_one_attacker(
    "Mode$ Attacks | ValidCard$ Creature",
    "TriggeredAttackers",
    "defined:TriggeredAttackers-in-Attacks"
)]
fn a_word_read_in_a_trigger_that_does_not_hold_it_stays_a_gap(
    #[case] trigger: &str,
    #[case] word: &str,
    #[case] gap: &str,
) {
    let script = creature(&format!(
        "T:{trigger} | Execute$ Hit\nSVar:Hit:DB$ DealDamage | Defined$ {word} | NumDmg$ 1"
    ));
    assert_eq!(gaps(&script), vec![gap.to_owned()]);
}

#[rstest]
#[case::group("Mode$ AttackersDeclared | AttackingPlayer$ You | ValidAttackers$ Creature")]
#[case::group_at_one_player(
    "Mode$ AttackersDeclaredOneTarget | ValidAttackers$ Creature.YouCtrl | AttackedTarget$ Opponent"
)]
fn the_number_of_attackers_is_the_amount_of_a_group(#[case] trigger: &str) {
    let script = creature(&format!(
        "T:{trigger} | Execute$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ X\nSVar:X:TriggerObjectsAttackers$Amount"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(
        abilities(&script)[0]["Triggered"]["effects"][0]["GainLife"]["amount"],
        json!("Triggering")
    );
}

#[test]
fn the_number_of_attackers_of_one_attacker_stays_a_gap() {
    let script = creature(
        "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ Gain\nSVar:Gain:DB$ GainLife | LifeAmount$ X\nSVar:X:TriggerObjectsAttackers$Amount",
    );
    assert_eq!(
        gaps(&script),
        vec!["amount:TriggerObjectsAttackers-outside-a-group".to_owned()]
    );
}
