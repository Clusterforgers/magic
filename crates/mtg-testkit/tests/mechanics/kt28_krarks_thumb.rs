//! Goal: Krark's Thumb, played from its corpus file, makes every coin its controller flips two coins at two, three and four seats, for a single flip (Tavern Scoundrel) and for each flip of a flip until a loss (Zndrsplt): the player calls once, the log carries both coins and the kept side, two coins that differ ask which to keep and two that agree ask nothing, only the kept coin wins or loses, and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{CardsChoice, ChoiceOption, CoinSide, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::rng::GameRng;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const H: CoinSide = CoinSide::Heads;
const T: CoinSide = CoinSide::Tails;

/// The answer to a question of a flip: the call is tails, the coin kept is a won one when `keep_won`, a lost one if not.
fn kt28_answer(options: &[ChoiceOption], keep_won: bool) -> Answer {
    let call = matches!(options.first(), Some(ChoiceOption::Side(_)));
    Answer::ChooseOption(OptionChoice(if call { 1 } else { usize::from(!keep_won) }))
}

/// Answers the flips as `kt28_answer` says and everything else passively until the stack is empty; returns how many
/// questions asked which coin to keep.
fn kt28_resolve(driver: &mut Driver, keep_won: bool) -> Result<usize, ScriptError> {
    let mut keeps = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(keeps),
            DecisionKind::ChooseOption(options) => {
                keeps += usize::from(matches!(
                    options.options.first(),
                    Some(ChoiceOption::Label(_))
                ));
                driver.answer(kt28_answer(&options.options, keep_won))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

/// The first seed whose stream, from the substream `index` on, flips the coins of each flip as `flips` says.
fn kt28_seed(index: u64, flips: &[[CoinSide; 2]]) -> u64 {
    (0u64..1_000_000)
        .find(|seed| {
            flips.iter().zip(index..).all(|(coins, at)| {
                let mut rng = GameRng::substream(*seed, at);
                coins.iter().all(|coin| (rng.below(2) == 0) == (*coin == H))
            })
        })
        .unwrap()
}

/// Every flip logged: the call, the kept side, whether it was won and the coins.
fn kt28_flips(driver: &Driver) -> Vec<(CoinSide, CoinSide, bool, Vec<CoinSide>)> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::CoinFlipped {
                call,
                side,
                won,
                coins,
                ..
            } => Some((*call, *side, *won, coins.clone())),
            _ => None,
        })
        .collect()
}

/// The side kept from two coins with the call tails.
fn kt28_kept(coins: [CoinSide; 2], keep_won: bool) -> CoinSide {
    if coins[0] == coins[1] {
        coins[0]
    } else if keep_won {
        T
    } else {
        H
    }
}

fn kt28_named(driver: &Driver, seat: usize, name: &str) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.controller() == pid(seat) && object.card().name == name)
        .count()
}

fn kt28_replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// Tavern Scoundrel's single flip under Krark's Thumb: both coins are logged, the kept one decides the Treasures.
#[rstest]
fn kt28_a_single_flip_flips_two_coins_and_keeps_one(
    #[values(2, 3, 4)] seats: usize,
    #[values([H, H], [T, T], [H, T], [T, H])] coins: [CoinSide; 2],
    #[values(true, false)] keep_won: bool,
) -> Result<(), ScriptError> {
    let start = Driver::from_scenario(Scenario::new(seats))?
        .state()
        .rng_index();
    let scenario = Scenario::new(seats)
        .seed(kt28_seed(start, &[coins]))
        .battlefield(
            0,
            [
                ready(card("Tavern Scoundrel").unwrap()),
                ready(card("Krark's Thumb").unwrap()),
                ready(free_creature("Fodder", 1, 1)),
                ready(mountain()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Tavern Scoundrel")?;
    let fodder = driver.id_of("Fodder")?;
    driver.answer(Answer::ChooseCards(CardsChoice(vec![fodder])))?;
    driver.pay_tapping(&["Mountain"], 0)?;
    let keeps = kt28_resolve(&mut driver, keep_won)?;
    let kept = kt28_kept(coins, keep_won);
    assert_eq!(kt28_flips(&driver), [(T, kept, kept == T, coins.to_vec())]);
    assert_eq!(
        keeps,
        usize::from(coins[0] != coins[1]),
        "asked only when the coins differ"
    );
    assert_eq!(
        kt28_named(&driver, 0, "Treasure"),
        2 * usize::from(kept == T)
    );
    kt28_replays(&driver);
    Ok(())
}

/// Zndrsplt flips until a loss under its controller's Krark's Thumb: each flip is two coins, a won flip flips again and
/// draws a card, the first kept loss ends the flips.
#[rstest]
#[case::lost_at_once(vec![[H, H]], 0)]
#[case::kept_wins_then_a_loss(vec![[H, T], [T, T], [T, H], [H, H]], 3)]
fn kt28_a_flip_until_a_loss_flips_two_coins_each_time(
    #[values(2, 3, 4)] seats: usize,
    #[case] flips: Vec<[CoinSide; 2]>,
    #[case] wins: usize,
) -> Result<(), ScriptError> {
    let start = Driver::from_scenario(Scenario::new(seats))?
        .state()
        .rng_index();
    let scenario = Scenario::new(seats)
        .seed(kt28_seed(start, &flips))
        .battlefield(
            0,
            [
                ready(card("Zndrsplt, Eye of Wisdom").unwrap()),
                ready(card("Krark's Thumb").unwrap()),
            ],
        )
        .library(0, [island(), island(), island(), island()]);
    let mut driver = Driver::from_scenario(scenario)?;
    let before = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.pass_until(Step::BeginCombat)?;
    let keeps = kt28_resolve(&mut driver, true)?;
    let expected: Vec<_> = flips
        .iter()
        .map(|coins| {
            let kept = kt28_kept(*coins, true);
            (T, kept, kept == T, coins.to_vec())
        })
        .collect();
    assert_eq!(kt28_flips(&driver), expected);
    let differ = flips.iter().filter(|coins| coins[0] != coins[1]).count();
    assert_eq!(keeps, differ);
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        before + wins,
        "a card for each kept win"
    );
    kt28_replays(&driver);
    Ok(())
}
