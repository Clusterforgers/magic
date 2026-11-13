//! Goal: Fortify attaches a Fortification to a land its controller controls as a sorcery, and the fortified land gets
//! what the Fortification gives (Darksteel Garrison: indestructible, and a pump when it becomes tapped); a crew that
//! may be activated only once each turn is offered once and not again that turn (Luxurious Locomotive), and its attack
//! makes a Treasure for each creature that crewed it; at two to four seats, the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_engine::choose::{CardsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Answers every question with its first option and passes priority until the stack is empty.
fn r5510_settle(driver: &mut Driver) {
    for _ in 0..60 {
        match driver.decision().unwrap().kind.clone() {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

fn r5510_replays(driver: &Driver, scenario: &Scenario) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn r5510_fortified_land_is_indestructible(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats).at(3, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Darksteel Garrison").unwrap()),
            ready(mountain()),
            ready(mountain()),
            ready(mountain()),
            ready(mountain()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    assert_eq!(
        driver
            .offered_abilities("Darksteel Garrison")
            .unwrap()
            .len(),
        1
    );
    let land = driver.state().zone(Zone::Battlefield)[1];
    let ability = driver.offered_abilities("Darksteel Garrison").unwrap()[0];
    driver.begin_ability("Darksteel Garrison", ability).unwrap();
    for _ in 0..10 {
        match driver.decision().unwrap().kind.clone() {
            DecisionKind::ChooseTargets(_) => {
                let target = Target::Object(driver.state().object(land).unwrap().reference());
                driver.choose_targets(&[target]).unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => break,
        }
    }
    r5510_settle(&mut driver);
    let fortified = mtg_engine::keywords::keywords_of(driver.state(), land);
    let garrison = driver.id_of("Darksteel Garrison").unwrap();
    let attached = driver
        .state()
        .object(garrison)
        .and_then(|object| object.permanent())
        .and_then(mtg_engine::object::PermanentState::attached_to)
        .map(|host| host.id);
    assert_eq!(attached, Some(land));
    assert!(fortified.contains(&&Keyword::Indestructible));
    r5510_replays(&driver, &scenario);
}

#[rstest]
#[case::one_crew(&[2], 1)]
#[case::two_crew(&[1, 1], 2)]
fn r5510_locomotive_crews_once_each_turn(
    #[case] powers: &[i32],
    #[case] treasures: usize,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut crew = vec![ready(card("Luxurious Locomotive").unwrap())];
    for (at, power) in powers.iter().enumerate() {
        crew.push(ready(free_creature(&format!("Hand {at}"), *power, 1)));
    }
    crew.push(ready(free_creature("Spare", 3, 3)));
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, crew);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let picked: Vec<_> = (0..powers.len())
        .map(|at| driver.id_of(&format!("Hand {at}")).unwrap())
        .collect();
    let crew_ability = driver.offered_abilities("Luxurious Locomotive").unwrap()[0];
    driver
        .begin_ability("Luxurious Locomotive", crew_ability)
        .unwrap();
    if matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseCards(_)
    ) {
        driver
            .answer(Answer::ChooseCards(CardsChoice(picked)))
            .unwrap();
    }
    r5510_settle(&mut driver);
    assert_eq!(
        driver
            .offered_abilities("Luxurious Locomotive")
            .unwrap()
            .len(),
        0
    );
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Luxurious Locomotive", 1)]).unwrap();
    r5510_settle(&mut driver);
    let made = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|o| o.card().name == "Treasure")
        })
        .count();
    assert_eq!(made, treasures);
    r5510_replays(&driver, &scenario);
}
