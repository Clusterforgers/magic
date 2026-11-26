//! Goal: the p164 cards play whole, at two to four seats, and are `Verified` with the engine's own capabilities: Chorus
//! of the Conclave lets its controller pay any amount of mana as they cast a creature spell, the creature entering with
//! that many more +1/+1 counters, and offers nothing for other spells or other players; each game replays to its state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, instant, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn replays(driver: &Driver) {
    let state = mtg_engine::replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 10])
    })
}

/// Answers every question until the stack is empty and `seat` has priority again: mana is paid automatically, `pick`
/// answers what it knows and the rest is answered passively; the texts of the numbers asked come back.
fn settle(
    driver: &mut Driver,
    seat: usize,
    pick: &dyn Fn(&Decision) -> Option<Answer>,
) -> Vec<String> {
    let mut asked = Vec::new();
    for _ in 0..300 {
        let decision = driver.decision().unwrap().clone();
        if let DecisionKind::ChooseNumber(options) = &decision.kind {
            asked.push(options.text.clone());
        }
        if let Some(answer) = pick(&decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().resolving().is_none()
                    && decision.player == pid(seat) =>
            {
                return asked;
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game did not settle");
}

fn number(n: u32) -> impl Fn(&Decision) -> Option<Answer> {
    move |decision| match &decision.kind {
        DecisionKind::ChooseNumber(_) => Some(Answer::ChooseNumber(NumberChoice::Number(n))),
        _ => None,
    }
}

fn plus_counters(driver: &Driver, seat: usize, name: &str) -> u32 {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| object.card().name == name && object.controller() == pid(seat))
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&CounterKind::PlusOnePlusOne))
        .unwrap()
}

#[rstest]
#[case("Chorus of the Conclave")]
fn p164_the_cards_are_verified(#[case] name: &str) {
    let support = mtg_testkit::corpus::corpus()
        .unwrap()
        .support(name)
        .unwrap();
    assert_eq!(
        support.support,
        mtg_cards::support::Support::Verified,
        "{name}: {support:?}"
    );
}

#[rstest]
#[case::nothing_paid(0)]
#[case::one_paid(1)]
#[case::all_paid(6)]
fn chorus_of_the_conclave_pays_for_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
) {
    let mut board = vec![ready(card("Chorus of the Conclave").unwrap())];
    board.extend((0..8).map(|_| ready(forest())));
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .hand(0, [bear(), instant("Shock", 0, &[])]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver.begin_cast("Grizzly Bears").unwrap();
    let asked = settle(&mut driver, 0, &number(counters));
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(plus_counters(&driver, 0, "Grizzly Bears"), counters);
    let untapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| {
            object.card().name == "Forest"
                && object
                    .permanent()
                    .is_some_and(|permanent| !permanent.tapped())
        })
        .count();
    assert_eq!(
        untapped,
        8 - 2 - counters as usize,
        "each counter cost {{1}}"
    );
    // An instant is not a creature spell: nothing more is offered.
    driver.begin_cast("Shock").unwrap();
    let asked = settle(&mut driver, 0, &number(1));
    assert!(asked.is_empty(), "{asked:?}");
    replays(&driver);
}

#[rstest]
fn chorus_of_the_conclave_offers_nothing_to_an_opponent(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(card("Chorus of the Conclave").unwrap())])
        .battlefield(1, (0..4).map(|_| ready(forest())).collect::<Vec<_>>())
        .hand(1, [bear()]);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver.begin_cast("Grizzly Bears").unwrap();
    let asked = settle(&mut driver, 1, &number(2));
    assert!(asked.is_empty(), "{asked:?}");
    assert_eq!(plus_counters(&driver, 1, "Grizzly Bears"), 0);
    replays(&driver);
}
