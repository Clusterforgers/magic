//! Goal: Contentious Plan, Scheming Aspirant and the other proliferate cards play whole at tables of two, three and four seats, as their cards say: the player chooses none, some or all of the permanents that hold counters and says yes or no for each player who holds some, every one chosen gets one more counter of each kind it holds (a replacement effect that adds counters adds to it), a proliferate with nothing to choose still happens and triggers "whenever you proliferate", and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

const LIBRARY: [&str; 4] = ["Grizzly Bears", "Serra Angel", "Counterspell", "Sol Ring"];

fn named(list: &[&str]) -> Vec<Arc<CardDefinition>> {
    list.iter().map(|name| card(name).unwrap()).collect()
}

/// Seat 0 holds `spell` and controls three Islands and a Wolf with two +1/+1 and one charge counter, `extra` besides;
/// seat 1 controls a Boar with one -1/-1 counter and holds two poison counters.
fn table(seats: usize, spell: &str, extra: &[&str]) -> Scenario {
    let mut mine = vec![
        ready(island()),
        ready(island()),
        ready(island()),
        with_counters(
            with_counters(
                ready(free_creature("Wolf", 2, 2)),
                CounterKind::PlusOnePlusOne,
                2,
            ),
            CounterKind::Charge,
            1,
        ),
    ];
    mine.extend(extra.iter().map(|name| ready(card(name).unwrap())));
    let boar = with_counters(
        ready(free_creature("Boar", 3, 3)),
        CounterKind::MinusOneMinusOne,
        1,
    );
    let mut scenario = Scenario::new(seats)
        .hand(0, [card(spell).unwrap()])
        .library(0, named(&LIBRARY))
        .battlefield(0, mine)
        .battlefield(1, [boar])
        .counters(1, CounterKind::Poison, 2);
    for seat in 1..seats {
        scenario = scenario.library(seat, named(&LIBRARY));
    }
    scenario
}

fn passes_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

fn cards(ids: &[ObjectId]) -> Answer {
    Answer::ChooseCards(CardsChoice(ids.to_vec()))
}

fn held(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(kind)
}

fn poison(driver: &Driver, seat: usize) -> u32 {
    driver
        .state()
        .player(pid(seat))
        .unwrap()
        .counters_of(&CounterKind::Poison)
}

fn proliferated(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::Proliferated { .. }))
        .count()
}

#[rstest]
#[case::nothing_chosen(false, false, false)]
#[case::the_wolf(true, false, false)]
#[case::the_boar(false, true, false)]
#[case::only_the_player(false, false, true)]
#[case::everything(true, true, true)]
fn a_proliferate_gives_each_chosen_permanent_and_player_one_more_counter_of_each_kind_it_holds(
    #[case] wolf: bool,
    #[case] boar: bool,
    #[case] player: bool,
    #[values(2, 3, 4)] seats: usize,
    #[values("Contentious Plan", "Steady Progress")] spell: &str,
) -> Result<(), ScriptError> {
    let scenario = table(seats, spell, &[]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(spell)?;
    passes_until_asked(&mut driver)?;
    // The choice of permanents names the two that hold counters, in the order of the battlefield.
    let decision = driver.decision()?.clone();
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!("the proliferate asks which permanents");
    };
    assert_eq!(decision.player, pid(0));
    assert_eq!(
        options
            .from
            .iter()
            .map(|id| name_of(driver.state(), *id))
            .collect::<Vec<_>>(),
        ["Wolf", "Boar"]
    );
    assert_eq!((options.min, options.max), (0, 2));
    let mut chosen = Vec::new();
    if wolf {
        chosen.push(driver.id_of("Wolf")?);
    }
    if boar {
        chosen.push(driver.id_of("Boar")?);
    }
    driver.answer(cards(&chosen))?;
    let DecisionKind::YesNo(question) = &driver.decision()?.kind else {
        panic!("the poisoned player is asked about");
    };
    assert_eq!(
        question.text,
        "Proliferate seat 2: another counter of each kind they have?"
    );
    driver.answer(Answer::YesNo(YesNoChoice(player)))?;
    passes_until_asked(&mut driver)?;

    let plus = 2 + u32::from(wolf);
    assert_eq!(held(&driver, "Wolf", &CounterKind::PlusOnePlusOne), plus);
    assert_eq!(
        held(&driver, "Wolf", &CounterKind::Charge),
        1 + u32::from(wolf)
    );
    assert_eq!(
        held(&driver, "Boar", &CounterKind::MinusOneMinusOne),
        1 + u32::from(boar)
    );
    assert_eq!(poison(&driver, 1), 2 + u32::from(player));
    assert_eq!(proliferated(driver.engine().events()), 1);
    assert_eq!(
        driver
            .state()
            .zone(mtg_engine::zones::Zone::Hand(pid(0)))
            .len(),
        1
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn a_replacement_that_adds_counters_adds_to_the_counters_a_proliferate_gives(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Contentious Plan", &["Hardened Scales"]))?;
    driver.cast("Contentious Plan")?;
    passes_until_asked(&mut driver)?;
    let wolf = driver.id_of("Wolf")?;
    driver.answer(cards(&[wolf]))?;
    driver.answer(Answer::YesNo(YesNoChoice(false)))?;
    passes_until_asked(&mut driver)?;
    // Hardened Scales adds one +1/+1 counter to the one the proliferate puts; the charge counter is not changed by it.
    assert_eq!(held(&driver, "Wolf", &CounterKind::PlusOnePlusOne), 4);
    assert_eq!(held(&driver, "Wolf", &CounterKind::Charge), 2);
    Ok(())
}

#[rstest]
fn a_proliferate_with_nothing_to_choose_asks_nothing_and_still_triggers_what_waits_for_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Nothing holds a counter: the Wolf and the Boar are plain and nobody is poisoned.
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Contentious Plan").unwrap()])
        .library(0, named(&LIBRARY))
        .battlefield(
            0,
            [
                ready(island()),
                ready(island()),
                ready(card("Scheming Aspirant").unwrap()),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.library(seat, named(&LIBRARY));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let life: Vec<i32> = (0..seats)
        .map(|seat| driver.state().player(pid(seat)).unwrap().life())
        .collect();
    driver.cast("Contentious Plan")?;
    passes_until_asked(&mut driver)?;
    assert_eq!(proliferated(driver.engine().events()), 1);
    // Scheming Aspirant: each opponent loses 2 life and its controller gains 2.
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life[0] + 2);
    for (seat, before) in life.iter().enumerate().skip(1) {
        assert_eq!(driver.state().player(pid(seat)).unwrap().life(), before - 2);
    }
    Ok(())
}
