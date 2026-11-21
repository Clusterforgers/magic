//! Goal: a choice as a permanent transforms (`ReplaceBy::Choose` over `ReplaceWhen::R6603Transforms`, CR 614.1c,
//! 701.28) at 2 to 4 seats: Zenos yae Galvus remembers the creature it chose as it entered and shrinks every other
//! creature, transforms when that creature leaves, Shinryu chooses an opponent as it turns over, and that opponent's loss
//! wins the game while another opponent's does not; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::state::GameResult;
use mtg_engine::turn::Step;
use mtg_testkit::cards::cost;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;

const ZENOS: &str = "Zenos yae Galvus // Shinryu, Transcendent Rival";

/// The corpus card with no mana cost.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut card = (*card(name).unwrap()).clone();
    card.mana_cost = Some(cost(0, &[]));
    Arc::new(card)
}

/// A vanilla 3/3.
fn bear(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(3, 3),
    )
}

/// Seat 0 has Ally and Zenos, Murder and two Lightning Bolts in hand, each free; seat 1 has Foe, the others a Bystander; every
/// opponent is at 2 life.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(
            0,
            [ZENOS, "Murder", "Lightning Bolt", "Lightning Bolt"].map(free),
        )
        .battlefield(0, [ready(bear("Ally"))])
        .battlefield(1, [ready(bear("Foe"))]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(bear(&format!("Bystander {seat}")))]);
    }
    for seat in 1..seats {
        scenario = scenario.life(seat, 2);
    }
    scenario
}

/// Plays on until the stack is empty: `targets` when asked, `pick` for a card choice, the player `seat` for a player
/// choice, the rest the default way.
fn settle(
    driver: &mut Driver,
    targets: &[Target],
    pick: Option<ObjectId>,
    seat: usize,
) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.engine().result().is_some()
            || (driver.state().stack().is_empty()
                && driver.state().resolving().is_none()
                && matches!(driver.decision()?.kind, DecisionKind::Priority(_)))
        {
            return Ok(());
        }
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(targets)?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseCards(_) => {
                driver.answer(Answer::ChooseCards(CardsChoice(pick.into_iter().collect())))?;
            }
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| matches!(option, ChoiceOption::Player(p) if *p == pid(seat)))
                    .unwrap_or(0);
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Power and toughness now.
fn stats(driver: &Driver, id: ObjectId) -> (i32, i32) {
    let chars = characteristics(driver.state(), id).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

/// Casts Zenos choosing Foe, then Murders Foe, Shinryu choosing the opponent in seat `chosen`.
fn transform(driver: &mut Driver, chosen: usize) -> Result<ObjectId, ScriptError> {
    driver.begin_cast("Zenos yae Galvus")?;
    let foe = driver.id_of("Foe")?;
    settle(driver, &[], Some(foe), chosen)?;
    let zenos = driver.id_of("Zenos yae Galvus")?;
    let at = driver.target_named("Foe")?;
    driver.begin_cast("Murder")?;
    settle(driver, &[at], None, chosen)?;
    Ok(zenos)
}

#[rstest]
fn zenos_shrinks_all_but_itself_and_the_chosen_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    let [ally, foe] = ["Ally", "Foe"].map(|name| driver.id_of(name).unwrap());
    driver.begin_cast("Zenos yae Galvus")?;
    settle(&mut driver, &[], Some(foe), 1)?;
    let zenos = driver.id_of("Zenos yae Galvus")?;
    assert_eq!(stats(&driver, zenos), (4, 4));
    assert_eq!(stats(&driver, foe), (3, 3));
    assert_eq!(stats(&driver, ally), (1, 1));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::the_chosen_opponent_loses(true)]
#[case::another_opponent_loses(false)]
fn shinryu_wins_when_the_chosen_opponent_loses(
    #[values(2, 3, 4)] seats: usize,
    #[case] chosen_loses: bool,
) -> Result<(), ScriptError> {
    if !chosen_loses && seats == 2 {
        return Ok(());
    }
    let setup = table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats))?;
    let chosen = seats - 1;
    let zenos = transform(&mut driver, chosen)?;
    let shinryu = driver.state().object(zenos).unwrap();
    assert_eq!(shinryu.face(), Some(1), "the chosen creature left");
    let loser = if chosen_loses { chosen } else { 1 };
    driver.begin_cast("Lightning Bolt")?;
    let player = driver.target_player(loser);
    settle(&mut driver, &[player], None, chosen)?;
    let result = driver.engine().result();
    if chosen_loses {
        assert_eq!(result, Some(GameResult::Winner(pid(0))));
    } else {
        assert_eq!(result, None);
    }
    replays(&driver, &setup);
    Ok(())
}
