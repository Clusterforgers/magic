//! Goal: the energy that mana abilities, the other parts of a cost and an effect's payment pay together never exceeds what the player holds: a cost of mana is offered only while the energy covers every source it taps (and what the rest of the cost pays), it is paid with exactly that energy and no more than the plain taps need, at two to four seats, with no energy, exactly enough, one short and several to pay.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::mana::ManaOption;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{forest, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn energy_kind() -> CounterKind {
    CounterKind::Other("energy".into())
}

/// A permanent by name: a basic land of the testkit or a corpus card.
fn permanent(name: &str) -> PresetPermanent {
    ready(match name {
        "Forest" => forest(),
        "Island" => island(),
        "Plains" => plains(),
        other => card(other).unwrap(),
    })
}

fn spell(cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// A creature for nothing whose only ability is `ability`.
fn creature(name: &str, ability: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str(ability).unwrap()),
    )
}

fn energy(driver: &Driver) -> u32 {
    driver
        .state()
        .player(pid(0))
        .unwrap()
        .counters_of(&energy_kind())
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

fn abilities(driver: &Driver) -> Vec<ManaOption> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.mana_abilities.clone(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

/// Every payment in the log removed energy that was there: the log replays to the state it was played to.
fn replays(driver: &Driver, setup_of: &Scenario) {
    let setup = setup_of.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::no_energy_for_two_colours(2, "{U}{G}", &["Aether Hub", "Aether Hub"], 0, None)]
#[case::one_short_of_two(3, "{U}{G}", &["Aether Hub", "Aether Hub"], 1, None)]
#[case::exactly_two(4, "{U}{G}", &["Aether Hub", "Aether Hub"], 2, Some(0))]
#[case::more_than_enough(2, "{U}{G}", &["Aether Hub", "Aether Hub"], 5, Some(3))]
#[case::three_colours_one_short(3, "{U}{G}{R}", &["Aether Hub", "Aether Hub", "Aether Hub"], 2, None)]
#[case::three_colours_exactly(4, "{U}{G}{R}", &["Aether Hub", "Aether Hub", "Aether Hub"], 3, Some(0))]
#[case::a_servant_and_a_hub_one_short(2, "{W}{B}", &["Servant of the Conduit", "Aether Hub"], 1, None)]
#[case::a_servant_and_a_hub_exactly(3, "{W}{B}", &["Servant of the Conduit", "Aether Hub"], 2, Some(0))]
#[case::one_land_spares_one_energy(4, "{W}{U}", &["Aether Hub", "Plains"], 1, Some(0))]
#[case::one_land_and_no_energy(2, "{W}{U}", &["Aether Hub", "Plains"], 0, None)]
#[case::two_of_three_sources_pay_energy(3, "{U}{W}{R}", &["Aether Hub", "Servant of the Conduit", "Island"], 1, None)]
#[case::two_of_three_sources_exactly(3, "{U}{W}{R}", &["Aether Hub", "Servant of the Conduit", "Island"], 2, Some(0))]
#[case::plain_taps_pay_no_energy(2, "{1}{1}{1}", &["Aether Hub", "Aether Hub", "Aether Hub"], 2, Some(2))]
fn a_spell_is_offered_only_while_the_energy_covers_every_source_it_taps(
    #[case] seats: usize,
    #[case] cost: &str,
    #[case] sources: &[&str],
    #[case] held: u32,
    #[case] left: Option<u32>,
) -> Result<(), ScriptError> {
    let table = Scenario::new(seats)
        .hand(0, [spell(cost)])
        .battlefield(0, sources.iter().map(|name| permanent(name)))
        .counters(0, energy_kind(), held);
    let mut driver = Driver::from_scenario(table.clone())?;
    assert_eq!(
        castable(&driver),
        left.is_some(),
        "{cost} with {held} energy"
    );
    if let Some(left) = left {
        driver.cast("Spell")?;
        assert_eq!(energy(&driver), left);
        replays(&driver, &table);
    }
    Ok(())
}

/// The mana abilities at priority: each is a source on its own, so the energy they would pay together is not judged
/// there; paying for a spell is.
#[rstest]
#[case(2, 1)]
#[case(3, 2)]
#[case(4, 2)]
fn each_mana_ability_is_offered_alone_while_it_can_be_paid(
    #[case] seats: usize,
    #[case] held: u32,
) -> Result<(), ScriptError> {
    let table = Scenario::new(seats)
        .battlefield(0, [permanent("Aether Hub"), permanent("Aether Hub")])
        .counters(0, energy_kind(), held);
    let driver = Driver::from_scenario(table)?;
    let any_colour = abilities(&driver).iter().filter(|o| o.energy > 0).count();
    assert_eq!(any_colour, 10, "five colours for each of the two Hubs");
    Ok(())
}

/// Seat 0 controls a Servant of the Conduit (the only green source) and the creature with `ability`, holding `held`
/// energy, at `seats` seats.
fn with_servant(seats: usize, ability: &str, held: u32) -> Scenario {
    Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(creature("Payer", ability)),
                permanent("Servant of the Conduit"),
            ],
        )
        .counters(0, energy_kind(), held)
}

const ENERGY_AND_MANA: &str = r#"[{"Mana":"{G}"},{"PayEnergy":1}]"#;

#[rstest]
#[case::none(2, 0, false)]
#[case::one_short(3, 1, false)]
#[case::exactly(4, 2, true)]
#[case::more(2, 3, true)]
fn an_ability_that_pays_energy_and_mana_leaves_the_mana_the_energy_it_needs(
    #[case] seats: usize,
    #[case] held: u32,
    #[case] works: bool,
) -> Result<(), ScriptError> {
    let ability = format!(
        r#"{{"Activated":{{"cost":{ENERGY_AND_MANA},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    );
    let table = with_servant(seats, &ability, held);
    let mut driver = Driver::from_scenario(table.clone())?;
    if !works {
        assert!(driver.begin_activation("Payer").is_err());
        assert_eq!(energy(&driver), held);
        return Ok(());
    }
    driver.begin_activation("Payer")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(
        energy(&driver),
        held - 2,
        "one for the cost, one for the Servant"
    );
    replays(&driver, &table);
    Ok(())
}

#[rstest]
#[case::none(2, 0, false)]
#[case::one_short(3, 1, false)]
#[case::exactly(4, 2, true)]
#[case::more(2, 3, true)]
fn an_effect_that_asks_for_energy_and_mana_leaves_the_mana_the_energy_it_needs(
    #[case] seats: usize,
    #[case] held: u32,
    #[case] works: bool,
) -> Result<(), ScriptError> {
    let ability = format!(
        r#"{{"Activated":{{"cost":[],"effects":[{{"Pay":{{"cost":{ENERGY_AND_MANA},"paid":[{{"GainLife":{{"amount":1}}}}],"unpaid":[]}}}}]}}}}"#
    );
    let table = with_servant(seats, &ability, held);
    let mut driver = Driver::from_scenario(table.clone())?;
    driver.begin_activation("Payer")?;
    while !matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
        driver.pass()?;
    }
    let DecisionKind::YesNo(question) = &driver.decision()?.kind else {
        unreachable!("a yes-or-no question was waited for");
    };
    assert_eq!(question.only_no, !works, "{}", question.text);
    if works {
        driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        assert_eq!(energy(&driver), held - 2);
        replays(&driver, &table);
    }
    Ok(())
}
