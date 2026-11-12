//! Goal: the r30-0c activated abilities play as printed at tables of two, three and four seats: an activation keeps the
//! mana it spent, so Jetmir's Fixer puts a counter instead of its pump only when a Treasure paid and Paragon of Modernity
//! only when exactly three colors paid; Rubinia keeps a creature only while she stays tapped under her controller; a
//! Clockwork Avian puts up to X +1/+0 counters but never past four, and only during its controller's upkeep; and the log
//! replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::characteristics::{self, characteristics};
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, cost, creature, forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::pid;

/// An artifact Treasure: "{T}, Sacrifice this artifact: Add one mana of any color."
fn r300c_treasure() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Treasure")
            .with_type(CardType::Artifact)
            .with_subtype("Treasure")
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

/// A free instant: "Untap target permanent."
fn r300c_untapper() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Untapper")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"Untap":{"what":{"Target":0}}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

fn r300c_counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .filter(|(held, _)| *held == kind)
        .map(|(_, n)| *n)
        .sum()
}

fn r300c_power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

fn r300c_resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

fn r300c_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::treasure_paid("Jetmir's Fixer", &["Treasure", "Forest"], 1, 0)]
#[case::lands_paid("Jetmir's Fixer", &["Mountain", "Forest"], 0, 1)]
#[case::three_colors("Paragon of Modernity", &["Plains", "Island", "Swamp"], 1, 0)]
#[case::two_colors("Paragon of Modernity", &["Island", "Island", "Swamp"], 0, 1)]
fn r300c_the_mana_of_an_activation_decides_counter_or_pump(
    #[case] name: &str,
    #[case] paying: &[&str],
    #[case] counters: u32,
    #[case] pumped: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
            0,
            [
                ready(card(name).unwrap()),
                ready(r300c_treasure()),
                ready(mountain()),
                ready(forest()),
                ready(plains()),
                ready(island()),
                ready(island()),
                ready(swamp()),
            ],
        ),
    )?;
    let before = r300c_power(&driver, name);
    driver.begin_activation(name)?;
    driver.pay_tapping(paying, 0)?;
    r300c_resolve(&mut driver)?;
    assert_eq!(
        r300c_counters(&driver, name, &CounterKind::PlusOnePlusOne),
        counters
    );
    assert_eq!(
        r300c_power(&driver, name) - before,
        pumped + i32::try_from(counters).unwrap()
    );
    r300c_replays(&driver);
    Ok(())
}

#[rstest]
fn r300c_rubinia_keeps_a_creature_while_she_stays_tapped(
    #[values(false, true)] untapped: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(card("Rubinia Soulsinger").unwrap())])
            .hand(0, vec![r300c_untapper()])
            .battlefield(1, [ready(bear())]),
    )?;
    let bear = driver.id_of("Bear")?;
    driver.begin_activation("Rubinia Soulsinger")?;
    let target = driver.target_named("Bear")?;
    driver.choose_targets(&[target])?;
    r300c_resolve(&mut driver)?;
    assert_eq!(characteristics::controller_of(driver.state(), bear), Some(pid(0)));
    if untapped {
        driver.begin_cast("Untapper")?;
        let rubinia = driver.target_named("Rubinia Soulsinger")?;
        driver.choose_targets(&[rubinia])?;
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        }
        r300c_resolve(&mut driver)?;
    }
    let holder = if untapped { pid(1) } else { pid(0) };
    assert_eq!(characteristics::controller_of(driver.state(), bear), Some(holder));
    r300c_replays(&driver);
    Ok(())
}

#[rstest]
#[case::room_for_x(1, 2, 2, 3)]
#[case::capped(3, 3, 1, 4)]
#[case::full(4, 2, 0, 4)]
fn r300c_a_clockwork_never_passes_its_cap(
    #[case] held: u32,
    #[case] x: u32,
    #[case] most: u32,
    #[case] after: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let kind = CounterKind::Other("+1/+0".into());
    let avian = with_counters(ready(card("Clockwork Avian").unwrap()), kind.clone(), held);
    let lands = (0..4).map(|_| ready(mountain()));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Upkeep)
            .battlefield(0, std::iter::once(avian).chain(lands)),
    )?;
    driver.begin_activation("Clockwork Avian")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    driver.pass()?;
    if let DecisionKind::ChooseNumber(options) = &driver.decision()?.kind {
        assert_eq!(options.max, Some(most));
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(most)))?;
    } else {
        assert_eq!(most, 0, "the number is asked while there is room");
    }
    r300c_resolve(&mut driver)?;
    assert_eq!(r300c_counters(&driver, "Clockwork Avian", &kind), after);
    r300c_replays(&driver);
    Ok(())
}

#[rstest]
fn r300c_a_clockwork_winds_only_in_its_controllers_upkeep(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(0, [ready(card("Clockwork Avian").unwrap()), ready(mountain())]),
    )?;
    assert!(driver.offered_abilities("Clockwork Avian")?.is_empty());
    Ok(())
}

#[rstest]
fn r300c_marshland_bloodcaster_lets_the_next_spell_be_paid_in_life(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [ready(card("Marshland Bloodcaster").unwrap()), ready(swamp()), ready(swamp())],
            )
            .hand(0, vec![creature("Ogre", 3, &[], 3, 3), creature("Elk", 2, &[], 2, 2)]),
    )?;
    driver.begin_activation("Marshland Bloodcaster")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r300c_resolve(&mut driver)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.begin_cast("Ogre")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{0}");
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    r300c_resolve(&mut driver)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life - 3);
    // Only the next spell: the Elk has no life cost left and no mana to pay for it.
    assert!(driver.begin_cast("Elk").is_err() || driver.pay_options()?.cost.to_string() != "{0}");
    r300c_replays(&driver);
    Ok(())
}
