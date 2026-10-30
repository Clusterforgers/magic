//! Goal: the opening asks every seat at once, and any mix of keep and mulligan answers in any arrival order reaches the state that answering in turn order reaches; the log keeps the arrival order and replays to the same state, also from the middle of an opening.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, Decision, DecisionKind, MulliganChoice};
use mtg_engine::state::Stage;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, GameSetup, replay};
use mtg_testkit::{ScriptError, commander_fuzz_setup, fuzz_setup};
use proptest::prelude::*;
use rstest::rstest;

/// The two tables an opening differs on: the London mulligan alone, and Commander with its free first mulligan.
#[derive(Debug, Clone, Copy)]
enum Table {
    Plain,
    Commander,
}

fn setup(table: Table, seats: usize, seed: u64) -> Result<GameSetup, ScriptError> {
    Ok(match table {
        Table::Plain => fuzz_setup(seats, seed),
        Table::Commander => commander_fuzz_setup(seats, seed)
            .map_err(|e| ScriptError::Expectation(e.to_string()))?,
    })
}

/// The keep-or-mulligan answer of `seat` in a mix: bit `seat` set means a mulligan.
fn choice(mix: u32, seat: PlayerId) -> Answer {
    Answer::Mulligan(if mix >> seat.index() & 1 == 1 {
        MulliganChoice::Mulligan
    } else {
        MulliganChoice::Keep
    })
}

/// Every order of `items` (Heap's algorithm).
fn permutations(items: &[PlayerId]) -> Vec<Vec<PlayerId>> {
    fn heap(k: usize, items: &mut Vec<PlayerId>, out: &mut Vec<Vec<PlayerId>>) {
        if k <= 1 {
            out.push(items.clone());
            return;
        }
        for i in 0..k {
            heap(k - 1, items, out);
            let j = if k % 2 == 0 { i } else { 0 };
            items.swap(j, k - 1);
        }
    }
    let mut out = Vec::new();
    heap(items.len(), &mut items.to_vec(), &mut out);
    out
}

fn asked(engine: &Engine) -> Vec<PlayerId> {
    engine.decisions().iter().map(|d| d.player).collect()
}

/// The hand of every seat, in seat order.
fn hands(engine: &Engine) -> Vec<Vec<ObjectId>> {
    (0..engine.state().players().len())
        .map(|seat| {
            engine
                .state()
                .zone(Zone::Hand(PlayerId::from_index(seat)))
                .to_vec()
        })
        .collect()
}

#[rstest]
fn every_seat_is_asked_at_once_and_the_first_is_the_engines_decision(
    #[values(Table::Plain, Table::Commander)] table: Table,
    #[values(2, 3, 4, 6)] seats: usize,
) -> Result<(), ScriptError> {
    let engine = Engine::new(setup(table, seats, 7)?)?;
    let decisions = engine.decisions();
    let first = engine.state().first_player();
    let order: Vec<PlayerId> = engine.state().turn_order_from(first);
    assert_eq!(
        decisions.iter().map(|d| d.player).collect::<Vec<_>>(),
        order
    );
    assert_eq!(engine.decision(), decisions.first());
    for Decision { player, kind } in &decisions {
        let DecisionKind::Mulligan(options) = kind else {
            panic!("{player:?} is asked {}", kind.name());
        };
        assert_eq!(
            options.hand,
            engine.state().zone(Zone::Hand(*player)),
            "only its own hand"
        );
    }
    Ok(())
}

/// At six seats every order is checked through the sets of seats that answered: if the state after a set is the same
/// whichever of its seats answered last, then by induction every order of arrival reaches the same state, and so the
/// 720 orders cost 192 answers per mix.
#[rstest]
fn at_six_seats_any_mix_in_any_order_matches_answering_in_turn_order(
    #[values(Table::Plain, Table::Commander)] table: Table,
) -> Result<(), ScriptError> {
    let setup = setup(table, 6, 11)?;
    let start = Engine::new(setup.clone())?;
    let order = asked(&start);
    for mix in 0..1u32 << 6 {
        // The game after each set of seats answered; the empty set is the start.
        let mut reached: Vec<Engine> = vec![start.clone()];
        for set in 1..1usize << 6 {
            let mut hashes = Vec::new();
            for (bit, seat) in order.iter().enumerate() {
                if set >> bit & 1 == 0 {
                    continue;
                }
                let mut last = reached[set & !(1 << bit)].clone();
                last.apply_for(*seat, choice(mix, *seat))?;
                hashes.push(last.state_hash());
                if hashes.len() == 1 {
                    reached.push(last);
                }
            }
            assert!(
                hashes.iter().all(|h| *h == hashes[0]),
                "mix {mix:b}, set {set:b}: {hashes:?}"
            );
        }
    }
    Ok(())
}

#[rstest]
fn any_mix_in_any_order_matches_answering_in_turn_order(
    #[values(Table::Plain, Table::Commander)] table: Table,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = setup(table, seats, 11)?;
    let mut engine = Engine::new(setup.clone())?;
    let start = engine.checkpoint();
    let order = asked(&engine);
    let orders = permutations(&order);
    for mix in 0..1u32 << seats {
        engine.restore(start.clone())?;
        for seat in &order {
            engine.apply(choice(mix, *seat))?;
        }
        let (expected, expected_hands) = (engine.state_hash(), hands(&engine));
        for arrival in &orders {
            engine.restore(start.clone())?;
            for (at, seat) in arrival.iter().enumerate() {
                let mut waiting = asked(&engine);
                waiting.sort_unstable();
                let mut left = arrival[at..].to_vec();
                left.sort_unstable();
                assert_eq!(waiting, left, "every seat not yet answered is asked");
                engine.apply_for(*seat, choice(mix, *seat))?;
            }
            assert_eq!(
                engine.state_hash(),
                expected,
                "mix {mix:b}, arrival {arrival:?}"
            );
            assert_eq!(hands(&engine), expected_hands);
            let answers: Vec<PlayerId> = engine
                .events()
                .iter()
                .filter_map(|event| match event {
                    Event::MulliganKept { player } | Event::MulliganDeclared { player } => {
                        Some(*player)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                &answers, arrival,
                "the log keeps the order the answers came in"
            );
        }
    }
    let replayed = replay(&setup, engine.events())?;
    assert_eq!(replayed.state_hash(), engine.state_hash());
    Ok(())
}

#[rstest]
fn a_seat_with_nothing_to_answer_is_refused(
    #[values(Table::Plain, Table::Commander)] table: Table,
) -> Result<(), ScriptError> {
    let mut engine = Engine::new(setup(table, 4, 3)?)?;
    let last = *asked(&engine).last().ok_or(EngineError::NoDecision)?;
    engine.apply_for(last, choice(0, last))?;
    let before = engine.events().len();
    assert!(matches!(
        engine.apply_for(last, choice(0, last)),
        Err(EngineError::NoDecision)
    ));
    assert!(matches!(
        engine.apply_for(PlayerId(9), choice(0, last)),
        Err(EngineError::NoDecision)
    ));
    assert_eq!(engine.events().len(), before, "a refusal changes nothing");
    Ok(())
}

#[rstest]
fn an_opening_resumed_from_its_log_asks_the_same_seats_and_ends_the_same(
    #[values(Table::Plain, Table::Commander)] table: Table,
    #[values(2, 3, 4, 6)] seats: usize,
    #[values(0, 1)] answered: usize,
) -> Result<(), ScriptError> {
    let setup = setup(table, seats, 5)?;
    let mut live = Engine::new(setup.clone())?;
    // The last seats answer first, the first of them with a mulligan.
    let order = asked(&live);
    for (at, seat) in order.iter().rev().take(answered + 1).enumerate() {
        live.apply_for(*seat, choice(u32::from(at == 0) << seat.index(), *seat))?;
    }
    let mut resumed = Engine::resume(setup, live.events().to_vec())?;
    assert_eq!(resumed.decisions(), live.decisions());
    assert_eq!(resumed.state_hash(), live.state_hash());
    finish_opening(&mut live, &[])?;
    finish_opening(&mut resumed, &[])?;
    assert_eq!(resumed.state_hash(), live.state_hash());
    assert_eq!(resumed.events(), live.events());
    Ok(())
}

/// One seat's script: whether it takes a mulligan after each number of mulligans taken, and which of its hand it bottoms.
#[derive(Debug, Clone)]
struct Script {
    mulligans: Vec<bool>,
    rotate: usize,
}

fn answer_of(script: &Script, decision: &Decision) -> Answer {
    match &decision.kind {
        DecisionKind::Mulligan(options) => {
            let again = script
                .mulligans
                .get(options.mulligans_taken as usize)
                .copied()
                .unwrap_or(false);
            Answer::Mulligan(if again && options.can_mulligan {
                MulliganChoice::Mulligan
            } else {
                MulliganChoice::Keep
            })
        }
        DecisionKind::BottomCards(options) => {
            let mut hand = options.hand.clone();
            let turn = script.rotate % hand.len().max(1);
            hand.rotate_left(turn);
            Answer::BottomCards(hand.into_iter().take(options.count).collect())
        }
        other => panic!("the opening asks {}", other.name()),
    }
}

/// Answers the opening to its end: each pick chooses, among the seats asked, the one whose answer arrives next.
fn finish_opening(engine: &mut Engine, picks: &[usize]) -> Result<(), EngineError> {
    let keep = Script {
        mulligans: Vec::new(),
        rotate: 0,
    };
    for at in 0.. {
        if engine.state().stage() != Stage::Mulligan || at > 200 {
            break;
        }
        let decisions = engine.decisions();
        let pick = picks.get(at).copied().unwrap_or(0) % decisions.len().max(1);
        let Some(decision) = decisions.get(pick) else {
            break;
        };
        engine.apply_for(decision.player, answer_of(&keep, decision))?;
    }
    Ok(())
}

/// Plays the opening with `scripts`, the next answer coming from the seat `picks` names among those asked (always the
/// engine's own decision when `picks` is `None`); returns the hands and the state hash once turn one begins.
fn play(
    setup: &GameSetup,
    scripts: &[Script],
    picks: Option<&[usize]>,
) -> Result<(Vec<Vec<ObjectId>>, u64), EngineError> {
    let mut engine = Engine::new(setup.clone())?;
    for at in 0..400 {
        if engine.state().stage() != Stage::Mulligan {
            break;
        }
        let decisions = engine.decisions();
        let pick = picks.map_or(0, |picks| picks.get(at).copied().unwrap_or(0));
        let Some(decision) = decisions.get(pick % decisions.len().max(1)) else {
            break;
        };
        let answer = answer_of(&scripts[decision.player.index()], decision);
        engine.apply_for(decision.player, answer)?;
    }
    assert_ne!(engine.state().stage(), Stage::Mulligan, "the opening ends");
    Ok((hands(&engine), engine.state_hash()))
}

fn scripts(seats: usize) -> impl Strategy<Value = Vec<Script>> {
    prop::collection::vec(
        (prop::collection::vec(any::<bool>(), 0..4), 0usize..8)
            .prop_map(|(mulligans, rotate)| Script { mulligans, rotate }),
        seats,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn sequential_and_concurrent_answers_reach_identical_hands(
        (seats, scripts) in (2usize..=6).prop_flat_map(|seats| (Just(seats), scripts(seats))),
        picks in prop::collection::vec(0usize..6, 0..40),
        commander in any::<bool>(),
        seed in 0u64..1_000,
    ) {
        let table = if commander { Table::Commander } else { Table::Plain };
        let setup = setup(table, seats, seed).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let fail = |e: EngineError| TestCaseError::fail(e.to_string());
        let sequential = play(&setup, &scripts, None).map_err(fail)?;
        let concurrent = play(&setup, &scripts, Some(&picks)).map_err(fail)?;
        prop_assert_eq!(sequential, concurrent);
    }
}
