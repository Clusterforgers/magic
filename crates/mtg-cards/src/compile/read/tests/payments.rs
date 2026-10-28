//! Goal: "you may pay {1}. If you do, ..." and "... unless that player pays 3 life", "... unless you discard a card" read as a `Pay` with one cost part (mana, life, energy, a discard, after "you may" a sacrifice), write back as the same text, and a payment with two parts, an X or a number counted by the game stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::mana(
    "Whenever ~ attacks, you may pay {1}. If you do, draw a card.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":"{1}","paid":[{"Draw":{}}]}}]}}"#
)]
#[case::life(
    "Whenever ~ attacks, you may pay 2 life. If you do, draw a card.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":[{"PayLife":2}],"paid":[{"Draw":{}}]}}]}}"#
)]
#[case::energy(
    "Whenever ~ attacks, you may pay {E}{E}. If you do, you gain 2 life.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":[{"PayEnergy":2}],"paid":[{"GainLife":{"amount":2}}]}}]}}"#
)]
#[case::a_discard(
    "Whenever ~ attacks, you may discard a card. If you do, draw a card.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":[{"Discard":{}}],"paid":[{"Draw":{}}]}}]}}"#
)]
#[case::a_sacrifice(
    "Whenever ~ attacks, you may sacrifice a creature. If you do, draw two cards.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"paid":[{"Draw":{"count":2}}]}}]}}"#
)]
#[case::scry(
    "Whenever ~ attacks, you may pay {2}. If you do, scry 2.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":"{2}","paid":[{"Scry":{"count":2}}]}}]}}"#
)]
fn a_payment_after_you_may_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::life(
    "Counter target spell unless its controller pays 3 life.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":[{"PayLife":3}],"unpaid":[{"Counter":{"what":{"Target":0}}}]}}]}}"#
)]
#[case::a_discard(
    "Counter target spell unless its controller discards a card.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":[{"Discard":{}}],"unpaid":[{"Counter":{"what":{"Target":0}}}]}}]}}"#
)]
#[case::energy(
    "Counter target spell unless its controller pays {E}{E}.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":[{"PayEnergy":2}],"unpaid":[{"Counter":{"what":{"Target":0}}}]}}]}}"#
)]
#[case::you_discard(
    "Draw a card unless you discard a card.",
    r#"{"Spell":{"effects":[{"Pay":{"cost":[{"Discard":{}}],"unpaid":[{"Draw":{}}]}}]}}"#
)]
#[case::you_pay(
    "You gain 2 life unless you pay {1}.",
    r#"{"Spell":{"effects":[{"Pay":{"cost":"{1}","unpaid":[{"GainLife":{"amount":2}}]}}]}}"#
)]
fn a_payment_after_unless_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case("Whenever an opponent casts a spell, you gain 1 life unless that player pays 2 life.")]
#[case("Whenever an opponent casts a spell, you gain 1 life unless that player discards a card.")]
#[case(
    "Whenever an opponent casts a spell, you may draw a card unless that player discards a card."
)]
#[case("Whenever a player casts a spell, draw a card unless that player pays {E}.")]
fn a_payment_by_the_player_of_an_event_reads_and_writes_back(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::two_parts("You may pay {1} and 2 life. If you do, draw a card.")]
#[case::two_parts_by_comma("You may pay {1}, discard a card. If you do, draw a card.")]
#[case::x_life("You may pay X life. If you do, draw a card.")]
#[case::x_discards("You may discard X cards. If you do, draw a card.")]
#[case::no_if_you_do("You may pay {1}. Draw a card.")]
#[case::if_you_dont("You may pay {1}. If you don't, draw a card.")]
#[case::nothing_after("You may pay {1}. If you do,")]
#[case::an_unread_effect("You may pay {1}. If you do, wish for a card.")]
#[case::another_player("Target player may pay {1}. If you do, draw a card.")]
#[case::tap("You may tap an untapped creature you control. If you do, draw a card.")]
#[case::life_the_game_counted("You may pay life equal to its power. If you do, draw a card.")]
#[case::no_cost("You may pay. If you do, draw a card.")]
#[case::sacrifice_in_unless("Draw a card unless you sacrifice a creature.")]
#[case::unless_x_life("Draw a card unless you pay X life.")]
#[case::unless_a_target("Draw a card unless target player pays {1}.")]
#[case::unless_two_parts("Draw a card unless you pay {1} and discard a card.")]
#[case::unless_you_counter("Counter target spell unless you discard a card.")]
#[case::unless_you_may("You may draw a card unless you discard a card.")]
fn payments_the_template_does_not_model_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
    assert_eq!(
        trigger(&format!("Whenever ~ attacks, {}", lower_first(text))),
        None,
        "{text}"
    );
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| format!("{}{}", first.to_lowercase(), chars.as_str()))
        .unwrap_or_default()
}
