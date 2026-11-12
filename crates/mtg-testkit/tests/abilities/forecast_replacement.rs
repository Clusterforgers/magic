//! Goal: the combat forecast and the block preview give the numbers the real damage step gives with replacement and prevention effects in play (Furnace of Rath, a life doubler, damage shields, Fog, regeneration, protection, Rest in Peace and their mixes): the damage, the deaths and the life after of a table of boards equal what is dealt for real, a question the replacement step asks is answered with the first option and flagged when another answer would change the numbers, a preview equals the forecast after the real block, and bot-like random boards agree with the real step whatever the answers when nothing depends on a choice.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::forecast::{damage_was_dealt, damage_was_dealt_in, disagreement};
use common::name_of;
use common::pid;
use common::replaced::{Board, Piece, Shield, forecast_now, lethal_first, outcome, play_out};
use mtg_core::color::Color;
use mtg_engine::choose::{OptionChoice, ReplacementChoice};
use mtg_engine::decision::DecisionKind;
use mtg_engine::forecast::{CombatForecast, pending_combat_forecast};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Engine, Event, GameSetup};
use mtg_testkit::corpus::CorpusError;
use mtg_testkit::{
    Driver, RandomBot, ScriptError, prevent_fuzz_setup, protect_fuzz_setup, replace_fuzz_setup,
};
use proptest::prelude::*;
use rstest::rstest;

use Piece::{Body, Card};

const A: Piece = Body("A", 3, 3, None);
const B: Piece = Body("B", 2, 2, None);
const FURNACE: Piece = Card("Furnace of Rath");
const RHOX: Piece = Card("Rhox Faithmender");
const KNIGHT: Piece = Card("Black Knight");
const REST: Piece = Card("Rest in Peace");

/// What a table row expects: the damage to player 1 and their life after, who dies, and whether a choice matters.
struct Want {
    player: (u32, i32),
    dies: &'static [&'static str],
    choice: bool,
}

const fn want(player: (u32, i32), dies: &'static [&'static str], choice: bool) -> Want {
    Want {
        player,
        dies,
        choice,
    }
}

fn dying(driver: &Driver, forecast: &CombatForecast) -> Vec<String> {
    let mut names: Vec<String> = forecast
        .dying()
        .map(|object| name_of(driver.state(), object))
        .collect();
    names.sort();
    names
}

fn defender(forecast: &CombatForecast) -> (u32, i32) {
    let player = forecast
        .players
        .iter()
        .find(|p| p.player == pid(1))
        .expect("player 1 is aimed at");
    (player.damage, player.life_after)
}

#[rstest]
// Without effects, for reference.
#[case::plain(Board::new(&[A], &[B], &["A"], &[]), want((3, 17), &[], false))]
// Furnace of Rath doubles each packet, whoever controls it.
#[case::furnace_of_the_defender(Board::new(&[A], &[B, FURNACE], &["A"], &[]), want((6, 14), &[], false))]
#[case::furnace_of_the_attacker(Board::new(&[A, FURNACE], &[B], &["A"], &[]), want((6, 14), &[], false))]
#[case::two_furnaces_in_either_order(Board::new(&[A, FURNACE], &[B, FURNACE], &["A"], &[]), want((12, 8), &[], false))]
#[case::furnace_kills_what_the_plain_damage_does_not(Board::new(&[Body("A", 2, 3, None)], &[Body("B", 2, 3, None), FURNACE], &["A"], &[("B", "A")]), want((0, 20), &["A", "B"], false))]
#[case::furnace_makes_a_lethal_attack(Board::new(&[A, FURNACE], &[B], &["A"], &[]).at_life(5), want((6, -1), &[], false))]
// Rhox Faithmender has lifelink and doubles the life its controller gains.
#[case::a_lifelink_blocker_gains_twice(Board::new(&[A], &[RHOX], &["A"], &[("Rhox Faithmender", "A")]), want((0, 22), &[], false))]
#[case::furnace_then_lifelink_then_the_doubler(Board::new(&[A, FURNACE], &[RHOX], &["A"], &[("Rhox Faithmender", "A")]), want((0, 24), &["Rhox Faithmender"], false))]
// Prevention shields.
#[case::a_shield_for_the_defender(Board::new(&[Body("A", 5, 5, None)], &[B], &["A"], &[]).shielded(Shield::ToDefender(2)), want((3, 17), &[], false))]
#[case::a_shield_larger_than_the_damage(Board::new(&[Body("A", 5, 5, None)], &[B], &["A"], &[]).shielded(Shield::ToDefender(9)), want((0, 20), &[], false))]
#[case::fog_stops_every_packet(Board::new(&[A], &[Body("B", 3, 3, None)], &["A"], &[("B", "A")]).shielded(Shield::Fog), want((0, 20), &[], false))]
#[case::fog_beats_furnace(Board::new(&[A, FURNACE], &[B], &["A"], &[]).shielded(Shield::Fog), want((0, 20), &[], false))]
// A shield and Furnace: the order is the defender's to pick, and it changes the damage (4 against 2).
#[case::a_shield_and_furnace(Board::new(&[A], &[B, FURNACE], &["A"], &[]).shielded(Shield::ToDefender(2)), want((4, 16), &[], true))]
#[case::two_shields_whose_order_is_nothing(Board::new(&[A], &[B], &["A"], &[]).shielded(Shield::ToDefender(2)).shielded(Shield::ToDefender(2)), want((0, 20), &[], false))]
// Regeneration, protection and Rest in Peace.
#[case::regeneration_takes_the_death(Board::new(&[Body("A", 4, 4, None)], &[B], &["A"], &[("B", "A")]).shielded(Shield::Regenerate("B")), want((0, 20), &[], false))]
#[case::protection_takes_the_damage(Board::new(&[Body("A", 4, 4, Some(Color::White))], &[KNIGHT], &["A"], &[("Black Knight", "A")]), want((0, 20), &[], false))]
#[case::protection_against_a_doubled_first_strike(Board::new(&[Body("A", 4, 4, Some(Color::White)), FURNACE], &[KNIGHT], &["A"], &[("Black Knight", "A")]), want((0, 20), &["A"], false))]
#[case::the_dead_are_exiled(Board::new(&[A, REST], &[Body("B", 3, 3, None)], &["A"], &[("B", "A")]), want((0, 20), &["A", "B"], false))]
fn the_forecast_shows_the_numbers_of_the_real_damage(
    #[case] board: Board,
    #[case] expected: Want,
) -> Result<(), ScriptError> {
    let driver = board.driver_before_damage()?;
    let forecast = forecast_now(&driver);
    assert_eq!(
        defender(&forecast),
        expected.player,
        "damage and life after"
    );
    assert_eq!(dying(&driver, &forecast), expected.dies, "deaths");
    assert_eq!(forecast.depends_on_choice, expected.choice, "the choice");

    // The first answer to every question is what the forecast takes: the real damage is the same.
    let mut first = board.driver_before_damage()?;
    let events = play_out(&mut first, 0)?;
    assert_eq!(disagreement(&forecast, &events, first.state()), None);

    // Another answer changes the outcome exactly when the forecast says so.
    let mut last = board.driver_before_damage()?;
    play_out(&mut last, usize::MAX)?;
    assert_eq!(
        outcome(&first) != outcome(&last),
        expected.choice,
        "{:?} against {:?}",
        outcome(&first),
        outcome(&last)
    );
    Ok(())
}

#[rstest]
#[case::furnace(Board::new(&[A, FURNACE], &[B], &["A"], &[("B", "A")]))]
#[case::a_shield_and_furnace(Board::new(&[A], &[B, FURNACE], &["A"], &[]).shielded(Shield::ToDefender(2)))]
#[case::lifelink_doubled(Board::new(&[A], &[RHOX, FURNACE], &["A"], &[("Rhox Faithmender", "A")]))]
#[case::fog(Board::new(&[A], &[B], &["A"], &[("B", "A")]).shielded(Shield::Fog))]
fn the_preview_of_a_block_equals_the_forecast_after_it(
    #[case] board: Board,
) -> Result<(), ScriptError> {
    let mut driver = board.driver_facing_blocks()?;
    let answer = Answer::DeclareBlockers(
        board
            .blocks
            .iter()
            .map(|(blocker, attacker)| {
                (
                    driver.id_of(blocker).unwrap(),
                    driver.id_of(attacker).unwrap(),
                )
            })
            .collect(),
    );
    let (hash, logged) = (driver.engine().state_hash(), driver.engine().events().len());
    let previewed = driver
        .engine()
        .preview_forecast(pid(1), &answer)
        .expect("a forecast");
    assert_eq!(
        (driver.engine().state_hash(), driver.engine().events().len()),
        (hash, logged),
        "the preview moved the game"
    );
    driver.answer(answer)?;
    assert_eq!(pending_combat_forecast(driver.state()), Some(previewed));
    Ok(())
}

/// A body of the property: power, toughness and colour, by index.
fn body(name: &'static str, power: i32, toughness: i32, color: usize) -> Piece {
    Body(
        name,
        power,
        toughness,
        [None, Some(Color::White), Some(Color::Red)][color],
    )
}

/// A random board: one to three attackers, up to three blockers each blocking one or none, and a mix of the cards
/// and shields above.
fn boards() -> impl Strategy<Value = Board> {
    let attacker = (0..6i32, 1..6i32, 0..3usize);
    let blocker = (0..5i32, 1..6i32, 0..3usize, 0..4usize);
    (
        prop::collection::vec(attacker, 1..=3),
        prop::collection::vec(blocker, 0..=3),
        (any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>()),
        (0..5u32, any::<bool>(), any::<bool>()),
        1..30i32,
    )
        .prop_map(|(attackers, blockers, cards, shields, life)| {
            const ATTACKERS: [&str; 3] = ["A0", "A1", "A2"];
            const BLOCKERS: [&str; 3] = ["B0", "B1", "B2"];
            let mut mine: Vec<Piece> = attackers
                .iter()
                .enumerate()
                .map(|(at, &(p, t, c))| body(ATTACKERS[at], p, t, c))
                .collect();
            let mut theirs: Vec<Piece> = blockers
                .iter()
                .enumerate()
                .map(|(at, &(p, t, c, _))| body(BLOCKERS[at], p, t, c))
                .collect();
            let blocks: Vec<(&str, &str)> = blockers
                .iter()
                .enumerate()
                .filter_map(|(at, &(.., block))| {
                    ATTACKERS
                        .get(block)
                        .filter(|_| block < attackers.len())
                        .map(|attacker| (BLOCKERS[at], *attacker))
                })
                .collect();
            let (furnace_mine, furnace_theirs, rhox, rest) = cards;
            if furnace_mine {
                mine.push(FURNACE);
            }
            if furnace_theirs {
                theirs.push(FURNACE);
            }
            if rhox {
                theirs.push(RHOX);
            }
            if rest {
                mine.push(REST);
            }
            let mut board =
                Board::new(&mine, &theirs, &ATTACKERS[..attackers.len()], &blocks).at_life(life);
            let (left, fog, regenerate) = shields;
            if left > 0 {
                board = board.shielded(Shield::ToDefender(left));
            }
            if fog {
                board = board.shielded(Shield::Fog);
            }
            if regenerate && !blockers.is_empty() {
                board = board.shielded(Shield::Regenerate("B0"));
            }
            board
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(160))]

    #[test]
    fn the_forecast_equals_the_real_damage_over_random_boards(board in boards()) {
        let driver = board.driver_before_damage().expect("a board");
        let forecast = forecast_now(&driver);
        let mut first = board.driver_before_damage().expect("a board");
        let events = play_out(&mut first, 0).expect("the damage is dealt");
        prop_assert_eq!(disagreement(&forecast, &events, first.state()), None);

        // When no answer matters, every answer gives the forecast.
        if !forecast.depends_on_choice {
            let mut last = board.driver_before_damage().expect("a board");
            let events = play_out(&mut last, usize::MAX).expect("the damage is dealt");
            prop_assert_eq!(disagreement(&forecast, &events, last.state()), None);
        }
    }
}

/// What the games exercised, so a passing property cannot hide an empty one.
#[derive(Debug, Default)]
struct Coverage {
    compared: u32,
    gained: u32,
    prevented: u32,
    choices: u32,
}

/// Plays a bot game on `setup` where each question of the replacement step is answered with its first option, which
/// is what the forecast takes, and compares the last forecast before the damage of each combat with everything the
/// combat dealt.
fn play(setup: GameSetup, players: usize, seed: u64) -> Result<Coverage, String> {
    let mut engine = Engine::new(setup).map_err(|e| e.to_string())?;
    let mut bots: Vec<RandomBot> = (0..players as u64)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    let mut coverage = Coverage::default();
    // The forecast taken before the first damage of the combat, the events since the damage began, and whether it has.
    let mut held: Option<(CombatForecast, Vec<Event>, bool)> = None;
    let mut regular = false;
    for _ in 0..200_000 {
        let Some(decision) = engine.decision().cloned() else {
            if let Some((forecast, events, true)) = held.take() {
                compare(&forecast, &events, &engine, &mut coverage)
                    .map_err(|e| format!("seed {seed}, {players} players, at the end: {e}"))?;
            }
            return Ok(coverage);
        };
        if held.as_ref().is_none_or(|(_, _, dealing)| !dealing) {
            held = pending_combat_forecast(engine.state())
                .map(|forecast| (forecast, Vec::new(), false));
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseReplacement(_) => Answer::ChooseReplacement(ReplacementChoice(0)),
            DecisionKind::ChooseOption(_) => Answer::ChooseOption(OptionChoice(0)),
            DecisionKind::AssignDamage(options) => Answer::AssignDamage(lethal_first(options)),
            _ => bots[decision.player.index()].choose(&decision),
        };
        let step = engine.state().turn().step();
        let events = engine
            .apply(answer)
            .map_err(|e| format!("seed {seed}: engine refused an answer: {e}"))?;
        regular |= damage_was_dealt(step, &events);
        if let Some((_, seen, dealing)) = held.as_mut() {
            if regular || damage_was_dealt_in(Step::FirstStrikeDamage, step, &events) {
                *dealing = true;
            }
            if *dealing {
                seen.extend(events);
            }
        }
        // The state-based actions after the damage may ask about the order of effects too: wait for their end.
        if regular && !engine.replacing() {
            regular = false;
            if let Some((forecast, seen, _)) = held.take() {
                compare(&forecast, &seen, &engine, &mut coverage)
                    .map_err(|e| format!("seed {seed}, {players} players: {e}"))?;
            }
        }
    }
    Err(format!("seed {seed}: game did not finish"))
}

fn compare(
    forecast: &CombatForecast,
    events: &[Event],
    engine: &Engine,
    coverage: &mut Coverage,
) -> Result<(), String> {
    if let Some(difference) = disagreement(forecast, events, engine.state()) {
        return Err(difference);
    }
    let dealt = events
        .iter()
        .any(|event| matches!(event, Event::DamageDealt { .. }));
    coverage.compared += 1;
    coverage.choices += u32::from(forecast.depends_on_choice);
    coverage.gained += u32::from(events.iter().any(|e| matches!(e, Event::LifeGained { .. })));
    coverage.prevented +=
        u32::from(!dealt && forecast.attackers.iter().any(|a| !a.deals.is_empty()));
    Ok(())
}

type Deck = fn(usize, u64) -> Result<GameSetup, CorpusError>;

#[rstest]
#[case::replacement_cards(replace_fuzz_setup, true)]
#[case::prevention_cards(prevent_fuzz_setup, true)]
#[case::protection_cards(protect_fuzz_setup, false)]
fn the_forecast_agrees_with_games_of_replacement_cards(
    #[case] make: Deck,
    #[case] reaches_effects: bool,
    #[values(2, 3, 4)] players: usize,
) {
    let mut total = Coverage::default();
    for seed in 0..30 {
        let setup = make(players, seed).expect("a deck");
        let coverage = play(setup, players, seed).unwrap_or_else(|e| panic!("{e}"));
        total.compared += coverage.compared;
        total.gained += coverage.gained;
        total.prevented += coverage.prevented;
        total.choices += coverage.choices;
    }
    assert!(total.compared > 0, "{total:?}");
    // The games reach the effects: life gained, damage all prevented, or a question about the order.
    assert!(
        !reaches_effects || total.gained + total.prevented + total.choices > 0,
        "{total:?}"
    );
}
