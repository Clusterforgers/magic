//! Goal: a target "chosen at random" is drawn by the game among the legal targets as the ability is announced, never asked of a player: over seats and seeds the drawn target is always legal (an opponent, never the controller), the draw spends one substream of the random source, a trigger with no legal target is removed, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{game, life, replays, run};
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Activators, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, damage, one_target};
use mtg_testkit::{Driver, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// A 1/1 with "{T}: Pilot deals 2 damage to target opponent chosen at random."
fn pilot() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Pilot")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Activated {
                cost: Cost(vec![CostItem::Tap]),
                timing: Timing::Instant,
                condition: None,
                limit: None,
                by: Activators::Controller,
                from: ZoneKind::Battlefield,
                targets: vec![one_target(Targetable::AtRandom(Box::new(
                    Targetable::Opponent,
                )))],
                effects: vec![damage(Ref::Target(0), 2)],
            }),
    )
}

/// A 1/1 with "When Giant enters, it deals 3 damage to target creature an opponent controls chosen at random."
fn giant() -> Arc<CardDefinition> {
    let theirs = Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(Ref::Opponents),
    ]);
    Arc::new(
        CardDefinition::new("Giant")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Triggered {
                when: TriggerPattern::ZoneChange {
                    what: Selector::this(),
                    from: None,
                    to: Some(ZoneKind::Battlefield),
                },
                intervening_if: None,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: vec![one_target(Targetable::AtRandom(Box::new(
                    Targetable::Object(theirs),
                )))],
                effects: vec![damage(Ref::Target(0), 3)],
            }),
    )
}

#[rstest]
fn the_game_draws_an_opponent_and_spends_one_substream(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 7, 42)] seed: u64,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 0, 0)
            .seed(seed)
            .battlefield(0, vec![ready(pilot())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let before = driver.state().rng_index();
    driver.begin_activation("Pilot")?;
    assert!(
        !matches!(
            driver.decision().map(|d| &d.kind),
            Ok(DecisionKind::ChooseTargets(_))
        ),
        "nobody is asked for the target"
    );
    assert_eq!(driver.state().rng_index(), before + 1);
    run(&mut driver, seats)?;
    assert_eq!(life(&driver, 0), 20, "the controller is never drawn");
    let hit: Vec<usize> = (1..seats)
        .filter(|seat| life(&driver, *seat) == 18)
        .collect();
    assert_eq!(hit.len(), 1, "exactly one opponent is dealt the damage");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::an_opposing_creature_is_drawn(true, 1)]
#[case::no_legal_target_removes_the_trigger(false, 0)]
fn an_entering_trigger_draws_among_the_legal_targets(
    #[case] foe_has_a_bear: bool,
    #[case] dead_bears: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut s = game(2, &[], 0, 0)
            .hand(0, [giant()])
            .battlefield(0, vec![ready(bear())]);
        if foe_has_a_bear {
            s = s.battlefield(1, vec![ready(bear())]);
        }
        s
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Giant")?;
    run(&mut driver, 2)?;
    run(&mut driver, 2)?;
    let graves: usize = (0..2)
        .map(|seat| driver.state().zone(Zone::Graveyard(PlayerId(seat))).len())
        .sum();
    assert_eq!(graves, dead_bears, "only the opponent's bear can be drawn");
    assert_eq!(driver.state().zone(Zone::Graveyard(PlayerId(0))).len(), 0);
    replays(&driver, &setup);
    Ok(())
}
