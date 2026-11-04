//! Goal: an effect that changes targets (CR 115.7) is played whole at tables of two and three seats: the controller of the effect chooses the new target, which must be legal for the controller of the spell and different from the old one; "change the target" leaves a spell with no other legal target alone and cannot name a spell with two targets, "choose new targets" asks first and may keep the targets, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{game, life, on_the_stack, replays, run, say};
use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::{Effect, NewTargets};
use mtg_cards::refs::{Pred, Ref, Selector, Targetable};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::{burn, cost, damage, island, one_target, twin_burn};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// An instant for `{U}` that changes the targets of a target spell with a single target.
fn redirect(to: NewTargets) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Redirect")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Blue]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Object(Selector(vec![
                    Pred::Type(CardType::Instant),
                    Pred::SingleTarget,
                    Pred::In(mtg_cards::refs::ZoneKind::Stack),
                ])))],
                effects: vec![Effect::ChangeTargets {
                    what: Ref::Target(0),
                    to,
                }],
            }),
    )
}

/// An instant for `{R}` that deals 2 damage to a target opponent.
fn jab() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Jab")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Opponent)],
                effects: vec![damage(Ref::Target(0), 2)],
            }),
    )
}

fn table(seats: usize, spell: Arc<CardDefinition>, to: NewTargets) -> Scenario {
    game(seats, &[], 0, 2)
        .hand(0, [spell])
        .hand(1, [redirect(to)])
        .battlefield(1, vec![ready(island())])
}

/// Seat 0 casts `spell` at `target`; seat 1 answers with Redirect, and the driver stops at the first question.
fn respond(
    driver: &mut Driver,
    spell: &str,
    target: &[Target],
    seats: usize,
) -> Result<(), ScriptError> {
    driver.cast_at(spell, target)?;
    for _ in 0..seats {
        if driver.decision()?.player == pid(1) {
            break;
        }
        driver.pass()?;
    }
    let on_stack = on_the_stack(driver, spell);
    driver.cast_at("Redirect", &[on_stack])?;
    run(driver, seats)
}

#[rstest]
fn the_controller_of_the_effect_picks_another_target_legal_for_the_spell(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, burn("Zap", 2), NewTargets::Another);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    respond(&mut driver, "Zap", &[foe], seats)?;
    let decision = driver.decision()?;
    assert_eq!(
        decision.player,
        pid(1),
        "the controller of Redirect chooses"
    );
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!("{:?}", decision.kind.name());
    };
    assert!(!options.legal.contains(&foe), "the old target is no choice");
    assert!(options.legal.contains(&driver.target_player(0)));
    let new = driver.target_player(0);
    driver.choose_targets(&[new])?;
    run(&mut driver, seats)?;
    assert_eq!(life(&driver, 1), 20, "the old target is spared");
    assert_eq!(life(&driver, 0), 18, "the new target takes the damage");
    let events = driver.engine().events();
    assert!(events.iter().any(|e| matches!(e, Event::TargetsChanged(_))));
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn the_new_target_is_legal_for_the_controller_of_the_spell_not_of_the_effect()
-> Result<(), ScriptError> {
    let seats = 3;
    let scenario = || table(seats, jab(), NewTargets::Another);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    respond(&mut driver, "Jab", &[foe], seats)?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("a question was expected");
    };
    // Opponents of seat 0, the caster of the spell: seat 0 itself is no choice though it is an opponent of seat 1.
    assert_eq!(options.legal, vec![driver.target_player(2)]);
    let third = driver.target_player(2);
    driver.choose_targets(&[third])?;
    run(&mut driver, seats)?;
    assert_eq!(life(&driver, 2), 18);
    assert_eq!(life(&driver, 1), 20);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_spell_with_no_other_legal_target_keeps_its_target() -> Result<(), ScriptError> {
    let scenario = || table(2, jab(), NewTargets::Another);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    respond(&mut driver, "Jab", &[foe], 2)?;
    assert_eq!(life(&driver, 1), 18, "nothing to change it to");
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|e| matches!(e, Event::TargetsChanged(_)))
    );
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_spell_with_two_targets_is_no_target_of_change_the_target() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        game(2, &[], 0, 3)
            .hand(0, [twin_burn("Twin")])
            .hand(1, [redirect(NewTargets::Another)])
            .battlefield(1, vec![ready(island())]),
    )?;
    let (a, b) = (driver.target_player(0), driver.target_player(1));
    driver.begin_cast("Twin")?;
    driver.choose_targets(&[a])?;
    driver.choose_targets(&[b])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    driver.pass()?;
    let twin = on_the_stack(&driver, "Twin");
    assert!(driver.begin_cast("Redirect").is_err() || driver.choose_targets(&[twin]).is_err());
    Ok(())
}

#[rstest]
fn choosing_new_targets_asks_first_and_may_keep_them(
    #[values(true, false)] change: bool,
) -> Result<(), ScriptError> {
    let seats = 2;
    let scenario = || table(seats, burn("Zap", 2), NewTargets::Choose);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    respond(&mut driver, "Zap", &[foe], seats)?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::YesNo(_)));
    say(&mut driver, change)?;
    if change {
        let me = driver.target_player(0);
        driver.choose_targets(&[me])?;
    }
    run(&mut driver, seats)?;
    assert_eq!(life(&driver, usize::from(!change)), 18);
    assert_eq!(life(&driver, usize::from(change)), 20);
    replays(&driver, &setup);
    Ok(())
}
