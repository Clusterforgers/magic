//! Goal: the cards the engine-gap pass frees play as printed at tables of two, three and four seats: Unbreathing Horde enters with a +1/+1 counter for each other Zombie its controller controls and each Zombie card in their graveyard, damage to it is prevented and takes one counter, and with no Zombie around it enters with none and dies as a 0/0; Niv-Mizzet, Ghost Counsel pays the life just gained to draw as many cards, or keeps the life when its controller declines.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::Answer;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, cost, gain_life, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, on_battlefield};

const C6_EG_HORDE: &str = "Unbreathing Horde";

fn c6_eg_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn c6_eg_counters(driver: &Driver) -> Option<u32> {
    let id = on_battlefield(driver.state(), C6_EG_HORDE)?;
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map(|held| held.counters_of(&CounterKind::PlusOnePlusOne))
}

#[rstest]
fn c6_eg_the_horde_counts_zombies_and_pays_a_counter_for_damage(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] on_field: usize,
    #[values(0, 2)] in_graveyard: usize,
) -> Result<(), ScriptError> {
    let zombie = || card("Dregscape Zombie").unwrap();
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(C6_EG_HORDE).unwrap(), burn("Bolt", 3)])
        .battlefield(
            0,
            (0..4)
                .map(|_| ready(swamp()))
                .chain([ready(mountain())])
                .chain((0..on_field).map(|_| ready(zombie()))),
        )
        .graveyard(0, (0..in_graveyard).map(|_| zombie()));
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping(C6_EG_HORDE, &["Swamp", "Swamp", "Swamp"])?;
    c6_eg_settle(&mut driver)?;
    let entered = u32::try_from(on_field + in_graveyard).unwrap();
    assert_eq!(c6_eg_counters(&driver), (entered > 0).then_some(entered));
    if entered == 0 {
        assert!(
            names(driver.state(), Zone::Graveyard(PlayerId(0))).contains(&C6_EG_HORDE.to_string())
        );
        return Ok(());
    }
    let target = driver.target_named(C6_EG_HORDE)?;
    driver.cast_at("Bolt", &[target])?;
    c6_eg_settle(&mut driver)?;
    assert_eq!(c6_eg_counters(&driver), (entered > 1).then(|| entered - 1));
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::DamageDealt { target, .. } if format!("{target:?}").contains("Object"))),
        "no damage is dealt to the Horde"
    );
    Ok(())
}

/// An instant for `{0}` that gains `amount` life.
fn c6_eg_brew(amount: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Brew")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(amount)],
            }),
    )
}

#[rstest]
fn c6_eg_niv_mizzet_pays_the_life_gained_to_draw_as_many(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] gained: i32,
    #[values(true, false)] pays: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [c6_eg_brew(gained)])
        .library(0, (0..6).map(|_| swamp()))
        .battlefield(0, [ready(card("Niv-Mizzet, Ghost Counsel").unwrap())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Brew")?;
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        if driver.decision()?.kind.name() == "yes-no" {
            driver.answer(Answer::YesNo(YesNoChoice(pays)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    driver.expect_life(0, if pays { 20 } else { 20 + gained })?;
    let drawn = names(driver.state(), Zone::Hand(PlayerId(0))).len();
    assert_eq!(
        drawn,
        if pays {
            usize::try_from(gained).unwrap()
        } else {
            0
        }
    );
    Ok(())
}
