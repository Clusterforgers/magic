//! Goal: "change a target of target spell or ability to ~" (CR 115.7c) moves one target of the spell to the source when the source is a legal target in its place: a spell that cannot target it, or already targets it, keeps its targets; with two targets that could change, the controller of the effect picks which; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{game, life, on_the_stack, replays, run};
use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Activators, Timing};
use mtg_cards::cost::Cost;
use mtg_cards::effect::{Effect, NewTargets};
use mtg_cards::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::Event;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::{burn, cost, damage, one_target, twin_burn};
use mtg_testkit::{Driver, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// A 0/4 creature with "{0}: Change a target of target instant spell to Skite."
fn skite() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Skite")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 4)
            .with_rules(Ability::Activated {
                cost: Cost(Vec::new()),
                timing: Timing::Instant,
                condition: None,
                limit: None,
                by: Activators::Controller,
                from: ZoneKind::Battlefield,
                targets: vec![one_target(Targetable::Object(Selector(vec![
                    Pred::Type(CardType::Instant),
                    Pred::In(ZoneKind::Stack),
                ])))],
                effects: vec![Effect::ChangeTargets {
                    what: Ref::Target(0),
                    to: NewTargets::One,
                    onto: Some(Ref::This),
                }],
            }),
    )
}

/// An instant for `{R}` that deals 2 damage to a target opponent, which no creature can be.
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

/// The damage marked on Skite.
fn skite_damage(driver: &Driver) -> u32 {
    let id = driver.id_of("Skite").unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .damage()
}

/// Seat 0 casts `spell` at `targets`; seat 1 activates Skite at it, and the driver runs to the first question.
fn respond(
    driver: &mut Driver,
    spell: &str,
    targets: &[&[mtg_engine::choose::Target]],
) -> Result<(), ScriptError> {
    driver.begin_cast(spell)?;
    for target in targets {
        driver.choose_targets(target)?;
    }
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    driver.pass()?;
    let on_stack = on_the_stack(driver, spell);
    driver.begin_activation("Skite")?;
    driver.choose_targets(&[on_stack])?;
    run(driver, 2)
}

#[rstest]
#[case::any_target_moves_to_skite("Zap", false, 20, 2)]
#[case::an_opponent_only_spell_keeps_its_target("Jab", false, 18, 0)]
#[case::a_spell_already_at_skite_keeps_it("Zap", true, 20, 2)]
fn a_target_moves_to_the_source_only_where_it_is_legal(
    #[case] spell: &str,
    #[case] at_skite: bool,
    #[case] foe_life: i32,
    #[case] marked: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(2, &[], 0, 3)
            .hand(0, [burn("Zap", 2), jab()])
            .battlefield(1, vec![ready(skite())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let target = if at_skite {
        driver.target_named("Skite")?
    } else {
        driver.target_player(1)
    };
    respond(&mut driver, spell, &[&[target]])?;
    run(&mut driver, 2)?;
    assert_eq!(life(&driver, 1), foe_life);
    assert_eq!(skite_damage(&driver), marked);
    let changed = driver
        .engine()
        .events()
        .iter()
        .any(|e| matches!(e, Event::TargetsChanged(_)));
    assert_eq!(changed, spell == "Zap" && !at_skite);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn with_two_targets_the_controller_of_the_effect_picks_the_one_that_changes(
    #[values(0, 1)] picked: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(2, &[], 0, 3)
            .hand(0, [twin_burn("Twin")])
            .battlefield(1, vec![ready(skite())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let (me, foe) = (driver.target_player(0), driver.target_player(1));
    respond(&mut driver, "Twin", &[&[me], &[foe]])?;
    let decision = driver.decision()?;
    assert_eq!(decision.player, pid(1), "the controller of Skite picks");
    let DecisionKind::ChooseTargets(options) = &decision.kind else {
        panic!("{:?}", decision.kind.name());
    };
    assert_eq!(options.legal, vec![me, foe]);
    driver.choose_targets(&[[me, foe][picked]])?;
    run(&mut driver, 2)?;
    assert_eq!(life(&driver, picked), 20, "the picked target is spared");
    assert_eq!(life(&driver, 1 - picked), 18);
    assert_eq!(skite_damage(&driver), 2);
    replays(&driver, &setup);
    Ok(())
}
