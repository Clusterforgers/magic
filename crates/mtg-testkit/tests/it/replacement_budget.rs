//! Goal: an effect that makes more than ten thousand objects at once (a token for each of thousands of tokens) resolves, because the work the replacement stage may do grows with the events it holds, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::{Effect, TokenRef};
use mtg_cards::refs::Ref;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

use crate::common::copying::{replays, settle};

/// An instant for `{R}` that makes `count` Clue tokens.
fn swarm(count: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Swarm")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::CreateToken {
                    who: Ref::You,
                    token: TokenRef::Named("clue".into()),
                    count: Amount::Fixed(count),
                    tapped: false,
                    attacking: false,
                    bind: None,
                }],
            }),
    )
}

#[rstest]
#[case::a_few(3)]
#[case::just_over_the_old_limit(10_500)]
fn a_spell_that_makes_thousands_of_tokens_at_once_resolves_and_replays(
    #[case] count: i32,
    #[values(2)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        Scenario::new(seats)
            .hand(0, [swarm(count)])
            .battlefield(0, [ready(mountain())])
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.cast_tapping("Swarm", &["Mountain"])?;
    settle(&mut driver)?;
    let tokens = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().kind() == ObjectKind::Token)
        .count();
    assert_eq!(tokens, usize::try_from(count).unwrap());
    replays(&driver, &setup);
    Ok(())
}
