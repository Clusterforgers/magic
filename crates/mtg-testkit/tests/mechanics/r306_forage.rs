//! Goal: "Whenever you forage" triggers once for each forage of its controller, a Food sacrificed or three graveyard
//! cards exiled alike, at two and at four seats; a declined or impossible forage and another player's forage trigger
//! nothing, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_testkit::cards::{creature, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const FORAGER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Pay":{"cost":["Forage"]}}]}}"#;
const WATCHER: &str = r#"{"Triggered":{"when":{"Did":{"action":"Foraged","who":"You"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#;

fn with(name: &str, json: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(json).unwrap();
    Arc::new((*free_creature(name, 1, 1)).clone().with_rules(rules))
}

fn food() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Snack")
            .with_type(CardType::Artifact)
            .with_subtype("Food"),
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

fn counters(driver: &Driver, name: &str) -> u32 {
    driver
        .state()
        .object(id(driver, name))
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

/// Answers every question to the end: `yes` to a "may", the first legal target, every card offered for a forage.
fn play_out(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        let done = matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty();
        if done {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::YesNo(o) => Answer::YesNo(YesNoChoice(yes && !o.only_no)),
            DecisionKind::ChooseTargets(o) => Answer::ChooseTargets(TargetChoice::Targets(
                o.legal.iter().copied().take(1).collect(),
            )),
            DecisionKind::ChooseCards(o) => Answer::ChooseCards(CardsChoice(
                o.from.iter().copied().take(o.max as usize).collect(),
            )),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("forage".into()))
}

#[rstest]
#[case::a_food(2, 1, 0, true, 1)]
#[case::three_cards(2, 0, 3, true, 1)]
#[case::declined(2, 1, 3, false, 0)]
#[case::nothing_to_forage(2, 0, 2, true, 0)]
#[case::a_food_at_four_seats(4, 1, 0, true, 1)]
#[case::three_cards_at_four_seats(4, 0, 4, true, 1)]
fn whenever_you_forage_counts_each_forage_of_its_controller_only(
    #[case] seats: usize,
    #[case] foods: usize,
    #[case] cards: usize,
    #[case] yes: bool,
    #[case] expected: u32,
) -> Result<(), ScriptError> {
    let dead: Vec<_> = (0..cards)
        .map(|at| creature(&format!("Dead {at}"), 1, &[], 1, 1))
        .collect();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [with("Forager", FORAGER)])
            .battlefield(
                0,
                std::iter::once(ready(with("Watcher", WATCHER)))
                    .chain((0..foods).map(|_| ready(food()))),
            )
            .battlefield(1, [ready(with("Rival", WATCHER))])
            .graveyard(0, dead),
    )?;
    driver.cast("Forager")?;
    play_out(&mut driver, yes)?;
    assert_eq!(counters(&driver, "Watcher"), expected);
    assert_eq!(
        counters(&driver, "Rival"),
        0,
        "another player's forage is not yours"
    );
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
