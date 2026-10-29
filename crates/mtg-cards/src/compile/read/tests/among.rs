//! Goal: looking at or revealing the top cards of the library and taking cards "from among them" (one or several, to the hand or the battlefield, a filter of types, subtypes and values) with the rest on the bottom or in the graveyard reads in one shape, alone, in a trigger and in an activated ability, writes back as the same text, and leaves every other filter, count, destination and order to the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::look_and_reveal_to_the_hand(
    "Look at the top five cards of your library. You may reveal a creature card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.",
    r#"{"Spell":{"effects":[{"LookAtTop":{"count":5,"as":"top"}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Type":"Creature"}]},"count":{"min":0,"max":1},"as":"kept"}},{"Reveal":{"what":{"Bound":"kept"}}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#
)]
#[case::reveal_and_put_into_the_hand(
    "Reveal the top four cards of your library. You may put a land card from among them into your hand. Put the rest into your graveyard.",
    r#"{"Spell":{"effects":[{"LookAtTop":{"count":4,"as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Type":"Land"}]},"count":{"min":0,"max":1},"as":"kept"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"top"},"to":"Graveyard"}}]}}"#
)]
#[case::put_onto_the_battlefield_tapped(
    "Look at the top five cards of your library. You may put a land card from among them onto the battlefield tapped. Put the rest on the bottom of your library in a random order.",
    r#"{"Spell":{"effects":[{"LookAtTop":{"count":5,"as":"top"}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Type":"Land"}]},"count":{"min":0,"max":1},"as":"kept"}},{"Move":{"what":{"Bound":"kept"},"to":{"Battlefield":{"tapped":true}}}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#
)]
#[case::put_all_revealed(
    "Reveal the top four cards of your library. Put all land cards revealed this way into your hand and the rest into your graveyard.",
    r#"{"Spell":{"effects":[{"LookAtTop":{"count":4,"as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"Move":{"what":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Type":"Land"}]},"to":"Hand"}},{"Move":{"what":{"Bound":"top"},"to":"Graveyard"}}]}}"#
)]
fn the_top_cards_read_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::any_order(
    "Look at the top three cards of your library. You may reveal an instant or sorcery card from among them and put it into your hand. Put the rest on the bottom of your library in any order."
)]
#[case::a_subtype_and_a_type(
    "Look at the top six cards of your library. You may reveal an Elf creature card from among them and put it into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::a_value(
    "Look at the top seven cards of your library. You may reveal a creature card with mana value 3 or less from among them and put it into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::that_card(
    "Look at the top four cards of your library. You may reveal an artifact card from among them and put that card into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::two_types(
    "Reveal the top five cards of your library. You may put a creature or land card from among them into your hand. Put the rest into your graveyard."
)]
#[case::up_to_two_to_the_battlefield(
    "Look at the top six cards of your library. Put up to two land cards from among them onto the battlefield tapped. Put the rest on the bottom of your library in a random order."
)]
#[case::up_to_two_revealed(
    "Look at the top six cards of your library. You may reveal up to two creature cards from among them and put them into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::any_number(
    "Reveal the top six cards of your library. Put any number of enchantment cards from among them into your hand. Put the rest into your graveyard."
)]
#[case::a_mandatory_card(
    "Reveal the top five cards of your library. Put a land card from among them onto the battlefield. Put the rest into your graveyard."
)]
#[case::all_to_the_battlefield(
    "Look at the top six cards of your library. Put all land cards from among them onto the battlefield tapped and the rest on the bottom of your library in a random order."
)]
#[case::all_revealed_to_the_bottom(
    "Reveal the top three cards of your library. Put all nonland permanent cards revealed this way into your hand and the rest on the bottom of your library in any order."
)]
#[case::two_cards(
    "Look at the top two cards of your library. You may put a land card from among them onto the battlefield tapped. Put the rest on the bottom of your library in a random order."
)]
fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::enters(
    "When ~ enters, look at the top four cards of your library. You may reveal an artifact card from among them and put it into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::attacks(
    "Whenever ~ attacks, look at the top six cards of your library. You may put a creature card from among them onto the battlefield. Put the rest on the bottom of your library in a random order."
)]
#[case::enters_reveal(
    "When ~ enters, reveal the top four cards of your library. You may put a land card from among them into your hand. Put the rest into your graveyard."
)]
fn the_wordings_read_in_a_trigger(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[test]
fn the_wordings_read_in_an_activated_ability() {
    let text = "{3}, {T}: Look at the top three cards of your library. You may reveal a Hero card from among them and put it into your hand. Put the rest on the bottom of your library in any order.";
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::the_rest_on_top(
    "Look at the top five cards of your library. You may reveal a creature card from among them and put it into your hand. Put the rest on top of your library in any order."
)]
#[case::no_order(
    "Look at the top five cards of your library. You may reveal a creature card from among them and put it into your hand. Put the rest on the bottom of your library."
)]
#[case::no_rest(
    "Look at the top five cards of your library. You may reveal a creature card from among them and put it into your hand."
)]
#[case::an_amount_that_is_not_a_number(
    "Look at the top X cards of your library. You may reveal a creature card from among them and put it into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::an_opponent_library(
    "Look at the top four cards of target opponent's library. You may put a creature card from among them onto the battlefield. Put the rest into their graveyard."
)]
#[case::a_chosen_type(
    "Look at the top five cards of your library. You may reveal a creature card of the chosen type from among them and put it into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::up_to_with_may(
    "Look at the top five cards of your library. You may put up to two land cards from among them onto the battlefield. Put the rest into your graveyard."
)]
#[case::exile_instead(
    "Look at the top four cards of your library. You may exile a creature card from among them. Put the rest on the bottom of your library in any order."
)]
#[case::a_cast(
    "Look at the top six cards of your library. You may cast a spell from among them without paying its mana cost. Put the rest on the bottom of your library in a random order."
)]
#[case::an_unknown_word(
    "Look at the top five cards of your library. You may reveal a creature card from among them and shuffle it into your library. Put the rest into your graveyard."
)]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert!(spell(text).is_none(), "{text}");
}
