//! Goal: "put two cards from your hand on top of your library in any order" reads as a choice of cards of the hand and a move of them, alone and after "draw ..., then", and any other place, order, count or owner stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::brainstorm(
    "Draw three cards, then put two cards from your hand on top of your library in any order.",
    r#"{"Spell":{"effects":[{"Draw":{"count":3}},{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]},"count":{"min":2,"max":2},"as":"back"}},{"Move":{"what":{"Bound":"back"},"to":{"Library":{"place":"Top","order":"Chosen"}}}}]}}"#
)]
#[case::alone(
    "Put three cards from your hand on top of your library in any order.",
    r#"{"Spell":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]},"count":{"min":3,"max":3},"as":"back"}},{"Move":{"what":{"Bound":"back"},"to":{"Library":{"place":"Top","order":"Chosen"}}}}]}}"#
)]
#[case::one_card(
    "Draw a card, then put a card from your hand on top of your library.",
    r#"{"Spell":{"effects":[{"Draw":{}},{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]},"as":"back"}},{"Move":{"what":{"Bound":"back"},"to":{"Library":{"place":"Top"}}}}]}}"#
)]
#[case::in_two_sentences(
    "Draw three cards. Put two cards from your hand on top of your library in any order.",
    r#"{"Spell":{"effects":[{"Draw":{"count":3}},{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]},"count":{"min":2,"max":2},"as":"back"}},{"Move":{"what":{"Bound":"back"},"to":{"Library":{"place":"Top","order":"Chosen"}}}}]}}"#
)]
fn a_put_back_reads_in_the_shape_of_the_hand_written_files(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case("Draw three cards, then put two cards from your hand on top of your library in any order.")]
#[case("Draw two cards, then put two cards from your hand on top of your library in any order.")]
#[case("Draw a card, then put a card from your hand on top of your library.")]
#[case("Draw two cards, then put a card from your hand on top of your library.")]
#[case("Scry 1, then put a card from your hand on top of your library.")]
#[case("Put a card from your hand on top of your library.")]
#[case("Put two cards from your hand on top of your library in any order.")]
#[case("Put four cards from your hand on top of your library in any order.")]
#[case("Draw three cards. Put two cards from your hand on top of your library in any order.")]
#[case("Put a card from your hand on top of your library. Draw a card.")]
#[case("Draw a card, then put a card from your hand on top of your library. You gain 2 life.")]
fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When ~ enters, draw two cards, then put a card from your hand on top of your library.")]
#[case("Whenever ~ attacks, put a card from your hand on top of your library.")]
#[case(
    "At the beginning of your upkeep, draw a card, then put a card from your hand on top of your library."
)]
fn the_wordings_read_in_a_trigger(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::no_order("Put two cards from your hand on top of your library.")]
#[case::an_order_for_one_card("Put a card from your hand on top of your library in any order.")]
#[case::a_random_order("Put two cards from your hand on top of your library in a random order.")]
#[case::the_bottom("Put two cards from your hand on the bottom of your library in any order.")]
#[case::the_graveyard("Put two cards from your graveyard on top of your library in any order.")]
#[case::into_the_library("Put a card from your hand into your library.")]
#[case::shuffled("Put a card from your hand into your library, then shuffle.")]
#[case::up_to("Put up to two cards from your hand on top of your library in any order.")]
#[case::x_cards("Put X cards from your hand on top of your library in any order.")]
#[case::all_cards("Put all cards from your hand on top of your library in any order.")]
#[case::another_library(
    "Put two cards from your hand on top of target player's library in any order."
)]
#[case::another_hand(
    "Put two cards from target player's hand on top of your library in any order."
)]
#[case::one_written_out("Put one card from your hand on top of your library.")]
#[case::a_plural_for_one("Put a cards from your hand on top of your library.")]
#[case::a_singular_for_two("Put two card from your hand on top of your library in any order.")]
#[case::twice(
    "Put a card from your hand on top of your library. Put a card from your hand on top of your library."
)]
#[case::then_twice(
    "Draw a card, then put a card from your hand on top of your library, then put a card from your hand on top of your library."
)]
#[case::then_shuffle(
    "Draw three cards, then put two cards from your hand on top of your library in any order, then shuffle."
)]
#[case::a_creature_card("Put a creature card from your hand on top of your library.")]
#[case::no_zone("Put two cards on top of your library in any order.")]
fn another_place_order_or_count_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
