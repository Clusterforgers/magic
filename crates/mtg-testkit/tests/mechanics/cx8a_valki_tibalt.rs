//! Goal: Valki, God of Lies // Tibalt, Cosmic Impostor plays whole at two to four seats: Valki exiles a creature card
//! from each opponent's revealed hand until it leaves (none when it left first), {X} makes it a copy of an exiled card
//! with mana value X (nothing without one), and Tibalt's emblem plays the cards that Tibalt exiled, with mana of any
//! color, while it stays and after it left; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::abilities::AbilityRef;
use mtg_engine::choose::{CardsChoice, NumberChoice, OptionChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

const CARD: &str = "Valki, God of Lies // Tibalt, Cosmic Impostor";
const VALKI: &str = "Valki, God of Lies";
const TIBALT: &str = "Tibalt, Cosmic Impostor";

fn cx8a_replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Answers until nothing is on the stack or waits to go there: the first cards a choice offers, `x` for a number and
/// `face` for the face a spell is cast as.
fn cx8a_settle(driver: &mut Driver, x: u32, face: usize) {
    for _ in 0..300 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
            && matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
        {
            return;
        }
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(x)),
            DecisionKind::ChooseOption(_) => Answer::ChooseOption(OptionChoice(face)),
            DecisionKind::ChooseCards(options) => Answer::ChooseCards(CardsChoice(
                options
                    .from
                    .iter()
                    .copied()
                    .take((options.max as usize).max(options.min as usize).max(1))
                    .collect(),
            )),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the stack does not empty");
}

fn cx8a_named(driver: &Driver, zone: Zone, name: &str) -> Vec<ObjectId> {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == name)
        })
        .collect()
}

/// Starts casting the card in seat 0's hand as the face at `face`.
fn cx8a_cast(driver: &mut Driver, face: usize) {
    let id = driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .first()
        .copied()
        .unwrap();
    driver
        .answer(Answer::Priority(PriorityAction::CastSpell(id)))
        .unwrap();
    cx8a_settle(driver, 0, face);
}

/// Activates the ability at `index` among the rules of the permanent.
fn cx8a_activate(driver: &mut Driver, object: ObjectId, index: usize, x: u32) {
    let ability = AbilityRef::printed(object, index);
    driver
        .answer(Answer::Priority(PriorityAction::Activate(ability)))
        .unwrap();
    cx8a_settle(driver, x, 0);
}

/// A 2/2 creature card of mana value 2 and a 3/3 of mana value 3.
fn cx8a_bear() -> Arc<CardDefinition> {
    creature("Hand Bear", 1, &[Color::Green], 2, 2)
}

fn cx8a_ogre() -> Arc<CardDefinition> {
    creature("Hand Ogre", 2, &[Color::Red], 3, 3)
}

/// Destroys the permanent with Beast Within from seat 0's hand.
fn cx8a_destroy(driver: &mut Driver, target: ObjectId) {
    let reference = driver.state().object(target).unwrap().reference();
    driver.begin_cast("Beast Within").unwrap();
    driver.choose_targets(&[Target::Object(reference)]).unwrap();
    cx8a_settle(driver, 0, 0);
}

fn cx8a_valki_scenario(seats: usize) -> Scenario {
    let mut board: Vec<_> = (0..4).map(|_| ready(swamp())).collect();
    board.extend((0..6).map(|_| ready(forest())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .hand(0, vec![card(CARD).unwrap(), card("Beast Within").unwrap()]);
    for seat in 1..seats {
        scenario = scenario.hand(seat, vec![island(), cx8a_bear(), island()]);
    }
    scenario
}

#[rstest]
fn cx8a_valki_exiles_a_creature_card_from_each_opponent_until_it_leaves(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = cx8a_valki_scenario(seats);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    cx8a_cast(&mut driver, 0);
    let valki = driver.id_of(VALKI).unwrap();
    assert_eq!(
        cx8a_named(&driver, Zone::Exile, "Hand Bear").len(),
        seats - 1
    );
    for seat in 1..seats {
        let hand = cx8a_named(&driver, Zone::Hand(pid(seat)), "Island").len();
        assert_eq!(hand, 2, "only the creature card leaves seat {seat}'s hand");
    }
    cx8a_destroy(&mut driver, valki);
    assert_eq!(cx8a_named(&driver, Zone::Exile, "Hand Bear").len(), 0);
    for seat in 1..seats {
        let back = cx8a_named(&driver, Zone::Hand(pid(seat)), "Hand Bear").len();
        assert_eq!(back, 1, "seat {seat}'s card returns as Valki leaves");
    }
    cx8a_replays(&scenario, &driver);
}

#[rstest]
fn cx8a_valki_leaving_before_its_trigger_exiles_nothing(#[values(2, 3, 4)] seats: usize) {
    let scenario = cx8a_valki_scenario(seats);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let id = driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .first()
        .copied()
        .unwrap();
    driver
        .answer(Answer::Priority(PriorityAction::CastSpell(id)))
        .unwrap();
    // Cast and resolve Valki only: its enters trigger waits on the stack.
    for _ in 0..50 {
        if driver.id_of(VALKI).is_ok() && !driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseOption(_) => Answer::ChooseOption(OptionChoice(0)),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    let valki = driver.id_of(VALKI).unwrap();
    assert_eq!(driver.state().stack().len(), 1, "the enters trigger waits");
    cx8a_destroy(&mut driver, valki);
    assert_eq!(cx8a_named(&driver, Zone::Exile, "Hand Bear").len(), 0);
    for seat in 1..seats {
        assert_eq!(
            cx8a_named(&driver, Zone::Hand(pid(seat)), "Hand Bear").len(),
            1
        );
    }
    cx8a_replays(&scenario, &driver);
}

#[rstest]
#[case::the_bears_mana_value(2, ("Hand Bear", 2))]
#[case::the_ogres_mana_value(3, ("Hand Ogre", 3))]
#[case::no_card_has_it(1, (VALKI, 2))]
fn cx8a_valki_becomes_a_copy_of_an_exiled_card_with_mana_value_x(
    #[values(3, 4)] seats: usize,
    #[case] x: u32,
    #[case] seen_as: (&str, i32),
) {
    // The last opponent holds the Ogre instead of the Bear.
    let scenario = cx8a_valki_scenario(seats).hand(seats - 1, vec![cx8a_ogre(), island()]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    cx8a_cast(&mut driver, 0);
    let valki = driver.id_of(VALKI).unwrap();
    cx8a_activate(&mut driver, valki, 1, x);
    let seen = mtg_engine::characteristics::characteristics(driver.state(), valki).unwrap();
    let name = driver.state().object(valki).unwrap().card().name.clone();
    assert_eq!((name.as_str(), seen.power), (seen_as.0, Some(seen_as.1)));
    // A copy keeps the exiled cards: they return as it leaves.
    cx8a_destroy(&mut driver, valki);
    assert_eq!(
        cx8a_named(&driver, Zone::Hand(pid(seats - 1)), "Hand Ogre").len(),
        1
    );
    assert_eq!(cx8a_named(&driver, Zone::Exile, "Hand Bear").len(), 0);
    cx8a_replays(&scenario, &driver);
}

/// A blue 1/1 creature card seat 0 has no mana of its color for.
fn cx8a_sprite() -> Arc<CardDefinition> {
    creature("Sea Sprite", 1, &[Color::Blue], 1, 1)
}

#[rstest]
#[case::while_tibalt_stays(false)]
#[case::after_tibalt_left(true)]
fn cx8a_tibalts_emblem_plays_the_cards_it_exiled(
    #[values(2, 3, 4)] seats: usize,
    #[case] leaves: bool,
) {
    let mut board: Vec<_> = (0..4).map(|_| ready(swamp())).collect();
    board.extend((0..4).map(|_| ready(mountain())));
    board.extend((0..4).map(|_| ready(forest())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .hand(0, vec![card(CARD).unwrap(), card("Beast Within").unwrap()])
        .library(0, vec![island(), island()]);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![cx8a_sprite(), island()]);
    }
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    cx8a_cast(&mut driver, 1);
    let tibalt = driver.id_of(TIBALT).unwrap();
    let emblems = driver.state().zone(Zone::Command(pid(0))).len();
    assert_eq!(
        emblems, 1,
        "Tibalt's controller gets the emblem as it enters"
    );
    cx8a_activate(&mut driver, tibalt, 1, 0);
    assert_eq!(
        cx8a_named(&driver, Zone::Exile, "Sea Sprite").len(),
        seats - 1
    );
    assert_eq!(cx8a_named(&driver, Zone::Exile, "Island").len(), 1);
    if leaves {
        cx8a_destroy(&mut driver, tibalt);
        assert_eq!(cx8a_named(&driver, Zone::Battlefield, TIBALT).len(), 0);
    }
    // The land exiled from seat 0's library is played with the land play; a blue Sprite is cast with other colors.
    let land = cx8a_named(&driver, Zone::Exile, "Island")[0];
    driver
        .answer(Answer::Priority(PriorityAction::PlayLand(land)))
        .unwrap();
    assert_eq!(cx8a_named(&driver, Zone::Battlefield, "Island").len(), 1);
    let sprite = cx8a_named(&driver, Zone::Exile, "Sea Sprite")[0];
    driver
        .answer(Answer::Priority(PriorityAction::CastSpell(sprite)))
        .unwrap();
    cx8a_settle(&mut driver, 0, 0);
    assert_eq!(
        cx8a_named(&driver, Zone::Battlefield, "Sea Sprite").len(),
        1
    );
    cx8a_replays(&scenario, &driver);
}
