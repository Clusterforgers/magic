//! Goal: a draw limit ("Each opponent can't draw more than one card each turn") stops the draws past it and reaches only the players it names, and "Spells and abilities your opponents control can't cause you to sacrifice permanents" spares its controller from an opponent's edict and no one else; both replay to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::copying::{replays, run};
use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, draw, edict, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An enchantment with the rule.
fn holder(rule: GameRule) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Holder")
            .with_type(CardType::Enchantment)
            .with_rules(Ability::Rule {
                rule,
                condition: None,
                from: ZoneKind::Battlefield,
            }),
    )
}

/// A free sorcery: draw two cards.
fn study() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Study")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![draw(2)],
            }),
    )
}

/// Seats 0 and 1 with these permanents, seat 0 in its precombat main phase with `hand` and five cards in each library.
fn table(
    mine: Vec<Arc<CardDefinition>>,
    theirs: Vec<Arc<CardDefinition>>,
    hand: Arc<CardDefinition>,
) -> Scenario {
    Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [hand])
        .library(0, (0..5).map(|_| bear()))
        .library(1, (0..5).map(|_| bear()))
        .battlefield(0, mine.into_iter().map(ready).collect::<Vec<_>>())
        .battlefield(1, theirs.into_iter().map(ready).collect::<Vec<_>>())
}

fn limit(who: Ref) -> Arc<CardDefinition> {
    holder(GameRule::DrawLimit { who, most: 1 })
}

#[rstest]
#[case::no_limit(vec![], vec![], 2)]
#[case::an_opponent_s_limit(vec![], vec![limit(Ref::Opponents)], 1)]
#[case::my_limit_on_opponents(vec![limit(Ref::Opponents)], vec![], 2)]
#[case::my_limit_on_every_player(vec![limit(Ref::EachPlayer)], vec![], 1)]
fn the_draws_past_the_limit_do_not_happen(
    #[case] mine: Vec<Arc<CardDefinition>>,
    #[case] theirs: Vec<Arc<CardDefinition>>,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(mine.clone(), theirs.clone(), study());
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Study")?;
    run(&mut driver, 2)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), drawn);
    replays(&driver, &setup);
    Ok(())
}

fn spared() -> Arc<CardDefinition> {
    holder(GameRule::OpponentsCantMakeSacrifice(Ref::You))
}

#[rstest]
#[case::no_rule(vec![], vec![bear()], false)]
#[case::the_target_is_spared(vec![], vec![bear(), spared()], true)]
#[case::the_caster_s_rule_spares_only_the_caster(vec![spared()], vec![bear()], false)]
fn an_opponent_s_edict_spares_the_player_the_rule_names(
    #[case] mut mine: Vec<Arc<CardDefinition>>,
    #[case] theirs: Vec<Arc<CardDefinition>>,
    #[case] kept: bool,
) -> Result<(), ScriptError> {
    mine.extend([swamp(), swamp()]);
    let scenario = || table(mine.clone(), theirs.clone(), edict("Edict"));
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    driver.cast_at("Edict", &[foe])?;
    run(&mut driver, 2)?;
    let bears = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Grizzly Bears")
        .count();
    assert_eq!(bears == 1, kept);
    replays(&driver, &setup);
    Ok(())
}
