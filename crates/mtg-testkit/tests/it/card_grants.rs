//! Goal: a keyword a static ability gives to the cards of a hand plays as the rules say at two to four seats. Rhet-Tomb Mystic gives each creature card in its controller's hand cycling {1}{U}, Tectonic Reformation gives each land card in its controller's hand cycling {R}, and Molecule Man gives each nonland card in its controller's hand miracle {0}: the grant is offered only for the cards it names, only to the player whose cards they are, only while its source is on the battlefield and only when the cost can be paid, the cycling is logged and draws a card, and the game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, names, pid};
use mtg_cards::keyword::Keyword;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::keywords::keywords_of;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{bear, forest, island, mountain};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 has `source` in play with `lands`, and holds `hand`; seat 1 holds a bear and a forest; any other seat just exists.
fn table(seats: usize, source: Option<&str>, lands: &[&str], hand: &[&str]) -> Scenario {
    let card = |name: &str| match name {
        "Grizzly Bears" => bear(),
        "Forest" => forest(),
        other => corpus_card(other).unwrap(),
    };
    let land = |name: &str| match name {
        "Island" => ready(island()),
        _ => ready(mountain()),
    };
    Scenario::new(seats)
        .battlefield(
            0,
            source
                .map(|name| ready(corpus_card(name).unwrap()))
                .into_iter()
                .chain(lands.iter().map(|name| land(name))),
        )
        .hand(0, hand.iter().map(|name| card(name)))
        .hand(1, [bear(), forest()])
}

/// How many abilities of the cards called `name` seat 0 is offered.
fn offered(driver: &Driver, name: &str) -> usize {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    options
        .abilities
        .iter()
        .filter(|ability| name_of(driver.state(), ability.object) == name)
        .count()
}

/// The card called `name` that seat `seat` holds in a hand.
fn held(driver: &Driver, seat: usize, name: &str) -> mtg_core::ids::ObjectId {
    driver
        .state()
        .zone(Zone::Hand(pid(seat)))
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap()
}

#[rstest]
#[case::a_creature_card_cycles_for_the_grant("Rhet-Tomb Mystic", &["Island", "Mountain"], "Grizzly Bears", 1)]
#[case::the_cost_must_be_payable("Rhet-Tomb Mystic", &["Island"], "Grizzly Bears", 0)]
#[case::a_land_card_is_not_a_creature_card("Rhet-Tomb Mystic", &["Island", "Mountain"], "Forest", 0)]
#[case::a_land_card_cycles_for_the_other_grant("Tectonic Reformation", &["Mountain"], "Forest", 1)]
#[case::a_creature_card_is_not_a_land_card("Tectonic Reformation", &["Mountain"], "Grizzly Bears", 0)]
fn the_granted_cycling_is_offered_for_the_cards_the_source_names(
    #[values(2, 3, 4)] seats: usize,
    #[case] source: &str,
    #[case] lands: &[&str],
    #[case] hand: &str,
    #[case] abilities: usize,
) {
    let driver = Driver::from_scenario(table(seats, Some(source), lands, &[hand])).unwrap();
    assert_eq!(offered(&driver, hand), abilities);
}

#[rstest]
fn the_grant_is_gone_without_its_source(#[values(2, 3, 4)] seats: usize) {
    let driver = Driver::from_scenario(table(
        seats,
        None,
        &["Island", "Mountain"],
        &["Grizzly Bears"],
    ))
    .unwrap();
    assert_eq!(offered(&driver, "Grizzly Bears"), 0);
    assert!(
        !keywords_of(driver.state(), held(&driver, 0, "Grizzly Bears"))
            .iter()
            .any(|keyword| matches!(keyword, Keyword::Cycling(_)))
    );
}

#[rstest]
fn the_grant_is_for_the_cards_of_its_controller(#[values(2, 3, 4)] seats: usize) {
    let driver = Driver::from_scenario(table(
        seats,
        Some("Rhet-Tomb Mystic"),
        &["Island"],
        &["Grizzly Bears"],
    ))
    .unwrap();
    let cycling = |seat: usize| {
        keywords_of(driver.state(), held(&driver, seat, "Grizzly Bears"))
            .iter()
            .any(|keyword| matches!(keyword, Keyword::Cycling(_)))
    };
    assert!(cycling(0), "the controller's creature card has it");
    assert!(!cycling(1), "an opponent's creature card does not");
}

#[rstest]
fn miracle_is_granted_to_nonland_cards_of_the_hand(#[values(2, 3, 4)] seats: usize) {
    let driver = Driver::from_scenario(table(
        seats,
        Some("Molecule Man"),
        &[],
        &["Grizzly Bears", "Forest"],
    ))
    .unwrap();
    let miracle = |name: &str| {
        keywords_of(driver.state(), held(&driver, 0, name))
            .iter()
            .any(|keyword| matches!(keyword, Keyword::Miracle(_)))
    };
    assert!(miracle("Grizzly Bears"));
    assert!(!miracle("Forest"));
}

#[rstest]
fn cycling_a_granted_card_discards_it_draws_and_replays(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        Some("Rhet-Tomb Mystic"),
        &["Island", "Mountain"],
        &["Grizzly Bears"],
    )
    .library(0, [forest(), forest()]);
    let setup = scenario.setup()?;
    let mut driver = Driver::from_scenario(
        table(
            seats,
            Some("Rhet-Tomb Mystic"),
            &["Island", "Mountain"],
            &["Grizzly Bears"],
        )
        .library(0, [forest(), forest()]),
    )?;
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    let ability = options
        .abilities
        .iter()
        .find(|ability| name_of(driver.state(), ability.object) == "Grizzly Bears")
        .copied()
        .unwrap();
    driver.answer(Answer::Priority(PriorityAction::Activate(ability)))?;
    let events = driver.pay_tapping(&["Island", "Mountain"], 0)?;
    assert!(
        events.iter().any(|e| matches!(e, Event::Cycled { .. })),
        "it is a cycling"
    );
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Grizzly Bears"]
    );
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Forest"]);
    let replayed = replay(&setup, driver.engine().events())?;
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
