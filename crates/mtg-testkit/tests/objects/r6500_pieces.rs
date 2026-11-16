//! Goal: the r65-00 pieces play as printed at two to four seats: a creature a rule says stations using its toughness
//! puts charge counters equal to its toughness (Tapestry Warden, CR 702.184a) and others still use their power; a
//! counter cap keeps a permanent at its most (Rasputin Dreamweaver) and its upkeep trigger reads whether it started the
//! turn untapped; planeswalkers stay with 0 loyalty under Sanctum Lurker (CR 704.5i); a devourer counts the permanents
//! of a kind it devoured as they last were (Voracious Dragon, CR 702.82).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready, tapped, with_counters};
use rstest::rstest;

/// Passes and answers passively until the stack is empty and nothing waits to go on it.
fn r6500_settle(driver: &mut Driver) {
    for _ in 0..80 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return;
            }
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

/// The counters of `kind` on the permanent named `name`.
fn r6500_counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(kind))
}

fn r6500_replays(driver: &Driver, scenario: &Scenario) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
#[case::wall_with_the_warden(1, 3, true, 3)]
#[case::wall_alone(1, 3, false, 1)]
#[case::brute_with_the_warden(3, 1, true, 3)]
#[case::even_with_the_warden(2, 2, true, 2)]
fn r6500_station_reads_toughness_under_tapestry_warden(
    #[case] power: i32,
    #[case] toughness: i32,
    #[case] warden: bool,
    #[case] charge: u32,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut mine = vec![
        ready(card("Debris Field Crusher").unwrap()),
        ready(free_creature("Crewman", power, toughness)),
    ];
    if warden {
        mine.push(ready(card("Tapestry Warden").unwrap()));
    }
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let crewman = driver.id_of("Crewman").unwrap();
    let station = driver.offered_abilities("Debris Field Crusher").unwrap()[0];
    driver
        .begin_ability("Debris Field Crusher", station)
        .unwrap();
    if matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseCards(_)
    ) {
        driver
            .answer(Answer::ChooseCards(CardsChoice(vec![crewman])))
            .unwrap();
    }
    r6500_settle(&mut driver);
    assert_eq!(
        r6500_counters(&driver, "Debris Field Crusher", &CounterKind::Charge),
        charge
    );
    r6500_replays(&driver, &scenario);
}

#[rstest]
#[case::grows_to_the_cap(6, false, 7)]
#[case::stays_at_the_cap(7, false, 7)]
#[case::grows_from_none(0, false, 1)]
#[case::started_tapped(3, true, 3)]
fn r6500_rasputin_dream_counters(
    #[case] dreams: u32,
    #[case] start_tapped: bool,
    #[case] after: u32,
    #[values(2, 3, 4)] seats: usize,
) {
    let rasputin = card("Rasputin Dreamweaver").unwrap();
    let placed = if start_tapped {
        tapped(rasputin)
    } else {
        ready(rasputin)
    };
    let dream = CounterKind::Other("dream".to_owned());
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main2)
        .battlefield(0, [with_counters(placed, dream.clone(), dreams)]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Upkeep).unwrap();
    r6500_settle(&mut driver);
    driver.idle_until(next, Step::Main1).unwrap();
    assert_eq!(
        r6500_counters(&driver, "Rasputin Dreamweaver", &dream),
        after
    );
    r6500_replays(&driver, &scenario);
}

#[rstest]
fn r6500_planeswalkers_stay_with_no_loyalty(
    #[values(true, false)] lurker: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut mine = vec![with_counters(
        ready(card("Jace Beleren").unwrap()),
        CounterKind::Loyalty,
        1,
    )];
    if lurker {
        mine.push(ready(card("Sanctum Lurker").unwrap()));
    }
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    // Jace's -1 takes his only loyalty counter.
    let jace = driver.id_of("Jace Beleren").unwrap();
    let minus = driver.offered_abilities("Jace Beleren").unwrap()[1];
    driver.begin_ability("Jace Beleren", minus).unwrap();
    if matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseTargets(_)
    ) {
        let me = driver.target_player(0);
        driver.choose_targets(&[me]).unwrap();
    }
    r6500_settle(&mut driver);
    let on_field = driver.state().zone(Zone::Battlefield).contains(&jace);
    assert_eq!(on_field, lurker);
    r6500_replays(&driver, &scenario);
}

#[rstest]
#[case::two_goblins_and_a_bear(2, 1, 4)]
#[case::no_goblin(0, 2, 0)]
#[case::one_goblin(1, 0, 2)]
fn r6500_voracious_dragon_counts_the_goblins_it_devoured(
    #[case] goblins: usize,
    #[case] others: usize,
    #[case] damage: i32,
    #[values(2, 3, 4)] seats: usize,
) {
    use std::sync::Arc;
    let mut mine: Vec<_> = (0..6)
        .map(|_| ready(mtg_testkit::cards::mountain()))
        .collect();
    for at in 0..goblins {
        let goblin = (*free_creature(&format!("Goblin {at}"), 1, 1))
            .clone()
            .with_subtype("Goblin");
        mine.push(ready(Arc::new(goblin)));
    }
    for at in 0..others {
        mine.push(ready(free_creature(&format!("Bear {at}"), 2, 2)));
    }
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Voracious Dragon").unwrap()])
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let eaten: Vec<_> = (0..goblins)
        .map(|at| driver.id_of(&format!("Goblin {at}")).unwrap())
        .chain((0..others).map(|at| driver.id_of(&format!("Bear {at}")).unwrap()))
        .collect();
    let before = driver.state().player(crate::common::pid(1)).unwrap().life();
    driver.cast("Voracious Dragon").unwrap();
    for _ in 0..80 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::ChooseCards(_) => {
                driver
                    .answer(Answer::ChooseCards(CardsChoice(eaten.clone())))
                    .unwrap();
            }
            DecisionKind::ChooseTargets(_) => {
                let them = driver.target_player(1);
                driver.choose_targets(&[them]).unwrap();
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                break;
            }
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
    let dragon = driver.id_of("Voracious Dragon").unwrap();
    let counters = driver
        .state()
        .object(dragon)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        });
    assert_eq!(counters, u32::try_from(goblins + others).unwrap());
    let after = driver.state().player(crate::common::pid(1)).unwrap().life();
    assert_eq!(before - after, damage);
    r6500_replays(&driver, &scenario);
}
