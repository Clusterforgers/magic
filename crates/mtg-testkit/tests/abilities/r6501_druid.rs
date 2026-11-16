//! Goal: Druid of Purification plays as printed at two to four seats: as it enters, each player in turn order starting
//! with its controller (on any player's turn) may choose an artifact or enchantment the Druid's controller doesn't
//! control, and each permanent chosen this way is destroyed; one chosen twice is destroyed once, and declining is a choice.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A permanent for {0} of this type and name.
fn r6501_thing(name: &str, kind: CardType) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(0, &[])),
    )
}

/// Druid of Purification with flash, so it can enter on another player's turn.
fn r6501_flash_druid() -> Arc<CardDefinition> {
    let flash: Ability = serde_json::from_str(r#"{"Keyword":"Flash"}"#).unwrap();
    Arc::new((*card("Druid of Purification").unwrap()).clone().with_rules(flash))
}

/// Seat 0 holds the Druid, four Forests and its own Relic 0; every other seat `s` controls Relic s and Charm s; seat
/// `active` takes the turn.
fn r6501_table(seats: usize, active: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, active, Step::Main1)
        .battlefield(
            0,
            (0..4)
                .map(|_| ready(forest()))
                .chain([ready(r6501_thing("Relic 0", CardType::Artifact))]),
        )
        .hand(0, [r6501_flash_druid()]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(r6501_thing(&format!("Relic {seat}"), CardType::Artifact)),
                ready(r6501_thing(&format!("Charm {seat}"), CardType::Enchantment)),
            ],
        );
    }
    scenario
}

/// Casts the Druid for seat 0 when it next has priority, answers each choice with `pick(chooser)` and settles; returns
/// the players asked, in order, and what each was offered.
fn r6501_enter(
    driver: &mut Driver,
    pick: impl Fn(&Driver, PlayerId) -> Option<ObjectId>,
) -> Result<Vec<(PlayerId, Vec<ObjectId>)>, ScriptError> {
    let mut cast = false;
    let mut asked = Vec::new();
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if !cast && decision.player == pid(0) => {
                driver.cast_tapping("Druid of Purification", &["Forest"; 4])?;
                cast = true;
            }
            DecisionKind::ChooseCards(options) => {
                asked.push((decision.player, options.from.clone()));
                let chosen: Vec<ObjectId> = pick(driver, decision.player).into_iter().collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            _ if cast && driver.state().stack().is_empty() && driver.state().resolving().is_none() => {
                return Ok(asked);
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the Druid".into()))
}

fn r6501_on_battlefield(driver: &Driver, name: &str) -> bool {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| state.object(*id).unwrap().card().name == name)
}

#[rstest]
#[case::on_its_controllers_turn(0)]
#[case::on_another_players_turn(1)]
fn r6501_each_player_from_the_controller_may_choose_one_to_destroy(
    #[values(2, 3, 4)] seats: usize,
    #[case] active: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r6501_table(seats, active))?;
    let last = seats - 1;
    // Seat 0 and the last seat both choose the last seat's Relic; every other seat declines.
    let asked = r6501_enter(&mut driver, |driver, chooser| {
        (chooser == pid(0) || chooser == pid(last))
            .then(|| driver.id_of(&format!("Relic {last}")).unwrap())
    })?;
    let order: Vec<PlayerId> = asked.iter().map(|(player, _)| *player).collect();
    let expected: Vec<PlayerId> = (0..seats).map(pid).collect();
    assert_eq!(order, expected, "turn order starting with the Druid's controller");
    let own = driver.id_of("Relic 0")?;
    for (_, offered) in &asked {
        assert!(!offered.contains(&own), "the controller's own artifact is never offered");
        assert_eq!(offered.len(), 2 * (seats - 1));
    }
    assert!(!r6501_on_battlefield(&driver, &format!("Relic {last}")));
    assert!(r6501_on_battlefield(&driver, "Relic 0"));
    for seat in 1..seats {
        assert!(r6501_on_battlefield(&driver, &format!("Charm {seat}")));
        if seat != last {
            assert!(r6501_on_battlefield(&driver, &format!("Relic {seat}")));
        }
    }
    Ok(())
}
