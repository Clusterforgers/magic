//! Goal: the engine pieces of ep29 play by the rules at tables of two and three seats. "{T}, Remove all elixir counters from ~: You gain 2 life for each elixir counter removed this way" takes every counter of the kind, none included, and the effect reads how many it took; "You may cast this spell as though it had flash if you pay {2} more to cast it" casts the card on another player's turn for its mana cost and {2} more, on its owner's main phase for its mana cost alone, and not at all on another turn without the permission.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

const BOTTLE: &str = r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"counter":{"Other":"elixir"},"count":{"Counters":{"on":"This","counter":{"Other":"elixir"}}}}}],"effects":[{"GainLife":{"amount":{"Times":{"by":2,"of":{"RemovedThisWay":{"Other":"elixir"}}}}}}]}}"#;

fn elixir() -> CounterKind {
    CounterKind::Other("elixir".into())
}

fn bottle() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(BOTTLE).unwrap();
    Arc::new(
        CardDefinition::new("Bottle")
            .with_type(CardType::Artifact)
            .with_rules(rule),
    )
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
fn removing_all_counters_counts_what_it_removed(
    #[values(0, 1, 3)] counters: u32,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![with_counters(ready(bottle()), elixir(), counters)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Bottle")?;
    resolve(&mut driver, seats)?;
    let life = driver.state().player(PlayerId(0)).unwrap().life();
    assert_eq!(life, 20 + 2 * i32::try_from(counters).unwrap());
    let id = driver.id_of("Bottle")?;
    assert_eq!(
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&elixir()),
        0
    );
    Ok(())
}

const PAID_FLASH: &str = r#"{"Rule":{"rule":{"MayCast":{"what":["This"],"flash":true,"additional":[{"Mana":"{2}"}]}},"from":"Hand"}}"#;

/// A creature for `{G}` its owner may cast as though it had flash by paying {2} more.
fn sprinter(permission: bool) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Sprinter")
        .with_type(CardType::Creature)
        .with_mana_cost(cost(0, &[Color::Green]))
        .with_power_toughness(1, 1);
    if permission {
        card = card.with_rules(serde_json::from_str::<Ability>(PAID_FLASH).unwrap());
    }
    Arc::new(card)
}

/// Passes until `seat` has priority and says whether it may cast the sprinter.
fn may_cast(driver: &mut Driver, seat: usize, seats: usize) -> Result<bool, ScriptError> {
    for _ in 0..seats {
        if driver.decision()?.player.index() == seat {
            break;
        }
        driver.pass()?;
    }
    let decision = driver.decision()?;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("the seat has priority");
    };
    Ok(options
        .castable
        .iter()
        .any(|id| name_of(driver.state(), *id) == "Sprinter"))
}

#[rstest]
#[case::another_turn_pays_two_more(true, false, Some(3))]
#[case::own_main_phase_pays_the_mana_cost(true, true, Some(1))]
#[case::another_turn_without_the_permission(false, false, None)]
fn flash_for_two_more(
    #[case] permission: bool,
    #[case] own_turn: bool,
    #[case] tapped: Option<usize>,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let holder = seats - 1;
    let active = if own_turn { holder } else { 0 };
    let scenario = Scenario::new(seats)
        .at(1, active, Step::Main1)
        .battlefield(holder, (0..4).map(|_| ready(forest())).collect::<Vec<_>>())
        .hand(holder, [sprinter(permission)]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(may_cast(&mut driver, holder, seats)?, tapped.is_some());
    let Some(tapped) = tapped else {
        return Ok(());
    };
    driver.cast("Sprinter")?;
    let state = driver.state();
    let used = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).unwrap().permanent().unwrap().tapped())
        .count();
    assert_eq!(used, tapped, "the mana paid");
    resolve(&mut driver, seats)?;
    let entered = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| name_of(driver.state(), *id) == "Sprinter");
    assert!(entered, "the cast resolves");
    Ok(())
}

const ROLLER: &str = r#"{"Activated":{"cost":[],"effects":[{"RollDice":{"sides":6,"results":[{"from":1,"effects":[{"GainLife":{"amount":{"DieResult":"Only"}}}]}]}}]}}"#;
const ADVANTAGE: &str = r#"{"Rule":{"rule":{"ExtraDie":"You"}}}"#;

fn permanent(name: &str, rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(rules).unwrap()),
    )
}

/// The life seat 0 gains from one roll of a d6 with `advantages` rules that add a die, and the dice logged as rolled.
fn rolled(seed: u64, advantages: usize, seats: usize) -> Result<(i32, Vec<u32>), ScriptError> {
    let mut permanents = vec![ready(permanent("Roller", ROLLER))];
    for at in 0..advantages {
        permanents.push(ready(permanent(&format!("Guide {at}"), ADVANTAGE)));
    }
    let scenario = Scenario::new(seats)
        .seed(seed)
        .at(1, 0, Step::Main1)
        .battlefield(0, permanents);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Roller")?;
    resolve(&mut driver, seats)?;
    let gained = driver.state().player(PlayerId(0)).unwrap().life() - 20;
    let dice = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            mtg_engine::Event::DieRolled { result, .. } => Some(*result),
            _ => None,
        })
        .collect();
    Ok((gained, dice))
}

/// "Roll that many dice plus one and ignore the lowest roll": one die is logged and read, the highest of the dice rolled,
/// so over many seeds the results run higher with each rule that adds a die and never fall outside the die.
#[rstest]
fn an_extra_die_keeps_the_highest(#[values(2, 3)] seats: usize) -> Result<(), ScriptError> {
    let mut totals = [0i32; 3];
    for seed in 0..40 {
        for (advantages, total) in totals.iter_mut().enumerate() {
            let (gained, dice) = rolled(seed, advantages, seats)?;
            assert_eq!(dice.len(), 1, "one roll counts");
            assert_eq!(
                i64::from(gained),
                i64::from(dice[0]),
                "the result read is the one logged"
            );
            assert!((1..=6).contains(&gained));
            *total += gained;
        }
    }
    assert!(totals[0] < totals[1] && totals[1] < totals[2], "{totals:?}");
    Ok(())
}
