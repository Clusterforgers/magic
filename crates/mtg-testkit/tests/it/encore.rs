//! Goal: encore is a sorcery-speed ability from a graveyard that exiles the card as a cost and makes one token copy for each opponent, with haste and the abilities of the card; each token must attack its own opponent in the turn's combat (and only that one), is sacrificed at the next end step, and a token whose opponent has left is free; the ability is not offered without the mana; the log replays to the same state; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{lands, pay};
use crate::common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::has_keyword;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, replay};
use mtg_testkit::cards::{island, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// The tokens called `name` in play.
fn tokens(state: &GameState, name: &str) -> Vec<mtg_core::ids::ObjectId> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            name_of(state, *id) == name
                && state
                    .object(*id)
                    .is_some_and(|o| o.kind() == ObjectKind::Token)
        })
        .collect()
}

/// Answers passively until a player holds priority with nothing on the stack.
fn settle_all(driver: &mut Driver) -> Result<(), ScriptError> {
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        || !driver.state().stack().is_empty()
    {
        driver.answer_passively()?;
    }
    Ok(())
}

/// Seat 0 holds `card` in a graveyard with `count` lands of `land` in play.
fn game(
    seats: usize,
    card: &str,
    land: fn() -> Arc<CardDefinition>,
    count: usize,
) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(seats)
            .graveyard_named(0, [card])
            .battlefield(0, lands(count, land)),
    )
}

/// Seat 0 activates encore of `card` and lets the ability and what it triggers resolve.
fn encore(driver: &mut Driver, card: &str) -> Result<(), ScriptError> {
    driver.begin_activation(card)?;
    pay(driver)?;
    settle_all(driver)
}

/// The attack the declare attackers decision requires, at the first step of combat in which seat 0 declares.
fn required_attack(
    driver: &mut Driver,
) -> Result<Vec<(mtg_core::ids::ObjectId, AttackTarget)>, ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("attackers are declared");
    };
    Ok(options.required_attack())
}

fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn encore_makes_one_hasty_copy_for_each_opponent_and_exiles_the_card(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, "Impulsive Pilferer", mountain, 4)?;
    driver.begin_activation("Impulsive Pilferer")?;
    pay(&mut driver)?;
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(0))),
        [],
        "exiled as a cost"
    );
    assert_eq!(driver.state().zone(Zone::Exile).len(), 1);
    settle_all(&mut driver)?;
    let made = tokens(driver.state(), "Impulsive Pilferer");
    assert_eq!(made.len(), seats - 1, "one for each opponent");
    for token in &made {
        assert_eq!(driver.state().object(*token).unwrap().controller(), pid(0));
        assert!(has_keyword(driver.state(), *token, &Keyword::Haste));
    }
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn each_copy_attacks_its_own_opponent_and_is_sacrificed_at_the_end_step(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, "Impulsive Pilferer", mountain, 4)?;
    encore(&mut driver, "Impulsive Pilferer")?;
    let made = tokens(driver.state(), "Impulsive Pilferer");
    let required = required_attack(&mut driver)?;
    let DecisionKind::DeclareAttackers(options) = driver.decision()?.kind.clone() else {
        panic!("attackers are declared");
    };
    assert_eq!(options.required.len(), seats - 1, "every copy must attack");
    let mut aimed: Vec<AttackTarget> = Vec::new();
    for token in &made {
        let defenders = &options
            .attackers
            .iter()
            .find(|a| a.object == *token)
            .unwrap()
            .defenders;
        assert_eq!(
            defenders.len(),
            1,
            "a copy may attack only its own opponent"
        );
        aimed.push(defenders[0]);
    }
    aimed.sort_by_key(|target| format!("{target:?}"));
    let mut opponents: Vec<AttackTarget> = (1..seats)
        .map(|seat| AttackTarget::Player(pid(seat)))
        .collect();
    opponents.sort_by_key(|target| format!("{target:?}"));
    assert_eq!(
        aimed, opponents,
        "each opponent is attacked by exactly one copy"
    );
    driver.answer(Answer::DeclareAttackers(required))?;
    driver.idle_until(1, Step::End)?;
    settle_all(&mut driver)?;
    for seat in 1..seats {
        assert_eq!(
            driver.state().player(pid(seat)).unwrap().life(),
            19,
            "seat {seat} was hit by its copy once"
        );
    }
    assert_eq!(
        tokens(driver.state(), "Impulsive Pilferer"),
        [],
        "sacrificed"
    );
    let treasures = tokens(driver.state(), "Treasure");
    assert_eq!(
        treasures.len(),
        seats - 1,
        "each sacrificed copy died and made a Treasure"
    );
    assert_replays(&driver);
    Ok(())
}

#[test]
fn a_declaration_that_leaves_a_copy_out_or_aims_it_elsewhere_is_refused() -> Result<(), ScriptError>
{
    let mut driver = game(3, "Impulsive Pilferer", mountain, 4)?;
    encore(&mut driver, "Impulsive Pilferer")?;
    let required = required_attack(&mut driver)?;
    assert!(driver.answer(Answer::DeclareAttackers(Vec::new())).is_err());
    assert!(
        driver
            .answer(Answer::DeclareAttackers(required[..1].to_vec()))
            .is_err()
    );
    let swapped: Vec<_> = required
        .iter()
        .map(|(token, _)| (*token, required[0].1))
        .collect();
    assert!(driver.answer(Answer::DeclareAttackers(swapped)).is_err());
    driver.answer(Answer::DeclareAttackers(required))?;
    Ok(())
}

#[test]
fn a_copy_whose_opponent_has_left_the_game_is_free_to_attack_anyone_or_no_one()
-> Result<(), ScriptError> {
    let mut driver = game(4, "Impulsive Pilferer", mountain, 4)?;
    encore(&mut driver, "Impulsive Pilferer")?;
    driver.concede(pid(3))?;
    driver.pass_until(Step::DeclareAttackers)?;
    let DecisionKind::DeclareAttackers(options) = driver.decision()?.kind.clone() else {
        panic!("attackers are declared");
    };
    let made = tokens(driver.state(), "Impulsive Pilferer");
    let free: Vec<_> = options
        .attackers
        .iter()
        .filter(|a| made.contains(&a.object) && !options.required.contains(&a.object))
        .collect();
    assert_eq!(free.len(), 1, "the copy made for the opponent who left");
    assert_eq!(free[0].defenders.len(), 2, "either remaining opponent");
    assert_eq!(options.required.len(), 2);
    driver.answer(Answer::DeclareAttackers(options.required_attack()))?;
    Ok(())
}

#[rstest]
#[case::dies_trigger_draws("Trove Tracker", island, 7)]
#[case::enters_trigger_counts("Kinsbaile Courier", plains, 3)]
#[case::attack_trigger("Briarblade Adept", swamp, 4)]
#[case::token_maker("Exquisite Huntmaster", swamp, 5)]
fn the_copies_have_the_abilities_of_the_card(
    #[case] card: &str,
    #[case] land: fn() -> Arc<CardDefinition>,
    #[case] count: usize,
) -> Result<(), ScriptError> {
    let seats = 3;
    let mut driver = game(seats, card, land, count)?;
    let library = driver.state().zone(Zone::Library(pid(0))).len();
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    encore(&mut driver, card)?;
    let made = tokens(driver.state(), card);
    assert_eq!(made.len(), seats - 1);
    let required = required_attack(&mut driver)?;
    driver.answer(Answer::DeclareAttackers(required))?;
    driver.idle_until(1, Step::End)?;
    settle_all(&mut driver)?;
    assert_eq!(
        tokens(driver.state(), card),
        [],
        "sacrificed at the end step"
    );
    match card {
        "Trove Tracker" => {
            assert_eq!(
                driver.state().zone(Zone::Hand(pid(0))).len(),
                hand + seats - 1
            );
            assert_eq!(
                driver.state().zone(Zone::Library(pid(0))).len(),
                library - (seats - 1)
            );
        }
        "Exquisite Huntmaster" => {
            assert_eq!(tokens(driver.state(), "Elf Warrior").len(), seats - 1)
        }
        _ => {}
    }
    assert_replays(&driver);
    Ok(())
}

#[test]
fn encore_is_offered_only_with_the_mana_to_pay_it() -> Result<(), ScriptError> {
    let driver = game(2, "Impulsive Pilferer", mountain, 3)?;
    assert!(driver.offered_abilities("Impulsive Pilferer")?.is_empty());
    let driver = game(2, "Impulsive Pilferer", mountain, 4)?;
    assert_eq!(driver.offered_abilities("Impulsive Pilferer")?.len(), 1);
    Ok(())
}
