//! Goal: a surveil the next steps read "this way" plays as printed at tables of two, three and four seats: Chandra,
//! Chill of Compliance returns the card she surveiled into the graveyard to her controller's hand only when it is a
//! noncreature, nonland card, and Starving Revenant draws a card and loses 3 life for each card kept on top, none for a
//! card put into the graveyard; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, pid};
use mtg_cards::value::CounterKind;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, instant, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// Answers until the stack is empty: the surveil puts the cards named in `graved` into the graveyard; everything else
/// passively.
fn r407_settle(driver: &mut Driver, graved: &[&str], most: usize) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::ChooseCards(options) => {
                let chosen: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| graved.contains(&name_of(driver.state(), *id).as_str()))
                    .take(most)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn r407_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// How many cards with the name are in the zone.
fn r407_in(driver: &Driver, zone: Zone, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

#[rstest]
fn r407_chandra_returns_a_noncreature_nonland_card_she_surveiled_away(
    #[values(2, 3, 4)] seats: usize,
    #[values(("Opt Spell", true), ("Grizzly Bears", false), ("Swamp", false))] top: (&str, bool),
    #[values(true, false)] graved: bool,
) -> Result<(), ScriptError> {
    let (name, returns) = top;
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1);
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..3).map(|_| bear()));
    }
    let top_card = match name {
        "Opt Spell" => instant("Opt Spell", 0, &[]),
        "Grizzly Bears" => bear(),
        _ => swamp(),
    };
    let scenario = scenario.library(0, [top_card, bear(), bear()]).battlefield(
        0,
        [with_counters(
            ready(card("Chandra, Chill of Compliance").unwrap()),
            CounterKind::Loyalty,
            4,
        )],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Chandra, Chill of Compliance", 0)?;
    let picks: &[&str] = if graved { &[name] } else { &[] };
    r407_settle(&mut driver, picks, 1)?;
    let in_hand = r407_in(&driver, Zone::Hand(pid(0)), name);
    let in_graveyard = r407_in(&driver, Zone::Graveyard(pid(0)), name);
    assert_eq!(in_hand, usize::from(graved && returns));
    assert_eq!(in_graveyard, usize::from(graved && !returns));
    r407_replays(&driver);
    Ok(())
}

#[rstest]
fn r407_starving_revenant_draws_and_loses_for_each_card_kept_on_top(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] graved: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1);
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..3).map(|_| bear()));
    }
    let scenario = scenario
        .library(
            0,
            [
                instant("Top One", 0, &[]),
                instant("Top Two", 0, &[]),
                bear(),
                bear(),
            ],
        )
        .battlefield(0, (0..4).map(|_| ready(swamp())))
        .hand(0, [card("Starving Revenant").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Starving Revenant")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r407_settle(&mut driver, &["Top One", "Top Two"], graved)?;
    let kept = 2 - graved;
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, 20 - 3 * i32::try_from(kept).unwrap());
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), kept);
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(0))).len(), graved);
    r407_replays(&driver);
    Ok(())
}
