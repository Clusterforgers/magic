//! Goal: Contraptions play at tables of two and four seats: the deck lies face down in its owner's command zone; assembling puts its top cards onto the battlefield face up, each on the sprocket its controller chooses, as many as asked and no more than the deck holds, and nothing for an empty deck; Steamflogger Boss doubles the Contraptions a Rigger its controller controls assembles and gives the other Riggers +1/+0 and haste; the sprocket marker moves 1, 2, 3, 1 as its player's precombat main phase begins only while they control a Contraption, and the Contraptions they choose on that sprocket are cranked, each crank ability triggering once; a destroyed Contraption goes to the scrapheap in the command zone face up; as each end step of its controller's turn begins, and on no other seat's turn, they may reassemble one Contraption onto another sprocket, which the next crank follows; and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{mountain, p170b_contraption, p170b_rigger};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 brings `deck` Contraptions and a ready Rigger that assembles `count`, with Steamflogger Boss when `boss`.
fn table(seats: usize, deck: usize, count: i32, boss: bool) -> Scenario {
    let mut battlefield = vec![ready(p170b_rigger("Tinker", count))];
    if boss {
        battlefield.push(ready(
            mtg_testkit::corpus::card("Steamflogger Boss").unwrap(),
        ));
    }
    battlefield.extend([ready(mountain()), ready(mountain())]);
    Scenario::new(seats)
        .at(1, 0, Step::Upkeep)
        .contraptions(
            0,
            (0..deck).map(|n| p170b_contraption(&format!("Gizmo {n}"))),
        )
        .hand(
            0,
            [mtg_testkit::corpus::card("Smash to Smithereens").unwrap()],
        )
        .battlefield(0, battlefield)
}

/// Answers until the stack and the pending triggers are empty, assigning each assembled Contraption to the next
/// sprocket of `sprockets` (cycling) and cranking every Contraption a crank offers.
fn settle(driver: &mut Driver, sprockets: &[u8]) -> Result<(), ScriptError> {
    let mut next = sprockets.iter().cycle();
    loop {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) if options.text.ends_with("to a sprocket.") => {
                let sprocket = *next.next().unwrap_or(&1);
                driver.answer(Answer::ChooseOption(OptionChoice(
                    usize::from(sprocket) - 1,
                )))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
}

/// The Contraptions of seat 0 on the battlefield with their sprockets.
fn assembled(driver: &Driver) -> Vec<(ObjectId, Option<u8>)> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            driver
                .state()
                .name_of(*id)
                .is_some_and(|name| name.starts_with("Gizmo"))
        })
        .map(|id| (id, driver.state().sprocket(id)))
        .collect()
}

#[rstest]
#[case::one(3, 1, false, 1)]
#[case::two(3, 2, false, 2)]
#[case::more_than_the_deck(1, 2, false, 1)]
#[case::empty_deck(0, 1, false, 0)]
#[case::boss_doubles(3, 1, true, 2)]
#[case::boss_runs_the_deck_out(3, 2, true, 3)]
#[case::boss_with_an_empty_deck(0, 1, true, 0)]
fn assembling_puts_the_top_cards_on_the_chosen_sprockets(
    #[values(2, 4)] seats: usize,
    #[case] deck: usize,
    #[case] count: i32,
    #[case] boss: bool,
    #[case] expected: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, deck, count, boss);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(driver.state().contraption_deck(pid(0)).len(), deck);
    for id in driver.state().contraption_deck(pid(0)) {
        assert_eq!(
            driver.state().object(*id).unwrap().zone(),
            Zone::Command(pid(0))
        );
        assert!(driver.state().object(*id).unwrap().face_down().is_some());
    }
    let top: Vec<ObjectId> = driver
        .state()
        .contraption_deck(pid(0))
        .iter()
        .rev()
        .take(expected)
        .copied()
        .collect();
    driver.begin_activation("Tinker")?;
    settle(&mut driver, &[3, 1, 2])?;
    let now = assembled(&driver);
    assert_eq!(
        now.iter()
            .map(|(id, _)| *id)
            .collect::<std::collections::BTreeSet<_>>(),
        top.iter().copied().collect(),
        "the top cards"
    );
    for (at, id) in top.iter().enumerate() {
        assert_eq!(
            driver.state().sprocket(*id),
            Some([3, 1, 2][at % 3]),
            "{id:?}"
        );
        assert_eq!(driver.state().object(*id).unwrap().face_down(), None);
        assert!(!driver.state().in_contraption_deck(*id));
    }
    assert_eq!(
        driver.state().contraption_deck(pid(0)).len(),
        deck - expected
    );
    for seat in 1..seats {
        assert_eq!(
            driver.state().contraption_deck(pid(seat)),
            &[] as &[ObjectId]
        );
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn the_boss_pumps_and_hastes_the_other_riggers(
    #[values(false, true)] boss: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(2, 0, 1, boss))?;
    let tinker = driver.id_of("Tinker")?;
    let chars = mtg_engine::characteristics::characteristics(driver.state(), tinker).unwrap();
    assert_eq!(chars.power, Some(1 + i32::from(boss)));
    assert_eq!(chars.toughness, Some(1));
    let haste = mtg_engine::keywords::has_keyword(
        driver.state(),
        tinker,
        &mtg_cards::keyword::Keyword::Haste,
    );
    assert_eq!(haste, boss);
    Ok(())
}

/// Each of seat 0's precombat main phases in turn order, from its first: the sprocket the marker moved to and the
/// Contraptions the crank offered.
fn cranks(driver: &mut Driver, turns: usize, crank: bool) -> Result<Vec<(u8, usize)>, ScriptError> {
    let mut seen = Vec::new();
    while seen.len() < turns {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options) if options.text.contains("to crank") => {
                let marker = driver.state().sprocket_marker(pid(0)).unwrap();
                seen.push((marker, options.from.len()));
                let chosen = if crank {
                    options.from.clone()
                } else {
                    Vec::new()
                };
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Ok(seen)
}

#[rstest]
#[case::all_on_one(&[1, 1, 1], vec![(1, 3), (1, 3)])]
#[case::spread(&[1, 2, 3], vec![(1, 1), (2, 1), (3, 1), (1, 1)])]
#[case::skips_an_empty_sprocket(&[2, 2, 2], vec![(2, 3), (2, 3)])]
fn the_marker_moves_and_the_chosen_contraptions_crank(
    #[values(2, 4)] seats: usize,
    #[values(false, true)] crank: bool,
    #[case] sprockets: &[u8],
    #[case] expected: Vec<(u8, usize)>,
) -> Result<(), ScriptError> {
    let scenario = table(seats, 3, 3, false);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Tinker")?;
    settle(&mut driver, sprockets)?;
    let seen = cranks(&mut driver, expected.len(), crank)?;
    assert_eq!(seen, expected);
    settle(&mut driver, &[])?;
    let cranked: usize = driver
        .engine()
        .events()
        .iter()
        .map(|event| match event {
            Event::P170bCranked { contraptions, .. } => contraptions.len(),
            _ => 0,
        })
        .sum();
    let offered: usize = expected.iter().map(|(_, n)| n).sum();
    assert_eq!(cranked, if crank { offered } else { 0 });
    // Each Contraption cranked triggers its crank ability once, which draws a card.
    let triggered = driver
        .engine()
        .events()
        .iter()
        .filter(|event| {
            matches!(event, Event::AbilityTriggered(trigger) if driver
                .state()
                .name_of(trigger.source.id)
                .is_some_and(|name| name.starts_with("Gizmo")))
        })
        .count();
    assert_eq!(triggered, cranked);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[test]
fn no_contraption_no_marker() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, 3, 1, false))?;
    driver.pass_until(Step::Main1)?;
    assert_eq!(driver.state().sprocket_marker(pid(0)), None);
    Ok(())
}

#[test]
fn a_destroyed_contraption_goes_to_the_scrapheap() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, 3, 1, false))?;
    driver.begin_activation("Tinker")?;
    settle(&mut driver, &[1])?;
    let [(gizmo, _)] = assembled(&driver)[..] else {
        panic!("one assembled");
    };
    driver.begin_cast("Smash to Smithereens")?;
    let name = driver.state().name_of(gizmo).unwrap();
    let target = driver.target_named(&name)?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver, &[])?;
    let card = driver.state().object(gizmo).unwrap();
    assert_eq!(card.zone(), Zone::Command(pid(0)));
    assert_eq!(card.face_down(), None);
    assert!(!driver.state().in_contraption_deck(gizmo));
    assert_eq!(driver.state().contraption_deck(pid(0)).len(), 2);
    Ok(())
}

/// Answers passively up to seat 0's reassembly question, answers it with `choice` and returns the options it offered.
fn reassemble(driver: &mut Driver, choice: usize) -> Result<Vec<String>, ScriptError> {
    loop {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) if options.text.contains("reassemble") => {
                assert_eq!(decision.player, pid(0));
                assert_eq!(driver.state().turn().step(), Step::End);
                let labels = options
                    .options
                    .iter()
                    .map(|option| match option {
                        mtg_engine::choose::ChoiceOption::Label(label) => label.clone(),
                        other => panic!("{other:?}"),
                    })
                    .collect();
                driver.answer(Answer::ChooseOption(OptionChoice(choice)))?;
                return Ok(labels);
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
}

#[rstest]
#[case::keep(&[1, 2, 3], 0, &[1, 2, 3], (2, 1))]
#[case::the_first_to_two(&[1, 1, 1], 1, &[2, 1, 1], (2, 1))]
#[case::the_second_to_two(&[1, 1, 1], 3, &[1, 2, 1], (2, 1))]
#[case::onto_a_taken_sprocket(&[2, 1, 1], 3, &[2, 2, 1], (2, 2))]
#[case::the_first_to_three(&[2, 2, 2], 2, &[3, 2, 2], (2, 2))]
fn the_end_step_reassembles_one_contraption_the_next_crank_follows(
    #[values(2, 4)] seats: usize,
    #[case] sprockets: &[u8],
    #[case] choice: usize,
    #[case] after: &[u8],
    #[case] next_crank: (u8, usize),
) -> Result<(), ScriptError> {
    let scenario = table(seats, 3, 3, false);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Tinker")?;
    settle(&mut driver, sprockets)?;
    let labels = reassemble(&mut driver, choice)?;
    // Keeping, then each Contraption to each of the two other sprockets.
    assert_eq!(labels.len(), 1 + 3 * 2, "{labels:?}");
    assert_eq!(labels[0], "Move none");
    assert!(labels[1].starts_with("Sprocket "), "{labels:?}");
    let now: Vec<u8> = assembled(&driver)
        .iter()
        .map(|(_, sprocket)| sprocket.unwrap())
        .collect();
    assert_eq!(now, after);
    let turn = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(turn, Step::Upkeep)?;
    let asked: Vec<_> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::P170bReassembled { player, moved } => Some((*player, moved.is_some())),
            _ => None,
        })
        .collect();
    assert_eq!(asked, vec![(pid(0), choice > 0)], "once, on seat 0's turn only");
    let seen = cranks(&mut driver, 1, false)?;
    assert_eq!(seen, vec![next_crank]);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[test]
fn no_contraption_no_reassembly() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, 3, 1, false))?;
    driver.idle_until(2, Step::Upkeep)?;
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::P170bReassembled { .. }))
    );
    Ok(())
}
