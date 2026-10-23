//! Goal: "Its controller may search their library for a basic land card, put it onto the battlefield [tapped], then shuffle." after an exile or a destroy reads into a `May` of the controller of the target holding their `Search`, the move and their `Shuffle`, and writes back as the same text ("that card" and "That land's controller" included), in a spell, an activation and a trigger; a search by someone else, one without "may", without a permanent before it, and one the other player would make for you stay with the table.
use super::*;

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::path_to_exile(
    "Exile target creature. Its controller may search their library for a basic land card, put that card onto the battlefield tapped, then shuffle.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Exile":{"what":{"Target":0}}},{"May":{"who":{"ControllerOf":{"Target":0}},"effects":[{"Search":{"who":{"ControllerOf":{"Target":0}},"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{"who":{"ControllerOf":{"Target":0}}}}]}}]}}"#
)]
#[case::ghost_quarter(
    "Destroy target land. Its controller may search their library for a basic land card, put it onto the battlefield, then shuffle.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"May":{"who":{"ControllerOf":{"Target":0}},"effects":[{"Search":{"who":{"ControllerOf":{"Target":0}},"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{}}}},{"Shuffle":{"who":{"ControllerOf":{"Target":0}}}}]}}]}}"#
)]
fn the_controller_of_a_removed_permanent_may_search(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::tapped(
    "Exile target creature. Its controller may search their library for a basic land card, put it onto the battlefield tapped, then shuffle."
)]
#[case::that_card(
    "Exile target creature. Its controller may search their library for a basic land card, put that card onto the battlefield tapped, then shuffle."
)]
#[case::that_creatures_controller(
    "Exile target creature. That creature's controller may search their library for a basic land card, put it onto the battlefield tapped, then shuffle."
)]
#[case::untapped_destroy(
    "Destroy target land. Its controller may search their library for a basic land card, put it onto the battlefield, then shuffle."
)]
#[case::a_land_card(
    "Destroy target land. Its controller may search their library for a land card, put it onto the battlefield, then shuffle."
)]
#[case::up_to_two(
    "Destroy target land. Its controller may search their library for up to two basic land cards, put them onto the battlefield tapped, then shuffle."
)]
fn a_search_by_a_controller_writes_back_as_the_text(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case(
    "{T}, Sacrifice this land: Destroy target land. Its controller may search their library for a basic land card, put it onto the battlefield, then shuffle."
)]
#[case(
    "{2}, {T}, Sacrifice this land: Destroy target nonbasic land an opponent controls. That land's controller may search their library for a basic land card, put it onto the battlefield, then shuffle."
)]
fn an_activation_that_destroys_a_land_lets_its_controller_search(#[case] text: &str) {
    assert!(activated(text).is_some(), "{text}");
    golden(text, ClauseKind::Activated, false);
}

#[test]
fn a_trigger_that_destroys_a_land_lets_its_controller_search() {
    let text = "When this creature enters, destroy target land. Its controller may search their library for a basic land card, put it onto the battlefield, then shuffle.";
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::your_library(
    "Exile target creature. Its controller may search your library for a basic land card, put it onto the battlefield tapped, then shuffle."
)]
#[case::without_may(
    "Exile target creature. Its controller searches their library for a basic land card, put it onto the battlefield tapped, then shuffle."
)]
#[case::an_opponent(
    "Exile target creature. Target opponent may search their library for a basic land card, put it onto the battlefield tapped, then shuffle."
)]
#[case::no_shuffle(
    "Exile target creature. Its controller may search their library for a basic land card, put it onto the battlefield tapped."
)]
#[case::their_hand(
    "Exile target creature. Its controller may search their library for a basic land card, put it into their hand, then shuffle."
)]
fn a_search_that_is_not_worded_that_way_stays_unread(#[case] text: &str) {
    assert!(spell(text).is_none(), "{text}");
}

#[test]
fn its_controller_needs_a_permanent_before_it() {
    let text = "Its controller may search their library for a basic land card, put it onto the battlefield tapped, then shuffle.";
    assert!(spell(text).is_none(), "{text}");
}
