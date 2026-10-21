//! Goal: each house rule of the table changes its own part of the game at two to five seats: the attack range limits the players and planeswalkers an attacker is offered over the seats still in the game, the mulligan styles size the hand, the bottom and the scry, opening-hand smoothing keeps the hand closer to the land share of the library with a seeded and replayable shuffle, and the rules sit beside the format's own mulligan and first draw; plus the other rules that make another game, and bot games with every rule.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation,
    clippy::too_many_lines
)]

use crate::common::house::any_table_rules;
use crate::common::pid;
use crate::common::seeds::for_each_seed;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::house::{AttackRange, HouseRules, MulliganStyle};
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice};
use mtg_engine::effects::Duration;
use mtg_engine::error::SetupError;
use mtg_engine::format::{DeckError, FormatRules, MulliganRule, Normal};
use mtg_engine::player::MulliganStatus;
use mtg_engine::rules::Rule;
use mtg_engine::state::Stage;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, GameSetup, GameState, replay};
use mtg_testkit::cards::{bear, forest};
use mtg_testkit::loyalty_cards::dawn_walker;
use mtg_testkit::{Driver, GameFailure, Scenario, ScriptError, fuzz_setup, play_game, ready};
use proptest::prelude::*;
use rstest::rstest;

fn rules(set: impl FnOnce(&mut HouseRules)) -> HouseRules {
    let mut rules = HouseRules::default();
    set(&mut rules);
    rules
}

fn library_of(state: &GameState, seat: usize) -> &[ObjectId] {
    state.zone(Zone::Library(pid(seat)))
}

fn hand_of(state: &GameState, seat: usize) -> &[ObjectId] {
    state.zone(Zone::Hand(pid(seat)))
}

fn is_land(state: &GameState, card: ObjectId) -> bool {
    state
        .object(card)
        .is_some_and(|object| object.base_types().contains(CardType::Land))
}

// The attack range

/// The seats of the players and of the planeswalkers' controllers the attacker of seat 0 is offered.
fn defenders(driver: &Driver) -> (Vec<usize>, Vec<usize>) {
    let DecisionKind::DeclareAttackers(options) = &driver.decision().unwrap().kind else {
        panic!("attackers were expected");
    };
    let all = &options.attackers[0].defenders;
    let players = all
        .iter()
        .filter_map(|defender| match defender {
            AttackTarget::Player(seat) => Some(seat.index()),
            AttackTarget::Planeswalker(_) => None,
        })
        .collect();
    let walkers = all
        .iter()
        .filter_map(|defender| match defender {
            AttackTarget::Planeswalker(id) => driver
                .state()
                .object(*id)
                .map(|walker| walker.controller().index()),
            AttackTarget::Player(_) => None,
        })
        .collect();
    (players, walkers)
}

/// Seat 0 holds a creature and every other seat a planeswalker, at seat 0's declare attackers step.
fn at_the_attack(players: usize, range: Option<AttackRange>) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(players)
        .at(2, 0, Step::DeclareAttackers)
        .house_rules(rules(|r| r.attack_range = range))
        .battlefield(0, [ready(mtg_testkit::cards::free_creature("A", 2, 2))]);
    for seat in 1..players {
        scenario = scenario.battlefield(seat, [ready(dawn_walker().unwrap())]);
    }
    Driver::from_scenario(scenario)
}

#[rstest]
#[case::two_seats(2, &[], &[1], &[1], &[1])]
#[case::three_seats(3, &[], &[1, 2], &[1, 2], &[1])]
#[case::four_seats(4, &[], &[1, 2, 3], &[1, 3], &[1])]
#[case::five_seats(5, &[], &[1, 2, 3, 4], &[1, 4], &[1])]
#[case::the_next_seat_left(5, &[1], &[2, 3, 4], &[2, 4], &[2])]
#[case::the_previous_seat_left(5, &[4], &[1, 2, 3], &[1, 3], &[1])]
#[case::both_neighbours_left(5, &[1, 4], &[2, 3], &[2, 3], &[2])]
#[case::a_seat_between_left(4, &[2], &[1, 3], &[1, 3], &[1])]
#[case::two_left_of_four(4, &[1, 2], &[3], &[3], &[3])]
#[case::two_left_of_five(5, &[1, 2, 3], &[4], &[4], &[4])]
fn the_attacker_is_offered_the_players_and_planeswalkers_the_range_names(
    #[case] players: usize,
    #[case] gone: &[usize],
    #[case] any: &[usize],
    #[case] neighbours: &[usize],
    #[case] left: &[usize],
) -> Result<(), ScriptError> {
    for (range, wanted) in [
        (None, any),
        (Some(AttackRange::Any), any),
        (Some(AttackRange::Neighbours), neighbours),
        (Some(AttackRange::Left), left),
    ] {
        let mut driver = at_the_attack(players, range)?;
        for seat in gone {
            driver.concede(pid(*seat))?;
        }
        let (seats, walkers) = defenders(&driver);
        assert_eq!(seats, wanted, "{players} seats, {range:?}: players");
        assert_eq!(walkers, wanted, "{players} seats, {range:?}: planeswalkers");
    }
    Ok(())
}

#[rstest]
fn the_attack_range_sees_the_turn_order_of_the_active_seat(
    #[values(3, 4, 5)] players: usize,
    #[values(Some(AttackRange::Neighbours), Some(AttackRange::Left))] range: Option<AttackRange>,
) -> Result<(), ScriptError> {
    // Seat 1 attacks: its next seat is 2 and its previous one 0, whatever the table's first seat is.
    let mut scenario = Scenario::new(players)
        .at(2, 1, Step::DeclareAttackers)
        .house_rules(rules(|r| r.attack_range = range))
        .battlefield(1, [ready(mtg_testkit::cards::free_creature("A", 2, 2))]);
    for seat in [0, 2, 3, 4].into_iter().filter(|seat| *seat < players) {
        scenario = scenario.battlefield(seat, [ready(dawn_walker().unwrap())]);
    }
    let driver = Driver::from_scenario(scenario)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("attackers were expected");
    };
    let seats: Vec<usize> = options.attackers[0]
        .defenders
        .iter()
        .filter_map(|defender| match defender {
            AttackTarget::Player(seat) => Some(seat.index()),
            AttackTarget::Planeswalker(_) => None,
        })
        .collect();
    let wanted: &[usize] = if range == Some(AttackRange::Left) {
        &[2]
    } else {
        &[2, 0]
    };
    assert_eq!(seats, wanted);
    Ok(())
}

#[rstest]
fn an_attack_outside_the_range_is_refused_and_one_inside_it_is_made(
    #[values(3, 4)] players: usize,
    #[values(AttackRange::Neighbours, AttackRange::Left)] range: AttackRange,
) -> Result<(), ScriptError> {
    let mut driver = at_the_attack(players, Some(range))?;
    let attacker = driver.id_of("A")?;
    // Seat 2 is never the left player; it is a neighbour only at a table of three.
    let outside = Answer::DeclareAttackers(vec![(attacker, AttackTarget::Player(pid(2)))]);
    let outside_is_legal = players == 3 && range == AttackRange::Neighbours;
    assert_eq!(driver.answer(outside).is_ok(), outside_is_legal);
    if !outside_is_legal {
        driver.attack(&[("A", 1)])?;
        assert_eq!(driver.state().combat().attackers().len(), 1);
    }
    Ok(())
}

#[rstest]
fn a_creature_that_must_attack_attacks_a_player_in_the_range(
    #[values(3, 4, 5)] players: usize,
    #[values(Some(AttackRange::Neighbours), Some(AttackRange::Left), None)] range: Option<
        AttackRange,
    >,
) -> Result<(), ScriptError> {
    let mut driver = at_the_attack(players, range)?;
    let attacker = driver.id_of("A")?;
    let reference = driver.state().object(attacker).unwrap().reference();
    driver.add_rule(Rule::MustAttack(vec![reference]), Duration::Forever)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("attackers were expected");
    };
    assert_eq!(options.required, [attacker]);
    let forced = options.required_attack();
    assert_eq!(forced, [(attacker, AttackTarget::Player(pid(1)))]);
    assert!(driver.answer(Answer::DeclareAttackers(Vec::new())).is_err());
    driver.answer(Answer::DeclareAttackers(forced))?;
    assert_eq!(driver.state().combat().attackers().len(), 1);
    Ok(())
}

// The mulligan styles

/// What a table's openings asked: for each Mulligan question the seat, the hand, the mulligans taken, the size of the
/// next hand, the bottom count and the scry of a keep.
type Offer = (usize, usize, u32, Option<usize>, usize, Option<usize>);

struct Opening {
    offers: Vec<Offer>,
    bottoms: Vec<(usize, usize)>,
    scries: Vec<usize>,
}

/// Plays the opening of `setup` with every seat taking `mulligans` mulligans and then keeping; a seat scries its card to
/// the bottom when its seat is even.
fn open(setup: GameSetup, mulligans: u32) -> Result<(Driver, Opening), ScriptError> {
    let mut driver = Driver::new(Engine::new(setup)?)?;
    let mut opening = Opening {
        offers: Vec::new(),
        bottoms: Vec::new(),
        scries: Vec::new(),
    };
    while driver.state().stage() == Stage::Mulligan {
        let decision = driver.decision()?.clone();
        let seat = decision.player.index();
        match decision.kind {
            DecisionKind::Mulligan(options) => {
                opening.offers.push((
                    seat,
                    options.hand.len(),
                    options.mulligans_taken,
                    options.hand_if_mulligan,
                    options.bottom_if_keep,
                    options.scry_if_keep,
                ));
                let again = options.can_mulligan && options.mulligans_taken < mulligans;
                driver.answer(Answer::Mulligan(if again {
                    MulliganChoice::Mulligan
                } else {
                    MulliganChoice::Keep
                }))?;
            }
            DecisionKind::BottomCards(options) => {
                opening.bottoms.push((seat, options.count));
                let cards = options.hand.iter().copied().take(options.count).collect();
                driver.answer(Answer::BottomCards(cards))?;
            }
            DecisionKind::ChooseCards(options) => {
                opening.scries.push(seat);
                assert_eq!((options.from.len(), options.min, options.max), (1, 0, 1));
                let top = *library_of(driver.state(), seat).last().unwrap();
                assert_eq!(options.from, [top], "the card asked about is the top card");
                let bottom = seat % 2 == 0;
                driver.answer(Answer::ChooseCards(CardsChoice(if bottom {
                    options.from
                } else {
                    Vec::new()
                })))?;
                let library = library_of(driver.state(), seat);
                if bottom {
                    assert_eq!(library.first(), Some(&top), "the card went to the bottom");
                } else {
                    assert_eq!(library.last(), Some(&top), "the card stayed on top");
                }
            }
            other => panic!("unexpected {} in the opening", other.name()),
        }
    }
    Ok((driver, opening))
}

/// The mulligans a seat pays for after `taken` of them, `free` being free.
fn paid(taken: u32, free: u32) -> usize {
    taken.saturating_sub(free) as usize
}

#[rstest]
fn each_style_deals_bottoms_and_scries_what_it_names(
    #[values(2, 3, 4, 5)] players: usize,
    #[values(
        None,
        Some(MulliganStyle::London),
        Some(MulliganStyle::Paris),
        Some(MulliganStyle::Vancouver)
    )]
    style: Option<MulliganStyle>,
    #[values(0, 1)] free: u32,
    #[values(0, 1, 3)] mulligans: u32,
) -> Result<(), ScriptError> {
    let house = rules(|r| {
        r.mulligan_style = style;
        r.free_mulligans = Some(free);
    });
    let (driver, opening) = open(fuzz_setup(players, 5).with_house_rules(house), mulligans)?;
    let london = matches!(style, None | Some(MulliganStyle::London));
    let vancouver = style == Some(MulliganStyle::Vancouver);
    // The size of the hand a seat is dealt after `taken` mulligans.
    let dealt = |taken: u32| if london { 7 } else { 7 - paid(taken, free) };
    let bottom = |taken: u32| if london { paid(taken, free) } else { 0 };
    let scry = |taken: u32| (vancouver && taken > 0).then_some(1);
    for (seat, hand, taken, next, bottom_if_keep, scry_if_keep) in &opening.offers {
        assert_eq!(*hand, dealt(*taken), "seat {seat} after {taken}");
        assert_eq!(*next, Some(dealt(taken + 1)), "seat {seat} after {taken}");
        assert_eq!(*bottom_if_keep, bottom(*taken));
        assert_eq!(*scry_if_keep, scry(*taken));
    }
    assert_eq!(opening.offers.len(), players * (mulligans as usize + 1));
    let counts = |seats: &[(usize, usize)]| seats.iter().map(|(_, n)| *n).sum::<usize>();
    assert_eq!(counts(&opening.bottoms), players * bottom(mulligans));
    assert_eq!(
        opening.scries.len(),
        players * usize::from(scry(mulligans).is_some())
    );
    for seat in 0..players {
        let kept = 7 - paid(mulligans, free);
        assert_eq!(hand_of(driver.state(), seat).len(), kept, "seat {seat}");
        assert_eq!(
            library_of(driver.state(), seat).len(),
            seat_deck(players, seat) - kept
        );
        assert_eq!(
            driver
                .state()
                .player(pid(seat))
                .map(|p| p.mulligan_status()),
            Some(MulliganStatus::Kept)
        );
    }
    Ok(())
}

fn seat_deck(players: usize, seat: usize) -> usize {
    let setup = fuzz_setup(players, 5);
    setup.players[seat].library.len()
}

#[rstest]
fn the_cap_and_the_hand_size_of_a_seat_bound_the_mulligans_of_every_style(
    #[values(MulliganStyle::Paris, MulliganStyle::Vancouver)] style: MulliganStyle,
    #[values(0, 2, 7, 20)] cap: u32,
) -> Result<(), ScriptError> {
    let house = rules(|r| {
        r.mulligan_style = Some(style);
        r.mulligan_cap = Some(cap);
    });
    let (driver, opening) = open(fuzz_setup(2, 3).with_house_rules(house), 50)?;
    // A cap above the seven cards of a hand cannot be reached; the hand shrinks to nothing at the last mulligan.
    let most = cap.min(7);
    assert!(opening.offers.iter().all(|offer| offer.2 <= most));
    assert_eq!(hand_of(driver.state(), 0).len(), 7 - most as usize);
    Ok(())
}

/// The format of a game with a free first mulligan, as Commander plays it, and no rule of its own for the rest.
#[derive(Debug)]
struct OneFree;

impl FormatRules for OneFree {
    fn name(&self) -> &'static str {
        "one-free"
    }

    fn starting_life(&self) -> i32 {
        Normal.starting_life()
    }

    fn validate_deck(&self, _deck: &[Arc<CardDefinition>]) -> Result<(), DeckError> {
        Ok(())
    }

    fn mulligan_rule(&self) -> MulliganRule {
        MulliganRule {
            hand_size: 7,
            free_mulligans: 1,
        }
    }

    fn skips_first_draw(&self, player_count: usize) -> bool {
        Normal.skips_first_draw(player_count)
    }
}

fn one_free_setup(players: usize, seed: u64, house: HouseRules) -> GameSetup {
    let decks = (0..players)
        .map(|_| (0..40).map(|_| bear()).collect())
        .collect();
    GameSetup::new(Arc::new(OneFree), seed, decks).with_house_rules(house)
}

#[rstest]
fn a_style_keeps_the_free_mulligans_of_the_format(
    #[values(2, 3, 4)] players: usize,
    #[values(None, Some(MulliganStyle::Paris), Some(MulliganStyle::Vancouver))] style: Option<
        MulliganStyle,
    >,
    #[values(0, 1, 2, 3)] mulligans: u32,
) -> Result<(), ScriptError> {
    let house = rules(|r| r.mulligan_style = style);
    let (driver, opening) = open(one_free_setup(players, 4, house), mulligans)?;
    let shrink = paid(mulligans, 1);
    for seat in 0..players {
        assert_eq!(
            hand_of(driver.state(), seat).len(),
            7 - shrink,
            "seat {seat}"
        );
    }
    let bottomed: usize = opening.bottoms.iter().map(|(_, n)| *n).sum();
    assert_eq!(bottomed, if style.is_none() { players * shrink } else { 0 });
    // The first mulligan of the format is free in every style, so the first Paris hand is full.
    let second = opening.offers.iter().find(|offer| offer.2 == 1);
    assert_eq!(second.map(|offer| offer.1), (mulligans > 0).then_some(7));
    Ok(())
}

#[rstest]
fn a_house_value_for_the_free_mulligans_replaces_the_formats_in_every_style(
    #[values(MulliganStyle::London, MulliganStyle::Paris)] style: MulliganStyle,
    #[values(0, 2)] free: u32,
) -> Result<(), ScriptError> {
    let house = rules(|r| {
        r.mulligan_style = Some(style);
        r.free_mulligans = Some(free);
    });
    let (driver, _) = open(one_free_setup(3, 2, house), 2)?;
    assert_eq!(hand_of(driver.state(), 1).len(), 7 - paid(2, free));
    Ok(())
}

/// Plays the opening with every seat keeping, then passes to the first main step of turn 1.
fn first_main(setup: GameSetup) -> Result<Driver, ScriptError> {
    let mut driver = Driver::new(Engine::new(setup)?)?;
    for _ in 0..400 {
        let state = driver.state();
        if state.turn().number() == 1 && state.turn().step() == Step::Main1 {
            return Ok(driver);
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the first main step".into()))
}

#[rstest]
fn the_first_player_draws_or_skips_the_first_draw_after_any_opening(
    #[values(2, 3, 4)] players: usize,
    #[values(None, Some(MulliganStyle::Paris), Some(MulliganStyle::Vancouver))] style: Option<
        MulliganStyle,
    >,
    #[values(false, true)] smoothing: bool,
) -> Result<(), ScriptError> {
    let house = rules(|r| {
        r.mulligan_style = style;
        r.opening_hand_smoothing = smoothing.then_some(true);
    });
    let driver = first_main(fuzz_setup(players, 8).with_house_rules(house))?;
    let first = driver.state().first_player().index();
    for seat in 0..players {
        let drew = usize::from(seat == first && players > 2);
        assert_eq!(hand_of(driver.state(), seat).len(), 7 + drew, "seat {seat}");
    }
    Ok(())
}

// Opening-hand smoothing

/// The libraries as the pregame shuffle left them and the hands dealt, for each seat: the base order (bottom to top),
/// the order of the shuffle that precedes the deal if there was one, and the hand.
fn deals(
    events: &[Event],
    players: usize,
) -> Vec<Vec<(Vec<ObjectId>, Option<Vec<ObjectId>>, Vec<ObjectId>)>> {
    let mut base: Vec<Option<Vec<ObjectId>>> = vec![None; players];
    let mut pending: Vec<Option<Vec<ObjectId>>> = vec![None; players];
    let mut out = vec![Vec::new(); players];
    for event in events {
        match event {
            Event::LibraryShuffled { player, order } => {
                let seat = player.index();
                if base[seat].is_none() {
                    base[seat] = Some(order.clone());
                } else {
                    pending[seat] = Some(order.clone());
                }
            }
            Event::HandDealt { player, cards } => {
                let seat = player.index();
                out[seat].push((
                    base[seat].clone().unwrap(),
                    pending[seat].take(),
                    cards.clone(),
                ));
            }
            Event::MulliganTaken {
                player,
                library_order,
            } => {
                base[player.index()] = Some(library_order.clone());
            }
            _ => {}
        }
    }
    out
}

/// Whether seat 0 of `state` keeps the second hand: its land count is strictly closer to the share of lands in
/// its library, as whole numbers scaled by the library's size.
fn second_is_kept(state: &GameState, base: &[ObjectId], size: usize) -> (bool, usize) {
    let first: Vec<ObjectId> = base.iter().rev().take(size).copied().collect();
    let second: Vec<ObjectId> = base.iter().rev().skip(size).take(size).copied().collect();
    let lands = |cards: &[ObjectId]| cards.iter().filter(|c| is_land(state, **c)).count();
    let total = lands(base);
    let distance = |cards: &[ObjectId]| (lands(cards) * base.len()).abs_diff(total * size);
    (
        distance(&second) < distance(&first),
        distance(&first).abs_diff(distance(&second)),
    )
}

#[rstest]
fn the_hand_kept_is_the_one_closer_to_the_land_share_and_a_tie_takes_the_first(
    #[values(2, 3, 4, 5)] players: usize,
) -> Result<(), ScriptError> {
    let (mut first_kept, mut second_kept) = (0, 0);
    for seed in 0..40 {
        let house = rules(|r| r.opening_hand_smoothing = Some(true));
        let engine = Engine::new(fuzz_setup(players, seed).with_house_rules(house))?;
        for (seat, dealt) in deals(engine.events(), players).iter().enumerate() {
            let (base, shuffle, hand) = &dealt[0];
            let shuffle = shuffle.as_ref().expect("a library of sixty is smoothed");
            let (second, _) = second_is_kept(engine.state(), base, 7);
            let top: Vec<ObjectId> = base.iter().rev().take(7).copied().collect();
            let next: Vec<ObjectId> = base.iter().rev().skip(7).take(7).copied().collect();
            assert_eq!(
                hand,
                if second { &next } else { &top },
                "seed {seed} seat {seat}"
            );
            // The kept hand is back on top of a library that holds the same cards, the rest shuffled.
            let mut a = shuffle.clone();
            let mut b = base.clone();
            a.sort_unstable();
            b.sort_unstable();
            assert_eq!(a, b);
            let on_top: Vec<ObjectId> = shuffle.iter().rev().take(7).copied().collect();
            assert_eq!(&on_top, hand);
            // What stays in the library after the deal is the shuffled rest, the unused hand among it.
            assert_eq!(library_of(engine.state(), seat).len(), base.len() - 7);
            first_kept += usize::from(!second);
            second_kept += usize::from(second);
        }
    }
    assert!(
        first_kept > 0 && second_kept > 0,
        "{first_kept} first, {second_kept} second"
    );
    Ok(())
}

#[rstest]
fn equal_hands_are_a_tie_that_keeps_the_first(
    #[values(2, 4)] players: usize,
    #[values(true, false)] lands: bool,
) -> Result<(), ScriptError> {
    let decks = (0..players)
        .map(|_| {
            (0..60)
                .map(|_| if lands { forest() } else { bear() })
                .collect()
        })
        .collect();
    let mut setup = GameSetup::new(Arc::new(Normal), 6, decks)
        .with_house_rules(rules(|r| r.opening_hand_smoothing = Some(true)));
    setup.validate_decks = false;
    let engine = Engine::new(setup)?;
    for dealt in deals(engine.events(), players) {
        let (base, _, hand) = &dealt[0];
        let top: Vec<ObjectId> = base.iter().rev().take(7).copied().collect();
        assert_eq!(hand, &top);
    }
    Ok(())
}

#[test]
fn a_library_too_small_for_two_hands_is_not_smoothed() -> Result<(), ScriptError> {
    let decks = (0..2).map(|_| (0..13).map(|_| bear()).collect()).collect();
    let mut setup = GameSetup::new(Arc::new(Normal), 3, decks)
        .with_house_rules(rules(|r| r.opening_hand_smoothing = Some(true)));
    setup.validate_decks = false;
    let engine = Engine::new(setup)?;
    let shuffles = engine
        .events()
        .iter()
        .filter(|event| matches!(event, Event::LibraryShuffled { .. }))
        .count();
    assert_eq!(shuffles, 2, "one shuffle for each seat, none for the deal");
    assert_eq!(hand_of(engine.state(), 0).len(), 7);
    Ok(())
}

#[rstest]
fn every_deal_after_a_mulligan_is_smoothed_again(
    #[values(2, 3)] players: usize,
    #[values(MulliganStyle::London, MulliganStyle::Paris)] style: MulliganStyle,
) -> Result<(), ScriptError> {
    let house = rules(|r| {
        r.opening_hand_smoothing = Some(true);
        r.mulligan_style = Some(style);
    });
    let (driver, _) = open(fuzz_setup(players, 11).with_house_rules(house), 2)?;
    let dealt = deals(driver.engine().events(), players);
    for (seat, deals) in dealt.iter().enumerate() {
        assert_eq!(deals.len(), 3, "seat {seat} was dealt three hands");
        for (index, (base, shuffle, hand)) in deals.iter().enumerate() {
            let size = if style == MulliganStyle::London {
                7
            } else {
                7 - index
            };
            assert_eq!(hand.len(), size);
            if size > 0 && base.len() >= 2 * size {
                assert!(shuffle.is_some(), "seat {seat} hand {index}");
            }
        }
    }
    Ok(())
}

#[rstest]
fn the_smoothed_deal_is_seeded(#[values(2, 4)] players: usize) -> Result<(), ScriptError> {
    let house = rules(|r| r.opening_hand_smoothing = Some(true));
    let hands = |seed: u64| -> Result<Vec<Vec<ObjectId>>, ScriptError> {
        let engine = Engine::new(fuzz_setup(players, seed).with_house_rules(house.clone()))?;
        Ok((0..players)
            .map(|seat| hand_of(engine.state(), seat).to_vec())
            .collect())
    };
    assert_eq!(hands(5)?, hands(5)?);
    assert!((6..12).any(|seed| hands(seed).unwrap() != hands(5).unwrap()));
    Ok(())
}

#[rstest]
fn a_game_under_the_rules_of_the_table_replays_from_its_log(
    #[values(2, 3, 4)] players: usize,
    #[values(0, 7, 31)] seed: u64,
) -> Result<(), GameFailure> {
    let house = rules(|r| {
        r.opening_hand_smoothing = Some(true);
        r.mulligan_style = Some(MulliganStyle::Vancouver);
        r.attack_range = Some(AttackRange::Neighbours);
        r.open_hands = Some(true);
        r.revealed_library_top = Some(true);
    });
    let setup = || fuzz_setup(players, seed).with_house_rules(house.clone());
    let first = play_game(setup(), seed)?;
    let second = play_game(setup(), seed)?;
    assert_eq!(first.events, second.events);
    assert_eq!(first.final_hash, second.final_hash);
    let replayed = replay(&setup(), &first.events).unwrap();
    assert_eq!(replayed.state_hash(), first.final_hash);
    assert!(
        first
            .events
            .iter()
            .any(|e| matches!(e, Event::HandDealt { .. })),
        "the opening is in the log"
    );
    Ok(())
}

// The rules as a whole

#[rstest]
#[case::range(rules(|r| r.attack_range = Some(AttackRange::Left)))]
#[case::range_any(rules(|r| r.attack_range = Some(AttackRange::Any)))]
#[case::open(rules(|r| r.open_hands = Some(true)))]
#[case::top(rules(|r| r.revealed_library_top = Some(true)))]
#[case::smoothing(rules(|r| r.opening_hand_smoothing = Some(true)))]
#[case::paris(rules(|r| r.mulligan_style = Some(MulliganStyle::Paris)))]
#[case::london(rules(|r| r.mulligan_style = Some(MulliganStyle::London)))]
fn other_rules_of_the_table_make_another_game(
    #[case] house: HouseRules,
) -> Result<(), EngineError> {
    let hash = |house: HouseRules| -> Result<u64, EngineError> {
        Ok(Engine::new(fuzz_setup(2, 9).with_house_rules(house))?
            .state()
            .state_hash())
    };
    assert_ne!(hash(HouseRules::default())?, hash(house)?);
    Ok(())
}

#[rstest]
fn every_rule_of_the_table_is_valid_at_every_table_size(
    #[values(2, 3, 4, 5)] players: usize,
    #[values(AttackRange::Any, AttackRange::Neighbours, AttackRange::Left)] range: AttackRange,
    #[values(MulliganStyle::London, MulliganStyle::Paris, MulliganStyle::Vancouver)]
    style: MulliganStyle,
) {
    let house = rules(|r| {
        r.attack_range = Some(range);
        r.mulligan_style = Some(style);
        r.open_hands = Some(true);
        r.revealed_library_top = Some(true);
        r.opening_hand_smoothing = Some(true);
    });
    let setup = fuzz_setup(players, 1).with_house_rules(house);
    assert_eq!(setup.validate(), Ok::<(), SetupError>(()));
}

fn games() -> u64 {
    std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(100, |n| (n / 5).max(1))
}

#[rstest]
#[case::neighbours(rules(|r| r.attack_range = Some(AttackRange::Neighbours)))]
#[case::left(rules(|r| r.attack_range = Some(AttackRange::Left)))]
#[case::open_hands(rules(|r| r.open_hands = Some(true)))]
#[case::revealed_tops(rules(|r| r.revealed_library_top = Some(true)))]
#[case::smoothing(rules(|r| r.opening_hand_smoothing = Some(true)))]
#[case::paris(rules(|r| r.mulligan_style = Some(MulliganStyle::Paris)))]
#[case::vancouver(rules(|r| r.mulligan_style = Some(MulliganStyle::Vancouver)))]
#[case::vancouver_with_free_ones_and_a_cap(rules(|r| {
    r.mulligan_style = Some(MulliganStyle::Vancouver);
    r.free_mulligans = Some(2);
    r.mulligan_cap = Some(4);
    r.opening_hand_size = Some(5);
}))]
#[case::all_at_once(rules(|r| {
    r.attack_range = Some(AttackRange::Left);
    r.open_hands = Some(true);
    r.revealed_library_top = Some(true);
    r.opening_hand_smoothing = Some(true);
    r.mulligan_style = Some(MulliganStyle::Vancouver);
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
    fn a_game_under_any_rules_of_the_table_plays_to_its_end(
        house in any_table_rules(),
        players in 2usize..=5,
        seed in 0u64..10_000,
    ) {
        let outcome = play_game(fuzz_setup(players, seed).with_house_rules(house), seed);
        prop_assert!(outcome.is_ok(), "{:?}", outcome.err());
    }
}
