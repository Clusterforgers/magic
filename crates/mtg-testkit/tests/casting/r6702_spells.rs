//! Goal: the r67-02 spells play as they read at two to four seats: Berserker's Frenzy is cast only before blockers are
//! declared and rolls two d20 keeping the higher, Game of Chaos flips until the player who decides says no, the winner of
//! each flip deciding and the stakes doubling with each flip, and Spelltwine casts the copies of both exiled cards
//! without a question, in the order its caster picks, then exiles itself; Trap the Trespassers stuns each creature by
//! the secret votes it got.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

/// Seat 0 with the named card in hand and lands enough on turn 1, every seat with a library and a creature.
fn r6702_table(seats: usize, spell: &str, lands: usize) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .hand(0, [card(spell).unwrap()])
            .at(1, 0, Step::Main1),
        |scenario, seat| {
            let beast = ready(free_creature(&format!("Beast {seat}"), 2, 2));
            let own = if seat == 0 { lands } else { 0 };
            scenario.library(seat, vec![mountain(); 10]).battlefield(
                seat,
                std::iter::once(beast)
                    .chain((0..own).map(|_| ready(mountain())))
                    .chain((0..own).map(|_| ready(island())))
                    .collect::<Vec<_>>(),
            )
        },
    )
}

/// Whether seat 0 may cast the named card now.
fn r6702_castable(driver: &Driver, name: &str) -> Result<bool, ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        return Ok(false);
    };
    Ok(options.castable.iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == name)
    }))
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r6702_berserkers_frenzy_is_cast_only_before_blockers(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(r6702_table(seats, "Berserker's Frenzy", 3))?;
    assert!(r6702_castable(&driver, "Berserker's Frenzy")?);
    let after =
        Driver::from_scenario(r6702_table(seats, "Berserker's Frenzy", 3).at(1, 0, Step::Main2))?;
    assert!(!r6702_castable(&after, "Berserker's Frenzy")?);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r6702_berserkers_frenzy_keeps_the_higher_of_two_d20(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r6702_table(seats, "Berserker's Frenzy", 3))?;
    driver.cast("Berserker's Frenzy")?;
    let mut rolled = Vec::new();
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        // "Choose any number of creatures": the Beast of seat 1.
        if let DecisionKind::ChooseCards(options) = &decision.kind {
            let beast = driver.id_of("Beast 1")?;
            assert!(options.from.contains(&beast));
            assert_eq!(options.min, 0);
            let events =
                driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
                    beast,
                ])))?;
            rolled.extend(events);
            continue;
        }
        rolled.extend(driver.answer(Driver::passive_answer(&decision))?);
    }
    let all: Vec<Event> = driver.engine().events().to_vec();
    let rolls: Vec<u32> = all
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled { result, .. } => Some(*result),
            _ => None,
        })
        .collect();
    assert_eq!(rolls.len(), 2, "two d20 are rolled");
    let kept = rolls.iter().copied().max().unwrap();
    let beast = driver.id_of("Beast 1")?;
    assert_eq!(
        mtg_engine::rules::must_block(driver.state(), beast),
        kept <= 14
    );
    let _ = rolled;
    Ok(())
}

#[rstest]
#[case::two_seats_once(2, 0)]
#[case::three_seats_twice(3, 1)]
#[case::four_seats_three_times(4, 2)]
fn r6702_game_of_chaos_doubles_the_stakes_with_each_flip(
    #[case] seats: usize,
    #[case] again: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r6702_table(seats, "Game of Chaos", 3))?;
    driver.begin_cast("Game of Chaos")?;
    let mut said = 0;
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                let target = mtg_engine::choose::Target::Player(pid(1));
                driver.choose_targets(&[target])?;
            }
            DecisionKind::YesNo(_) => {
                // The winner of the latest flip decides.
                let won = driver
                    .engine()
                    .events()
                    .iter()
                    .rev()
                    .find_map(|event| match event {
                        Event::CoinFlipped { won, .. } => Some(*won),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(decision.player, if won { pid(0) } else { pid(1) });
                driver.answer(Answer::YesNo(mtg_engine::choose::YesNoChoice(said < again)))?;
                said += 1;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    let flips: Vec<bool> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::CoinFlipped { won, .. } => Some(*won),
            _ => None,
        })
        .collect();
    assert_eq!(flips.len(), again + 1);
    let swing: i32 = flips
        .iter()
        .enumerate()
        .map(|(at, won)| (1 << at) * if *won { 1 } else { -1 })
        .sum();
    let life = |seat: usize| driver.state().player(pid(seat)).unwrap().life();
    assert_eq!((life(0), life(1)), (20 + swing, 20 - swing));
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r6702_spelltwine_casts_both_copies_and_exiles_itself(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = r6702_table(seats, "Spelltwine", 3)
        .graveyard(0, [card("Opt").unwrap()])
        .graveyard(1, [card("Deep Analysis").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let held = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_cast("Spelltwine")?;
    let mut asked_to_cast = false;
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                let first = options.legal[0];
                driver.choose_targets(&[first])?;
            }
            DecisionKind::YesNo(options) if options.text.contains("Cast") => {
                asked_to_cast = true;
                driver.answer(Answer::YesNo(mtg_engine::choose::YesNoChoice(true)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    assert!(!asked_to_cast, "the copies are cast with no question");
    let exile = driver.state().zone(Zone::Exile);
    let names: Vec<String> = exile
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    for name in ["Spelltwine", "Opt", "Deep Analysis"] {
        assert!(
            names.iter().any(|n| n == name),
            "{name} in exile: {names:?}"
        );
    }
    // Opt draws one card, the copy of Deep Analysis two for the player it targets (seat 0, the only choice offered first).
    assert!(driver.state().zone(Zone::Hand(pid(0))).len() >= held);
    Ok(())
}

/// The stun counters on the named permanent.
fn r6702_stun(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&mtg_cards::value::CounterKind::Other("stun".to_owned()))
        })
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn r6702_trap_the_trespassers_stuns_by_the_secret_votes(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r6702_table(seats, "Trap the Trespassers", 3))?;
    driver.cast("Trap the Trespassers")?;
    let mut ballots = 0;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                // Every ballot is face down, and seat 0's own Beast is never on it.
                assert!(options.options.iter().all(|option| matches!(
                    option,
                    mtg_engine::choose::ChoiceOption::SecretObject(_)
                )));
                assert_eq!(options.options.len(), seats - 1);
                ballots += 1;
                driver.answer(Answer::ChooseOption(mtg_engine::choose::OptionChoice(0)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    // With one creature to vote for, nobody is asked.
    if seats > 2 {
        assert_eq!(ballots, seats);
    }
    let first = (1..seats)
        .map(|seat| format!("Beast {seat}"))
        .find(|name| r6702_stun(&driver, name) > 0)
        .expect("a Beast got the votes");
    assert_eq!(r6702_stun(&driver, &first), u32::try_from(seats).unwrap());
    let id = driver.id_of(&first)?;
    assert!(
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert_eq!(r6702_stun(&driver, "Beast 0"), 0);
    Ok(())
}
