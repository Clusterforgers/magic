//! Goal: a creature spell whose additional cost is "behold a Goat and exile it" is cast only with a Goat to behold, exiles
//! the Goat it beholds from the hand or the battlefield, and its leaves-the-battlefield trigger returns that card; a god
//! "isn't a creature" while the devotion it names is too low; at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use common::{names, pid};

const BEHOLD_EXILE: &str = r#"{"AdditionalCost":{"cost":[{"Behold":{"filter":[{"Subtype":"Goat"}],"exile":true}}],"required":true}}"#;
const RETURN: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"Move":{"to":"Hand","what":{"Each":[{"ExiledWith":"This"},{"In":"Exile"}]}}}]}}"#;
const SACRIFICE: &str =
    r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"GainLife":{"amount":1}}]}}"#;

fn card(name: &str, rules: &[&str]) -> CardDefinition {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(7, 3)
        .with_mana_cost(ManaCost::parse("{0}").unwrap());
    for rule in rules {
        card = card.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    card
}

fn goat(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Goat")
            .with_power_toughness(1, 1),
    )
}

/// Casts the champion, picking `goat` when asked, and lets everything resolve.
fn cast(driver: &mut Driver, goat: &str) -> Result<(), ScriptError> {
    driver.begin_cast("Champion")?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::ChooseCards(options) => {
                let id = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| common::name_of(driver.state(), *id) == goat)
                    .expect("the goat is offered");
                driver.answer(Answer::ChooseCards(CardsChoice(vec![id])))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the cast".into()))
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

#[rstest]
fn the_beheld_card_is_exiled_and_comes_back_when_the_champion_leaves(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] from_hand: bool,
) -> Result<(), ScriptError> {
    let champion = Arc::new(card("Champion", &[BEHOLD_EXILE, RETURN, SACRIFICE]));
    let mut scenario = Scenario::new(seats);
    scenario = if from_hand {
        scenario
            .hand(0, [champion, goat("Goat")])
            .battlefield(0, [])
    } else {
        scenario
            .hand(0, [champion])
            .battlefield(0, [ready(goat("Goat"))])
    };
    let mut driver = Driver::from_scenario(scenario)?;
    cast(&mut driver, "Goat")?;
    assert_eq!(names(driver.state(), Zone::Battlefield), ["Champion"]);
    assert_eq!(names(driver.state(), Zone::Exile), ["Goat"]);
    assert!(!names(driver.state(), Zone::Hand(pid(0))).contains(&"Goat".to_owned()));
    driver.begin_activation("Champion")?;
    settle(&mut driver)?;
    assert!(names(driver.state(), Zone::Exile).is_empty());
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Goat"]);
    Ok(())
}

#[rstest]
fn without_a_goat_the_champion_is_not_cast(#[values(2, 3, 4)] seats: usize) {
    let champion = Arc::new(card("Champion", &[BEHOLD_EXILE]));
    let scenario = Scenario::new(seats)
        .hand(0, [champion])
        .battlefield(0, [ready(Arc::new(card("Bear", &[])))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert!(driver.begin_cast("Champion").is_err());
}

/// "As long as your devotion to white is less than five, ~ isn't a creature."
const NOT_A_CREATURE: &str = r#"{"Static":{"affected":"This","mods":[{"RemoveType":"Creature"}],"condition":{"Compare":{"left":{"Devotion":["White"]},"op":"Lt","right":5}}}}"#;

#[rstest]
fn a_god_is_a_creature_only_with_enough_devotion(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] shrines: usize,
) {
    let god = CardDefinition::new("God")
        .with_type(CardType::Enchantment)
        .with_type(CardType::Creature)
        .with_power_toughness(5, 6)
        .with_mana_cost(ManaCost::parse("{3}{W}").unwrap())
        .with_rules(serde_json::from_str::<Ability>(NOT_A_CREATURE).unwrap());
    let shrine = Arc::new(
        CardDefinition::new("Shrine")
            .with_type(CardType::Enchantment)
            .with_mana_cost(ManaCost::parse("{W}{W}").unwrap()),
    );
    let mut permanents = vec![ready(Arc::new(god))];
    permanents.extend((0..shrines).map(|_| ready(shrine.clone())));
    let driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, permanents)).unwrap();
    let id = driver.id_of("God").unwrap();
    let chars = characteristics(driver.state(), id).unwrap();
    // Devotion 1 from the god, 2 more from each shrine: five or more makes it a creature.
    assert_eq!(chars.is_creature(), shrines == 2);
    assert!(chars.types.contains(CardType::Enchantment));
}
