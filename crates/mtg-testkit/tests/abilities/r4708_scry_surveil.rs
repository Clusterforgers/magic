//! Goal: "If you would scry a number of cards, draw that many cards instead." (Eligeth) turns its controller's scry
//! into that many draws with no scry asked and leaves another player's scry alone; "You may look at an additional two
//! cards each time you surveil." (Enhanced Surveillance) asks its controller before each surveil and shows two more
//! cards on a yes, the printed count on a no, and nothing more to another player's surveil, at tables of two to four.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A corpus card for no mana.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut definition = (*card(name).unwrap()).clone();
    definition.mana_cost = Some(ManaCost::parse("{0}").unwrap());
    Arc::new(definition)
}

/// `caster` holds `spell`; `owner` controls `permanent`; every library holds ten Forests.
fn table(seats: usize, caster: usize, spell: &str, owner: usize, permanent: &str) -> Driver {
    let mut scenario = Scenario::new(seats).hand(caster, [free(spell)]);
    scenario = if owner == caster {
        scenario.battlefield(caster, [ready(island()), ready(card(permanent).unwrap())])
    } else {
        scenario
            .battlefield(owner, [ready(card(permanent).unwrap())])
            .battlefield(caster, [ready(island())])
    };
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| forest()));
    }
    Driver::from_scenario(scenario).unwrap()
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(PlayerId(seat as u64))).len()
}

/// Passes priority until something other than priority with a full stack is asked, or the stack is empty.
fn settle(driver: &mut Driver) {
    for _ in 0..20 {
        match &driver.decision().unwrap().kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            _ => return,
        }
    }
}

#[rstest]
fn eligeth_draws_instead_of_its_controllers_scry(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] owner: usize,
) {
    // Seat 0 casts Opt (scry 1, then draw a card); Eligeth belongs to seat 0 or to an opponent.
    let mut driver = table(seats, 0, "Opt", owner, "Eligeth, Crossroads Augur");
    driver.cast("Opt").unwrap();
    settle(&mut driver);
    let mine = owner == 0;
    if !mine {
        assert!(
            matches!(driver.decision().unwrap().kind, DecisionKind::Scry(_)),
            "another player's Eligeth leaves the scry"
        );
        driver.answer_passively().unwrap();
        settle(&mut driver);
    }
    assert_eq!(driver.state().stack(), []);
    assert_eq!(hand(&driver, 0), if mine { 2 } else { 1 });
}

#[rstest]
fn enhanced_surveillance_offers_two_more_cards(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] yes: bool,
) {
    // Seat 0 casts Notion Rain (surveil 2, then draw two) beside its Enhanced Surveillance.
    let mut driver = table(seats, 0, "Notion Rain", 0, "Enhanced Surveillance");
    driver.cast("Notion Rain").unwrap();
    settle(&mut driver);
    let asked = driver.decision().unwrap().clone();
    assert!(matches!(asked.kind, DecisionKind::YesNo(_)), "{asked:?}");
    assert_eq!(asked.player, PlayerId(0));
    driver.answer(Answer::YesNo(YesNoChoice(yes))).unwrap();
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("the surveil asks which cards go");
    };
    assert_eq!(options.from.len(), if yes { 4 } else { 2 });
}

#[rstest]
fn enhanced_surveillance_leaves_another_players_surveil(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, 0, "Notion Rain", seats - 1, "Enhanced Surveillance");
    driver.cast("Notion Rain").unwrap();
    settle(&mut driver);
    let DecisionKind::ChooseCards(options) = &driver.decision().unwrap().kind else {
        panic!("no question comes before the surveil");
    };
    assert_eq!(options.from.len(), 2);
}
