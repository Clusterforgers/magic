//! Goal: the riders of a cast permission play as printed at tables of two, three and four seats: an artifact cast from
//! the graveyard through Edgar enters tapped, a creature cast through Leonardo enters with a finality counter and only
//! when its power or toughness is 1 or less, a creature cast from the top of the library through Thundermane Dragon has
//! haste and one with less power stays there, an instant cast through Kess goes to exile instead of the graveyard, and a
//! card cast from a hand gets none of it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, elf, forest, giant, instant, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, on_battlefield};

/// Answers passively until the stack and the waiting triggers are empty.
fn r401_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// A 1/1 artifact creature for `{2}`.
fn r401_clockwork() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Clockwork Toy")
            .with_type(CardType::Artifact)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(2, &[]))
            .with_power_toughness(1, 1),
    )
}

/// Seat 0 controls `name` and five Forests and five Mountains; every other seat has an Island.
fn r401_table(seats: usize, name: &str) -> Scenario {
    let mut mine = vec![ready(card(name).unwrap())];
    mine.extend((0..5).map(|_| ready(forest())));
    mine.extend((0..5).map(|_| ready(mountain())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(island())]);
    }
    scenario
}

/// Casts `name` from wherever seat 0 may cast it and lets it resolve.
fn r401_cast(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    driver.begin_cast(name)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r401_settle(driver)
}

#[rstest]
fn r401_an_artifact_cast_through_edgar_enters_tapped(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] from_graveyard: bool,
) -> Result<(), ScriptError> {
    let table = r401_table(seats, "Edgar, Master Machinist");
    let table = if from_graveyard {
        table.graveyard(0, [r401_clockwork()])
    } else {
        table.hand(0, [r401_clockwork()])
    };
    let mut driver = Driver::from_scenario(table)?;
    r401_cast(&mut driver, "Clockwork Toy")?;
    let toy = on_battlefield(driver.state(), "Clockwork Toy").expect("the artifact resolved");
    let tapped = driver
        .state()
        .object(toy)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped();
    assert_eq!(
        tapped, from_graveyard,
        "only the cast through Edgar enters tapped"
    );
    Ok(())
}

#[rstest]
fn r401_a_creature_cast_through_leonardo_enters_with_a_finality_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = r401_table(seats, "Leonardo, Sewer Samurai").graveyard(0, [elf(), bear()]);
    let mut driver = Driver::from_scenario(table)?;
    assert!(
        driver.begin_cast("Grizzly Bears").is_err(),
        "a 2/2 is neither power nor toughness 1 or less"
    );
    r401_cast(&mut driver, "Llanowar Elf")?;
    let elf = on_battlefield(driver.state(), "Llanowar Elf").expect("the elf resolved");
    let permanent = driver.state().object(elf).unwrap().permanent().unwrap();
    assert_eq!(
        permanent.counters_of(&CounterKind::Other("finality".into())),
        1
    );
    assert!(!permanent.tapped());
    Ok(())
}

#[rstest]
fn r401_leonardo_opens_the_graveyard_only_during_your_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = r401_table(seats, "Leonardo, Sewer Samurai")
        .graveyard(0, [elf()])
        .at(1, 1, Step::Main1);
    let mut driver = Driver::from_scenario(table)?;
    assert!(driver.begin_cast("Llanowar Elf").is_err());
    Ok(())
}

#[rstest]
fn r401_a_creature_cast_from_the_top_through_thundermane_has_haste(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = r401_table(seats, "Thundermane Dragon").library(0, [giant(), bear()]);
    let mut driver = Driver::from_scenario(table)?;
    r401_cast(&mut driver, "Craw Wurm")?;
    let wurm = on_battlefield(driver.state(), "Craw Wurm").expect("the wurm resolved");
    assert!(has_keyword(driver.state(), wurm, &Keyword::Haste));
    assert!(
        driver.begin_cast("Grizzly Bears").is_err(),
        "a creature with power less than 4 stays on top"
    );
    Ok(())
}

#[rstest]
fn r401_kess_exiles_the_graveyard_card_and_not_the_hand_card(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] from_graveyard: bool,
) -> Result<(), ScriptError> {
    let flare = || instant("Flare", 0, &[Color::Red]);
    let table = r401_table(seats, "Kess, Dissident Mage");
    let table = if from_graveyard {
        table.graveyard(0, [flare()])
    } else {
        table.hand(0, [flare()])
    };
    let mut driver = Driver::from_scenario(table)?;
    r401_cast(&mut driver, "Flare")?;
    let state = driver.state();
    let exiled = names(state, Zone::Exile) == ["Flare"];
    let graveyard = names(state, Zone::Graveyard(PlayerId(0))) == ["Flare"];
    assert_eq!((exiled, graveyard), (from_graveyard, !from_graveyard));
    Ok(())
}
