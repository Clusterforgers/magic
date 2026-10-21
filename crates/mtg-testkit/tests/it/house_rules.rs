//! Goal: each house rule changes its own part of the game and nothing else (starting life, the opening hand, free and capped mulligans, the first draw, the hand limit at cleanup), a set rule wins over the format and an unset one keeps its value, a refused rule stops the setup, the rules are part of the hash and the replay, and whole bot games under random rules end with an invariant-checked result.

use crate::common::pid;
use crate::common::seeds::for_each_seed;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::house::{HandLimit, HouseRules, SkipFirstDraw};
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice, MulliganOptions};
use mtg_engine::error::SetupError;
use mtg_engine::format::{DeckError, FormatRules, MulliganRule};
use mtg_engine::rng::GameRng;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, GameSetup, GameState, replay};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, GameFailure, Scenario, ScriptError, fuzz_setup, play_game};
use proptest::prelude::*;
use rstest::rstest;

fn rules(set: impl FnOnce(&mut HouseRules)) -> HouseRules {
    let mut rules = HouseRules::default();
    set(&mut rules);
    rules
}

fn opening(players: usize, seed: u64, house: HouseRules) -> Result<Driver, ScriptError> {
    Driver::new(Engine::new(
        fuzz_setup(players, seed).with_house_rules(house),
    )?)
}

/// What the first player went through before turn one began.
struct Opening {
    /// Every keep-or-mulligan question the first player was asked.
    offers: Vec<MulliganOptions>,
    /// How many cards the first player was asked to bottom, if they were asked.
    bottomed: Option<usize>,
}

/// Plays the opening: the first player takes up to `takes` mulligans and bottoms the first cards asked for, everybody
/// else keeps.
fn open(driver: &mut Driver, takes: u32) -> Result<Opening, ScriptError> {
    let first = driver.state().first_player();
    let mut done = Opening {
        offers: Vec::new(),
        bottomed: None,
    };
    for _ in 0..500 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Mulligan(options) => {
                let mulligan = decision.player == first && options.mulligans_taken < takes;
                if decision.player == first {
                    done.offers.push(options.clone());
                }
                driver.answer(Answer::Mulligan(if mulligan && options.can_mulligan {
                    MulliganChoice::Mulligan
                } else {
                    MulliganChoice::Keep
                }))?;
            }
            DecisionKind::BottomCards(options) => {
                done.bottomed = Some(options.count);
                let cards = options.hand.iter().copied().take(options.count).collect();
                driver.answer(Answer::BottomCards(cards))?;
            }
            _ => return Ok(done),
        }
    }
    Err(ScriptError::Stuck("the opening".into()))
}

fn hand_size(state: &GameState, player: PlayerId) -> usize {
    state.zone(Zone::Hand(player)).len()
}

#[rstest]
fn starting_life_follows_the_rule(
    #[values(2, 3, 4)] players: usize,
    #[values(None, Some(1), Some(40), Some(1000))] life: Option<i32>,
) -> Result<(), ScriptError> {
    let driver = opening(players, 3, rules(|r| r.starting_life = life))?;
    let lives: Vec<i32> = driver.state().players().iter().map(|p| p.life()).collect();
    assert_eq!(lives, vec![life.unwrap_or(20); players]);
    Ok(())
}

#[rstest]
fn every_opening_hand_follows_the_rule(
    #[values(2, 4)] players: usize,
    #[values(None, Some(1), Some(5), Some(7), Some(20))] size: Option<u32>,
) -> Result<(), ScriptError> {
    let mut driver = opening(players, 5, rules(|r| r.opening_hand_size = size))?;
    let mut asked = 0;
    while let DecisionKind::Mulligan(options) = &driver.decision()?.kind {
        assert_eq!(options.hand.len(), size.map_or(7, |n| n as usize));
        asked += 1;
        driver.answer(Answer::Mulligan(MulliganChoice::Keep))?;
    }
    assert_eq!(asked, players);
    Ok(())
}

#[rstest]
#[case::the_formats_rule(None, None, 3, Some(3), 4)]
#[case::one_free_mulligan(None, Some(1), 3, Some(2), 5)]
#[case::as_many_free_as_taken(None, Some(3), 3, None, 7)]
#[case::more_free_than_taken(None, Some(5), 2, None, 7)]
#[case::every_mulligan_free(None, Some(100), 6, None, 7)]
#[case::no_mulligan(None, Some(1), 0, None, 7)]
#[case::a_small_hand(Some(3), None, 2, Some(2), 1)]
#[case::a_small_hand_all_free(Some(3), Some(100), 2, None, 3)]
#[case::a_large_hand(Some(20), Some(2), 5, Some(3), 17)]
fn a_mulligan_costs_a_card_unless_it_is_free(
    #[case] hand: Option<u32>,
    #[case] free: Option<u32>,
    #[case] takes: u32,
    #[case] bottomed: Option<usize>,
    #[case] kept: usize,
) -> Result<(), ScriptError> {
    let house = rules(|r| {
        r.opening_hand_size = hand;
        r.free_mulligans = free;
    });
    let mut driver = opening(2, 11, house)?;
    let first = driver.state().first_player();
    let done = open(&mut driver, takes)?;
    assert_eq!(done.bottomed, bottomed);
    assert_eq!(hand_size(driver.state(), first), kept);
    // Whatever the rule, the offers state what the bottom step then asks for.
    let last = done
        .offers
        .last()
        .map(|o| (o.mulligans_taken, o.bottom_if_keep));
    assert_eq!(last, Some((takes, bottomed.unwrap_or(0))));
    Ok(())
}

#[rstest]
#[case::no_mulligan_at_all(0, 1)]
#[case::one(1, 2)]
#[case::three(3, 4)]
fn a_cap_removes_the_mulligan_once_it_is_reached(
    #[case] cap: u32,
    #[case] offers: usize,
) -> Result<(), ScriptError> {
    let mut driver = opening(2, 7, rules(|r| r.mulligan_cap = Some(cap)))?;
    let done = open(&mut driver, 99)?;
    let can: Vec<bool> = done.offers.iter().map(|o| o.can_mulligan).collect();
    assert_eq!(done.offers.len(), offers);
    assert_eq!(can.last(), Some(&false));
    assert!(can[..can.len() - 1].iter().all(|can| *can));
    Ok(())
}

#[test]
fn a_mulligan_over_the_cap_is_refused_and_the_question_stands() -> Result<(), ScriptError> {
    let mut driver = opening(2, 7, rules(|r| r.mulligan_cap = Some(0)))?;
    let before = driver.decision()?.clone();
    assert!(matches!(&before.kind, DecisionKind::Mulligan(o) if !o.can_mulligan));
    assert!(
        driver
            .answer(Answer::Mulligan(MulliganChoice::Mulligan))
            .is_err()
    );
    assert_eq!(driver.decision()?, &before);
    Ok(())
}

#[rstest]
#[case::the_formats_two_player_rule_in_two(2, None, true)]
#[case::the_formats_rule_in_four(4, None, false)]
#[case::always_in_two(2, Some(SkipFirstDraw::Always), true)]
#[case::always_in_four(4, Some(SkipFirstDraw::Always), true)]
#[case::never_in_two(2, Some(SkipFirstDraw::Never), false)]
#[case::never_in_four(4, Some(SkipFirstDraw::Never), false)]
fn the_first_player_skips_the_first_draw_by_the_rule(
    #[case] players: usize,
    #[case] rule: Option<SkipFirstDraw>,
    #[case] skips: bool,
) -> Result<(), ScriptError> {
    let mut driver = opening(players, 4, rules(|r| r.skip_first_draw = rule))?;
    open(&mut driver, 0)?;
    driver.idle_until(1, Step::Main1)?;
    let first = driver.state().first_player();
    assert_eq!(hand_size(driver.state(), first), 7 + usize::from(!skips));
    for seat in (0..players).map(pid).filter(|seat| *seat != first) {
        assert_eq!(hand_size(driver.state(), seat), 7);
    }
    Ok(())
}

/// How many cards the active player is asked to discard at the cleanup step of a turn that began with nine in hand;
/// `None` when the turn ends without asking.
fn discard_at_cleanup(limit: Option<HandLimit>) -> Result<(Driver, Option<usize>), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::End)
            .hand(0, std::iter::repeat_with(forest).take(9))
            .house_rules(rules(|r| r.max_hand_size = limit)),
    )?;
    for _ in 0..50 {
        let asked = match &driver.decision()?.kind {
            DecisionKind::Discard(options) => Some(options.count),
            _ => None,
        };
        if asked.is_some() || driver.state().turn().number() > 2 {
            return Ok((driver, asked));
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the cleanup step".into()))
}

#[rstest]
#[case::the_formats_seven(None, Some(2))]
#[case::seven(Some(HandLimit::Cards(7)), Some(2))]
#[case::eight(Some(HandLimit::Cards(8)), Some(1))]
#[case::nine_holds_the_hand(Some(HandLimit::Cards(9)), None)]
#[case::fifty(Some(HandLimit::Cards(50)), None)]
#[case::none_at_all(Some(HandLimit::Cards(0)), Some(9))]
#[case::unlimited(Some(HandLimit::Unlimited), None)]
fn the_hand_limit_decides_what_cleanup_asks_to_discard(
    #[case] limit: Option<HandLimit>,
    #[case] asked: Option<usize>,
) -> Result<(), ScriptError> {
    let (mut driver, got) = discard_at_cleanup(limit)?;
    assert_eq!(got, asked);
    let Some(count) = asked else {
        assert_eq!(hand_size(driver.state(), pid(0)), 9, "nobody discards");
        return Ok(());
    };
    let cards = match &driver.decision()?.kind {
        DecisionKind::Discard(options) => options.hand.iter().copied().take(count).collect(),
        _ => Vec::new(),
    };
    driver.answer(Answer::Discard(cards))?;
    assert_eq!(hand_size(driver.state(), pid(0)), 9 - count);
    Ok(())
}

/// A format of its own: 30 life, five-card hands with a free mulligan, a skipped first draw in every game and a hand
/// of nine at cleanup.
#[derive(Debug)]
struct Roomy;

impl FormatRules for Roomy {
    fn name(&self) -> &'static str {
        "roomy"
    }

    fn starting_life(&self) -> i32 {
        30
    }

    fn validate_deck(&self, _deck: &[Arc<CardDefinition>]) -> Result<(), DeckError> {
        Ok(())
    }

    fn mulligan_rule(&self) -> MulliganRule {
        MulliganRule {
            hand_size: 5,
            free_mulligans: 1,
        }
    }

    fn skips_first_draw(&self, _player_count: usize) -> bool {
        true
    }

    fn max_hand_size(&self) -> usize {
        9
    }
}

fn roomy_game(house: HouseRules) -> Result<Driver, ScriptError> {
    let decks = (0..3)
        .map(|_| (0..30).map(|_| free_creature("Bear", 2, 2)).collect())
        .collect();
    Driver::new(Engine::new(
        GameSetup::new(Arc::new(Roomy), 6, decks).with_house_rules(house),
    )?)
}

#[rstest]
#[case::nothing_set(HouseRules::default(), 30, 5, 1)]
#[case::life_set(rules(|r| r.starting_life = Some(12)), 12, 5, 1)]
#[case::hand_set(rules(|r| r.opening_hand_size = Some(6)), 30, 6, 1)]
#[case::free_set(rules(|r| r.free_mulligans = Some(0)), 30, 5, 0)]
#[case::values_equal_to_the_formats(rules(|r| { r.starting_life = Some(30); r.opening_hand_size = Some(5); r.free_mulligans = Some(1); }), 30, 5, 1)]
fn a_set_rule_wins_over_the_format_and_an_unset_one_keeps_its_value(
    #[case] house: HouseRules,
    #[case] life: i32,
    #[case] hand: usize,
    #[case] free: u32,
) -> Result<(), ScriptError> {
    let driver = roomy_game(house)?;
    assert!(driver.state().players().iter().all(|p| p.life() == life));
    match &driver.decision()?.kind {
        DecisionKind::Mulligan(options) => {
            assert_eq!((options.hand.len(), options.free_mulligans), (hand, free));
        }
        other => panic!("expected the mulligan, got {}", other.name()),
    }
    Ok(())
}

#[rstest]
#[case::never_over_the_format_that_always_skips(Some(SkipFirstDraw::Never), 6)]
#[case::always_agrees_with_the_format(Some(SkipFirstDraw::Always), 5)]
#[case::unset_keeps_the_format(None, 5)]
fn a_first_draw_rule_wins_over_a_format_that_always_skips(
    #[case] rule: Option<SkipFirstDraw>,
    #[case] first_hand: usize,
) -> Result<(), ScriptError> {
    let mut driver = roomy_game(rules(|r| r.skip_first_draw = rule))?;
    open(&mut driver, 0)?;
    driver.idle_until(1, Step::Main1)?;
    let first = driver.state().first_player();
    assert_eq!(hand_size(driver.state(), first), first_hand);
    Ok(())
}

#[rstest]
#[case::life_low(rules(|r| r.starting_life = Some(0)), "starting_life")]
#[case::life_high(rules(|r| r.starting_life = Some(1001)), "starting_life")]
#[case::hand_empty(rules(|r| r.opening_hand_size = Some(0)), "opening_hand_size")]
#[case::hand_large(rules(|r| r.opening_hand_size = Some(21)), "opening_hand_size")]
#[case::free(rules(|r| r.free_mulligans = Some(101)), "free_mulligans")]
#[case::cap(rules(|r| r.mulligan_cap = Some(101)), "mulligan_cap")]
#[case::limit(rules(|r| r.max_hand_size = Some(HandLimit::Cards(51))), "max_hand_size")]
fn a_rule_out_of_range_refuses_the_setup(
    #[case] house: HouseRules,
    #[case] field: &str,
) -> Result<(), ScriptError> {
    let setup = fuzz_setup(2, 1).with_house_rules(house);
    for refused in [
        setup.validate().err(),
        GameState::from_setup(&setup).err(),
        Engine::new(setup).err().and_then(|e| match e {
            EngineError::Setup(source) => Some(source),
            _ => None,
        }),
    ] {
        match refused {
            Some(SetupError::HouseRules(error)) => assert_eq!(error.field(), field),
            other => panic!("expected the rule {field} to be refused, got {other:?}"),
        }
    }
    Ok(())
}

/// The house rules as a random table sets them: each rule is left out about half the time.
fn random_rules(seed: u64) -> HouseRules {
    let mut rng = GameRng::substream(seed, 9_000);
    let mut some = |bound: u64| (rng.below(2) == 0).then(|| rng.below(bound));
    let (life, hand, free, cap, limit) = (some(1000), some(20), some(101), some(101), some(52));
    HouseRules {
        starting_life: life.and_then(|n| i32::try_from(n + 1).ok()),
        opening_hand_size: hand.and_then(|n| u32::try_from(n + 1).ok()),
        free_mulligans: free.and_then(|n| u32::try_from(n).ok()),
        mulligan_cap: cap.and_then(|n| u32::try_from(n).ok()),
        skip_first_draw: match rng.below(3) {
            0 => Some(SkipFirstDraw::Always),
            1 => Some(SkipFirstDraw::Never),
            _ => None,
        },
        max_hand_size: limit.and_then(|n| {
            if n == 51 {
                Some(HandLimit::Unlimited)
            } else {
                u32::try_from(n).ok().map(HandLimit::Cards)
            }
        }),
    }
}

#[test]
fn random_rules_are_always_in_range_and_often_set() {
    let mut set = 0;
    for seed in 0..300 {
        let house = random_rules(seed);
        assert_eq!(house.validate(), Ok(()), "seed {seed}");
        set += usize::from(!house.is_empty());
    }
    assert!(set > 250, "only {set} of 300 tables set a rule");
}

/// Bot games played under random rules per table size; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(100, |n| (n / 5).max(1))
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(4)]
fn bot_games_under_random_house_rules_end_with_the_invariants_kept(#[case] players: usize) {
    for_each_seed(0..games(), |seed| {
        let setup = fuzz_setup(players, seed).with_house_rules(random_rules(seed));
        play_game(setup, seed).unwrap_or_else(|failure| {
            panic!(
                "{players} seats, seed {seed}, {:?}: {failure}",
                random_rules(seed)
            )
        });
    });
}

fn any_rules() -> impl Strategy<Value = HouseRules> {
    (
        proptest::option::of(1..=1000i32),
        proptest::option::of(1..=20u32),
        proptest::option::of(0..=100u32),
        proptest::option::of(0..=100u32),
        proptest::option::of(prop_oneof![
            Just(SkipFirstDraw::Always),
            Just(SkipFirstDraw::Never)
        ]),
        proptest::option::of(prop_oneof![
            (0..=50u32).prop_map(HandLimit::Cards),
            Just(HandLimit::Unlimited)
        ]),
    )
        .prop_map(|(life, hand, free, cap, first, limit)| HouseRules {
            starting_life: life,
            opening_hand_size: hand,
            free_mulligans: free,
            mulligan_cap: cap,
            skip_first_draw: first,
            max_hand_size: limit,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn a_game_under_any_valid_rules_plays_to_its_end(
        house in any_rules(),
        players in 2usize..=4,
        seed in 0u64..10_000,
    ) {
        let outcome = play_game(fuzz_setup(players, seed).with_house_rules(house), seed);
        prop_assert!(outcome.is_ok(), "{:?}", outcome.err());
    }
}

#[rstest]
fn the_same_seed_and_rules_make_the_same_game_and_its_log_replays(
    #[values(2, 3, 4)] players: usize,
    #[values(0, 7, 31)] seed: u64,
) -> Result<(), GameFailure> {
    let house = random_rules(seed.wrapping_add(1));
    let setup = || fuzz_setup(players, seed).with_house_rules(house);
    let first = play_game(setup(), seed)?;
    let second = play_game(setup(), seed)?;
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    let replayed = replay(&setup(), &first.events).unwrap();
    assert_eq!(replayed.state_hash(), first.final_hash);
    assert_eq!(replayed.house_rules(), &house);
    Ok(())
}

#[rstest]
#[case::life(rules(|r| r.starting_life = Some(21)))]
#[case::hand(rules(|r| r.opening_hand_size = Some(6)))]
#[case::free(rules(|r| r.free_mulligans = Some(1)))]
#[case::cap(rules(|r| r.mulligan_cap = Some(3)))]
#[case::first_draw(rules(|r| r.skip_first_draw = Some(SkipFirstDraw::Never)))]
#[case::hand_limit(rules(|r| r.max_hand_size = Some(HandLimit::Cards(8))))]
#[case::unlimited(rules(|r| r.max_hand_size = Some(HandLimit::Unlimited)))]
#[case::a_value_equal_to_the_formats(rules(|r| r.starting_life = Some(20)))]
fn other_rules_make_another_game(#[case] house: HouseRules) -> Result<(), EngineError> {
    let hash = |house: HouseRules| -> Result<u64, EngineError> {
        Ok(Engine::new(fuzz_setup(2, 9).with_house_rules(house))?
            .state()
            .state_hash())
    };
    assert_eq!(hash(HouseRules::default())?, hash(HouseRules::default())?);
    assert_ne!(hash(HouseRules::default())?, hash(house)?);
    Ok(())
}

#[test]
fn a_table_with_no_rule_plays_the_game_it_always_did() -> Result<(), GameFailure> {
    let plain = play_game(fuzz_setup(2, 4), 4)?;
    let empty = play_game(fuzz_setup(2, 4).with_house_rules(HouseRules::default()), 4)?;
    assert_eq!(plain.events, empty.events);
    assert_eq!(plain.final_hash, empty.final_hash);
    Ok(())
}
