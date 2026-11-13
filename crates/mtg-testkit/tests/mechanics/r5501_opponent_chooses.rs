//! Goal: Callous Oppressor's "As this creature enters, an opponent chooses a creature type." plays at tables of two, three and four seats: its controller names the opponent who chooses when there are several (nobody is asked which when there is one), that opponent chooses the type, and "{T}: Gain control of target creature that isn't of the chosen type for as long as this creature remains tapped." offers only creatures of other types (itself included) and takes the one targeted; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::{Engine, replay};
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 holds Callous Oppressor and three Islands; every other seat controls Llanowar Elves and Grizzly Bears.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Callous Oppressor").unwrap()])
        .battlefield(0, [ready(island()), ready(island()), ready(island())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(card("Llanowar Elves").unwrap()),
                ready(card("Grizzly Bears").unwrap()),
            ],
        );
    }
    scenario
}

#[rstest]
fn the_chosen_opponent_names_the_type_the_oppressor_cant_take(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] pick: usize,
) {
    if pick + 1 >= seats {
        return;
    }
    let chooser = pid(pick + 1);
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    driver.cast("Callous Oppressor").unwrap();
    let mut asked_who = false;
    let mut asked_type = false;
    while !driver.state().stack().is_empty() {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) if decision.player == pid(0) => {
                assert_eq!(options.options.len(), seats - 1, "each opponent is offered");
                asked_who = true;
                driver
                    .answer(Answer::ChooseOption(OptionChoice(pick)))
                    .unwrap();
            }
            DecisionKind::ChooseOption(options) => {
                assert_eq!(decision.player, chooser, "the opponent named chooses");
                let elf = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::Label("Elf".to_owned()))
                    .expect("Elf is offered");
                asked_type = true;
                driver
                    .answer(Answer::ChooseOption(OptionChoice(elf)))
                    .unwrap();
            }
            _ => {
                driver.pass().unwrap();
            }
        }
    }
    assert_eq!(
        asked_who,
        seats > 2,
        "the controller names an opponent only among several"
    );
    assert!(asked_type);
    // Its next turn: the Oppressor may tap, and only the Bears are targets.
    let next = 1 + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::Main1).unwrap();
    driver.begin_activation("Callous Oppressor").unwrap();
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("the ability asks its target");
    };
    let elves: Vec<_> = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| crate::common::name_of(driver.state(), *id) == "Llanowar Elves")
        .collect();
    assert!(!elves.is_empty());
    assert!(
        options
            .legal
            .iter()
            .all(|target| !matches!(target, mtg_engine::choose::Target::Object(o) if elves.contains(&o.id))),
        "no Elf is a target"
    );
    // Each Bear and the Oppressor itself (an Octopus) are of other types.
    assert_eq!(options.legal.len(), seats, "each Bear and the Oppressor");
    let bear = *options
        .legal
        .iter()
        .find(|target| {
            matches!(target, mtg_engine::choose::Target::Object(o)
                if crate::common::name_of(driver.state(), o.id) == "Grizzly Bears")
        })
        .expect("a Bear is a target");
    driver.choose_targets(&[bear]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.answer_passively().unwrap();
    }
    let mtg_engine::choose::Target::Object(bear) = bear else {
        panic!("a creature is the target");
    };
    assert_eq!(
        mtg_engine::characteristics::controller_of(driver.state(), bear.id),
        Some(pid(0))
    );
    let setup = table(seats).setup().unwrap();
    let log = driver.engine().events().to_vec();
    assert_eq!(
        replay(&setup, &log).unwrap().state_hash(),
        driver.state().state_hash()
    );
    let resumed = Engine::resume(setup, log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
}
