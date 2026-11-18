//! Goal: the cost "Exile any number of <kind> cards from your graveyard with total mana value N or greater"
//! (`CostItem::ExileTotalValue`) is offered exactly while the cards of the kind reach N together, counts only cards of
//! the kind, refuses a pick short of N or with a card of another kind and exiles exactly the cards picked; The
//! Capitoline Triad's emblem gives creatures its controller controls base power and toughness 9/9, at every table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// An artifact: "Exile any number of artifact cards from your graveyard with total mana value `total` or greater: You
/// gain 1 life."
fn gadget(total: u32) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Activated":{{"cost":[{{"ExileTotalValue":{{"filter":[{{"Type":"Artifact"}}],"total":{total}}}}}],"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(&json).unwrap()),
    )
}

/// An artifact card of this mana value.
fn relic(name: &str, value: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(mtg_testkit::cards::cost(value, &[])),
    )
}

fn id(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .find(|found| found.card().name == name)
        .map(mtg_engine::object::GameObject::id)
        .unwrap()
}

fn pick(driver: &mut Driver, chosen: &[&str]) -> Result<(), ScriptError> {
    let ids = chosen.iter().map(|name| id(driver, name)).collect();
    driver.answer(Answer::ChooseCards(CardsChoice(ids)))?;
    Ok(())
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..50 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 has the Gadget and a graveyard of artifacts of these values and a creature card of mana value 9.
fn game(total: u32, values: &[u32], seats: usize) -> Result<Driver, ScriptError> {
    let mut grave: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(at, value)| relic(&format!("Relic {at}"), *value))
        .collect();
    grave.push(creature("Big Beast", 9, &[], 9, 9));
    Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(gadget(total))])
            .graveyard(0, grave),
    )
}

#[rstest]
#[case::exactly_enough(6, &[2, 4], true)]
#[case::more_than_enough(6, &[3, 4, 5], true)]
#[case::one_short(6, &[2, 3], false)]
#[case::other_kinds_do_not_count(10, &[1], false)]
#[case::nothing(1, &[], false)]
fn the_cost_is_offered_while_the_kind_reaches_the_total(
    #[case] total: u32,
    #[case] values: &[u32],
    #[case] offered: bool,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = game(total, values, seats)?;
    assert_eq!(!driver.offered_abilities("Gadget")?.is_empty(), offered);
    Ok(())
}

#[rstest]
fn a_pick_must_reach_the_total_with_cards_of_the_kind(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(6, &[3, 4, 5], seats)?;
    driver.begin_activation("Gadget")?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("a choice of cards was expected");
    };
    assert!(
        !options.from.contains(&id(&driver, "Big Beast")),
        "another kind is no candidate"
    );
    assert!(pick(&mut driver, &["Relic 0"]).is_err(), "3 is short of 6");
    assert!(
        pick(&mut driver, &["Relic 0", "Big Beast"]).is_err(),
        "a creature card is not an artifact"
    );
    pick(&mut driver, &["Relic 0", "Relic 1"])?;
    settle(&mut driver)?;
    let mut exiled = names(driver.state(), Zone::Exile);
    exiled.sort();
    assert_eq!(exiled, ["Relic 0", "Relic 1"]);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 21);
    Ok(())
}

#[rstest]
fn the_capitoline_emblem_makes_your_creatures_nine_nine(
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let triad = card("The Capitoline Triad").unwrap();
    let grave: Vec<_> = (0..3).map(|at| relic(&format!("Relic {at}"), 10)).collect();
    let mut scenario = Scenario::new(seats)
        .battlefield(0, [ready(triad), ready(free_creature("Bear", 2, 2))])
        .graveyard(0, grave);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Their Bear", 2, 2))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("The Capitoline Triad")?;
    while matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
        pick(&mut driver, &["Relic 0", "Relic 1", "Relic 2"])?;
    }
    settle(&mut driver)?;
    let stats = |name: &str| {
        let found = id(&driver, name);
        let derived = mtg_engine::characteristics(driver.state(), found).unwrap();
        (derived.power, derived.toughness)
    };
    assert_eq!(stats("Bear"), (Some(9), Some(9)));
    assert_eq!(stats("The Capitoline Triad"), (Some(9), Some(9)));
    assert_eq!(stats("Their Bear"), (Some(2), Some(2)));
    Ok(())
}
