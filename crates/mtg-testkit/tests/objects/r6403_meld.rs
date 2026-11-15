//! Goal: meld (CR 701.42, 712.4): the two cards of a meld pair that one player owns and controls are exiled and return as
//! one melded permanent with the melded card's characteristics, the mana value of both front faces and a planeswalker's
//! loyalty; it enters as a new object whose enters trigger fires, leaves the battlefield as two cards, and nothing melds
//! when the partner is another player's; from a trigger and from an activated ability, at two to four seats, and each log
//! replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, replay};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::spell;

const GISELA: &str = "Gisela, the Broken Blade";
const BRUNA: &str = "Bruna, the Fading Light";
const BRISELA: &str = "Brisela, Voice of Nightmares";

/// The melded cards the game knows, as a server resolves them for the decks.
fn r6403_melded() -> Vec<(String, Arc<CardDefinition>)> {
    [BRISELA, "Urza, Planeswalker", "Titania, Gaea Incarnate"]
        .into_iter()
        .map(|name| (name.to_owned(), card(name).unwrap()))
        .collect()
}

fn r6403_driver(scenario: &Scenario) -> Driver {
    let setup = scenario
        .clone()
        .setup()
        .unwrap()
        .with_conjurable(r6403_melded());
    Driver::new(Engine::new(setup).unwrap()).unwrap()
}

fn r6403_replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario
        .clone()
        .setup()
        .unwrap()
        .with_conjurable(r6403_melded());
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// The names of the cards in a zone.
fn r6403_names(driver: &Driver, zone: Zone) -> Vec<String> {
    let mut names: Vec<String> = driver
        .state()
        .zone(zone)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .map(|object| object.card().name.clone())
        .collect();
    names.sort();
    names
}

fn r6403_on_battlefield(driver: &Driver, name: &str) -> Option<mtg_core::ids::ObjectId> {
    let state = driver.state();
    state.zone(Zone::Battlefield).iter().copied().find(|id| {
        state
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    })
}

fn r6403_settle(driver: &mut Driver) {
    for _ in 0..200 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack does not empty");
}

/// Seat 0 controls Gisela and the Bruna of `bruna_seat`, and holds a free instant that destroys a creature (Brisela keeps
/// its opponents from casting one).
fn r6403_angels(seats: usize, bruna_seat: usize) -> Scenario {
    let kill = spell(
        "Kill",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"Destroy":{"what":{"Target":0}}}]"#,
    );
    let mut boards = vec![vec![ready(card(GISELA).unwrap())], Vec::new()];
    boards[bruna_seat].push(ready(card(BRUNA).unwrap()));
    let [mine, theirs] = <[_; 2]>::try_from(boards).unwrap();
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .battlefield(1, theirs)
        .hand(0, vec![kill])
}

#[rstest]
fn r6403_the_pair_melds_and_leaves_as_two_cards(#[values(2, 3, 4)] seats: usize) {
    let scenario = r6403_angels(seats, 0);
    let mut driver = r6403_driver(&scenario);
    driver.idle_until(2, Step::Upkeep).unwrap();
    let brisela = r6403_on_battlefield(&driver, BRISELA).expect("the angels meld at the end step");
    assert_eq!(r6403_names(&driver, Zone::Battlefield).len(), 1);
    assert_eq!(r6403_names(&driver, Zone::Exile).len(), 0);
    let object = driver.state().object(brisela).unwrap();
    assert_eq!(object.controller().index(), 0);
    // The mana value is the total of the front faces' (CR 712.8): {2}{W}{W} and {5}{W}{W}.
    assert_eq!(object.mana_value(), 11);
    assert_eq!(object.card().power, Some(9));
    // Its controller destroys it: both cards go to their owner's graveyard.
    while driver.decision().unwrap().player.index() != 0 {
        driver.pass().unwrap();
    }
    let at =
        mtg_engine::choose::Target::Object(driver.state().object(brisela).unwrap().reference());
    driver.cast_at("Kill", &[at]).unwrap();
    r6403_settle(&mut driver);
    assert!(r6403_on_battlefield(&driver, BRISELA).is_none());
    let owner = mtg_core::ids::PlayerId::from_index(0);
    let graveyard = r6403_names(&driver, Zone::Graveyard(owner));
    assert_eq!(graveyard, [BRUNA, GISELA, "Kill"]);
    r6403_replays(&scenario, &driver);
}

#[rstest]
fn r6403_another_players_partner_does_not_meld(#[values(2, 3, 4)] seats: usize) {
    let scenario = r6403_angels(seats, 1);
    let mut driver = r6403_driver(&scenario);
    driver.idle_until(2, Step::Upkeep).unwrap();
    assert!(r6403_on_battlefield(&driver, BRISELA).is_none());
    assert!(r6403_on_battlefield(&driver, GISELA).is_some());
    assert!(r6403_on_battlefield(&driver, BRUNA).is_some());
    assert_eq!(r6403_names(&driver, Zone::Exile).len(), 0);
    r6403_replays(&scenario, &driver);
}

#[rstest]
fn r6403_an_activated_meld_makes_a_planeswalker_with_loyalty(#[values(2, 3, 4)] seats: usize) {
    let mut board = vec![
        ready(card("Urza, Lord Protector").unwrap()),
        ready(card("The Mightstone and Weakstone").unwrap()),
    ];
    board.extend(lands(7, island));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board);
    let mut driver = r6403_driver(&scenario);
    driver.begin_activation("Urza, Lord Protector").unwrap();
    if matches!(driver.decision().unwrap().kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    }
    r6403_settle(&mut driver);
    let urza = r6403_on_battlefield(&driver, "Urza, Planeswalker").expect("Urza melds");
    let object = driver.state().object(urza).unwrap();
    assert_eq!(object.mana_value(), 3 + 5);
    let loyalty = object
        .permanent()
        .unwrap()
        .counters_of(&mtg_cards::value::CounterKind::Loyalty);
    assert_eq!(loyalty, 7);
    r6403_replays(&scenario, &driver);
}

#[rstest]
fn r6403_the_melded_permanent_has_its_enters_trigger(#[values(2, 3, 4)] seats: usize) {
    let mut board = vec![
        ready(card("Titania, Voice of Gaea").unwrap()),
        ready(card("Argoth, Sanctum of Nature").unwrap()),
    ];
    board.extend(lands(2, island));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .graveyard(0, vec![forest(), forest(), forest(), forest()]);
    let mut driver = r6403_driver(&scenario);
    let next = u32::try_from(seats + 1).unwrap();
    driver.idle_until(next, Step::Draw).unwrap();

    assert!(r6403_on_battlefield(&driver, "Titania, Gaea Incarnate").is_some());
    // Its enters trigger returned the four Forests.
    let forests = r6403_names(&driver, Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Forest")
        .count();
    assert_eq!(forests, 4);
    r6403_replays(&scenario, &driver);
}
