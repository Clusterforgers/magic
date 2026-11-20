//! Goal: graveyard facts at 2 to 4 seats: Glyph of Reincarnation destroys the creatures the target Wall blocked this
//! turn and returns a creature card from the graveyard of the player who controlled each when it was blocked; Phyrexian
//! Grimoire's target opponent picks one of the top two cards of its controller's graveyard, which is exiled while the
//! other goes to hand. Each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// Every seat with ten Forests in its library.
fn libraries(mut scenario: Scenario, seats: usize) -> Scenario {
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 10]);
    }
    scenario
}

/// The log replays to the live state.
fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// A 0/5 Wall creature.
fn wall() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Test Wall")
            .with_type(CardType::Creature)
            .with_subtype("Wall")
            .with_power_toughness(0, 5),
    )
}

#[rstest]
fn r111_glyph_returns_from_the_blocked_players_graveyard(#[values(2, 3, 4)] seats: usize) {
    let attacker = seats - 1;
    let scenario = libraries(
        Scenario::new(seats)
            .at(1, attacker, Step::Main1)
            .battlefield(attacker, [ready(free_creature("Raider", 2, 2))])
            .graveyard(attacker, [free_creature("Old Bones", 4, 4)])
            .battlefield(0, [ready(wall()), ready(forest())])
            .hand(0, [card("Glyph of Reincarnation").unwrap()]),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Raider", 0)]).unwrap();
    for _ in 0..200 {
        if let DecisionKind::DeclareBlockers(_) = driver.decision().unwrap().kind {
            break;
        }
        driver.answer_passively().unwrap();
    }
    driver.block(&[("Test Wall", "Raider")]).unwrap();
    for _ in 0..400 {
        let state = driver.state();
        if state.turn().step() == Step::Main2 && driver.decision().unwrap().player == pid(0) {
            break;
        }
        driver.answer_passively().unwrap();
    }
    let wall = driver.target_named("Test Wall").unwrap();
    driver.cast_at("Glyph of Reincarnation", &[wall]).unwrap();
    let mut picks = Vec::new();
    for _ in 0..200 {
        if driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        if let DecisionKind::ChooseCards(options) = &decision.kind {
            let pick = options
                .from
                .iter()
                .copied()
                .find(|id| {
                    driver
                        .state()
                        .object(*id)
                        .is_some_and(|o| o.card().name == "Old Bones")
                })
                .unwrap();
            picks.push(decision.player);
            driver
                .answer(Answer::ChooseCards(CardsChoice(vec![pick])))
                .unwrap();
            continue;
        }
        driver.answer_passively().unwrap();
    }
    assert_eq!(picks, vec![pid(0)], "the caster picks once");
    let field = names(driver.state(), Zone::Battlefield);
    assert!(field.contains(&"Old Bones".to_owned()), "{field:?}");
    assert!(!field.contains(&"Raider".to_owned()));
    let bones = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == "Old Bones")
        })
        .unwrap();
    assert_eq!(
        driver.state().object(bones).unwrap().controller(),
        pid(attacker)
    );
    replays(&scenario, &driver);
}

#[rstest]
fn r111_grimoire_lets_the_opponent_pick(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] exiled: usize,
) {
    let opponent = seats - 1;
    let scenario = libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(card("Phyrexian Grimoire").unwrap()),
                    ready(mountain()),
                    ready(mountain()),
                    ready(mountain()),
                    ready(mountain()),
                ],
            )
            .graveyard(
                0,
                [
                    free_creature("Bottom", 1, 1),
                    free_creature("Middle", 1, 1),
                    free_creature("Top", 1, 1),
                ],
            ),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.begin_activation("Phyrexian Grimoire").unwrap();
    driver
        .choose_targets(&[Target::Player(pid(opponent))])
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let mut offered = Vec::new();
    for _ in 0..200 {
        if driver.state().stack().is_empty()
            && matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
        {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        if let DecisionKind::ChooseCards(options) = &decision.kind {
            assert_eq!(decision.player, pid(opponent));
            offered = options
                .from
                .iter()
                .map(|id| driver.state().object(*id).unwrap().card().name.clone())
                .collect();
            offered.sort();
            let name = offered[exiled].clone();
            let pick = options
                .from
                .iter()
                .copied()
                .find(|id| driver.state().object(*id).unwrap().card().name == name)
                .unwrap();
            driver
                .answer(Answer::ChooseCards(CardsChoice(vec![pick])))
                .unwrap();
            continue;
        }
        driver.answer_passively().unwrap();
    }
    assert_eq!(offered, vec!["Middle".to_owned(), "Top".to_owned()]);
    let gone = &offered[exiled];
    let kept = &offered[1 - exiled];
    assert!(names(driver.state(), Zone::Exile).contains(gone));
    assert!(names(driver.state(), Zone::Hand(pid(0))).contains(kept));
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        vec!["Bottom".to_owned()]
    );
    replays(&scenario, &driver);
}
