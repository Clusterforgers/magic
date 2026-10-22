//! Goal: the wordings of an Aura that enchants a player (a creature attacking enchanted player, the creatures enchanted player controls) read into the shapes of the hand-written files, write back as the card's own text, and the near-misses stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

#[rstest]
#[case::a_draw(
    "Whenever a creature attacks enchanted player, draw a card.",
    r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Type":"Creature"}],"at":"AttachedPlayer"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::a_counter_on_the_attacker(
    "Whenever a creature attacks enchanted player, put a +1/+1 counter on it.",
    r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Type":"Creature"}],"at":"AttachedPlayer"}},"effects":[{"PutCounters":{"on":"Triggering","counter":"+1/+1"}}]}}"#
)]
#[case::the_attackers_controller_gains_life(
    "Whenever a creature attacks enchanted player, its controller gains 1 life.",
    r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Type":"Creature"}],"at":"AttachedPlayer"}},"effects":[{"GainLife":{"who":{"ControllerOf":"Triggering"},"amount":1}}]}}"#
)]
fn an_attack_on_the_enchanted_player_reads_as_a_trigger(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::must_attack(
    "Creatures enchanted player controls attack each combat if able.",
    r#"{"Rule":{"rule":{"MustAttack":{"Each":[{"Type":"Creature"},{"ControlledBy":"AttachedPlayer"}]}}}}"#
)]
fn the_creatures_of_the_enchanted_player_read_as_a_group(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::another_player("Whenever a creature attacks target player, put a +1/+1 counter on it.")]
#[case::an_attack_on_you("Whenever a creature attacks you, put a +1/+1 counter on it.")]
fn other_attacks_stay_with_the_table(#[case] text: &str) {
    assert_eq!(trigger(text), None);
}

#[test]
fn probe() {
    for text in [
        "Whenever a creature attacks enchanted player, you gain 1 life.",
        "Whenever a creature attacks enchanted player, its controller draws a card.",
        "Whenever a creature attacks enchanted player, its controller gains 1 life.",
    ] {
        eprintln!("{text} => {:?}", trigger(text).map(|a| render_variants(&a)));
    }
}

#[test]
fn a_group_of_another_player_stays_with_the_table() {
    assert_eq!(
        stat("Creatures target player controls attack each combat if able."),
        None
    );
}
