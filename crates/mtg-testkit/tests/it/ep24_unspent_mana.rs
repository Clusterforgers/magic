//! Goal: the rules of unspent mana hold as steps and phases end (CR 500.4): a player who keeps one type keeps only that type, a rule for every type keeps the whole pool, a rule that turns lost mana into one type turns the rest into it, the rules reach only the players they name, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::copying::replays;
use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_core::card::CardType;
use mtg_core::mana::ManaType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use ManaType::{Colorless, Green, Red};

/// An artifact with these rules of unspent mana.
fn keeper(rules: Vec<GameRule>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Keeper").with_type(CardType::Artifact);
    for rule in rules {
        card = card.with_rules(Ability::Rule {
            rule,
            condition: None,
            from: ZoneKind::Battlefield,
        });
    }
    Arc::new(card)
}

fn keep(who: Ref, mana: Option<ManaType>) -> GameRule {
    GameRule::KeepUnspentMana { who, mana }
}

fn becomes(into: ManaType) -> GameRule {
    GameRule::UnspentManaBecomes {
        who: Ref::You,
        into,
    }
}

/// The pool of `seat` by type, in the order red, green, colorless.
fn pool(driver: &Driver, seat: usize) -> [u32; 3] {
    let mana = driver.state().player(pid(seat)).unwrap().mana();
    [mana.get(Red), mana.get(Green), mana.get(Colorless)]
}

#[rstest]
#[case::no_rule(vec![], 0, [0, 0, 0])]
#[case::red_kept(vec![keep(Ref::You, Some(Red))], 0, [1, 0, 0])]
#[case::every_type_kept(vec![keep(Ref::EachPlayer, None)], 0, [1, 1, 0])]
#[case::lost_mana_becomes_colorless(vec![becomes(Colorless)], 0, [0, 0, 2])]
#[case::red_kept_and_green_becomes_colorless(
    vec![keep(Ref::You, Some(Red)), becomes(Colorless)],
    0,
    [1, 0, 1]
)]
#[case::lost_mana_becomes_red(vec![becomes(Red)], 0, [2, 0, 0])]
#[case::an_opponent_s_rule_keeps_nothing_of_mine(vec![keep(Ref::You, None)], 1, [0, 0, 0])]
#[case::an_opponent_s_rule_for_every_player(vec![keep(Ref::EachPlayer, None)], 1, [1, 1, 0])]
fn unspent_mana_stays_as_the_rules_say(
    #[case] rules: Vec<GameRule>,
    #[case] owner: usize,
    #[case] left: [u32; 3],
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut mine = vec![ready(mountain()), ready(forest())];
        let mut theirs = Vec::new();
        let held = if owner == 0 { &mut mine } else { &mut theirs };
        held.push(ready(keeper(rules.clone())));
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .battlefield(0, mine)
            .battlefield(1, theirs)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.tap_for_mana("Mountain")?;
    driver.tap_for_mana("Forest")?;
    assert_eq!(pool(&driver, 0), [1, 1, 0]);
    driver.pass_until(Step::Main2)?;
    assert_eq!(pool(&driver, 0), left, "after the main phase");
    driver.pass_until(Step::End)?;
    assert_eq!(pool(&driver, 0), left, "after another step");
    replays(&driver, &setup);
    Ok(())
}
