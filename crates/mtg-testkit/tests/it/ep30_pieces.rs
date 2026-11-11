//! Goal: the engine pieces of ep30 play by the rules at tables of two and three seats. "As an additional cost to cast this spell, you may exile any number of white cards from your hand. This spell costs {2} less to cast for each card exiled this way." asks how many, exiles that many and lowers the generic mana by two each; "move all +1/+1 counters from all creatures onto it" gathers every one; "move any number of +1/+1 counters" lets the controller stop; and a shield over "you and/or permanents you control" shares its budget between the player and the permanents.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{NumberChoice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

const REPEATABLE: &str = r#"{"AdditionalCost":{"cost":[{"ExileFromHand":{"filter":[{"Color":"White"}]}}],"repeatable":true}}"#;
const LESS: &str = r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{2}","per":"AdditionalPaid"}}"#;
const GAIN: &str = r#"{"Spell":{"effects":[{"GainLife":{"amount":"AdditionalPaid"}}]}}"#;

/// A sorcery for {4}{W}: you may exile any number of white cards, {2} less each; you gain 1 life for each.
fn march() -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("March")
        .with_type(CardType::Sorcery)
        .with_mana_cost(cost(4, &[Color::White]));
    for rule in [REPEATABLE, LESS, GAIN] {
        card = card.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    Arc::new(card)
}

/// Passes until the stack is empty.
fn resolve(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats * 3 {
        if driver.state().zone(Zone::Stack).is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::none(0, 5)]
#[case::one(1, 3)]
#[case::two(2, 1)]
fn exiling_any_number_lowers_the_cost_for_each(
    #[case] exiled: u32,
    #[case] mana_value: u32,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let white = || creature("Page", 1, &[Color::White], 1, 1);
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, (0..5).map(|_| ready(plains())).collect::<Vec<_>>())
        .hand(0, [march(), white(), white(), creature("Ogre", 1, &[Color::Red], 1, 1)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("March")?;
    let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind else {
        panic!("the times come first");
    };
    // Two white cards in hand: no more payments than that.
    assert_eq!((options.min, options.max), (0, Some(2)));
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(exiled)))?;
    for _ in 0..4 {
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.pay_options()?.cost.mana_value(), mana_value);
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    resolve(&mut driver, seats)?;
    let me = driver.state().player(PlayerId(0)).unwrap();
    assert_eq!(me.life(), 20 + i32::try_from(exiled).unwrap());
    let pages_in_exile = driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .filter(|id| crate::common::name_of(driver.state(), **id) == "Page")
        .count();
    assert_eq!(pages_in_exile, exiled as usize);
    Ok(())
}

fn counters_on(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne)
}

const GATHER: &str = r#"{"Activated":{"cost":"{0}","effects":[{"MoveCounters":{"from":{"Each":[{"Type":"Creature"}]},"to":"This","counter":"+1/+1","count":{"Counters":{"on":{"Each":[{"Type":"Creature"}]},"counter":"+1/+1"}}}}]}}"#;

fn cannibal() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Cannibal")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 0)
            .with_rules(serde_json::from_str::<Ability>(GATHER).unwrap()),
    )
}

#[rstest]
fn all_counters_from_all_creatures_gather_on_one(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let plus = CounterKind::PlusOnePlusOne;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            vec![
                with_counters(ready(cannibal()), plus.clone(), 1),
                with_counters(ready(creature("Wolf", 0, &[], 2, 2)), plus.clone(), 2),
            ],
        )
        .battlefield(
            seats - 1,
            vec![with_counters(ready(creature("Bear", 0, &[], 2, 2)), plus, 3)],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Cannibal")?;
    resolve(&mut driver, seats)?;
    assert_eq!(counters_on(&driver, "Cannibal"), 6);
    assert_eq!(counters_on(&driver, "Wolf"), 0);
    assert_eq!(counters_on(&driver, "Bear"), 0);
    Ok(())
}

const ANY_NUMBER: &str = r#"{"Activated":{"cost":"{0}","targets":[{"what":{"Object":[{"Type":"Creature"},"Other"]}}],"effects":[{"MoveCounters":{"from":"This","to":{"Target":0},"counter":"+1/+1","count":{"Counters":{"on":"This","counter":"+1/+1"}},"up_to":true}}]}}"#;

fn bandar() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Bandar")
            .with_type(CardType::Creature)
            .with_power_toughness(0, 0)
            .with_rules(serde_json::from_str::<Ability>(ANY_NUMBER).unwrap()),
    )
}

#[rstest]
#[case::none(0)]
#[case::two(2)]
#[case::all_three(3)]
fn any_number_of_counters_stops_where_the_controller_says(
    #[case] moved: u32,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            vec![
                with_counters(ready(bandar()), CounterKind::PlusOnePlusOne, 3),
                ready(creature("Wolf", 0, &[], 2, 2)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Bandar")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        let wolf = driver.target_named("Wolf")?;
        driver.choose_targets(&[wolf])?;
    }
    let mut picked = 0;
    for _ in 0..seats * 6 {
        if driver.state().zone(Zone::Stack).is_empty() {
            break;
        }
        match &driver.decision()?.kind {
            // One counter at a time, then "Stop".
            DecisionKind::ChooseOption(options) => {
                let pick = if picked < moved {
                    picked += 1;
                    0
                } else {
                    options.options.len() - 1
                };
                driver.answer(Answer::ChooseOption(OptionChoice(pick)))?;
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    assert_eq!(counters_on(&driver, "Wolf"), moved);
    assert_eq!(counters_on(&driver, "Bandar"), 3 - moved);
    Ok(())
}

/// A free instant: the next 3 damage to `to` this turn is prevented, then 2 damage to its caster and 2 to the Wolf.
fn wall(to: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"Spell":{{"effects":[{{"Shield":{{"when":{{"Damage":{{"to":{to}}}}},"by":{{"PreventUpTo":3}},"duration":"UntilEndOfTurn"}}}},{{"DealDamage":{{"to":"You","amount":2}}}},{{"DealDamage":{{"to":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}},"amount":2}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Wall")
            .with_type(CardType::Instant)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

const YOURS: &str = r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#;

#[rstest]
#[case::you_and_your_creatures(format!(r#"{{"Union":["You",{YOURS}]}}"#), 20, 1)]
#[case::you_alone(r#""You""#.to_owned(), 20, 2)]
#[case::your_creatures_alone(YOURS.to_owned(), 18, 0)]
fn two_recipients_share_one_shield(
    #[case] to: String,
    #[case] life: i32,
    #[case] wolf: u32,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(creature("Wolf", 0, &[], 3, 3))])
        .hand(0, [wall(&to)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Wall")?;
    resolve(&mut driver, seats)?;
    assert_eq!(driver.state().player(PlayerId(0)).unwrap().life(), life);
    let id = driver.id_of("Wolf")?;
    let damage = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .damage();
    assert_eq!(damage, wolf);
    Ok(())
}
