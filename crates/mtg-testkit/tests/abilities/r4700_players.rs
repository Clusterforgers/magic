//! Goal: a permanent that chooses two players as it enters (Sower of Discord) asks for two different players at two,
//! three and four seats; whenever damage is dealt to one of them, the other loses that much life, once for each chosen
//! player dealt damage in one batch, and damage to a player not chosen changes nothing; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::refs::Ref;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::Chosen;
use mtg_engine::replay;
use mtg_testkit::cards::{cost, damage, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const SOWER: &str = "Sower of Discord";

/// A sorcery for {R} that deals 2 damage to each player.
fn r4700_wave() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Searing Wave")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![damage(Ref::EachPlayer, 2)],
            }),
    )
}

/// Passes until the stack is empty, choosing the seats in `players` in turn when a player is asked for, and returns how
/// many players were asked for and how many were offered each time.
fn r4700_settle(driver: &mut Driver, players: &[usize]) -> Result<Vec<usize>, ScriptError> {
    let mut offered = Vec::new();
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                let want = ChoiceOption::Player(pid(players[offered.len()]));
                let at = options
                    .options
                    .iter()
                    .position(|o| *o == want)
                    .expect("the seat is offered");
                offered.push(options.options.len());
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(offered);
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack does not empty".to_owned()))
}

fn r4700_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
#[case::bolt_the_first_of_two(2, [0, 1], "bolt", 0)]
#[case::bolt_the_second_of_two(2, [0, 1], "bolt", 1)]
#[case::bolt_a_chosen_opponent(3, [1, 2], "bolt", 2)]
#[case::bolt_someone_not_chosen(4, [2, 3], "bolt", 1)]
#[case::each_player_at_three(3, [1, 2], "wave", 0)]
#[case::each_player_at_four(4, [0, 3], "wave", 0)]
fn r4700_damage_to_one_chosen_player_costs_the_other_that_much_life(
    #[case] seats: usize,
    #[case] chosen: [usize; 2],
    #[case] spell: &str,
    #[case] target: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut board: Vec<_> = (0..6).map(|_| ready(swamp())).collect();
        board.extend((0..2).map(|_| ready(mountain())));
        Scenario::new(seats)
            .at(1, 0, mtg_engine::turn::Step::Main1)
            .hand(
                0,
                [
                    card(SOWER).unwrap(),
                    card("Lightning Bolt").unwrap(),
                    r4700_wave(),
                ],
            )
            .battlefield(0, board)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast(SOWER)?;
    let offered = r4700_settle(&mut driver, &chosen)?;
    // The second player is asked for among the others, unless only one is left.
    let asked: Vec<usize> = [seats, seats - 1].into_iter().filter(|n| *n > 1).collect();
    assert_eq!(offered, asked, "two different players are asked for");
    let id = driver.id_of(SOWER)?;
    let held = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .chosen()
        .cloned();
    assert_eq!(
        held,
        Some(Chosen::Players(
            chosen.iter().map(|seat| pid(*seat)).collect()
        ))
    );
    let before: Vec<i32> = (0..seats).map(|seat| r4700_life(&driver, seat)).collect();
    let mut expected = before.clone();
    if spell == "bolt" {
        driver.begin_cast("Lightning Bolt")?;
        let at = driver.target_player(target);
        driver.choose_targets(&[at])?;
        driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
        expected[target] -= 3;
        if let Some(other) = chosen.iter().find(|seat| **seat != target)
            && chosen.contains(&target)
        {
            expected[*other] -= 3;
        }
    } else {
        driver.cast("Searing Wave")?;
        for life in &mut expected {
            *life -= 2;
        }
        // Each chosen player is dealt 2, so the other loses 2 more.
        for seat in chosen {
            expected[seat] -= 2;
        }
    }
    r4700_settle(&mut driver, &[])?;
    let after: Vec<i32> = (0..seats).map(|seat| r4700_life(&driver, seat)).collect();
    assert_eq!(after, expected);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
