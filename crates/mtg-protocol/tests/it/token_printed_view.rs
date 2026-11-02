//! Goal: a token's view carries the power and toughness it was created with beside its current ones, for every viewer, while a card and a spell on the stack carry none, so a client can tell a buffed token from a plain one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_protocol::StateView;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::replace_cards::raise_soldiers;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

#[rstest]
fn a_token_shows_its_created_stats_and_a_card_shows_none(#[values(0, 1)] viewer: usize) {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [raise_soldiers().unwrap()])
            .battlefield(
                0,
                [ready(plains()), ready(free_creature("Grizzly Bears", 2, 2))],
            ),
    )
    .unwrap();
    driver.cast("Raise Soldiers").unwrap();
    while !(driver.state().stack().is_empty() && driver.state().resolving().is_none()) {
        driver.answer_passively().unwrap();
    }
    let view = StateView::for_player(driver.state(), PlayerId::from_index(viewer));
    let on_battlefield = |o: &&mtg_protocol::ObjectView| o.zone == Zone::Battlefield;
    let stats = |kind: ObjectKind| -> Vec<_> {
        view.objects
            .iter()
            .filter(on_battlefield)
            .filter(|o| o.kind == kind && o.power.is_some())
            .map(|o| (o.power, o.printed_power, o.printed_toughness))
            .collect()
    };
    assert_eq!(
        stats(ObjectKind::Token),
        vec![(Some(1), Some(1), Some(1)); 2]
    );
    assert_eq!(stats(ObjectKind::Card), vec![(Some(2), None, None)]);
}
