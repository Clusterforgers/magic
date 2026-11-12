//! Goal: a flip with no list and a flip until a loss play whole through Tavern Scoundrel, Zndrsplt and Okaun at two, three and four seats: the outcomes are forced through the seed, "whenever you win a coin flip" triggers only for the player who won and "whenever a player wins a coin flip" for every won flip of any player, a flip until a loss flips once more after each win and stops at the first loss, and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::rng::GameRng;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::cards::{free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Passes priority and answers every question passively until nothing is on the stack and priority is asked again.
fn c8_vote_resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let idle = matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty();
        if idle {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

/// The first seed whose stream, from the substream `index` on, wins and loses the flips as `wins` says.
fn c8_vote_seed(index: u64, wins: &[bool]) -> u64 {
    (0u64..1_000_000)
        .find(|seed| {
            wins.iter()
                .zip(index..)
                .all(|(win, at)| (GameRng::substream(*seed, at).below(2) == 0) == *win)
        })
        .unwrap()
}

fn c8_vote_flips(driver: &Driver) -> Vec<bool> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::CoinFlipped { won, .. } => Some(*won),
            _ => None,
        })
        .collect()
}

fn c8_vote_hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn c8_vote_named(driver: &Driver, seat: usize, name: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(seat) && object.card().name == name)
        .count()
}

fn c8_vote_replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Tavern Scoundrel flips with no list: a win makes two Treasures for its controller and draws a card for the last seat's
/// Zndrsplt ("whenever a player wins a coin flip"); a loss does neither.
#[rstest]
fn c8_vote_a_bare_flip_counts_for_the_won_flip_triggers(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] win: bool,
) -> Result<(), ScriptError> {
    let start = Driver::from_scenario(Scenario::new(seats))?
        .state()
        .rng_index();
    let last = seats - 1;
    let scenario = Scenario::new(seats)
        .seed(c8_vote_seed(start, &[win]))
        .battlefield(
            0,
            [
                ready(card("Tavern Scoundrel").unwrap()),
                ready(free_creature("Fodder", 1, 1)),
                ready(mountain()),
            ],
        )
        .battlefield(last, [ready(card("Zndrsplt, Eye of Wisdom").unwrap())])
        .library(last, [island(), island()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let before = c8_vote_hand(&driver, last);
    driver.begin_activation("Tavern Scoundrel")?;
    let fodder = driver.id_of("Fodder")?;
    driver.answer(Answer::ChooseCards(CardsChoice(vec![fodder])))?;
    driver.pay_tapping(&["Mountain"], 0)?;
    c8_vote_resolve(&mut driver)?;
    assert_eq!(c8_vote_flips(&driver), [win]);
    assert_eq!(c8_vote_named(&driver, 0, "Treasure"), 2 * usize::from(win));
    assert_eq!(c8_vote_hand(&driver, last), before + usize::from(win));
    c8_vote_replays(&driver);
    Ok(())
}

/// Okaun flips until a loss at the beginning of combat: each won flip doubles its power and toughness and draws a card
/// for the last seat's Zndrsplt, and the first lost flip ends the flips.
#[rstest]
fn c8_vote_a_flip_until_a_loss_flips_again_after_each_win(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] wins: usize,
) -> Result<(), ScriptError> {
    let start = Driver::from_scenario(Scenario::new(seats))?
        .state()
        .rng_index();
    let last = seats - 1;
    let mut outcomes = vec![true; wins];
    outcomes.push(false);
    let scenario = Scenario::new(seats)
        .seed(c8_vote_seed(start, &outcomes))
        .battlefield(0, [ready(card("Okaun, Eye of Chaos").unwrap())])
        .battlefield(last, [ready(card("Zndrsplt, Eye of Wisdom").unwrap())])
        .library(last, [island(), island(), island(), island()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let before = c8_vote_hand(&driver, last);
    driver.pass_until(Step::BeginCombat)?;
    c8_vote_resolve(&mut driver)?;
    assert_eq!(c8_vote_flips(&driver), outcomes);
    assert_eq!(c8_vote_hand(&driver, last), before + wins);
    let okaun = driver.id_of("Okaun, Eye of Chaos")?;
    let power = characteristics(driver.state(), okaun).unwrap().power;
    assert_eq!(power, Some(3 << wins), "doubled once for each win");
    c8_vote_replays(&driver);
    Ok(())
}
