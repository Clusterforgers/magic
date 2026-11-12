//! Goal: Mirri, Weatherlight Duelist and The Eternal Wanderer play through the engine at tables of two, three and four
//! seats: Mirri lets each opponent block with one creature in the combat she attacks and none after, and lets one
//! creature attack her controller while she is tapped (tapped as the game set it or by her own attack) and any number
//! while she is untapped; the Wanderer's +1 returns the exiled card at its owner's next end step, not at another
//! player's, and its -4 leaves each player one creature of its controller's choice; every answer keeps the invariants
//! and the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped, with_counters};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;
use crate::w8a_declaration_limits::w8a_until;

type W14aResult = Result<(), ScriptError>;

/// Whether `result` is the refusal of a declaration over the limit `allowed`.
fn w14a_too_many(result: &Result<Vec<Event>, ScriptError>, allowed: usize) -> bool {
    matches!(
        result,
        Err(ScriptError::Engine(EngineError::Illegal(IllegalAnswer::TooManyCreatures { allowed: a, .. })))
            if *a == allowed
    )
}

/// The zone of the permanent or card named `name`.
fn w14a_zone(driver: &Driver, name: &str) -> Zone {
    driver
        .state()
        .objects()
        .find(|object| object.card().name == name)
        .unwrap()
        .zone()
}

#[rstest]
fn w14a_mirri_limits_the_attacks_at_her_controller_while_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] is_tapped: bool,
) -> W14aResult {
    let mirri = card("Mirri, Weatherlight Duelist").unwrap();
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(free_creature("Ash", 2, 2)),
                ready(free_creature("Birch", 2, 2)),
            ],
        )
        .battlefield(
            1,
            [if is_tapped {
                tapped(mirri)
            } else {
                ready(mirri)
            }],
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    w8a_until(&mut driver, 0, "declare-attackers")?;
    let both = driver.attack(&[("Ash", 1), ("Birch", 1)]);
    if is_tapped {
        assert!(w14a_too_many(&both, 1), "{both:?}");
        driver.attack(&[("Ash", 1)])?;
    } else {
        both?;
    }
    driver.idle_until(2, Step::Main2)?;
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w14a_mirri_lets_each_opponent_block_with_one_creature(
    #[values(2, 3, 4)] seats: usize,
) -> W14aResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Mirri, Weatherlight Duelist").unwrap()),
                ready(free_creature("Raider", 2, 2)),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Guard", 2, 2)),
                ready(free_creature("Ox", 1, 1)),
            ],
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    w8a_until(&mut driver, 0, "declare-attackers")?;
    driver.attack(&[("Mirri, Weatherlight Duelist", 1), ("Raider", 1)])?;
    w8a_until(&mut driver, 1, "declare-blockers")?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        panic!("not declaring blockers");
    };
    assert_eq!(options.most, Some(1));
    assert!(w14a_too_many(
        &driver.block(&[("Bear", "Raider"), ("Guard", "Mirri, Weatherlight Duelist")]),
        1
    ));
    driver.block(&[("Bear", "Raider")])?;
    driver.idle_until(2, Step::Main2)?;
    // Mirri is tapped from her attack: on the next turn one creature of seat 1 can attack seat 0, and blocks are free.
    driver.idle_until(3, Step::Main1)?;
    w8a_until(&mut driver, 1, "declare-attackers")?;
    assert!(w14a_too_many(&driver.attack(&[("Guard", 0), ("Ox", 0)]), 1));
    driver.attack(&[("Guard", 0)])?;
    replays(&driver, &setup);
    Ok(())
}

/// A table where seat 0 has The Eternal Wanderer with `loyalty` counters, and each seat has the creatures named.
fn w14a_wanderer_table(seats: usize, loyalty: u32, creatures: &[&str]) -> Scenario {
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1);
    for seat in 0..seats {
        let wanderer = (seat == 0).then(|| {
            with_counters(
                ready(card("The Eternal Wanderer").unwrap()),
                CounterKind::Loyalty,
                loyalty,
            )
        });
        scenario = scenario.battlefield(
            seat,
            wanderer.into_iter().chain(
                creatures
                    .iter()
                    .map(|name| ready(free_creature(&format!("{name} {seat}"), 2, 2))),
            ),
        );
    }
    scenario
}

#[rstest]
fn w14a_the_wanderer_returns_the_card_at_its_owners_next_end_step(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] owner: usize,
) -> W14aResult {
    let scenario = w14a_wanderer_table(seats, 5, &["Bear"]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let bear = format!("Bear {owner}");
    driver.begin_ability("The Eternal Wanderer", 1)?;
    let target = driver.target_named(&bear)?;
    driver.choose_targets(&[target])?;
    driver.idle_until(2, Step::Main2)?;
    assert_eq!(w14a_zone(&driver, &bear), Zone::Exile);
    driver.idle_until(3, Step::Upkeep)?;
    if owner == 1 {
        assert_eq!(
            w14a_zone(&driver, &bear),
            Zone::Exile,
            "seat 0's end step is not its owner's"
        );
        driver.idle_until(4, Step::Upkeep)?;
    }
    let back = driver.state().object(driver.id_of(&bear)?).unwrap();
    assert_eq!(back.zone(), Zone::Battlefield);
    assert_eq!(back.controller(), pid(owner));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w14a_the_wanderer_leaves_each_player_one_creature(
    #[values(2, 3, 4)] seats: usize,
) -> W14aResult {
    let scenario = w14a_wanderer_table(seats, 5, &["Ash", "Birch", "Cedar"]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("The Eternal Wanderer", 3)?;
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseCards(options) = &decision.kind {
            // Keep each player's Birch.
            let birch = options
                .from
                .iter()
                .copied()
                .find(|id| {
                    driver
                        .state()
                        .object(*id)
                        .unwrap()
                        .card()
                        .name
                        .starts_with("Birch")
                })
                .unwrap();
            assert_eq!(decision.player, pid(0), "the Wanderer's controller chooses");
            driver.answer(Answer::ChooseCards(CardsChoice(vec![birch])))?;
        } else {
            driver.answer_passively()?;
        }
    }
    for seat in 0..seats {
        assert_eq!(
            w14a_zone(&driver, &format!("Birch {seat}")),
            Zone::Battlefield
        );
        for gone in ["Ash", "Cedar"] {
            assert_eq!(
                w14a_zone(&driver, &format!("{gone} {seat}")),
                Zone::Graveyard(pid(seat))
            );
        }
    }
    replays(&driver, &setup);
    Ok(())
}
