//! Goal: the l32-a pieces play as printed at tables of two, three and four seats: an Adversary's cost is paid as many
//! times as its controller chooses (none, once, twice) and its reflexive trigger does "that many" only after a payment;
//! Gilded Drake exchanges itself for its target and is sacrificed when it has none or the target became illegal; Portal
//! Mage and Windshaper Planetar reselect what attackers attack only when they enter during the declare attackers step;
//! and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::characteristics::{characteristics, controller_of};
use mtg_engine::choose::{ChoiceOption, NumberChoice, OptionChoice, Target};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, burn, island, mountain, plains, swamp, walker};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// How the questions of a test are answered.
struct L32aAnswers {
    /// The number of payments chosen.
    times: u32,
    /// The targets taken, by name; the first legal one when empty, none when `Some(vec![])`.
    targets: Option<Vec<Target>>,
    /// The option taken in a choice of what an attacker attacks: the player of this seat, else the option so labelled.
    option: Option<ChoiceOption>,
}

/// Answers until the stack is empty and the active seat has priority again.
fn l32a_settle(driver: &mut Driver, answers: &L32aAnswers) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(answers.times)))?;
            }
            DecisionKind::ChooseOption(options) => {
                let index = answers
                    .option
                    .as_ref()
                    .and_then(|wanted| options.options.iter().position(|o| o == wanted))
                    .unwrap_or(0);
                driver.answer(Answer::ChooseOption(OptionChoice(index)))?;
            }
            DecisionKind::ChooseTargets(options) => {
                let chosen = answers
                    .targets
                    .clone()
                    .unwrap_or_else(|| options.legal.iter().take(1).copied().collect());
                driver.choose_targets(&chosen)?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn l32a_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Seat 0's first main phase with the card in hand and the lands on the battlefield; every seat has a library.
fn l32a_table(
    seats: usize,
    hand: &str,
    lands: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [card(hand).unwrap()])
        .battlefield(0, lands.into_iter().map(ready))
        .battlefield(1, [ready(bear())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| bear()));
    }
    scenario
}

fn l32a_counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(kind)
}

fn l32a_named(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).is_some_and(|o| o.card().name == name))
        .count()
}

#[rstest]
fn l32a_intrepid_pays_as_often_as_chosen(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] times: u32,
) -> Result<(), ScriptError> {
    // {1}{W} for the card and two more payments of {1}{W}.
    let table = || {
        l32a_table(
            seats,
            "Intrepid Adversary",
            (0..6).map(|_| plains()).collect(),
        )
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Intrepid Adversary")?;
    let answers = L32aAnswers {
        times,
        targets: None,
        option: None,
    };
    l32a_settle(&mut driver, &answers)?;
    let valor = CounterKind::Other("valor".to_owned());
    assert_eq!(l32a_counters(&driver, "Intrepid Adversary", &valor), times);
    let id = driver.id_of("Intrepid Adversary")?;
    let power = characteristics(driver.state(), id).unwrap().power.unwrap();
    assert_eq!(
        power,
        3 + i32::try_from(times).unwrap(),
        "its own anthem counts"
    );
    let untapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = driver.state().object(**id).unwrap();
            object.controller() == pid(0) && !object.permanent().unwrap().tapped()
        })
        .count();
    assert_eq!(
        untapped,
        1 + 4 - 2 * times as usize,
        "two lands for each payment"
    );
    l32a_replays(&driver, table());
    Ok(())
}

#[rstest]
fn l32a_tainted_makes_twice_that_many(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] times: u32,
) -> Result<(), ScriptError> {
    let table = || {
        l32a_table(
            seats,
            "Tainted Adversary",
            (0..8).map(|_| swamp()).collect(),
        )
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Tainted Adversary")?;
    let answers = L32aAnswers {
        times,
        targets: None,
        option: None,
    };
    l32a_settle(&mut driver, &answers)?;
    assert_eq!(
        l32a_counters(&driver, "Tainted Adversary", &CounterKind::PlusOnePlusOne),
        times
    );
    assert_eq!(l32a_named(&driver, "Zombie"), 2 * times as usize);
    l32a_replays(&driver, table());
    Ok(())
}

#[rstest]
fn l32a_the_most_offered_is_what_can_be_paid(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Five Plains: two pay for the card, the other three pay {1}{W} once.
    let mut driver = Driver::from_scenario(l32a_table(
        seats,
        "Intrepid Adversary",
        (0..5).map(|_| plains()).collect(),
    ))?;
    driver.cast("Intrepid Adversary")?;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        if let DecisionKind::ChooseNumber(options) = &decision.kind {
            assert_eq!((options.min, options.max), (0, Some(1)));
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck("no number was asked".to_owned()))
}

/// What happens to Gilded Drake's target before its trigger resolves.
#[derive(Clone, Copy, Debug)]
enum L32aDrake {
    /// It stays.
    Stays,
    /// No target is chosen.
    None,
    /// A Bolt kills it in response.
    Killed,
}

#[rstest]
fn l32a_gilded_drake_exchanges_or_is_sacrificed(
    #[values(2, 3, 4)] seats: usize,
    #[values(L32aDrake::Stays, L32aDrake::None, L32aDrake::Killed)] case: L32aDrake,
) -> Result<(), ScriptError> {
    let table = || {
        l32a_table(seats, "Gilded Drake", vec![island(), island(), mountain()])
            .hand(0, [card("Gilded Drake").unwrap(), burn("Bolt", 3)])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Gilded Drake")?;
    let bear = driver.target_named("Grizzly Bears")?;
    // The Drake resolves; its trigger asks for its target.
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::ChooseTargets(_)) {
            break;
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    let chosen = match case {
        L32aDrake::None => Vec::new(),
        L32aDrake::Stays | L32aDrake::Killed => vec![bear],
    };
    driver.choose_targets(&chosen)?;
    if matches!(case, L32aDrake::Killed) {
        driver.cast_at("Bolt", &[bear])?;
    }
    let answers = L32aAnswers {
        times: 0,
        targets: Some(Vec::new()),
        option: None,
    };
    l32a_settle(&mut driver, &answers)?;
    let state = driver.state();
    let drake = state
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| {
            state
                .object(**id)
                .is_some_and(|o| o.card().name == "Gilded Drake")
        })
        .copied();
    match case {
        L32aDrake::Stays => {
            let drake = drake.expect("the Drake stays");
            assert_eq!(controller_of(state, drake), Some(pid(1)));
            let bear = driver.id_of("Grizzly Bears")?;
            assert_eq!(controller_of(state, bear), Some(pid(0)));
        }
        L32aDrake::None | L32aDrake::Killed => {
            assert!(drake.is_none(), "{case:?}: the Drake is sacrificed");
            assert!(
                state.zone(Zone::Graveyard(pid(0))).iter().any(|id| state
                    .object(*id)
                    .unwrap()
                    .card()
                    .name
                    == "Gilded Drake")
            );
        }
    }
    l32a_replays(&driver, table());
    Ok(())
}

/// Seat 0 attacks seat 1 with a Bear (and a second Bear with `two`) and holds the flash creature and Islands; seat 1 has
/// a planeswalker.
fn l32a_attack_table(seats: usize, flash: &str, two: bool) -> Scenario {
    let mut attackers = vec![ready(bear())];
    if two {
        attackers.push(ready(bear()));
    }
    attackers.extend((0..5).map(|_| ready(island())));
    attackers.extend((0..2).map(|_| ready(plains())));
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [card(flash).unwrap()])
        .battlefield(0, attackers)
        .battlefield(1, [ready(walker("Walker", 3))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| bear()));
    }
    scenario
}

/// What each attacker of seat 0 attacks.
fn l32a_aims(driver: &Driver) -> Vec<AttackTarget> {
    driver
        .state()
        .combat()
        .attackers()
        .iter()
        .map(mtg_engine::combat::Attacker::target)
        .collect()
}

#[rstest]
fn l32a_portal_mage_reselects_during_declare_attackers(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] keep: bool,
) -> Result<(), ScriptError> {
    let table = || l32a_attack_table(seats, "Portal Mage", false);
    let mut driver = Driver::from_scenario(table())?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Grizzly Bears", 1)])?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.cast("Portal Mage")?;
    // With two seats the other choice is the planeswalker; with more, the last seat.
    let other = if seats == 2 {
        ChoiceOption::Label("Walker".to_owned())
    } else {
        ChoiceOption::Player(pid(seats - 1))
    };
    let answers = L32aAnswers {
        times: 0,
        targets: None,
        option: (!keep).then(|| other.clone()),
    };
    l32a_settle(&mut driver, &answers)?;
    let aims = l32a_aims(&driver);
    let walker = driver.id_of("Walker")?;
    let wanted = match (keep, seats) {
        (true, _) => AttackTarget::Player(pid(1)),
        (false, 2) => AttackTarget::Planeswalker(walker),
        (false, _) => AttackTarget::Player(pid(seats - 1)),
    };
    assert_eq!(aims, vec![wanted]);
    l32a_replays(&driver, table());
    Ok(())
}

#[rstest]
fn l32a_portal_mage_outside_combat_does_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(l32a_attack_table(seats, "Portal Mage", false))?;
    driver.cast("Portal Mage")?;
    let answers = L32aAnswers {
        times: 0,
        targets: None,
        option: None,
    };
    l32a_settle(&mut driver, &answers)?;
    assert_eq!(l32a_named(&driver, "Portal Mage"), 1);
    assert_eq!(driver.state().pending_triggers().len(), 0);
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, mtg_engine::events::Event::AbilityTriggered(_))),
        "it enters outside the declare attackers step, so it does not trigger"
    );
    Ok(())
}

#[rstest]
fn l32a_windshaper_reselects_each_attacker(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || l32a_attack_table(seats, "Windshaper Planetar", true);
    let mut driver = Driver::from_scenario(table())?;
    let before = driver.state().player(pid(2)).unwrap().life();
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Grizzly Bears", 1), ("Grizzly Bears", 1)])?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.cast("Windshaper Planetar")?;
    let answers = L32aAnswers {
        times: 0,
        targets: None,
        option: Some(ChoiceOption::Player(pid(2))),
    };
    l32a_settle(&mut driver, &answers)?;
    assert_eq!(
        l32a_aims(&driver),
        vec![AttackTarget::Player(pid(2)), AttackTarget::Player(pid(2))]
    );
    // The redirected Bears deal their damage to seat 2.
    driver.pass_until(Step::CombatDamage)?;
    driver.idle_until(2, Step::End)?;
    assert_eq!(driver.state().player(pid(2)).unwrap().life(), before - 4);
    l32a_replays(&driver, table());
    Ok(())
}

#[rstest]
#[case("Intrepid Adversary")]
#[case("Tainted Adversary")]
#[case("Gilded Drake")]
#[case("Portal Mage")]
#[case("Windshaper Planetar")]
#[case("Misleading Signpost")]
fn l32a_the_cards_are_verified(#[case] name: &str) {
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
