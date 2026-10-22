//! Goal: whole bot games at 2 to 4 seats with the cards that tax attackers keep the rules of the tax, and across the seeds each part of the mechanic occurs: an attack is offered with a cost and declared with the payment of exactly what the taxes in force add up to (mana spent and life paid for Phyrexian symbols), an attack its player cannot pay for is refused and leaves the engine as it was, taxes of several cards stack on one player, and a planeswalker is attacked for a cost.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_engine::combat::{AttackTarget, attack_cost};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::{Engine, Event};
use mtg_testkit::attack_tax_cards::TAX_CARDS;
use mtg_testkit::{RandomBot, tax_fuzz_setup};

use crate::common::seeds::{for_each_seed, fuzz_games};

/// The declaration that attacks with every creature offered, each at the defender that costs the most.
fn everything(kind: &DecisionKind) -> Option<Vec<(mtg_core::ids::ObjectId, AttackTarget)>> {
    let DecisionKind::DeclareAttackers(options) = kind else {
        return None;
    };
    let all: Vec<_> = options
        .attackers
        .iter()
        .filter_map(|option| {
            let target = option
                .tax
                .iter()
                .max_by_key(|(_, cost)| cost.mana_value())
                .map(|(target, _)| *target)
                .or_else(|| option.defenders.first().copied())?;
            Some((option.object, target))
        })
        .collect();
    options
        .attackers
        .iter()
        .any(|option| !option.tax.is_empty())
        .then_some(all)
}

#[test]
fn bot_games_with_attack_taxes_pay_for_each_attack_and_use_each_part_of_it() {
    let seen = std::sync::Mutex::new(BTreeMap::<&'static str, u64>::new());
    let games = fuzz_games(14);
    for_each_seed(0..games, |seed| {
        for players in [2usize, 3, 4] {
            let setup = tax_fuzz_setup(players, seed);
            let mut engine = Engine::new(setup).unwrap();
            let mut bots: Vec<RandomBot> = (0..players as u64)
                .map(|seat| RandomBot::new(seed, seat))
                .collect();
            let mut counts = BTreeMap::<&'static str, u64>::new();
            for step in 0..6000 {
                let Some(decision) = engine.decision().cloned() else {
                    break;
                };
                if matches!(decision.kind, DecisionKind::DeclareAttackers(_)) && step % 2 == 0 {
                    if let Some(over) = everything(&decision.kind) {
                        *counts.entry("taxed offer").or_default() += 1;
                        let log = engine.events().len();
                        if engine.apply(Answer::DeclareAttackers(over)).is_err() {
                            *counts.entry("refused").or_default() += 1;
                            assert_eq!(engine.events().len(), log, "a refused attack left a trace");
                            assert_eq!(engine.decision(), Some(&decision), "seed {seed}");
                        }
                        continue;
                    }
                }
                let before = engine.state().clone();
                let answer = bots[decision.player.index()].choose(&decision);
                let events = engine.apply(answer).unwrap_or_else(|error| {
                    panic!("seed {seed}, {players} seats, step {step}: {error}")
                });
                let Some(attackers) = events.iter().find_map(|event| match event {
                    Event::AttackersDeclared { attackers, .. } => Some(attackers),
                    _ => None,
                }) else {
                    continue;
                };
                let owed = attack_cost(&before, attackers).mana_value();
                let spent: u32 = events
                    .iter()
                    .map(|event| match event {
                        Event::ManaSpent {
                            mana, restricted, ..
                        } => u32::try_from(mana.len() + restricted.len()).unwrap_or(0),
                        Event::LifePaid { amount, .. } => amount / 2,
                        _ => 0,
                    })
                    .sum();
                if owed == 0 {
                    assert_eq!(spent, 0, "seed {seed}: an attack paid for nothing");
                    continue;
                }
                assert_eq!(spent, owed, "seed {seed}, {players} seats: paid {spent} for {owed}");
                *counts.entry("attack paid").or_default() += 1;
                if attackers
                    .iter()
                    .any(|(_, target)| matches!(target, AttackTarget::Planeswalker(_)))
                    && attackers.iter().any(|(object, target)| {
                        matches!(target, AttackTarget::Planeswalker(_))
                            && !attack_cost(&before, &[(*object, *target)])
                                .symbols()
                                .is_empty()
                    })
                {
                    *counts.entry("planeswalker paid").or_default() += 1;
                }
                let stacked = attackers.iter().any(|(_, target)| {
                    mtg_engine::combat::defending_player(&before, *target)
                        .is_some_and(|defender| taxers_in(&before, defender) >= 2)
                });
                if stacked {
                    *counts.entry("stacked").or_default() += 1;
                }
            }
            let mut seen = seen.lock().unwrap();
            for (name, count) in counts {
                *seen.entry(name).or_default() += count;
            }
        }
    });
    let seen = seen.lock().unwrap();
    for part in ["taxed offer", "attack paid", "refused", "stacked", "planeswalker paid"] {
        assert!(seen.get(part).copied().unwrap_or(0) > 0, "no {part}: {seen:?}");
    }
}

/// How many permanents of the cards that tax the player `defender` controls in `state`.
fn taxers_in(state: &mtg_engine::GameState, defender: mtg_core::ids::PlayerId) -> usize {
    state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.controller() == defender)
        .filter(|object| TAX_CARDS[..7].contains(&object.card().name.as_str()))
        .count()
}
