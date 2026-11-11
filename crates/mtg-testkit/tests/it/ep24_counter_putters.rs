//! Goal: a counter replacement that names who puts the counters ("If you would put", "If an opponent would put") applies only to the counters that player's spells and abilities put, halving rounds down and puts nothing for one counter, and an "effect" replacement applies to a resolving spell; each replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::copying::{replays, run};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Ref, Selector, Targetable, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, cost, one_target};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An enchantment with the replacement.
fn season(by: Option<Ref>, effect: bool, change: ReplaceBy) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Season")
            .with_type(CardType::Enchantment)
            .with_rules(Ability::Replacement {
                when: ReplaceWhen::CountersPut {
                    on: None,
                    counter: None,
                    by,
                    effect,
                },
                by: change,
                condition: None,
                from: ZoneKind::Battlefield,
            }),
    )
}

/// A free sorcery: put `count` +1/+1 counters on target creature.
fn growth(count: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Growth")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Object(Selector(vec![
                    mtg_cards::refs::Pred::Type(CardType::Creature),
                ])))],
                effects: vec![Effect::PutCounters {
                    on: Ref::Target(0),
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(count),
                }],
            }),
    )
}

#[rstest]
#[case::no_season(None, 3, 3)]
#[case::my_doubling(Some((0, Some(Ref::You), false, ReplaceBy::Multiply(2))), 3, 6)]
#[case::an_opponent_s_doubling(Some((1, Some(Ref::You), false, ReplaceBy::Multiply(2))), 3, 3)]
#[case::an_opponent_halves_mine(Some((1, Some(Ref::Opponents), false, ReplaceBy::Half)), 3, 1)]
#[case::half_of_one_is_none(Some((1, Some(Ref::Opponents), false, ReplaceBy::Half)), 1, 0)]
#[case::my_halving_spares_me(Some((0, Some(Ref::Opponents), false, ReplaceBy::Half)), 3, 3)]
#[case::an_effect_doubles(Some((1, None, true, ReplaceBy::Multiply(2))), 3, 6)]
fn the_putter_decides_which_counters_change(
    #[case] season_of: Option<(usize, Option<Ref>, bool, ReplaceBy)>,
    #[case] count: i32,
    #[case] put: u32,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut mine = vec![ready(bear())];
        let mut theirs = Vec::new();
        if let Some((owner, by, effect, change)) = season_of.clone() {
            let held = if owner == 0 { &mut mine } else { &mut theirs };
            held.push(ready(season(by, effect, change)));
        }
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .hand(0, [growth(count)])
            .battlefield(0, mine)
            .battlefield(1, theirs)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let bear = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Growth", &[bear])?;
    run(&mut driver, 2)?;
    let id = driver.id_of("Grizzly Bears")?;
    let held = driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| held.counters_of(&CounterKind::PlusOnePlusOne));
    assert_eq!(held, put);
    replays(&driver, &setup);
    Ok(())
}
