//! Goal: "if you revealed a Dragon card or controlled a Dragon as you cast this spell" holds when the optional reveal was paid
//! or the caster controlled a Dragon as the spell was cast, not for an opponent's Dragon, and the permanent the spell becomes
//! keeps what held, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, CostsChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// "If you revealed a Dragon card or controlled a Dragon as you cast this spell".
const PRESENCE: &str = r#"{"Any":["AdditionalCostPaid",{"AsCast":{"Compare":{"left":{"Count":[{"Subtype":"Dragon"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}]}"#;

/// "As an additional cost to cast this spell, you may reveal a Dragon card from your hand."
fn reveal() -> Ability {
    serde_json::from_str(
        r#"{"AdditionalCost":{"cost":[{"RevealFromHand":{"filter":[{"Subtype":"Dragon"}]}}]}}"#,
    )
    .unwrap()
}

/// An instant for {1}: you gain 4 life when the condition holds, else 2.
fn roar() -> Arc<CardDefinition> {
    let effects = format!(
        r#"{{"Spell":{{"effects":[{{"If":{{"condition":{PRESENCE},"then":[{{"GainLife":{{"amount":4}}}}],"otherwise":[{{"GainLife":{{"amount":2}}}}]}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Roar")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(1, &[]))
            .with_rules(reveal())
            .with_rules(serde_json::from_str::<Ability>(&effects).unwrap()),
    )
}

/// A 1/1 creature for {1}: "When this creature enters, if [the condition], you gain 3 life."
fn orator() -> Arc<CardDefinition> {
    let trigger = format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"if":{PRESENCE},"effects":[{{"GainLife":{{"amount":3}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Orator")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(mtg_testkit::cards::cost(1, &[]))
            .with_rules(reveal())
            .with_rules(serde_json::from_str::<Ability>(&trigger).unwrap()),
    )
}

fn dragon(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype("Dragon")
            .with_power_toughness(4, 4),
    )
}

/// Where the Dragons are.
#[derive(Clone, Copy)]
enum Dragons {
    None,
    Mine,
    Opponents,
    InHand,
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Casts `spell` at a table of `seats` with the Dragons as `dragons` says, revealing when `reveals`; seat 0's life after.
fn play(
    seats: usize,
    spell: Arc<CardDefinition>,
    dragons: Dragons,
    reveals: bool,
) -> Result<i32, ScriptError> {
    let name = spell.name.clone();
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    let mut mine = vec![ready(forest())];
    if matches!(dragons, Dragons::Mine) {
        mine.push(ready(dragon("Wyrm")));
    }
    scenario = scenario.battlefield(0, mine);
    scenario = match dragons {
        Dragons::None | Dragons::Mine => scenario.hand(0, [spell]),
        Dragons::Opponents => scenario
            .hand(0, [spell])
            .battlefield(1, [ready(dragon("Wyrm"))]),
        Dragons::InHand => scenario.hand(0, [spell, dragon("Wyrm")]),
    };
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(&name)?;
    if let DecisionKind::ChooseCosts(_) = &driver.decision()?.kind {
        let chosen = if reveals { vec![0] } else { vec![] };
        driver.answer(Answer::ChooseCosts(CostsChoice::Costs(chosen)))?;
    }
    while let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
        let first = options.from.iter().copied().take(1).collect();
        driver.answer(Answer::ChooseCards(CardsChoice(first)))?;
    }
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    settle(&mut driver)?;
    Ok(driver.state().player(pid(0)).unwrap().life())
}

#[rstest]
#[case::neither(Dragons::None, false, 22)]
#[case::a_dragon_of_mine(Dragons::Mine, false, 24)]
#[case::an_opponents_dragon(Dragons::Opponents, false, 22)]
#[case::revealed(Dragons::InHand, true, 24)]
#[case::not_revealed(Dragons::InHand, false, 22)]
fn the_spell_reads_what_held_as_it_was_cast(
    #[values(2, 3, 4)] seats: usize,
    #[case] dragons: Dragons,
    #[case] reveals: bool,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    assert_eq!(play(seats, roar(), dragons, reveals)?, life);
    Ok(())
}

#[rstest]
#[case::neither(Dragons::None, false, 20)]
#[case::a_dragon_of_mine(Dragons::Mine, false, 23)]
#[case::an_opponents_dragon(Dragons::Opponents, false, 20)]
#[case::revealed(Dragons::InHand, true, 23)]
fn the_permanent_keeps_what_held_as_its_spell_was_cast(
    #[values(2, 4)] seats: usize,
    #[case] dragons: Dragons,
    #[case] reveals: bool,
    #[case] life: i32,
) -> Result<(), ScriptError> {
    assert_eq!(play(seats, orator(), dragons, reveals)?, life);
    Ok(())
}
