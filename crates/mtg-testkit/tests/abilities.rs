//! Goal: an activated ability that is not a mana ability is offered at the right times, costs its tap, goes on the stack as an object of its own, resolves without needing its source (which it remembers as it was) and then ceases to exist, and a seat that holds an instant or an ability can answer what is on the stack.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Bounds, Ref, TargetSpec, Targetable, ZoneKind};
use mtg_cards::value::{Amount, StatKind};
use mtg_core::card::CardType;
use mtg_engine::decision::PriorityAction;
use mtg_engine::decision::{DecisionKind, PriorityOptions};
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event};
use mtg_testkit::cards::{burn, free_creature, library_of_cards, mountain, pinger};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick, tapped};
use rstest::rstest;
use std::sync::Arc;

fn options(driver: &Driver) -> PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.clone(),
        other => panic!("priority was expected, got {}", other.name()),
    }
}

fn resolve_the_stack(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    let mut events = Vec::new();
    while !driver.state().stack().is_empty() {
        events.extend(driver.pass()?);
    }
    Ok(events)
}

#[rstest]
#[case::ready(ready(pinger("Pinger", 0)), true)]
#[case::tapped(tapped(pinger("Pinger", 0)), false)]
#[case::summoning_sick(sick(pinger("Pinger", 0)), false)]
fn a_tap_ability_is_offered_only_to_a_permanent_that_can_tap(
    #[case] permanent: mtg_engine::PresetPermanent,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [permanent]))?;
    let options = options(&driver);
    assert_eq!(options.abilities.len(), usize::from(offered));
    assert_eq!(options.can_act, offered, "an ability is something to do");
    Ok(())
}

#[test]
fn an_artifact_taps_for_an_ability_the_turn_it_arrives() -> Result<(), ScriptError> {
    // Only creatures are summoning sick (CR 302.6).
    let driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [sick(library_of_cards("Library", 2))]),
    )?;
    assert_eq!(options(&driver).abilities.len(), 1);
    Ok(())
}

#[test]
fn the_other_seat_is_not_offered_the_ability() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(pinger("Pinger", 0))])
            .at(1, 0, Step::Main1),
    )?;
    let mut driver = driver;
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    assert_eq!(options(&driver).abilities, []);
    Ok(())
}

#[test]
fn activating_pays_the_tap_announces_the_target_and_puts_the_ability_on_the_stack()
-> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(pinger("Pinger", 0))]))?;
    let log = driver.engine().events().len();
    driver.begin_activation("Pinger")?;
    // Nothing is logged until the ability is announced and paid for.
    assert_eq!(driver.engine().events().len(), log);
    let DecisionKind::ChooseTargets(targets) = &driver.decision()?.kind else {
        panic!("the target is announced first");
    };
    assert!(targets.can_cancel);
    let pinger = driver.id_of("Pinger")?;
    let at = driver.target_player(1);
    let events = driver.choose_targets(&[at])?;
    let kinds: Vec<&str> = events
        .iter()
        .map(|e| match e {
            Event::PermanentTapped { .. } => "tap",
            Event::AbilityActivated { .. } => "activated",
            Event::AbilityPutOnStack(_) => "on the stack",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["tap", "activated", "on the stack"]);
    let Some(Event::AbilityPutOnStack(put)) = events
        .iter()
        .find(|e| matches!(e, Event::AbilityPutOnStack(_)))
    else {
        panic!("the ability was put on the stack");
    };
    assert_eq!(put.source.id, pinger);
    assert_eq!(put.controller, pid(0));
    assert_eq!(put.choices.targets, [vec![at]]);
    assert_eq!(put.snapshot.id, pinger);
    assert!(put.snapshot.characteristics.is_creature());
    assert_eq!(put.snapshot.characteristics.power, Some(1));
    // The ability is an object on top of the stack, and the pinger is tapped.
    let ability = driver.state().object(put.object).unwrap();
    assert_eq!(ability.kind(), ObjectKind::Ability);
    assert_eq!(driver.state().stack(), [put.object]);
    assert!(
        driver
            .state()
            .object(pinger)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert_eq!(
        driver.decision()?.player,
        pid(0),
        "the activator keeps priority"
    );
    Ok(())
}

#[test]
fn the_ability_resolves_and_ceases_to_exist() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(pinger("Pinger", 0))]))?;
    driver.begin_activation("Pinger")?;
    let at = driver.target_player(1);
    let events = driver.choose_targets(&[at])?;
    let ability = events
        .iter()
        .find_map(|e| match e {
            Event::AbilityPutOnStack(put) => Some(put.object),
            _ => None,
        })
        .unwrap();
    let events = resolve_the_stack(&mut driver)?;
    driver.expect_life(1, 19)?;
    assert!(
        driver.state().object(ability).is_none(),
        "an ability that resolved is gone"
    );
    assert_eq!(driver.state().stack(), []);
    let ended: Vec<ResolutionOutcome> = events
        .iter()
        .filter_map(|e| match e {
            Event::ResolutionEnded { outcome, .. } => Some(*outcome),
            _ => None,
        })
        .collect();
    assert_eq!(ended, [ResolutionOutcome::Resolved]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::SpellResolved { .. })),
        "an ability is not a spell"
    );
    // The damage was dealt by the pinger, and the tap is still paid.
    let pinger = driver.id_of("Pinger")?;
    assert!(
        events.iter().any(
            |e| matches!(e, Event::DamageDealt { source, amount: 1, .. } if *source == pinger)
        )
    );
    Ok(())
}

#[test]
fn backing_out_of_an_activation_costs_nothing() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(pinger("Pinger", 0))]))?;
    let before = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_activation("Pinger")?;
    driver.cancel_targets()?;
    assert_eq!(
        (driver.engine().events().len(), driver.engine().state_hash()),
        before
    );
    let pinger = driver.id_of("Pinger")?;
    assert!(
        !driver
            .state()
            .object(pinger)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert_eq!(
        options(&driver).abilities.len(),
        1,
        "it can still be activated"
    );
    Ok(())
}

#[test]
fn an_ability_that_is_not_offered_cannot_be_activated() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(2).battlefield(0, [tapped(pinger("Pinger", 0))]))?;
    let pinger = driver.id_of("Pinger")?;
    let before = driver.engine().state_hash();
    let attempt = driver.answer(Answer::Priority(PriorityAction::Activate(
        mtg_engine::abilities::AbilityRef {
            object: pinger,
            ability: 0,
        },
    )));
    assert!(attempt.is_err());
    assert_eq!(driver.engine().state_hash(), before);
    Ok(())
}

#[test]
fn a_sorcery_speed_ability_waits_for_your_main_phase_and_an_empty_stack() -> Result<(), ScriptError>
{
    let scenario = || {
        Scenario::new(2)
            .battlefield(
                0,
                [ready(library_of_cards("Library", 2)), ready(mountain())],
            )
            .hand(0, [burn("Spark", 1)])
    };
    // Offered in the main phase with an empty stack, and it needs no target, so no question is asked.
    let mut driver = Driver::from_scenario(scenario())?;
    assert_eq!(options(&driver).abilities.len(), 1);
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_activation("Library")?;
    assert_eq!(driver.state().stack().len(), 1);
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand + 1,
        "it drew a card"
    );
    // Not offered over something on the stack.
    let mut driver = Driver::from_scenario(scenario())?;
    let at = driver.target_player(1);
    driver.cast_at("Spark", &[at])?;
    assert_eq!(options(&driver).abilities, []);
    // Not offered outside the main phase, nor on another player's turn.
    let driver = Driver::from_scenario(scenario().at(1, 0, Step::BeginCombat))?;
    assert_eq!(options(&driver).abilities, []);
    let driver = Driver::from_scenario(scenario().at(1, 1, Step::Main1))?;
    assert_eq!(options(&driver).abilities, []);
    Ok(())
}

/// A 1/1 whose "{T}: deals damage equal to its power to any target" reads its own power.
fn thumper() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Thumper")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Activated {
                cost: Cost(vec![CostItem::Tap]),
                timing: Timing::Instant,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: vec![TargetSpec {
                    what: Targetable::AnyTarget,
                    bounds: Bounds::default(),
                    divide: None,
                }],
                effects: vec![Effect::DealDamage {
                    source: None,
                    to: Ref::Target(0),
                    amount: Amount::Stat {
                        of: Ref::This,
                        stat: StatKind::Power,
                    },
                }],
            }),
    )
}

#[test]
fn an_ability_whose_source_died_in_response_still_resolves_with_the_source_as_it_was()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(thumper())])
            .hand(1, [burn("Bolt", 3)])
            .battlefield(1, [ready(mountain())]),
    )?;
    // The thumper is a 3/3 for the turn.
    driver.add_effect(
        Selector::AllCreatures,
        Modification::ModifyPowerToughness {
            power: 2,
            toughness: 2,
        },
        Duration::UntilEndOfTurn,
    )?;
    driver.begin_activation("Thumper")?;
    let at = driver.target_player(1);
    driver.choose_targets(&[at])?;
    let thumper = driver.id_of("Thumper")?;
    driver.pass()?;
    // In response the opponent kills the thumper.
    let at = driver.target_named("Thumper")?;
    driver.cast_at("Bolt", &[at])?;
    let events = resolve_the_stack(&mut driver)?;
    assert_ne!(
        driver.state().object(thumper).unwrap().zone(),
        Zone::Battlefield
    );
    // 3 damage from the bolt that killed it, then the ability's damage: its power as it was, 3, not the printed 1.
    let dealt: Vec<u32> = events
        .iter()
        .filter_map(|e| match e {
            Event::DamageDealt {
                source,
                amount,
                target,
            } if *source == thumper
                && matches!(target, mtg_engine::combat::DamageTarget::Player(_)) =>
            {
                Some(*amount)
            }
            _ => None,
        })
        .collect();
    assert_eq!(dealt, [3]);
    driver.expect_life(1, 17)?;
    Ok(())
}

#[test]
fn an_ability_answers_the_spell_below_it_and_resolves_first() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [burn("Bolt", 3)])
            .battlefield(0, [ready(mountain())])
            .battlefield(
                1,
                [
                    ready(pinger("Pinger", 0)),
                    ready(free_creature("Bear", 2, 2)),
                ],
            ),
    )?;
    let bear = driver.target_named("Bear")?;
    driver.cast_at("Bolt", &[bear])?;
    driver.pass()?;
    // In response the opponent pings the caster.
    assert_eq!(driver.decision()?.player, pid(1));
    driver.begin_activation("Pinger")?;
    let at = driver.target_player(0);
    driver.choose_targets(&[at])?;
    assert_eq!(driver.state().stack().len(), 2);
    let top = *driver.state().stack().last().unwrap();
    assert_eq!(
        driver.state().object(top).unwrap().kind(),
        ObjectKind::Ability
    );
    resolve_the_stack(&mut driver)?;
    driver.expect_life(0, 19)?;
    driver.expect_zone(Zone::Graveyard(pid(1)), &["Bear"])?;
    Ok(())
}
