//! Goal: forage plays wherever the readers put it in a spell or an ability on the stack: a payment after "you may" in a
//! trigger and one side of a choice of additional costs; each time the payer chooses a Food or three graveyard cards, and
//! nothing happens when they cannot or will not forage.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, free_creature, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

const BODYGUARD: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Pay":{"cost":["Forage"],"paid":[{"PutCounters":{"on":"This","counter":"+1/+1","count":2}}]}}]}}"#;
const FEED: &str = r#"{"AdditionalCost":{"cost":[{"OneOf":[["Forage"],"{B}"]}],"required":true}}"#;
const DESTROY: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn food(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_subtype("Food"),
    )
}

fn dead(count: usize) -> Vec<Arc<CardDefinition>> {
    (0..count)
        .map(|at| creature(&format!("Dead {at}"), 1, &[], 1, 1))
        .collect()
}

fn id(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .find(|found| found.card().name == name)
        .map(mtg_engine::object::GameObject::id)
        .unwrap()
}

/// Answers every question to the end: `yes` to a "may", the first legal target, and `picks` (by name) for the cards a
/// forage takes.
fn play_out(driver: &mut Driver, yes: bool, picks: &[&str]) -> Result<(), ScriptError> {
    for _ in 0..40 {
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
            DecisionKind::ChooseCards(_) => Answer::ChooseCards(CardsChoice(
                picks.iter().map(|name| id(driver, name)).collect(),
            )),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => mtg_testkit::Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("forage".into()))
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

#[rstest]
#[case::a_food(1, 0, true, &[], 2, &["Snack"], 0)]
#[case::three_cards(0, 3, true, &[], 2, &[], 3)]
#[case::the_food_of_a_choice(1, 4, true, &["Snack"], 2, &["Snack"], 0)]
#[case::three_cards_of_a_choice(1, 4, true, &["Dead 0", "Dead 1", "Dead 2"], 2, &[], 3)]
#[case::declined(1, 3, false, &[], 0, &[], 0)]
#[case::nothing_to_forage(0, 2, true, &[], 0, &[], 0)]
fn a_forage_after_you_may_pays_for_the_counters(
    #[case] foods: usize,
    #[case] cards: usize,
    #[case] yes: bool,
    #[case] picks: &[&str],
    #[case] expected: u32,
    #[case] sacrificed: &[&str],
    #[case] exiled: usize,
) -> Result<(), ScriptError> {
    let bodyguard = Arc::new(
        (*free_creature("Bodyguard", 2, 1))
            .clone()
            .with_rules(rules(BODYGUARD)),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [bodyguard])
            .battlefield(0, (0..foods).map(|_| ready(food("Snack"))))
            .graveyard(0, dead(cards)),
    )?;
    driver.cast("Bodyguard")?;
    play_out(&mut driver, yes, picks)?;
    assert_eq!(counters(&driver, "Bodyguard"), expected);
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    for name in sacrificed {
        assert!(graveyard.contains(&(*name).to_owned()), "{name} sacrificed");
    }
    assert_eq!(names(driver.state(), Zone::Exile).len(), exiled);
    Ok(())
}

#[rstest]
#[case::by_forage(1, 1, &["Snack"])]
#[case::by_mana(0, 2, &[])]
fn a_choice_of_forage_or_mana_casts_the_spell(
    #[case] foods: usize,
    #[case] swamps: usize,
    #[case] gone: &[&str],
) -> Result<(), ScriptError> {
    let feed = Arc::new(
        CardDefinition::new("Feed")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Black]))
            .with_rules(rules(FEED))
            .with_rules(rules(DESTROY)),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [feed])
            .battlefield(
                0,
                (0..swamps)
                    .map(|_| ready(swamp()))
                    .chain((0..foods).map(|_| ready(food("Snack")))),
            )
            .battlefield(1, [ready(free_creature("Bear", 2, 2))]),
    )?;
    driver.begin_cast("Feed")?;
    play_out(&mut driver, true, gone)?;
    let graveyard = names(driver.state(), Zone::Graveyard(pid(1)));
    assert_eq!(graveyard, ["Bear"]);
    let mine = names(driver.state(), Zone::Graveyard(pid(0)));
    for name in gone {
        assert!(mine.contains(&(*name).to_owned()), "{name} sacrificed");
    }
    Ok(())
}
