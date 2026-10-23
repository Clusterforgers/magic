//! Goal: a search for two cards in one sentence ("a Forest card and a Plains card", "up to two basic land cards that share a land type") reads into two searches in a row, the second leaving out the card the first found, with a move for each and one shuffle, and writes back as the same text in a spell and in an activation, also for another player and after "you may"; two cards of different filters in any other wording, a shared type of anything but basic lands, and a count other than two stay with the table.
use super::*;

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

#[rstest]
#[case::krosan_verge(
    "Search your library for a Forest card and a Plains card, put them onto the battlefield tapped, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Subtype":"Forest"}],"as":"land"}},{"Search":{"filter":[{"Subtype":"Plains"},{"Not":{"Is":{"Bound":"land"}}}],"as":"land2"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Move":{"what":{"Bound":"land2"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#
)]
#[case::myriad_landscape(
    "Search your library for up to two basic land cards that share a land type, put them onto the battlefield tapped, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"count":{"min":0,"max":1},"as":"land"}},{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"},{"SharesSubtype":{"Bound":"land"}},{"Not":{"Is":{"Bound":"land"}}}],"count":{"min":0,"max":1},"as":"land2"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Move":{"what":{"Bound":"land2"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#
)]
fn two_cards_in_one_search_read_as_two_searches(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::forest_and_plains(
    "Search your library for a Forest card and a Plains card, put them onto the battlefield tapped, then shuffle."
)]
#[case::untapped(
    "Search your library for a Swamp card and a Mountain card, put them onto the battlefield, then shuffle."
)]
#[case::into_the_hand(
    "Search your library for an Island card and a Swamp card, reveal them, put them into your hand, then shuffle."
)]
#[case::basic_types(
    "Search your library for a basic Forest card and a basic Plains card, put them onto the battlefield tapped, then shuffle."
)]
#[case::card_types(
    "Search your library for a creature card and an artifact card, reveal them, put them into your hand, then shuffle."
)]
#[case::optional(
    "You may search your library for a Forest card and a Plains card, put them onto the battlefield tapped, then shuffle."
)]
#[case::shared_type(
    "Search your library for up to two basic land cards that share a land type, put them onto the battlefield tapped, then shuffle."
)]
#[case::shared_type_to_hand(
    "Search your library for up to two basic land cards that share a land type, reveal them, put them into your hand, then shuffle."
)]
fn two_cards_in_one_search_write_back_as_the_text(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case(
    "{2}, {T}, Sacrifice this land: Search your library for a Forest card and a Plains card, put them onto the battlefield tapped, then shuffle."
)]
#[case(
    "{2}, {T}, Sacrifice this land: Search your library for up to two basic land cards that share a land type, put them onto the battlefield tapped, then shuffle."
)]
fn a_search_for_two_cards_in_an_activation_reads_and_writes_back(#[case] text: &str) {
    assert!(activated(text).is_some(), "{text}");
    golden(text, ClauseKind::Activated, false);
}

#[test]
fn another_player_searches_for_two_cards() {
    let text = "Exile target creature. Its controller may search their library for a Forest card and a Plains card, put them onto the battlefield tapped, then shuffle.";
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::and_a_third(
    "Search your library for a Forest card and a Plains card and a Swamp card, put them onto the battlefield tapped, then shuffle."
)]
#[case::plural_second(
    "Search your library for a Forest card and two Plains cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::it_for_two(
    "Search your library for a Forest card and a Plains card, put it onto the battlefield tapped, then shuffle."
)]
#[case::up_to_three_sharing(
    "Search your library for up to three basic land cards that share a land type, put them onto the battlefield tapped, then shuffle."
)]
#[case::sharing_a_creature_type(
    "Search your library for up to two creature cards that share a creature type, put them onto the battlefield tapped, then shuffle."
)]
#[case::sharing_without_basic(
    "Search your library for up to two land cards that share a land type, put them onto the battlefield tapped, then shuffle."
)]
#[case::a_pair_on_top(
    "Search your library for a Forest card and a Plains card, then shuffle and put that card on top."
)]
#[case::unshared(
    "Search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle and share a land type."
)]
fn a_pair_that_is_not_worded_that_way_stays_unread(#[case] text: &str) {
    assert!(spell(text).is_none(), "{text}");
}
