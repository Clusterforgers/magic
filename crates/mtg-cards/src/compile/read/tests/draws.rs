//! Goal: the draw wordings read into the DSL and write back as the same text: a draw step that gives "an additional card" to the player whose step it is, a draw and a loss of life that each say "you", "you attack with two or more creatures", and "their second spell each turn" for an opponent; near-misses stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::each_players_draw_step(
    "At the beginning of each player's draw step, that player draws an additional card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw","whose":"EachPlayer"}},"effects":[{"Draw":{"who":"Triggering"}}]}}"#
)]
#[case::each_opponents_draw_step(
    "At the beginning of each opponent's draw step, that player draws two additional cards.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw","whose":"Opponents"}},"effects":[{"Draw":{"who":"Triggering","count":2}}]}}"#
)]
#[case::your_draw_step(
    "At the beginning of your draw step, draw an additional card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw","whose":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::your_draw_step_three(
    "At the beginning of your draw step, draw three additional cards.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw","whose":"You"}},"effects":[{"Draw":{"count":3}}]}}"#
)]
#[case::the_draw_step_of_each_player_as_the_card_says_it(
    "At the beginning of each draw step, that player draws a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw","whose":"EachPlayer"}},"effects":[{"Draw":{"who":"Triggering"}}]}}"#
)]
#[case::each_players_upkeep(
    "At the beginning of each player's upkeep, that player loses 1 life.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"EachPlayer"}},"effects":[{"LoseLife":{"who":"Triggering","amount":1}}]}}"#
)]
#[case::each_players_upkeep_for_you(
    "At the beginning of each player's upkeep, draw a card.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"EachPlayer"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::you_attack_with_two(
    "Whenever you attack with two or more creatures, draw a card.",
    r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at_least":2}},"effects":[{"Draw":{}}]}}"#
)]
#[case::you_attack_with_three(
    "Whenever you attack with three or more creatures, you gain 2 life.",
    r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at_least":3}},"effects":[{"GainLife":{"amount":2}}]}}"#
)]
#[case::creatures_you_control_attack(
    "Whenever two or more creatures you control attack, draw a card.",
    r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at_least":2}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponents_second_spell(
    "Whenever an opponent casts their second spell each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"SpellCast":{"what":[],"by":"Opponents"}},"nth":2}},"effects":[{"Draw":{}}]}}"#
)]
#[case::a_players_first_spell(
    "Whenever a player casts their first spell each turn, you gain 1 life.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"SpellCast":{"what":[],"by":"EachPlayer"}},"nth":1}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::an_opponents_second_card(
    "Whenever an opponent draws their second card each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"Drew":{"who":"Opponents"}},"nth":2}},"effects":[{"Draw":{}}]}}"#
)]
#[case::your_second_spell_as_before(
    "Whenever you cast your second spell each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"SpellCast":{"what":[],"by":"You"}},"nth":2}},"effects":[{"Draw":{}}]}}"#
)]
#[case::you_draw_and_you_lose(
    "When this creature enters, you draw a card and you lose 1 life.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}},{"LoseLife":{"amount":1}}]}}"#
)]
#[case::you_draw_and_lose(
    "When this creature enters, you draw a card and lose 1 life.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}},{"LoseLife":{"amount":1}}]}}"#
)]
fn a_draw_wording_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::two_cards_and_two_life(
    "You draw two cards and you lose 2 life.",
    r#"{"Spell":{"effects":[{"Draw":{"count":2}},{"LoseLife":{"amount":2}}]}}"#
)]
#[case::a_card_and_a_life(
    "You draw a card and you lose 1 life.",
    r#"{"Spell":{"effects":[{"Draw":{}},{"LoseLife":{"amount":1}}]}}"#
)]
#[case::gain_and_draw(
    "You gain 3 life and you draw a card.",
    r#"{"Spell":{"effects":[{"GainLife":{"amount":3}},{"Draw":{}}]}}"#
)]
#[case::the_subject_said_once(
    "You draw a card and lose 1 life.",
    r#"{"Spell":{"effects":[{"Draw":{}},{"LoseLife":{"amount":1}}]}}"#
)]
#[case::three_verbs_each_with_a_subject(
    "You draw a card and you lose 1 life and you gain 2 life.",
    r#"{"Spell":{"effects":[{"Draw":{}},{"LoseLife":{"amount":1}},{"GainLife":{"amount":2}}]}}"#
)]
#[case::another_player_and_you(
    "Target player draws a card and you lose 1 life.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Draw":{"who":{"Target":0}}},{"LoseLife":{"amount":1}}]}}"#
)]
fn two_verbs_of_you_read_with_one_subject_or_with_each_saying_it(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::in_an_activation("{1}{B}, Sacrifice a creature: You draw a card and you lose 1 life.")]
#[case::in_an_activation_with_a_sorcery_timing(
    "{T}: You draw a card and you lose 1 life. Activate only as a sorcery."
)]
fn an_activation_says_you_again(#[case] text: &str) {
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::the_other_step(
    "At the beginning of each player's end step, that player draws an additional card."
)]
#[case::additional_without_a_draw(
    "At the beginning of each player's draw step, that player discards an additional card."
)]
#[case::additional_outside_a_draw_step("When this creature enters, draw an additional card.")]
#[case::additional_with_a_count_of_one_word(
    "At the beginning of your draw step, draw additional cards."
)]
#[case::the_card_before_additional("At the beginning of your draw step, draw a additional card.")]
#[case::a_player_who_is_not_named(
    "At the beginning of each player's draw step, draws an additional card."
)]
#[case::the_possessive_without_each(
    "At the beginning of player's draw step, that player draws an additional card."
)]
#[case::each_other_player(
    "At the beginning of each other player's draw step, that player draws an additional card."
)]
#[case::each_players_combat("At the beginning of each player's combat, that player draws a card.")]
#[case::attack_with_a_creature_kind("Whenever you attack with two or more Goblins, draw a card.")]
#[case::attack_with_no_count("Whenever you attack with creatures, draw a card.")]
#[case::an_opponent_attacks(
    "Whenever an opponent attacks with two or more creatures, draw a card."
)]
#[case::attack_with_and("Whenever you attack with two creatures, draw a card.")]
#[case::attack_alone("Whenever you attack with a creature, draw a card.")]
#[case::second_spell_of_another_player(
    "Whenever an opponent casts your second spell each turn, draw a card."
)]
#[case::your_for_an_opponent("Whenever you cast their second spell each turn, draw a card.")]
#[case::their_without_a_turn("Whenever an opponent casts their second spell, draw a card.")]
#[case::a_sixth_spell("Whenever an opponent casts their sixth spell each turn, draw a card.")]
#[case::each_player_lose_and_you("You draw a card and you.")]
#[case::and_you_without_a_verb("You draw a card and you a card.")]
#[case::another_subject_after_and("You draw a card and target opponent loses 1 life.")]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text).or_else(|| spell(text)), None, "{text}");
}

#[rstest]
#[case::the_hand(
    "Discard your hand.",
    r#"{"Spell":{"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}]}}"#
)]
#[case::the_hand_and_then_cards(
    "Discard your hand, then draw three cards.",
    r#"{"Spell":{"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}},{"Draw":{"count":3}}]}}"#
)]
#[case::you_discard_the_hand(
    "You discard your hand and you draw a card.",
    r#"{"Spell":{"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}},{"Draw":{}}]}}"#
)]
#[case::a_target_player_discards_their_hand(
    "Target player discards their hand.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Discard":{"who":{"Target":0},"count":{"Count":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}]}}"#
)]
#[case::cards_as_before(
    "Discard two cards.",
    r#"{"Spell":{"effects":[{"Discard":{"count":2}}]}}"#
)]
fn the_hand_is_every_card_a_player_discards(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::end_step(
    "At the beginning of your end step, discard your hand.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}]}}"#
)]
#[case::enters_and_then_draws(
    "When this creature enters, discard your hand, then draw three cards.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}},{"Draw":{"count":3}}]}}"#
)]
#[case::that_player_discards_their_hand(
    "Whenever an opponent draws a card, that player discards their hand.",
    r#"{"Triggered":{"when":{"Drew":{"who":"Opponents"}},"effects":[{"Discard":{"who":"Triggering","count":{"Count":[{"In":"Hand"},{"OwnedBy":"Triggering"}]}}}]}}"#
)]
fn a_trigger_discards_a_hand(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::your_for_another_player("Target player discards your hand.")]
#[case::their_for_you("Discard their hand.")]
#[case::the_hand_of_a_card("Discard the hand.")]
#[case::hands("Discard your hands.")]
#[case::half_of_it("Discard half your hand.")]
#[case::with_a_second_verb_and_no_comma("Discard your hand draw a card.")]
fn near_misses_of_the_hand_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::counted_after_and_you(
    "You draw a card for each creature you control and you lose 1 life.",
    true
)]
#[case::counted_without_a_second_subject(
    "You draw a card and lose 1 life for each creature you control.",
    false
)]
fn a_count_after_two_verbs_belongs_to_the_verb_it_follows(#[case] text: &str, #[case] reads: bool) {
    assert_eq!(spell(text).is_some(), reads, "{text}");
}
