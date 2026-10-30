//! Goal: a replacement that triples damage, tokens, counters, life or cards drawn and the draw of an opponent that its controller makes instead give the oracle wording of the data, and another factor, player, source, exception or card count never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

#[rstest]
#[case::city_on_fire(
    r#"{"Replacement":{"when":{"Damage":{"source":[{"ControlledBy":"You"}]}},"by":{"Multiply":3}}}"#,
    "If a source you control would deal damage to a permanent or player, it deals triple that damage instead."
)]
#[case::fiery_emancipation(
    r#"{"Replacement":{"when":{"Damage":{"source":[{"ControlledBy":"You"}]}},"by":{"Multiply":3}}}"#,
    "If a source you control would deal damage to a permanent or player, it deals triple that damage to that permanent or player instead."
)]
#[case::to_an_opponent(
    r#"{"Replacement":{"when":{"Damage":{"to":"Opponents"}},"by":{"Multiply":3}}}"#,
    "If a source would deal damage to an opponent, it deals triple that damage to that player instead."
)]
#[case::noncombat_by_a_creature(
    r#"{"Replacement":{"when":{"Damage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"combat":false}},"by":{"Multiply":3}}}"#,
    "If a creature you control would deal noncombat damage to a permanent or player, it deals triple that damage instead."
)]
#[case::tokens_as_an_effect_creates(
    r#"{"Replacement":{"when":{"TokenCreation":{}},"by":{"Multiply":3}}}"#,
    "If an effect would create one or more tokens under your control, it creates three times that many of those tokens instead."
)]
#[case::tokens_are_created(
    r#"{"Replacement":{"when":{"TokenCreation":{}},"by":{"Multiply":3}}}"#,
    "If one or more tokens would be created under your control, three times that many of those tokens are created instead."
)]
#[case::counters_on_a_creature(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}},"by":{"Multiply":3}}}"#,
    "If one or more +1/+1 counters would be put on a creature you control, three times that many +1/+1 counters are put on it instead."
)]
#[case::counters_of_any_kind(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"ControlledBy":"You"}]}}},"by":{"Multiply":3}}}"#,
    "If one or more counters would be put on a permanent you control, three times that many of those counters are put on it instead."
)]
#[case::life(
    r#"{"Replacement":{"when":{"LifeGain":{}},"by":{"Multiply":3}}}"#,
    "If you would gain life, you gain three times that much life instead."
)]
#[case::cards(
    r#"{"Replacement":{"when":{"Draw":{}},"by":{"Multiply":3}}}"#,
    "If you would draw a card, draw three cards instead."
)]
#[case::cards_after_the_draw_step(
    r#"{"Replacement":{"when":{"Draw":{"except_draw_step":1}},"by":{"Multiply":3}}}"#,
    "If you would draw a card except the first one you draw in each of your draw steps, draw three cards instead."
)]
#[case::notion_thief(
    r#"{"Replacement":{"when":{"Draw":{"who":"Opponents","except_draw_step":1}},"by":{"Instead":[{"Draw":{}}]}}}"#,
    "If an opponent would draw a card except the first one they draw in each of their draw steps, instead that player skips that draw and you draw a card."
)]
#[case::every_draw_of_an_opponent(
    r#"{"Replacement":{"when":{"Draw":{"who":"Opponents"}},"by":{"Instead":[{"Draw":{}}]}}}"#,
    "If an opponent would draw a card, instead that player skips that draw and you draw a card."
)]
#[case::after_the_first_two(
    r#"{"Replacement":{"when":{"Draw":{"who":"Opponents","except_draw_step":2}},"by":{"Instead":[{"Draw":{}}]}}}"#,
    "If an opponent would draw a card except the first two they draw in each of their draw steps, instead that player skips that draw and you draw a card."
)]
fn a_replacement_is_written_as_the_card_words_it(#[case] json: &str, #[case] clause: &str) {
    assert!(
        written(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::double_for_triple(
    r#"{"Replacement":{"when":{"Damage":{"source":[{"ControlledBy":"You"}]}},"by":{"Multiply":2}}}"#,
    "If a source you control would deal damage to a permanent or player, it deals triple that damage instead."
)]
#[case::triple_for_double(
    r#"{"Replacement":{"when":{"Damage":{"source":[{"ControlledBy":"You"}]}},"by":{"Multiply":3}}}"#,
    "If a source you control would deal damage to a permanent or player, it deals double that damage instead."
)]
#[case::quadruple_for_triple(
    r#"{"Replacement":{"when":{"Damage":{"source":[{"ControlledBy":"You"}]}},"by":{"Multiply":4}}}"#,
    "If a source you control would deal damage to a permanent or player, it deals triple that damage instead."
)]
#[case::your_sources_for_an_opponents(
    r#"{"Replacement":{"when":{"Damage":{"source":[{"ControlledBy":"Opponents"}]}},"by":{"Multiply":3}}}"#,
    "If a source you control would deal damage to a permanent or player, it deals triple that damage instead."
)]
#[case::an_opponents_tokens(
    r#"{"Replacement":{"when":{"TokenCreation":{"controller":"Opponents"}},"by":{"Multiply":3}}}"#,
    "If an effect would create one or more tokens under your control, it creates three times that many of those tokens instead."
)]
#[case::counters_of_another_kind(
    r#"{"Replacement":{"when":{"CountersPut":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"-1/-1"}},"by":{"Multiply":3}}}"#,
    "If one or more +1/+1 counters would be put on a creature you control, three times that many +1/+1 counters are put on it instead."
)]
#[case::three_cards_for_two(
    r#"{"Replacement":{"when":{"Draw":{}},"by":{"Multiply":3}}}"#,
    "If you would draw a card, draw two cards instead."
)]
#[case::twice_for_three_times(
    r#"{"Replacement":{"when":{"LifeGain":{}},"by":{"Multiply":3}}}"#,
    "If you would gain life, you gain twice that much life instead."
)]
#[case::an_exception_the_rules_lack(
    r#"{"Replacement":{"when":{"Draw":{"who":"Opponents"}},"by":{"Instead":[{"Draw":{}}]}}}"#,
    "If an opponent would draw a card except the first one they draw in each of their draw steps, instead that player skips that draw and you draw a card."
)]
#[case::the_wrong_count_of_the_exception(
    r#"{"Replacement":{"when":{"Draw":{"who":"Opponents","except_draw_step":2}},"by":{"Instead":[{"Draw":{}}]}}}"#,
    "If an opponent would draw a card except the first one they draw in each of their draw steps, instead that player skips that draw and you draw a card."
)]
#[case::you_draw_two(
    r#"{"Replacement":{"when":{"Draw":{"who":"Opponents","except_draw_step":1}},"by":{"Instead":[{"Draw":{"count":2}}]}}}"#,
    "If an opponent would draw a card except the first one they draw in each of their draw steps, instead that player skips that draw and you draw a card."
)]
#[case::the_opponent_draws_instead(
    r#"{"Replacement":{"when":{"Draw":{"who":"Opponents","except_draw_step":1}},"by":{"Instead":[{"Draw":{"who":"Opponents"}}]}}}"#,
    "If an opponent would draw a card except the first one they draw in each of their draw steps, instead that player skips that draw and you draw a card."
)]
#[case::your_own_draw(
    r#"{"Replacement":{"when":{"Draw":{"except_draw_step":1}},"by":{"Instead":[{"Draw":{}}]}}}"#,
    "If an opponent would draw a card except the first one they draw in each of their draw steps, instead that player skips that draw and you draw a card."
)]
fn another_factor_player_or_count_is_not_the_text(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

#[rstest]
#[case::a_condition_on_the_triple(
    r#"{"Replacement":{"when":{"Damage":{}},"by":{"Multiply":3},"condition":"YourTurn"}}"#
)]
#[case::another_zone_for_the_triple(
    r#"{"Replacement":{"when":{"Damage":{}},"by":{"Multiply":3},"from":"Graveyard"}}"#
)]
#[case::a_condition_on_the_skip(
    r#"{"Replacement":{"when":{"Draw":{"who":"Opponents"}},"by":{"Instead":[{"Draw":{}}]},"condition":"YourTurn"}}"#
)]
fn a_replacement_the_writers_cannot_say_is_not_written(#[case] json: &str) {
    assert_eq!(written_variants(&ability(json)), Vec::<String>::new());
}
