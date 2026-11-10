//! Goal: the table lists for each seat the tokens its own cards make, in any zone and hidden ones too, with their words, and never another seat's; a spectator and a seat whose cards make none get no list, and the wire leaves the field out then.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_protocol::{StateView, TokenOfferView};
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0: Abstruse Interference in the library and Angelic Sleuth on the battlefield; seat 1: islands only.
fn table() -> Driver {
    let scenario = Scenario::new(2)
        .library(
            0,
            [card("Abstruse Interference").unwrap()]
                .into_iter()
                .chain((0..10).map(|_| island()))
                .collect::<Vec<_>>(),
        )
        .battlefield(0, [ready(card("Angelic Sleuth").unwrap())])
        .library(1, (0..10).map(|_| island()).collect::<Vec<_>>());
    Driver::from_scenario(scenario).unwrap()
}

fn offers(driver: &Driver, viewer: usize) -> Vec<TokenOfferView> {
    StateView::for_player(driver.state(), PlayerId::from_index(viewer))
        .table
        .tokens
}

#[rstest]
#[case::the_owner(0, &[("Abstruse Interference", "Eldrazi Scion"), ("Angelic Sleuth", "Clue")])]
#[case::an_opponent_whose_cards_make_none(1, &[])]
#[case::a_spectator(2, &[])]
fn each_seat_is_offered_the_tokens_of_its_own_cards(
    #[case] viewer: usize,
    #[case] expected: &[(&str, &str)],
) {
    let driver = table();
    let offers = offers(&driver, viewer);
    let seen: Vec<_> = offers
        .iter()
        .map(|offer| (offer.card.as_str(), offer.name.as_str()))
        .collect();
    assert_eq!(seen, expected);
    for offer in &offers {
        assert!(!offer.rules.is_empty(), "{} has words", offer.name);
    }
    let wire = serde_json::to_value(
        StateView::for_player(driver.state(), PlayerId::from_index(viewer)).table,
    )
    .unwrap();
    assert_eq!(wire.get("tokens").is_some(), !expected.is_empty());
}
