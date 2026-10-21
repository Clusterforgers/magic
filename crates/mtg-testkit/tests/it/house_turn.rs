//! Goal: each house rule of the turn and life changes its own part of the game and nothing else at two, three and four seats: the cards drawn in the draw step and the lands played each turn, mana burn, an empty library that reshuffles the graveyard (replayable and seeded), the turn limit and its result, the life total that wins, the poison threshold, the card rule of no maximum hand size beside the house limit, and the handicaps of single seats, each with the format's own first draw and mulligan; the refused setups; and bot games with every rule.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

use crate::common::house::any_turn_rules;
use crate::common::pid;
use crate::common::seeds::for_each_seed;

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::house::{
    EmptyLibrary, HandLimit, Handicap, HouseRuleError, HouseRules, SkipFirstDraw,
};
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice, PriorityAction};
use mtg_engine::error::SetupError;
use mtg_engine::player::{LossReason, PlayerStatus};
use mtg_engine::state::Stage;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameResult, GameSetup, GameState, replay};
use mtg_testkit::cards::{bear, forest, salve};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, GameFailure, Scenario, ScriptError, fuzz_setup, play_game, ready};
use proptest::prelude::*;
use rstest::rstest;

fn rules(set: impl FnOnce(&mut HouseRules)) -> HouseRules {
    let mut rules = HouseRules::default();
    set(&mut rules);
    rules
}

fn hand_of(state: &GameState, seat: usize) -> usize {
    state.zone(Zone::Hand(pid(seat))).len()
}

fn life_of(state: &GameState, seat: usize) -> i32 {
    state
        .player(pid(seat))
        .map_or(i32::MIN, mtg_engine::player::Player::life)
}

fn lost_to(state: &GameState, seat: usize) -> Option<LossReason> {
    match state.player(pid(seat)).map(|p| p.status().clone()) {
        Some(PlayerStatus::Lost(reason)) => Some(reason),
        _ => None,
    }
}

fn amount(n: Option<u32>) -> usize {
    n.map_or(1, |n| n as usize)
}

/// Passes priority, keeps and declares nothing until turn `turn` reaches its first main phase or the game is over.
fn to_main(driver: &mut Driver, turn: u32) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        if state.stage() == Stage::Finished
            || (state.turn().number() == turn && state.turn().step() == Step::Main1)
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck(format!("turn {turn}")))
}

/// Passes until the game is over; `Stuck` when it is not within 400 answers.
fn to_the_end(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..400 {
        if driver.state().stage() == Stage::Finished {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the end of the game".into()))
}

fn events_of(driver: &Driver) -> &[Event] {
    driver.engine().events()
}

#[rstest]
fn the_draw_step_draws_the_cards_the_rule_names(
    #[values(2, 3, 4)] players: usize,
    #[values(None, Some(1), Some(3), Some(10))] cards: Option<u32>,
) -> Result<(), ScriptError> {
    let house = rules(|r| r.draw_step_cards = cards);
    let mut driver = Driver::from_scenario(
        Scenario::new(players)
            .at(2, 0, Step::Upkeep)
            .house_rules(house),
    )?;
    to_main(&mut driver, 2)?;
    let state = driver.state();
    assert_eq!(hand_of(state, 0), amount(cards));
    assert_eq!(state.zone(Zone::Library(pid(0))).len(), 30 - amount(cards));
    for seat in 1..players {
        assert_eq!(hand_of(state, seat), 0, "only the active player draws");
    }
    Ok(())
}

#[rstest]
fn a_skipped_first_draw_skips_every_draw_of_the_step(
    #[values(2, 4)] players: usize,
    #[values(Some(SkipFirstDraw::Always), Some(SkipFirstDraw::Never), None)] skip: Option<
        SkipFirstDraw,
    >,
    #[values(1, 3, 10)] cards: u32,
) -> Result<(), ScriptError> {
    let house = rules(|r| {
        r.skip_first_draw = skip;
        r.draw_step_cards = Some(cards);
    });
    let mut driver = Driver::from_scenario(
        Scenario::new(players)
            .at(1, 0, Step::Upkeep)
            .house_rules(house),
    )?;
    let skips = match skip {
        Some(SkipFirstDraw::Always) => true,
        Some(SkipFirstDraw::Never) => false,
        None => players == 2,
    };
    driver.pass_until(Step::Main1)?;
    assert_eq!(
        hand_of(driver.state(), 0),
        if skips { 0 } else { cards as usize }
    );
    // Only the first turn is skipped: the next player draws all of theirs.
    to_main(&mut driver, 2)?;
    assert_eq!(hand_of(driver.state(), 1), cards as usize);
    Ok(())
}

/// Plays lands from seat `seat`'s hand until the engine offers no more; returns how many went down.
fn play_lands(driver: &mut Driver) -> usize {
    let mut played = 0;
    while played < 12 && driver.play_land("Forest").is_ok() {
        played += 1;
    }
    played
}

#[rstest]
fn a_player_plays_as_many_lands_each_turn_as_the_rule_allows(
    #[values(2, 3, 4)] players: usize,
    #[values(None, Some(1), Some(2), Some(10))] lands: Option<u32>,
) -> Result<(), ScriptError> {
    let hand = || std::iter::repeat_with(forest).take(12);
    let house = rules(|r| r.land_plays_per_turn = lands);
    let mut driver = Driver::from_scenario(
        Scenario::new(players)
            .hand(0, hand())
            .hand(1, hand())
            .house_rules(house),
    )?;
    assert_eq!(play_lands(&mut driver), amount(lands));
    // One more is refused by the engine, not only left out of the options, and nothing changes.
    let extra = driver.state().zone(Zone::Hand(pid(0)))[0];
    let before = driver.state().state_hash();
    assert!(
        driver
            .answer(Answer::Priority(PriorityAction::PlayLand(extra)))
            .is_err()
    );
    assert_eq!(driver.state().state_hash(), before);
    // The next player has their own turn's worth, and the first player has theirs again later.
    to_main(&mut driver, 2)?;
    assert_eq!(play_lands(&mut driver), amount(lands));
    to_main(&mut driver, players as u32 + 1)?;
    let held = hand_of(driver.state(), 0);
    assert_eq!(play_lands(&mut driver), amount(lands).min(held));
    Ok(())
}

/// A table where seat 0 has `forests` Forests and, if asked, an Ancient Ziggurat (restricted mana) in play.
fn burn_table(players: usize, house: HouseRules, forests: usize, ziggurat: bool) -> Driver {
    let mut mine: Vec<_> = (0..forests).map(|_| ready(forest())).collect();
    if ziggurat {
        mine.push(ready(card("Ancient Ziggurat").unwrap()));
    }
    Driver::from_scenario(
        Scenario::new(players)
            .battlefield(0, mine)
            .battlefield(1, [ready(forest())])
            .house_rules(house),
    )
    .unwrap()
}

fn tap_all(driver: &mut Driver, forests: usize, ziggurat: bool) -> Result<(), ScriptError> {
    for _ in 0..forests {
        driver.tap_for_mana("Forest")?;
    }
    if ziggurat {
        driver.tap_for_mana("Ancient Ziggurat")?;
    }
    Ok(())
}

#[rstest]
#[case::off_by_default(None, 3, false, 0)]
#[case::off_when_set_off(Some(false), 3, false, 0)]
#[case::nothing_left(Some(true), 0, false, 0)]
#[case::one_each_mana(Some(true), 3, false, 3)]
#[case::restricted_mana_counts(Some(true), 0, true, 1)]
#[case::both_kinds(Some(true), 2, true, 3)]
fn mana_left_in_the_pool_costs_life_when_the_step_ends(
    #[values(2, 3, 4)] players: usize,
    #[case] burn: Option<bool>,
    #[case] forests: usize,
    #[case] ziggurat: bool,
    #[case] lost: i32,
) -> Result<(), ScriptError> {
    let mut driver = burn_table(players, rules(|r| r.mana_burn = burn), forests, ziggurat);
    tap_all(&mut driver, forests, ziggurat)?;
    assert_eq!(
        life_of(driver.state(), 0),
        20,
        "mana in the pool costs nothing yet"
    );
    driver.pass_until(Step::BeginCombat)?;
    assert_eq!(life_of(driver.state(), 0), 20 - lost);
    let events = events_of(&driver);
    let burned = events.iter().position(|e| {
        matches!(e, Event::ManaBurned { player, amount } if *player == pid(0) && i64::from(*amount) == i64::from(lost))
    });
    match (lost, burned) {
        (0, found) => assert_eq!(found, None),
        (n, Some(at)) => {
            // The pool empties, the rule is named, and the normal life loss follows.
            assert!(
                matches!(events[at - 1], Event::ManaPoolEmptied { player } if player == pid(0))
            );
            assert!(
                matches!(events[at + 1], Event::LifeLost { player, amount } if player == pid(0) && i64::from(amount) == i64::from(n))
            );
        }
        (n, None) => panic!("{n} mana were left and nothing was burned"),
    }
    for seat in 1..players {
        assert_eq!(life_of(driver.state(), seat), 20);
    }
    Ok(())
}

#[rstest]
fn a_player_who_is_not_the_active_player_burns_too(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), ScriptError> {
    let mut driver = burn_table(players, rules(|r| r.mana_burn = Some(true)), 0, false);
    driver.pass()?;
    // Seat 1 holds priority in seat 0's first main phase and taps its Forest.
    driver.tap_for_mana("Forest")?;
    driver.pass_until(Step::BeginCombat)?;
    assert_eq!(
        (life_of(driver.state(), 0), life_of(driver.state(), 1)),
        (20, 19)
    );
    Ok(())
}

#[rstest]
fn mana_burn_can_end_the_game_and_only_for_a_player_who_cannot_pay_it(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), ScriptError> {
    let house = rules(|r| r.mana_burn = Some(true));
    let mut driver = Driver::from_scenario(
        Scenario::new(players)
            .battlefield(0, (0..3).map(|_| ready(forest())))
            .life(0, 3)
            .house_rules(house),
    )?;
    tap_all(&mut driver, 3, false)?;
    driver.pass_until(Step::BeginCombat).ok();
    assert_eq!(lost_to(driver.state(), 0), Some(LossReason::ZeroLife));
    assert_eq!(
        driver.state().result(),
        (players == 2).then_some(GameResult::Winner(pid(1)))
    );
    Ok(())
}

fn zone_cards(card: &Arc<CardDefinition>, n: usize) -> Vec<Arc<CardDefinition>> {
    vec![Arc::clone(card); n]
}

/// A table at the upkeep of turn 2, where seat 0 draws from `library` cards with `graveyard` cards in the graveyard.
fn empty_library_setup(
    players: usize,
    house: HouseRules,
    library: usize,
    graveyard: usize,
    seed: u64,
) -> GameSetup {
    Scenario::new(players)
        .seed(seed)
        .at(2, 0, Step::Upkeep)
        .library(0, zone_cards(&forest(), library))
        .graveyard(0, zone_cards(&bear(), graveyard))
        .house_rules(house)
        .setup()
        .unwrap()
}

#[rstest]
// The format's rule: an empty library loses.
#[case::the_default_loses(None, 1, 0, 5, None)]
#[case::lose_is_the_default_spelled_out(Some(EmptyLibrary::Lose), 1, 0, 5, None)]
// A reshuffle puts the whole graveyard in and draws.
#[case::reshuffled(Some(EmptyLibrary::ReshuffleGraveyard), 1, 0, 5, Some((1, 4)))]
#[case::with_cards_left_nothing_is_shuffled(Some(EmptyLibrary::ReshuffleGraveyard), 1, 5, 5, Some((1, 4)))]
#[case::the_graveyard_is_empty_too(Some(EmptyLibrary::ReshuffleGraveyard), 1, 0, 0, None)]
// Several draws: those the library gives, then the reshuffled graveyard's.
#[case::a_draw_runs_out_part_way(Some(EmptyLibrary::ReshuffleGraveyard), 3, 1, 5, Some((3, 3)))]
#[case::exactly_enough(Some(EmptyLibrary::ReshuffleGraveyard), 3, 1, 2, Some((3, 0)))]
#[case::one_card_short(Some(EmptyLibrary::ReshuffleGraveyard), 3, 1, 1, None)]
#[case::far_short(Some(EmptyLibrary::ReshuffleGraveyard), 10, 0, 4, None)]
fn an_empty_library_loses_or_reshuffles_the_graveyard(
    #[values(2, 3, 4)] players: usize,
    #[case] rule: Option<EmptyLibrary>,
    #[case] draws: u32,
    #[case] library: usize,
    #[case] graveyard: usize,
    #[case] outcome: Option<(usize, usize)>,
) -> Result<(), ScriptError> {
    let house = rules(|r| {
        r.empty_library = rule;
        r.draw_step_cards = Some(draws);
    });
    let mut driver = Driver::new(Engine::new(empty_library_setup(
        players, house, library, graveyard, 5,
    ))?)?;
    to_main(&mut driver, 2)?;
    let state = driver.state();
    if let Some((hand, in_library)) = outcome {
        assert_eq!(lost_to(state, 0), None);
        assert_eq!(hand_of(state, 0), hand);
        assert_eq!(state.zone(Zone::Library(pid(0))).len(), in_library);
        let reshuffled = events_of(&driver)
            .iter()
            .any(|e| matches!(e, Event::LibraryRefilled { .. }));
        assert_eq!(
            reshuffled,
            library < draws as usize,
            "a shuffle only when one is needed"
        );
        if reshuffled {
            assert_eq!(state.zone(Zone::Graveyard(pid(0))), []);
        }
    } else {
        assert_eq!(lost_to(state, 0), Some(LossReason::EmptyLibrary));
        // Whatever the graveyard could give was drawn before the player lost.
        let drawn = events_of(&driver)
            .iter()
            .filter(|e| matches!(e, Event::CardDrawn { .. }))
            .count();
        let from_graveyard = if rule == Some(EmptyLibrary::ReshuffleGraveyard) {
            graveyard.min((draws as usize).saturating_sub(library))
        } else {
            0
        };
        assert_eq!(drawn, library.min(draws as usize) + from_graveyard);
        assert_eq!(
            state.result(),
            (players == 2).then_some(GameResult::Winner(pid(1)))
        );
    }
    Ok(())
}

fn refilled_order(events: &[Event]) -> Option<Vec<mtg_core::ids::ObjectId>> {
    events.iter().find_map(|event| match event {
        Event::LibraryRefilled { order, .. } => Some(order.clone()),
        _ => None,
    })
}

#[test]
fn the_reshuffle_is_seeded_and_the_log_replays_to_the_same_game() -> Result<(), ScriptError> {
    let house = rules(|r| r.empty_library = Some(EmptyLibrary::ReshuffleGraveyard));
    let mut orders = BTreeSet::new();
    for seed in 0..12 {
        let setup = empty_library_setup(2, house.clone(), 0, 8, seed);
        let mut driver = Driver::new(Engine::new(setup.clone())?)?;
        to_main(&mut driver, 2)?;
        let order = refilled_order(events_of(&driver)).expect("the graveyard was shuffled in");
        assert_eq!(order.len(), 8);
        // The same seed shuffles the same way, and the log alone rebuilds the game.
        let mut again = Driver::new(Engine::new(setup.clone())?)?;
        to_main(&mut again, 2)?;
        assert_eq!(refilled_order(events_of(&again)), Some(order.clone()));
        let replayed = replay(&setup, events_of(&driver)).unwrap();
        assert_eq!(replayed.state_hash(), driver.state().state_hash());
        assert_eq!(
            replayed.zone(Zone::Library(pid(0))),
            driver.state().zone(Zone::Library(pid(0)))
        );
        // The drawn card is the top of the new order.
        assert_eq!(driver.state().zone(Zone::Hand(pid(0))), [order[7]]);
        orders.insert(order);
    }
    assert!(
        orders.len() > 6,
        "other seeds shuffle another way: {}",
        orders.len()
    );
    Ok(())
}

#[test]
fn a_reshuffle_in_a_game_of_bots_replays() -> Result<(), GameFailure> {
    let house = rules(|r| {
        r.empty_library = Some(EmptyLibrary::ReshuffleGraveyard);
        r.draw_step_cards = Some(6);
    });
    let mut shuffled = 0;
    for seed in 0..8 {
        let setup = fuzz_setup(3, seed).with_house_rules(house.clone());
        let first = play_game(setup.clone(), seed)?;
        let second = play_game(setup.clone(), seed)?;
        assert_eq!(first.events, second.events);
        let replayed = replay(&setup, &first.events).unwrap();
        assert_eq!(replayed.state_hash(), first.final_hash);
        shuffled += first
            .events
            .iter()
            .filter(|e| matches!(e, Event::LibraryRefilled { .. }))
            .count();
    }
    assert!(shuffled > 0, "no game ran out of cards");
    Ok(())
}

#[rstest]
// Two seats.
#[case::a_tie_is_a_draw(2, 1, &[20, 20], None)]
#[case::the_first_seat_wins(2, 1, &[20, 15], Some(0))]
#[case::the_second_seat_wins(2, 1, &[15, 20], Some(1))]
#[case::two_rounds_in_two(2, 2, &[3, 4], Some(1))]
// Three and four seats.
#[case::three_seats(3, 2, &[10, 30, 20], Some(1))]
#[case::a_tie_for_the_highest_of_four(4, 1, &[5, 9, 9, 3], None)]
#[case::a_tie_below_the_highest_does_not_matter(4, 1, &[5, 9, 9, 12], Some(3))]
#[case::three_rounds_of_four(4, 3, &[7, 7, 7, 8], Some(3))]
fn the_turn_limit_ends_the_game_at_the_end_of_its_last_round(
    #[case] players: usize,
    #[case] rounds: u32,
    #[case] lives: &[i32],
    #[case] winner: Option<usize>,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(players).house_rules(rules(|r| r.turn_limit = Some(rounds)));
    for (seat, life) in lives.iter().enumerate() {
        scenario = scenario.life(seat, *life);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    to_the_end(&mut driver)?;
    let want = winner.map_or(GameResult::Draw, |seat| GameResult::Winner(pid(seat)));
    assert_eq!(driver.state().result(), Some(want));
    let last = players as u32 * rounds;
    assert_eq!(driver.state().turn().number(), last);
    assert_eq!(driver.state().turn().round(), rounds);
    // The last turn was played to its end: nothing began after it.
    let events = events_of(&driver);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::TurnBegan { number, .. } if *number > last))
    );
    assert!(matches!(events.last(), Some(Event::GameEnded { .. })));
    Ok(())
}

#[rstest]
#[case::the_last_seat_leaves_a_round_of_the_others(4, 3, 3)]
#[case::the_seat_with_the_most_life_is_out_so_it_does_not_count(3, 1, 2)]
#[case::the_first_players_turn_goes_on_without_them(4, 0, 4)]
fn a_seat_that_has_lost_does_not_take_a_turn_or_count(
    #[case] players: usize,
    #[case] leaver: usize,
    #[case] turns: u32,
) -> Result<(), ScriptError> {
    // A seat concedes at once with the most life: the round is one turn of each seat that is left.
    let mut scenario = Scenario::new(players).house_rules(rules(|r| r.turn_limit = Some(1)));
    for seat in 0..players {
        scenario = scenario.life(seat, if seat == leaver { 50 } else { 10 + seat as i32 });
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.concede(pid(leaver))?;
    to_the_end(&mut driver)?;
    assert_eq!(driver.state().turn().number(), turns);
    let best = (0..players)
        .filter(|seat| *seat != leaver)
        .max_by_key(|seat| 10 + *seat as i32)
        .unwrap();
    assert_eq!(driver.state().result(), Some(GameResult::Winner(pid(best))));
    Ok(())
}

#[rstest]
fn a_round_starts_at_whoever_goes_first(
    #[values(2, 3, 4)] players: usize,
    #[values(1, 3)] rounds: u32,
) -> Result<(), ScriptError> {
    let mut firsts = BTreeSet::new();
    for seed in 0..8 {
        let house = rules(|r| r.turn_limit = Some(rounds));
        let mut driver = Driver::new(Engine::new(
            fuzz_setup(players, seed).with_house_rules(house),
        )?)?;
        to_the_end(&mut driver)?;
        let state = driver.state();
        let first = state.first_player();
        firsts.insert(first);
        // Every seat took `rounds` turns, the last of them the seat before the first player's.
        assert_eq!(
            state.turn().number(),
            players as u32 * rounds,
            "seed {seed}"
        );
        assert_eq!(
            state.turn().active().index(),
            (first.index() + players - 1) % players
        );
    }
    assert!(
        firsts.len() > 1,
        "the first player was always seat {firsts:?}"
    );
    Ok(())
}

#[rstest]
#[case::two_seats_at_the_same_life(2, &[20, 20], 20, None)]
#[case::the_first_reaches_it(2, &[30, 20], 30, Some(&[0][..]))]
#[case::three_seats(3, &[20, 30, 25], 30, Some(&[1][..]))]
#[case::beyond_it(3, &[40, 29, 25], 30, Some(&[0][..]))]
#[case::two_at_it_draw(3, &[40, 30, 25], 30, None)]
fn a_player_at_the_winning_life_wins_and_the_others_lose(
    #[case] players: usize,
    #[case] lives: &[i32],
    #[case] goal: i32,
    #[case] winners: Option<&[usize]>,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(players).house_rules(rules(|r| r.winning_life = Some(goal)));
    for (seat, life) in lives.iter().enumerate() {
        scenario = scenario.life(seat, *life);
    }
    let driver = Driver::from_scenario(scenario)?;
    let state = driver.state();
    match winners {
        None => assert_eq!(state.result(), Some(GameResult::Draw)),
        Some(&[winner]) => {
            assert_eq!(state.result(), Some(GameResult::Winner(pid(winner))));
            for seat in (0..players).filter(|seat| *seat != winner) {
                assert_eq!(lost_to(state, seat), Some(LossReason::OpponentReachedLife));
            }
            assert_eq!(lost_to(state, winner), None);
        }
        Some(_) => unreachable!("the table has one winner or none"),
    }
    Ok(())
}

#[test]
fn several_players_at_the_winning_life_draw_and_the_rest_lose() -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(4).house_rules(rules(|r| r.winning_life = Some(30)));
    for (seat, life) in [30, 35, 10, 5].into_iter().enumerate() {
        scenario = scenario.life(seat, life);
    }
    let driver = Driver::from_scenario(scenario)?;
    let state = driver.state();
    assert_eq!(state.result(), Some(GameResult::Draw));
    assert_eq!(lost_to(state, 2), Some(LossReason::OpponentReachedLife));
    assert_eq!(lost_to(state, 3), Some(LossReason::OpponentReachedLife));
    assert!(state.is_alive(pid(0)) && state.is_alive(pid(1)));
    Ok(())
}

#[rstest]
fn a_life_gain_that_reaches_the_winning_life_wins_the_game(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(players)
            .hand(0, [salve("Salve")])
            .battlefield(0, [ready(forest()), ready(forest())])
            .house_rules(rules(|r| r.winning_life = Some(22))),
    )?;
    assert_eq!(driver.state().result(), None, "20 life is not enough");
    let target = driver.target_player(1);
    driver.cast_at("Salve", &[target])?;
    driver.pass()?;
    to_the_end(&mut driver).ok();
    let state = driver.state();
    assert_eq!(life_of(state, 0), 22);
    assert_eq!(state.result(), Some(GameResult::Winner(pid(0))));
    for seat in 1..players {
        assert_eq!(lost_to(state, seat), Some(LossReason::OpponentReachedLife));
    }
    Ok(())
}

#[rstest]
#[case::the_formats_ten_wins_at_ten(None, 10, true)]
#[case::nine_is_not_enough(None, 9, false)]
#[case::three_wins_at_three(Some(3), 3, true)]
#[case::two_of_three(Some(3), 2, false)]
#[case::one_wins_at_one(Some(1), 1, true)]
#[case::none_is_not_poisoned(Some(1), 0, false)]
#[case::a_hundred(Some(100), 99, false)]
#[case::and_at_a_hundred(Some(100), 100, true)]
fn the_poison_threshold_is_the_rules_or_the_formats(
    #[values(2, 3, 4)] players: usize,
    #[case] threshold: Option<u32>,
    #[case] counters: u32,
    #[case] loses: bool,
) -> Result<(), ScriptError> {
    let mut scenario =
        Scenario::new(players).house_rules(rules(|r| r.poison_threshold = threshold));
    if counters > 0 {
        scenario = scenario.counters(0, CounterKind::Poison, counters);
    }
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        lost_to(driver.state(), 0),
        loses.then_some(LossReason::Poison)
    );
    for seat in 1..players {
        assert_eq!(lost_to(driver.state(), seat), None);
    }
    Ok(())
}

/// A card that gives its controller, or each player, no maximum hand size.
fn hand_keeper(who: &str) -> Arc<CardDefinition> {
    let rule = format!(r#"{{"Rule":{{"rule":{{"NoMaxHandSize":"{who}"}}}}}}"#);
    Arc::new(
        CardDefinition::new("Bottomless Satchel")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

/// How many cards seat 0 is asked to discard at the cleanup step of a turn that began with nine in hand; `None` when
/// the turn ends without asking.
fn discard_asked(
    players: usize,
    limit: Option<HandLimit>,
    keeper: Option<(usize, &str)>,
) -> Result<Option<usize>, ScriptError> {
    let mut scenario = Scenario::new(players)
        .at(2, 0, Step::End)
        .hand(0, std::iter::repeat_with(forest).take(9))
        .house_rules(rules(|r| r.max_hand_size = limit));
    if let Some((seat, who)) = keeper {
        scenario = scenario.battlefield(seat, [ready(hand_keeper(who))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..50 {
        if let DecisionKind::Discard(options) = &driver.decision()?.kind {
            return Ok(Some(options.count));
        }
        if driver.state().turn().number() > 2 {
            return Ok(None);
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the cleanup step".into()))
}

#[rstest]
#[case::no_card_the_formats_seven(None, None, Some(2))]
#[case::no_card_a_house_limit(Some(HandLimit::Cards(5)), None, Some(4))]
#[case::the_card_lifts_the_formats_limit(None, Some((0, "You")), None)]
#[case::the_card_lifts_a_house_limit(Some(HandLimit::Cards(0)), Some((0, "You")), None)]
#[case::the_card_for_every_player(Some(HandLimit::Cards(5)), Some((0, "EachPlayer")), None)]
#[case::the_card_of_another_seat_for_its_own_player(None, Some((1, "You")), Some(2))]
#[case::the_card_of_another_seat_for_every_player(Some(HandLimit::Cards(3)), Some((1, "EachPlayer")), None)]
fn the_card_rule_of_no_maximum_hand_size_beats_the_limit(
    #[values(2, 3, 4)] players: usize,
    #[case] limit: Option<HandLimit>,
    #[case] keeper: Option<(usize, &str)>,
    #[case] asked: Option<usize>,
) -> Result<(), ScriptError> {
    assert_eq!(discard_asked(players, limit, keeper)?, asked);
    Ok(())
}

/// The mulligan question and the opening hand of every seat, with each seat keeping its first hand.
fn opening_hands(
    players: usize,
    house: HouseRules,
) -> Result<Vec<(PlayerId, usize, u32, bool)>, ScriptError> {
    let mut driver = Driver::new(Engine::new(fuzz_setup(players, 3).with_house_rules(house))?)?;
    let mut seen = Vec::new();
    while let DecisionKind::Mulligan(options) = &driver.decision()?.kind {
        let player = driver.decision()?.player;
        seen.push((
            player,
            options.hand.len(),
            options.mulligans_taken,
            options.can_mulligan,
        ));
        driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    }
    Ok(seen)
}

#[rstest]
fn a_handicap_sets_life_and_the_hand_of_its_seat_only(
    #[values(2, 3, 4)] players: usize,
    #[values(None, Some(40))] table_life: Option<i32>,
) -> Result<(), ScriptError> {
    let house = HouseRules {
        starting_life: table_life,
        handicaps: vec![
            Handicap {
                seat: pid(0),
                starting_life: None,
                opening_hand_size: Some(4),
            },
            Handicap {
                seat: pid(1),
                starting_life: Some(5),
                opening_hand_size: Some(9),
            },
        ],
        ..HouseRules::default()
    };
    let driver = Driver::new(Engine::new(
        fuzz_setup(players, 3).with_house_rules(house.clone()),
    )?)?;
    for seat in 0..players {
        let want = match seat {
            1 => 5,
            _ => table_life.unwrap_or(20),
        };
        assert_eq!(life_of(driver.state(), seat), want, "seat {seat}");
    }
    let mut hands: Vec<(usize, usize)> = opening_hands(players, house)?
        .into_iter()
        .map(|(player, hand, _, _)| (player.index(), hand))
        .collect();
    hands.sort_unstable();
    let want: Vec<(usize, usize)> = (0..players)
        .map(|seat| (seat, [4, 9, 7, 7][seat]))
        .collect();
    assert_eq!(hands, want);
    Ok(())
}

#[rstest]
fn the_mulligan_of_a_seat_uses_its_own_hand_size_for_the_cap_and_the_bottom(
    #[values(2, 3, 4)] players: usize,
    #[values(0, 1)] handicapped: usize,
) -> Result<(), ScriptError> {
    let house = rules(|r| {
        r.handicaps = vec![Handicap {
            seat: pid(handicapped),
            starting_life: None,
            opening_hand_size: Some(3),
        }];
    });
    let mut driver = Driver::new(Engine::new(fuzz_setup(players, 8).with_house_rules(house))?)?;
    let (mut offers, mut bottoms) = (Vec::new(), Vec::new());
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Mulligan(options) if decision.player == pid(handicapped) => {
                offers.push((
                    options.hand.len(),
                    options.can_mulligan,
                    options.bottom_if_keep,
                    options.bottom_if_mulligan,
                ));
                let again = options.can_mulligan;
                driver.answer(Answer::Mulligan(if again {
                    MulliganChoice::Mulligan
                } else {
                    MulliganChoice::Keep
                }))?;
            }
            DecisionKind::BottomCards(options) => {
                if decision.player == pid(handicapped) {
                    bottoms.push(options.count);
                }
                let cards = options.hand.iter().copied().take(options.count).collect();
                driver.answer(Answer::BottomCards(cards))?;
            }
            DecisionKind::Mulligan(_) => {
                driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
            }
            _ => break,
        }
    }
    // A hand of three can be mulliganed three times, each hand of three, and the keep bottoms what was taken.
    assert_eq!(
        offers,
        [
            (3, true, 0, 1),
            (3, true, 1, 2),
            (3, true, 2, 3),
            (3, false, 3, 4)
        ]
    );
    assert_eq!(bottoms, [3]);
    assert_eq!(hand_of(driver.state(), handicapped), 0);
    Ok(())
}

#[rstest]
#[case::the_seat_after_the_last(rules(|r| r.handicaps = vec![Handicap { seat: pid(2), starting_life: Some(5), opening_hand_size: None }]), 2)]
#[case::far_beyond(rules(|r| r.handicaps = vec![Handicap { seat: pid(1), starting_life: Some(5), opening_hand_size: None }, Handicap { seat: pid(99), starting_life: Some(5), opening_hand_size: None }]), 2)]
fn a_handicap_for_a_seat_the_table_lacks_refuses_the_setup(
    #[case] house: HouseRules,
    #[case] players: usize,
) {
    let setup = fuzz_setup(players, 1).with_house_rules(house);
    for refused in [
        setup.validate().err(),
        GameState::from_setup(&setup).err(),
        Engine::new(setup).err().and_then(|e| match e {
            mtg_engine::EngineError::Setup(source) => Some(source),
            _ => None,
        }),
    ] {
        assert!(
            matches!(
                refused,
                Some(SetupError::HouseRules(HouseRuleError::NoSuchSeat { .. }))
            ),
            "{refused:?}"
        );
    }
}

#[rstest]
#[case::draws(rules(|r| r.draw_step_cards = Some(11)), "draw_step_cards")]
#[case::lands(rules(|r| r.land_plays_per_turn = Some(0)), "land_plays_per_turn")]
#[case::limit(rules(|r| r.turn_limit = Some(1001)), "turn_limit")]
#[case::wins(rules(|r| r.winning_life = Some(0)), "winning_life")]
#[case::poison(rules(|r| r.poison_threshold = Some(101)), "poison_threshold")]
#[case::handicap_life(rules(|r| r.handicaps = vec![Handicap { seat: pid(0), starting_life: Some(0), opening_hand_size: None }]), "handicaps[0].starting_life")]
#[case::handicap_hand(rules(|r| r.handicaps = vec![Handicap { seat: pid(0), starting_life: None, opening_hand_size: Some(21) }]), "handicaps[0].opening_hand_size")]
#[case::the_same_seat_twice(rules(|r| r.handicaps = vec![Handicap { seat: pid(0), starting_life: Some(5), opening_hand_size: None }, Handicap { seat: pid(0), starting_life: None, opening_hand_size: Some(5) }]), "handicaps")]
fn a_rule_out_of_range_refuses_the_setup(#[case] house: HouseRules, #[case] field: &str) {
    let setup = fuzz_setup(2, 1).with_house_rules(house);
    match setup.validate() {
        Err(SetupError::HouseRules(error)) => assert_eq!(error.field(), field),
        other => panic!("expected {field} to be refused, got {other:?}"),
    }
    assert!(Engine::new(setup).is_err());
}

#[rstest]
#[case::draws(rules(|r| r.draw_step_cards = Some(2)))]
#[case::lands(rules(|r| r.land_plays_per_turn = Some(2)))]
#[case::burn(rules(|r| r.mana_burn = Some(true)))]
#[case::library(rules(|r| r.empty_library = Some(EmptyLibrary::ReshuffleGraveyard)))]
#[case::limit(rules(|r| r.turn_limit = Some(9)))]
#[case::wins(rules(|r| r.winning_life = Some(40)))]
#[case::poison(rules(|r| r.poison_threshold = Some(5)))]
#[case::handicap(rules(|r| r.handicaps = vec![Handicap { seat: pid(1), starting_life: Some(9), opening_hand_size: None }]))]
#[case::a_value_equal_to_the_formats(rules(|r| r.poison_threshold = Some(10)))]
fn other_rules_of_the_turn_make_another_game(
    #[case] house: HouseRules,
) -> Result<(), mtg_engine::EngineError> {
    let hash = |house: HouseRules| -> Result<u64, mtg_engine::EngineError> {
        Ok(Engine::new(fuzz_setup(2, 9).with_house_rules(house))?
            .state()
            .state_hash())
    };
    assert_ne!(hash(HouseRules::default())?, hash(house)?);
    Ok(())
}

fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(100, |n| (n / 5).max(1))
}

#[rstest]
#[case::draws(rules(|r| r.draw_step_cards = Some(4)))]
#[case::lands(rules(|r| r.land_plays_per_turn = Some(5)))]
#[case::burn(rules(|r| r.mana_burn = Some(true)))]
#[case::reshuffle(rules(|r| { r.empty_library = Some(EmptyLibrary::ReshuffleGraveyard); r.draw_step_cards = Some(8); }))]
#[case::limit(rules(|r| r.turn_limit = Some(3)))]
#[case::wins(rules(|r| r.winning_life = Some(26)))]
#[case::poison(rules(|r| r.poison_threshold = Some(1)))]
#[case::handicaps(rules(|r| r.handicaps = vec![Handicap { seat: pid(0), starting_life: Some(3), opening_hand_size: Some(2) }, Handicap { seat: pid(1), starting_life: Some(60), opening_hand_size: None }]))]
#[case::all_at_once(rules(|r| {
    r.draw_step_cards = Some(3);
    r.land_plays_per_turn = Some(3);
    r.mana_burn = Some(true);
    r.empty_library = Some(EmptyLibrary::ReshuffleGraveyard);
    r.turn_limit = Some(6);
    r.winning_life = Some(45);
    r.poison_threshold = Some(2);
    r.handicaps = vec![Handicap { seat: pid(1), starting_life: Some(15), opening_hand_size: Some(5) }];
}))]
fn bot_games_under_each_rule_end_with_the_invariants_kept(
    #[values(2, 3, 4)] players: usize,
    #[case] house: HouseRules,
) {
    for_each_seed(0..games(), |seed| {
        let setup = fuzz_setup(players, seed).with_house_rules(house.clone());
        play_game(setup, seed)
            .unwrap_or_else(|failure| panic!("{players} seats, seed {seed}, {house:?}: {failure}"));
    });
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn a_game_under_any_rules_of_the_turn_and_life_plays_to_its_end(
        house in any_turn_rules(),
        players in 2usize..=4,
        seed in 0u64..10_000,
    ) {
        let outcome = play_game(fuzz_setup(players, seed).with_house_rules(house), seed);
        prop_assert!(outcome.is_ok(), "{:?}", outcome.err());
    }
}
