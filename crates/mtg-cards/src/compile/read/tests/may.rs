//! Goal: "you may" and one step read as a choice of the controller in a spell and in a trigger, and a chain of steps, a condition on the choice, a choice of another player and a step that has a subject of its own stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::draw(
    "You may draw a card.",
    r#"{"Spell":{"effects":[{"May":{"effects":[{"Draw":{}}]}}]}}"#
)]
#[case::discard(
    "You may discard two cards.",
    r#"{"Spell":{"effects":[{"May":{"effects":[{"Discard":{"count":2}}]}}]}}"#
)]
#[case::exile(
    "You may exile target creature.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"May":{"effects":[{"Exile":{"what":{"Target":0}}}]}}]}}"#
)]
#[case::after_a_sentence(
    "Draw a card. You may discard a card.",
    r#"{"Spell":{"effects":[{"Draw":{}},{"May":{"effects":[{"Discard":{}}]}}]}}"#
)]
fn a_choice_in_a_spell_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::attack(
    "Whenever ~ attacks, you may draw a card.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"May":{"effects":[{"Draw":{}}]}}]}}"#
)]
#[case::upkeep(
    "At the beginning of your upkeep, you may scry 1.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"May":{"effects":[{"Scry":{}}]}}]}}"#
)]
#[case::a_target(
    "When ~ enters, you may destroy target artifact.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"May":{"effects":[{"Destroy":{"what":{"Target":0}}}]}}]}}"#
)]
fn a_choice_in_a_trigger_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("You may draw a card.")]
#[case("You may draw two cards.")]
#[case("You may scry 2.")]
#[case("You may mill three cards.")]
#[case("You may discard a card.")]
#[case("You may exile target nonland permanent.")]
#[case("You may destroy target creature an opponent controls.")]
#[case("You may counter target spell.")]
#[case("Scry 1. You may draw a card.")]
#[case("Destroy target creature. You may draw a card.")]
#[case("~ deals 2 damage to any target. You may draw a card.")]
fn more_oracle_wordings_in_a_spell_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("Whenever you cast a spell, you may draw a card.")]
#[case("Whenever a creature you control dies, you may draw a card.")]
#[case("When ~ enters, you may scry 2.")]
#[case("When ~ dies, you may draw a card.")]
#[case("When ~ leaves the battlefield, you may exile target creature.")]
#[case("At the beginning of your end step, you may discard a card.")]
#[case("At the beginning of each upkeep, you may mill a card.")]
#[case("Whenever ~ enters or attacks, you may destroy target artifact.")]
#[case("Landfall \u{2014} Whenever a land you control enters, you may scry 1.")]
#[case("Whenever you sacrifice a creature, you may draw a card.")]
#[case("Whenever an opponent casts a spell, you may draw a card.")]
fn more_oracle_wordings_in_a_trigger_read_and_write_back(#[case] text: &str) {
    let rest = text.split_once(" \u{2014} ").map_or(text, |(_, rest)| rest);
    assert_eq!(
        trigger(text),
        Some(golden(rest, ClauseKind::Triggered, false)),
        "{text}"
    );
}

#[rstest]
#[case::a_chain("You may draw a card, then discard a card.")]
#[case::a_chain_of_other_steps("You may scry 1, then draw a card.")]
#[case::if_you_do("You may draw a card. If you do, discard a card.")]
#[case::unless("You may draw a card unless that player pays {1}.")]
#[case::another_player("Target player may draw a card.")]
#[case::each_player("Each player may draw a card.")]
#[case::an_opponent("Each opponent may discard a card.")]
#[case::a_pump("You may target creature gets +2/+2 until end of turn.")]
#[case::damage("You may ~ deals 2 damage to any target.")]
#[case::have("You may have target creature get +2/+2 until end of turn.")]
#[case::life("You may gain 3 life.")]
#[case::or("You may draw a card or discard a card.")]
#[case::nothing("You may.")]
#[case::no_step("You may")]
#[case::regeneration("You may destroy target creature. It can't be regenerated.")]
#[case::an_unread_step("You may goad target creature.")]
#[case::lowercase("you may draw a card.")]
#[case::x_without_an_x_cost("You may draw X cards.")]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::a_chain("When ~ enters, you may draw a card, then discard a card.")]
#[case::if_you_do("When ~ enters, you may draw a card. If you do, discard a card.")]
#[case::another_player("When ~ enters, target player may draw a card.")]
#[case::each_player("At the beginning of your upkeep, each player may draw a card.")]
fn near_misses_in_a_trigger_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[test]
fn the_target_of_a_choice_is_the_clauses_target() {
    let Some(Ability::Triggered {
        targets, effects, ..
    }) = trigger("When ~ enters, you may destroy target artifact. You may draw a card.")
    else {
        panic!("not read");
    };
    assert_eq!(targets.len(), 1);
    assert_eq!(effects.len(), 2);
}
