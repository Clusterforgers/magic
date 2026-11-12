//! Goal: the cards of the second engine-gap pass play as printed at tables of two, three and four seats: Gravecrawler and Oathsworn Vampire are castable from their owner's graveyard exactly while their condition holds, only by their owner, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, gain_life, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, on_battlefield};

fn c8_eg_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// An instant for `{0}` that gains 1 life.
fn c8_eg_brew() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Brew")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(1)],
            }),
    )
}

/// A table where seat 0 has the card in its graveyard and four Swamps, and every other seat a Swamp and the same card.
fn c8_eg_table(seats: usize, name: &str, zombie: bool) -> Scenario {
    let mut mine: Vec<_> = (0..4).map(|_| ready(swamp())).collect();
    if zombie {
        mine.push(ready(card("Dregscape Zombie").unwrap()));
    }
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [c8_eg_brew()])
        .battlefield(0, mine)
        .graveyard(0, [card(name).unwrap()]);
    for seat in 1..seats {
        scenario = scenario
            .battlefield(seat, [ready(swamp())])
            .graveyard(seat, [card(name).unwrap()]);
    }
    scenario
}

fn c8_eg_cast_and_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    c8_eg_settle(driver)
}

#[rstest]
fn c8_eg_gravecrawler_is_cast_from_the_graveyard_while_its_owner_controls_a_zombie(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] zombie: bool,
) -> Result<(), ScriptError> {
    let scenario = c8_eg_table(seats, "Gravecrawler", zombie);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(driver.begin_cast("Gravecrawler").is_ok(), zombie);
    if zombie {
        c8_eg_cast_and_settle(&mut driver)?;
        assert!(on_battlefield(driver.state(), "Gravecrawler").is_some());
        assert_eq!(
            names(driver.state(), Zone::Graveyard(PlayerId(0))),
            Vec::<String>::new()
        );
    }
    for seat in 1..seats {
        assert_eq!(
            names(
                driver.state(),
                Zone::Graveyard(PlayerId(u64::try_from(seat).unwrap()))
            ),
            ["Gravecrawler"],
            "another player's copy stays"
        );
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn c8_eg_oathsworn_vampire_is_cast_from_the_graveyard_after_its_owner_gained_life(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] gained: bool,
) -> Result<(), ScriptError> {
    let scenario = c8_eg_table(seats, "Oathsworn Vampire", false);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    if gained {
        driver.cast("Brew")?;
        c8_eg_settle(&mut driver)?;
    }
    assert_eq!(driver.begin_cast("Oathsworn Vampire").is_ok(), gained);
    if gained {
        c8_eg_cast_and_settle(&mut driver)?;
        let vampire = on_battlefield(driver.state(), "Oathsworn Vampire").unwrap();
        let object = driver.state().object(vampire).unwrap();
        assert!(object.permanent().unwrap().tapped(), "it enters tapped");
    }
    replays(&driver, &setup);
    Ok(())
}
