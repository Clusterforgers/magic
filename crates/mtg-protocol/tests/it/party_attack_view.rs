//! Goal: under shared team turns every seat's projected view lists the attackers of every hero once the party declares: each hero's `AttackersChosen` leaves the combat undeclared and empty, the declaration marks all attackers of all heroes and the folded batches keep every seat in step with the true state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::house::HouseRules;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::Event;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::table::Table;

/// `heroes` heroes each with `bears` ready Bears, then a boss, under shared team turns at the party's main phase.
fn party(heroes: usize, bears: usize) -> Scenario {
    party_with(heroes, bears, None)
}

/// The same table, where `extra` is one more ready card on the battlefield of a hero.
fn party_with(
    heroes: usize,
    bears: usize,
    extra: Option<&(usize, Arc<CardDefinition>)>,
) -> Scenario {
    let mut seats = vec![0; heroes];
    seats.push(1);
    let mut scenario = Scenario::new(heroes + 1)
        .house_rules(HouseRules {
            teams: Some(Teams {
                mode: TeamMode::SharedTurns,
                seats,
            }),
            ..HouseRules::default()
        })
        .at(1, 0, Step::Main1)
        .life(heroes, 100);
    for seat in 0..heroes {
        let mut board = (0..bears)
            .map(|at| ready(free_creature(&format!("Bear {seat}.{at}"), 2, 2)))
            .collect::<Vec<_>>();
        if let Some((at, card)) = extra
            && *at == seat
        {
            board.push(ready(card.clone()));
        }
        scenario = scenario.battlefield(seat, board);
    }
    scenario
}

/// The ids of the creatures the combat of `viewer`'s table view lists, in order.
fn attacking(table: &Table, viewer: usize) -> Vec<ObjectId> {
    table.views[viewer]
        .table
        .combat
        .attackers
        .iter()
        .map(|attacker| attacker.object)
        .collect()
}

#[rstest]
#[case::two_heroes_one_bear_each(2, 1)]
#[case::two_heroes_many_bears(2, 6)]
#[case::four_heroes(4, 3)]
fn every_seat_sees_the_attackers_of_every_hero_once_the_party_declares(
    #[case] heroes: usize,
    #[case] bears: usize,
) {
    let mut table = Table::new(party(heroes, bears));
    while !matches!(
        table.driver.decision().unwrap().kind,
        DecisionKind::DeclareAttackers(_)
    ) {
        table.run(Driver::answer_passively);
    }
    let mut expected: Vec<ObjectId> = Vec::new();
    for hero in 0..heroes {
        assert_eq!(table.driver.decision().unwrap().player.index(), hero);
        let chosen: Vec<(ObjectId, AttackTarget)> = (0..bears)
            .map(|at| {
                (
                    table.id(&format!("Bear {hero}.{at}")),
                    AttackTarget::Player(PlayerId::from_index(heroes)),
                )
            })
            .collect();
        expected.extend(chosen.iter().map(|(id, _)| *id));
        let run = table.run(|driver| driver.answer(Answer::DeclareAttackers(chosen.clone())));
        let last = hero + 1 == heroes;
        assert_eq!(
            run.events
                .iter()
                .any(|event| matches!(event, Event::AttackersDeclared { .. })),
            last,
            "hero {hero}: the party declares once the last hero chose"
        );
        for viewer in 0..=heroes {
            let combat = &table.views[viewer].table.combat;
            assert_eq!(
                combat.attackers_declared, last,
                "seat {viewer} after hero {hero}"
            );
            let seen = attacking(&table, viewer);
            if last {
                assert_eq!(seen, expected, "seat {viewer} sees every hero's attackers");
                assert!(
                    combat
                        .attackers
                        .iter()
                        .all(|attacker| !attacker.blocked && attacker.blockers.is_empty()),
                    "seat {viewer}: nobody is blocked yet"
                );
            } else {
                assert!(
                    seen.is_empty(),
                    "seat {viewer}: a choice is not an attack yet"
                );
            }
        }
    }
    // Priority going round through the declare-blockers step takes nobody out of the combat.
    for _ in 0..50 {
        if !matches!(
            table.driver.state().turn().step(),
            Step::DeclareAttackers | Step::DeclareBlockers
        ) {
            break;
        }
        for viewer in 0..=heroes {
            assert_eq!(attacking(&table, viewer), expected, "seat {viewer}");
        }
        table.run(Driver::answer_passively);
    }
}

#[test]
fn what_a_heros_trigger_adds_to_the_party_attack_is_seen_beside_every_declared_attacker() {
    let hero = card("Hero of Bladehold").unwrap();
    let mut table = Table::new(party_with(2, 1, Some(&(1, hero))));
    while !matches!(
        table.driver.decision().unwrap().kind,
        DecisionKind::DeclareAttackers(_)
    ) {
        table.run(Driver::answer_passively);
    }
    let boss = AttackTarget::Player(PlayerId::from_index(2));
    let first = vec![(table.id("Bear 0.0"), boss)];
    table.run(|driver| driver.answer(Answer::DeclareAttackers(first.clone())));
    let second = vec![
        (table.id("Bear 1.0"), boss),
        (table.id("Hero of Bladehold"), boss),
    ];
    table.run(|driver| driver.answer(Answer::DeclareAttackers(second.clone())));
    let declared: Vec<ObjectId> = first.iter().chain(&second).map(|(id, _)| *id).collect();
    for _ in 0..30 {
        if attacking(&table, 0).len() > declared.len() {
            break;
        }
        table.run(Driver::answer_passively);
    }
    for viewer in 0..=2 {
        let seen = &table.views[viewer].table.combat.attackers;
        assert_eq!(
            seen.len(),
            declared.len() + 2,
            "seat {viewer}: two Soldiers join"
        );
        assert!(
            declared
                .iter()
                .all(|id| seen.iter().any(|attacker| attacker.object == *id)),
            "seat {viewer} still lists every declared attacker"
        );
        assert!(
            seen.iter().all(|attacker| attacker.target == boss),
            "seat {viewer}: everything attacks the boss"
        );
    }
}
