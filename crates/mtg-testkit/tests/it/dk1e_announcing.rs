//! Goal: a manual action taken out of band while a cast or an activation is being announced leaves what state-based actions would do until the announcement ends (CR 704.3), so the invariants hold at the question under way and the creature it left at zero toughness dies as soon as the cast is backed out of; one that moves the card or permanent being announced to any zone ends the announcement with no event and priority returns, instead of the spell failing to go on the stack later; at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{names, pid};
use mtg_cards::ability::Ability;
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::value::Amount;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, forest, free_creature, mountain, pinger};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

#[rstest]
fn a_creature_a_manual_action_leaves_dying_waits_for_the_end_of_the_announcement(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] by_damage: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain()), ready(free_creature("Bear", 2, 2))])
            .library(0, (0..10).map(|_| free_creature("Filler", 1, 1))),
    )?;
    let bear = driver.id_of("Bear")?;
    driver.begin_cast("Bolt")?;
    let action = if by_damage {
        ManualAction::Damage {
            source: bear,
            target: mtg_engine::combat::DamageTarget::Object(bear),
            amount: 2,
        }
    } else {
        ManualAction::Pump {
            object: bear,
            power: -2,
            toughness: -2,
        }
    };
    driver.manual_by(pid(0), &action)?;
    assert_eq!(
        names(driver.state(), Zone::Battlefield),
        ["Mountain", "Bear"]
    );
    driver.cancel_targets()?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Bear"]);
    Ok(())
}

#[rstest]
fn a_cast_or_activation_whose_card_a_manual_action_moved_is_dropped_and_priority_returns(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        ManualZone::Battlefield,
        ManualZone::Hand,
        ManualZone::Graveyard,
        ManualZone::Exile,
        ManualZone::LibraryTop,
        ManualZone::LibraryBottom
    )]
    to: ManualZone,
    #[values(true, false)] activation: bool,
    #[values(true, false)] paying: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(
                0,
                [
                    ready(mountain()),
                    ready(free_creature("Bear", 2, 2)),
                    ready(pinger("Pinger", 0)),
                ],
            )
            .library(0, (0..10).map(|_| free_creature("Filler", 1, 1))),
    )?;
    let name = if activation { "Pinger" } else { "Bolt" };
    let object = if activation {
        driver.id_of(name)?
    } else {
        driver.state().zone(Zone::Hand(pid(0)))[0]
    };
    if activation {
        driver.begin_activation(name)?;
    } else {
        driver.begin_cast(name)?;
    }
    if paying {
        if activation {
            // A tap ability has no mana to pay: the activation is on the stack once the target is chosen.
            return Ok(());
        }
        driver.choose_targets(&[driver.target_player(1)])?;
        assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    }
    assert!(driver.engine().announcing());
    let moved = driver.manual_by(pid(0), &ManualAction::Move { object, to });
    if moved.is_err() {
        // A move the manual rules refuse (the zone it is in, a spell onto the battlefield) changes nothing.
        assert!(driver.engine().announcing());
        return Ok(());
    }
    assert!(!driver.engine().announcing());
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(
        driver.state().object(object).unwrap().zone(),
        to.zone(pid(0))
    );
    Ok(())
}

#[rstest]
fn a_cast_whose_discard_a_manual_action_took_away_is_dropped_before_the_payment(
    #[values(2, 3, 4)] seats: usize,
    #[values(ManualZone::Graveyard, ManualZone::Exile, ManualZone::LibraryTop)] to: ManualZone,
) -> Result<(), ScriptError> {
    // "As an additional cost to cast this spell, discard a card."
    let grim = Arc::new(
        (*burn("Grim Bolt", 3))
            .clone()
            .with_rules(Ability::AdditionalCost {
                cost: Cost(vec![CostItem::Discard {
                    count: Amount::one(),
                    filter: None,
                }]),
                required: true,
            }),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .hand(0, [grim, forest(), forest()])
            .battlefield(0, [ready(mountain())])
            .library(0, (0..10).map(|_| free_creature("Filler", 1, 1))),
    )?;
    driver.begin_cast("Grim Bolt")?;
    driver.expect_decision(0, "choose-cards")?;
    let forests = driver.state().zone(Zone::Hand(pid(0))).to_vec();
    let discard = *forests.last().unwrap();
    driver.answer(Answer::ChooseCards(CardsChoice(vec![discard])))?;
    driver.choose_targets(&[driver.target_player(1)])?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    driver.manual_by(
        pid(0),
        &ManualAction::Move {
            object: discard,
            to,
        },
    )?;
    assert!(!driver.engine().announcing());
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    assert_eq!(driver.state().stack(), []);
    Ok(())
}
