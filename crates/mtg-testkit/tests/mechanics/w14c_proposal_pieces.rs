//! Goal: the corpus cards of the proposal pieces play through the engine at two to four seats: Two-Headed Giant of
//! Foriys and the creatures of Brave the Sands block two attackers (named once for each, never twice for one, never for
//! a third) and divide their damage with lethal damage first; bots under Brave the Sands block two attackers with one
//! creature and the log replays; Crash is cast by sacrificing a Mountain and Snuff Out by paying 4 life beside their mana
//! cost, each only while what it asks is there; every log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::{free_creature, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, RandomBot, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, pay, ways};
use crate::common::copying::replays;
use crate::common::{on_battlefield, pid};
use crate::w8a_declaration_limits::w8a_until;

type W14cResult = Result<(), ScriptError>;

/// Whether `result` is the refusal of a blocker named once too often.
fn w14c_named_twice(result: &Result<Vec<Event>, ScriptError>) -> bool {
    matches!(
        result,
        Err(ScriptError::Engine(EngineError::Illegal(
            IllegalAnswer::Duplicate("blocker")
        )))
    )
}

/// Seat 0 attacks the last seat with a 2/2 Wolf, a 1/1 Fox and a 1/1 Cub; the last seat holds `blockers`.
fn w14c_combat(seats: usize, blockers: Vec<mtg_engine::PresetPermanent>) -> Scenario {
    Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Wolf", 2, 2)),
                ready(free_creature("Fox", 1, 1)),
                ready(free_creature("Cub", 1, 1)),
            ],
        )
        .battlefield(seats - 1, blockers)
}

#[rstest]
fn w14c_one_creature_blocks_two_attackers_and_divides_its_damage(
    #[values(2, 3, 4)] seats: usize,
    #[values("Two-Headed Giant of Foriys", "Brave the Sands")] source: &str,
) -> W14cResult {
    let blockers = if source == "Brave the Sands" {
        vec![
            ready(card(source).unwrap()),
            ready(free_creature("Giant", 3, 4)),
        ]
    } else {
        vec![ready(card(source).unwrap())]
    };
    let giant = if source == "Brave the Sands" {
        "Giant"
    } else {
        source
    };
    let scenario = w14c_combat(seats, blockers);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let defender = seats - 1;
    w8a_until(&mut driver, 0, "declare-attackers")?;
    driver.attack(&[("Wolf", defender), ("Fox", defender), ("Cub", defender)])?;
    w8a_until(&mut driver, defender, "declare-blockers")?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        panic!("not declaring blockers");
    };
    assert_eq!(options.blockers[0].extra, 1);
    let log = driver.engine().events().len();
    assert!(w14c_named_twice(
        &driver.block(&[(giant, "Wolf"), (giant, "Wolf")])
    ));
    // A third naming is not offered to the script at all.
    assert!(
        driver
            .block(&[(giant, "Wolf"), (giant, "Fox"), (giant, "Cub")])
            .is_err()
    );
    assert_eq!(
        driver.engine().events().len(),
        log,
        "a refusal left a trace"
    );
    driver.block(&[(giant, "Wolf"), (giant, "Fox")])?;
    driver.idle_until(2, Step::Main2)?;
    let state = driver.state();
    assert!(
        on_battlefield(state, "Wolf").is_none(),
        "lethal damage first"
    );
    assert!(
        on_battlefield(state, "Fox").is_none(),
        "the rest on the last"
    );
    assert!(on_battlefield(state, giant).is_some());
    assert_eq!(state.player(pid(defender)).unwrap().life(), 19);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w14c_bots_under_brave_the_sands_block_two_attackers_with_one_creature(
    #[values(2, 3, 4)] seats: usize,
) -> W14cResult {
    let mut doubled = 0;
    for seed in 1..=6 {
        let mut scenario = Scenario::new(seats).seed(seed).at(2, 0, Step::Main1);
        for seat in 0..seats {
            scenario = scenario.battlefield(
                seat,
                [
                    ready(card("Brave the Sands").unwrap()),
                    ready(free_creature("Bear", 2, 2)),
                    ready(free_creature("Ox", 1, 4)),
                    ready(free_creature("Elk", 2, 3)),
                ],
            );
        }
        let setup = scenario.clone().setup().unwrap();
        let mut driver = Driver::from_scenario(scenario)?;
        let mut bots: Vec<RandomBot> = (0..seats as u64)
            .map(|seat| RandomBot::new(seed, seat))
            .collect();
        for _ in 0..1500 {
            if driver.state().turn().number() > 8 {
                break;
            }
            let Ok(decision) = driver.decision().cloned() else {
                break;
            };
            let answer = bots[decision.player.index()].choose(&decision);
            for event in driver.answer(answer)? {
                if let Event::BlockersDeclared { blocks, .. } = event {
                    let distinct = blocks
                        .iter()
                        .map(|(blocker, _)| *blocker)
                        .collect::<std::collections::BTreeSet<_>>();
                    doubled += usize::from(distinct.len() < blocks.len());
                }
            }
        }
        replays(&driver, &setup);
    }
    assert!(
        doubled > 0,
        "no bot ever blocked two attackers with one creature"
    );
    Ok(())
}

/// Casts `name` in seat 0's main phase for the way whose label starts with `way`, at the permanent called `target`, and
/// lets it resolve; the labels of the ways asked are returned.
fn w14c_cast(
    driver: &mut Driver,
    name: &str,
    target: &str,
    way: &str,
) -> Result<Vec<String>, ScriptError> {
    let mut asked = Vec::new();
    driver.begin_cast(name)?;
    for _ in 0..8 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(_) if asked.is_empty() => {
                asked = ways(driver)?;
                choose_way(driver, way)?;
            }
            DecisionKind::ChooseTargets(_) => {
                let at = on_battlefield(driver.state(), target).unwrap();
                let reference = driver.state().object(at).unwrap().reference();
                driver.choose_targets(&[Target::Object(reference)])?;
            }
            kind if kind.name() == "pay-mana" => {
                pay(driver)?;
            }
            _ if !driver.state().stack().is_empty() => break,
            _ => {
                let decision = driver.decision()?.clone();
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    for _ in 0..8 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(asked)
}

#[rstest]
#[case::crash_for_a_mountain("Crash", "Sol Ring", "Cast for Sacrifice", 20, false)]
#[case::snuff_out_for_life("Snuff Out", "Bear", "Cast for Pay 4 life", 16, true)]
fn w14c_a_spell_is_cast_for_its_alternative_cost(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] target: &str,
    #[case] way: &str,
    #[case] life: i32,
    #[case] mountain_stays: bool,
) -> W14cResult {
    let theirs = if target == "Bear" {
        ready(free_creature("Bear", 2, 2))
    } else {
        ready(card(target).unwrap())
    };
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand_named(0, [spell])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(swamp()),
            ],
        )
        .battlefield(1, [theirs]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let asked = w14c_cast(&mut driver, spell, target, way)?;
    assert_eq!(
        asked.len(),
        2,
        "the mana cost and the alternative: {asked:?}"
    );
    let state = driver.state();
    assert!(
        on_battlefield(state, target).is_none(),
        "{target} is destroyed"
    );
    assert_eq!(state.player(pid(0)).unwrap().life(), life);
    let mountains = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).unwrap().card().name == "Mountain")
        .count();
    assert_eq!(mountains, if mountain_stays { 3 } else { 2 });
    assert_eq!(
        state.zone(Zone::Graveyard(pid(0))).len(),
        if mountain_stays { 1 } else { 2 }
    );
    replays(&driver, &setup);
    Ok(())
}
