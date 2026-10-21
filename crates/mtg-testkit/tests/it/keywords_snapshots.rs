//! Goal: what a source or a creature had when it left is last-known information (CR 603.10, 608.2h), played whole at two, three and four seats: a creature given deathtouch or lifelink for the turn that is then sacrificed to deal damage deals it as it was (the creature it hurts dies, its controller gains the life), one that never had them deals plain damage, and the condition of a "when it dies" trigger that asks for a keyword finds the one the creature had and not the ones its card prints; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Condition;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::{burn, damage, free_creature, gain_life, mountain, one_target};
use mtg_testkit::pump_cards::grant;
use mtg_testkit::trigger_cards::{creature_with, trigger};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
#[allow(clippy::unnecessary_wraps)] // the tests end with it, as they end with their last script step
fn assert_replays(driver: &Driver, setup: &GameSetup) -> Result<(), ScriptError> {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    Ok(())
}

/// Passes priority while something is on the stack and nobody is asked anything else.
fn resolve_everything(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

/// A 1/1 that prints these keywords and has "Sacrifice this creature: it deals 2 damage to any target."
fn fanatic(keywords: &[Keyword]) -> Arc<CardDefinition> {
    let mut card = (*creature_with(
        "Fanatic",
        vec![Ability::Activated {
            cost: Cost(vec![CostItem::SacrificeSelf]),
            timing: Timing::Instant,
            condition: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: vec![one_target(Targetable::AnyTarget)],
            effects: vec![damage(Ref::Target(0), 2)],
        }],
    ))
    .clone();
    for keyword in keywords {
        card = card.with_rules(Ability::Keyword(keyword.clone()));
    }
    Arc::new(card)
}

/// Seat 0 controls the Fanatic and a Mountain and holds `hand`; every other seat controls a 1/5 Giant.
fn table(seats: usize, printed: &[Keyword], hand: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut table = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(fanatic(printed)), ready(mountain())])
        .hand(0, hand);
    for seat in 1..seats {
        table = table.battlefield(seat, [ready(free_creature("Giant", 1, 5))]);
    }
    table
}

/// Gives the Fanatic `keyword` for the turn, if there is one, then sacrifices it for 2 damage at `target`.
fn sacrifice_at(
    driver: &mut Driver,
    given: Option<&str>,
    target: mtg_engine::choose::Target,
) -> Result<(), ScriptError> {
    if given.is_some() {
        let fanatic = driver.target_named("Fanatic")?;
        driver.cast_at("Gift", &[fanatic])?;
        resolve_everything(driver)?;
    }
    driver.begin_activation("Fanatic")?;
    driver.choose_targets(&[target])?;
    resolve_everything(driver)
}

#[rstest]
fn a_creature_given_lifelink_that_is_sacrificed_deals_its_damage_with_lifelink(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] given: bool,
) -> Result<(), ScriptError> {
    let hand = || {
        if given {
            vec![grant("Gift", Keyword::Lifelink)]
        } else {
            Vec::new()
        }
    };
    let setup = table(seats, &[], hand()).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &[], hand()))?;
    let foe = driver.target_player(1);
    sacrifice_at(&mut driver, given.then_some("Gift"), foe)?;
    assert_eq!(life(&driver, 1), 18, "2 damage");
    // The Fanatic is a card in a graveyard that prints no lifelink: only what it had when it left gains the life.
    assert_eq!(life(&driver, 0), if given { 22 } else { 20 });
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_creature_given_deathtouch_that_is_sacrificed_destroys_what_its_damage_hurts(
    #[values(2, 3)] seats: usize,
    #[values(true, false)] given: bool,
) -> Result<(), ScriptError> {
    let hand = || {
        if given {
            vec![grant("Gift", Keyword::Deathtouch)]
        } else {
            Vec::new()
        }
    };
    let setup = table(seats, &[], hand()).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &[], hand()))?;
    let giant = driver.target_named("Giant")?;
    sacrifice_at(&mut driver, given.then_some("Gift"), giant)?;
    let mtg_engine::choose::Target::Object(hurt) = giant else {
        panic!("the Giant is an object");
    };
    let alive = driver
        .state()
        .object(hurt.id)
        .is_some_and(|o| o.counter() == hurt.counter && o.zone() == Zone::Battlefield);
    assert_eq!(
        alive, !given,
        "a 1/5 survives 2 damage unless the damage is deathtouch"
    );
    assert_replays(&driver, &setup)
}

#[rstest]
fn a_creature_that_prints_lifelink_deals_its_damage_with_lifelink_when_it_is_sacrificed(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // The control case: what it printed is what it had when it left.
    let printed = [Keyword::Lifelink];
    let setup = table(seats, &printed, Vec::new()).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &printed, Vec::new()))?;
    let foe = driver.target_player(1);
    sacrifice_at(&mut driver, None, foe)?;
    assert_eq!((life(&driver, 0), life(&driver, 1)), (22, 18));
    assert_replays(&driver, &setup)
}

/// A watcher: "Whenever a creature you control dies, if it had flying, you gain 3 life."
fn mourner_of_fliers() -> Arc<CardDefinition> {
    let mut dies = trigger(
        TriggerPattern::ZoneChange {
            what: Selector(vec![
                Pred::ControlledBy(Ref::You),
                Pred::Type(mtg_core::card::CardType::Creature),
            ]),
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        },
        vec![gain_life(3)],
    );
    if let Ability::Triggered { intervening_if, .. } = &mut dies {
        *intervening_if = Some(Condition::Matches {
            subject: Ref::Triggering,
            selector: Selector(vec![Pred::HasKeyword(Keyword::Flying)]),
        });
    }
    creature_with("Mourner of Fliers", vec![dies])
}

#[rstest]
fn the_condition_of_a_dies_trigger_reads_the_keywords_the_creature_had(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] flying: bool,
) -> Result<(), ScriptError> {
    // Seat 1 controls the Mourner and a Bear, takes the turn, gives the Bear flying for it if `flying`, and Bolts it.
    let table = || {
        let mut hand = vec![burn("Bolt", 3)];
        if flying {
            hand.push(grant("Gift", Keyword::Flying));
        }
        let mut table = Scenario::new(seats)
            .at(1, 1, Step::Main1)
            .battlefield(
                1,
                [
                    ready(mourner_of_fliers()),
                    ready(free_creature("Bear", 2, 2)),
                    ready(mountain()),
                    ready(mountain()),
                ],
            )
            .hand(1, hand);
        for seat in (0..seats).filter(|seat| *seat != 1) {
            table = table.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
        }
        table
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    let bear = driver.target_named("Bear")?;
    if flying {
        driver.cast_at("Gift", &[bear])?;
        resolve_everything(&mut driver)?;
    }
    driver.cast_at("Bolt", &[bear])?;
    resolve_everything(&mut driver)?;
    assert!(driver.id_of("Bear").is_err(), "the Bear died");
    // The card in the graveyard prints no flying; the Bear had it when it died or it did not.
    assert_eq!(life(&driver, 1), if flying { 23 } else { 20 });
    assert_replays(&driver, &setup)
}
