//! Goal: whole bot games whose creatures cost hybrid and Phyrexian symbols, paid by bots that choose the ways to pay them (life included), finish with a winner or a draw, keep every invariant (life paid is accounted for, replay agrees), and use every way of paying.

use std::sync::atomic::{AtomicU64, Ordering};

use mtg_core::mana::ManaSymbol;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::player::LossReason;
use mtg_engine::{Engine, Event, GameResult};
use mtg_testkit::{PlayOptions, RandomBot, play_game_with, symbol_fuzz_setup};
use rstest::rstest;

use crate::common::seeds::{for_each_seed, fuzz_games};

/// Games per table size; override with `MTG_FUZZ_GAMES` to run longer or shorter campaigns.
fn games() -> u64 {
    fuzz_games(60)
}

#[rstest]
#[case(2, false)]
#[case(3, false)]
#[case(4, true)]
#[case(6, true)]
fn bot_games_with_hybrid_and_phyrexian_costs_finish_and_keep_every_invariant(
    #[case] players: usize,
    #[case] partial_payments: bool,
) {
    let options = PlayOptions {
        symbol_choices: true,
        partial_payments,
        ..PlayOptions::default()
    };
    let (winners, draws) = (AtomicU64::new(0), AtomicU64::new(0));
    for_each_seed(0..games() / 2, |seed| {
        let outcome = play_game_with(symbol_fuzz_setup(players, seed), seed, options)
            .unwrap_or_else(|failure| panic!("{failure}"));
        match outcome.result {
            GameResult::Winner(_) => winners.fetch_add(1, Ordering::Relaxed),
            GameResult::Draw => draws.fetch_add(1, Ordering::Relaxed),
        };
    });
    let (winners, draws) = (winners.into_inner(), draws.into_inner());
    assert_eq!(winners + draws, games() / 2);
    assert!(winners > 0, "no game produced a winner");
}

#[test]
fn bots_that_do_not_choose_symbols_still_play_every_game_by_paying_automatically() {
    // Without the opt-in a cost with a hybrid or Phyrexian symbol is paid automatically or cancelled.
    for_each_seed(0..games() / 5, |seed| {
        play_game_with(symbol_fuzz_setup(3, seed), seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"));
    });
}

#[derive(Debug, Default)]
struct Tally {
    manual_with_life: u32,
    manual_with_hybrid: u32,
    complete_keeping_life: u32,
    complete_that_added_life: u32,
    automatic_with_life: u32,
    lost_paying_life: u32,
    life_paid: u32,
    casts: u32,
}

#[test]
fn every_way_of_paying_a_symbol_occurs_in_the_games() {
    let mut tally = Tally::default();
    for seed in 0..games().min(120) {
        let players = 2 + usize::try_from(seed % 3).unwrap_or(0);
        let setup = symbol_fuzz_setup(players, seed);
        let mut engine = Engine::new(setup).expect("a fuzz setup is valid");
        let mut bots: Vec<RandomBot> = (0u64..)
            .take(players)
            .map(|seat| {
                RandomBot::new(seed, seat)
                    .with_symbol_choices()
                    .with_partial_payments()
            })
            .collect();
        while let Some(decision) = engine.decision().cloned() {
            let answer = bots[decision.player.index()].choose(&decision);
            let events = engine
                .apply(answer.clone())
                .unwrap_or_else(|e| panic!("seed {seed}: {e} for {answer:?}"));
            tally.casts += u32::try_from(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::SpellCast { .. }))
                    .count(),
            )
            .unwrap_or(0);
            let paid: u32 = events
                .iter()
                .filter_map(|e| match e {
                    Event::LifePaid { amount, .. } => Some(*amount),
                    _ => None,
                })
                .sum();
            tally.life_paid += paid;
            if paid > 0
                && events.iter().any(|e| {
                    matches!(
                        e,
                        Event::PlayerLost {
                            reason: LossReason::ZeroLife,
                            ..
                        }
                    )
                })
            {
                tally.lost_paying_life += 1;
            }
            let (DecisionKind::PayMana(options), Answer::PayMana(choice)) =
                (&decision.kind, &answer)
            else {
                continue;
            };
            let hybrid = options
                .cost
                .symbols()
                .iter()
                .any(|s| matches!(s, ManaSymbol::Hybrid(..) | ManaSymbol::ColorlessHybrid(_)));
            match choice {
                PayChoice::Manual(payment) => {
                    tally.manual_with_life += u32::from(payment.life > 0);
                    tally.manual_with_hybrid += u32::from(hybrid);
                }
                PayChoice::Complete(partial) => {
                    tally.complete_keeping_life += u32::from(partial.life > 0);
                    tally.complete_that_added_life += u32::from(paid > partial.life);
                }
                PayChoice::Auto => tally.automatic_with_life += u32::from(paid > 0),
                PayChoice::Cancel => {}
            }
        }
    }
    assert!(
        tally.casts > 500
            && tally.life_paid > 0
            && tally.manual_with_life > 0
            && tally.manual_with_hybrid > 0
            && tally.complete_keeping_life > 0
            && tally.complete_that_added_life > 0
            && tally.automatic_with_life > 0,
        "every way of paying should occur: {tally:?}"
    );
}
