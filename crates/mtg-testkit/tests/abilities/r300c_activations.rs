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
use mtg_core::mana::ManaType;
use mtg_engine::characteristics::{self, characteristics};
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::mana::Payment;
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
        .map(|(_, n)| n)
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
#[case::treasure_paid("Jetmir's Fixer", true, &["Forest"], 1, 0)]
#[case::lands_paid("Jetmir's Fixer", false, &["Mountain", "Forest"], 0, 1)]
#[case::three_colors("Paragon of Modernity", false, &["Plains", "Island", "Swamp"], 1, 0)]
#[case::two_colors("Paragon of Modernity", false, &["Island", "Island", "Swamp"], 0, 1)]
#[case::treasure_red_and_two_colors("Paragon of Modernity", true, &["Island", "Swamp"], 1, 0)]
fn r300c_the_mana_of_an_activation_decides_counter_or_pump(
    #[case] name: &str,
    #[case] treasure: bool,
    #[case] paying: &[&str],
    #[case] counters: u32,
    #[case] pumped: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
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
        ))?;
    let before = r300c_power(&driver, name);
    if treasure {
        driver.tap_for_mana_of("Treasure", ManaType::Red)?;
    }
    driver.begin_activation(name)?;
    let DecisionKind::PayMana(options) = &driver.decision()?.kind else {
        panic!("the activation asks for mana");
    };
    let mut taps = Vec::new();
    for land in paying {
        let found = options
            .sources
            .iter()
            .find(|o| {
                !taps.contains(&o.source)
                    && driver.state().object(o.source.object).unwrap().card().name == *land
            })
            .unwrap()
            .source;
        taps.push(found);
    }
    let pool = if treasure {
        vec![ManaType::Red]
    } else {
        Vec::new()
    };
    driver.answer(Answer::PayMana(PayChoice::Manual(Payment {
        pool,
        taps,
        ..Payment::default()
    })))?;
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
    let bear = driver.id_of("Grizzly Bears")?;
    driver.begin_activation("Rubinia Soulsinger")?;
    let target = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[target])?;
    r300c_resolve(&mut driver)?;
    assert_eq!(
        characteristics::controller_of(driver.state(), bear),
        Some(pid(0))
    );
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
    assert_eq!(
        characteristics::controller_of(driver.state(), bear),
        Some(holder)
    );
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
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
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
    let driver = Driver::from_scenario(Scenario::new(seats).at(2, 0, Step::Main1).battlefield(
        0,
        [ready(card("Clockwork Avian").unwrap()), ready(mountain())],
    ))?;
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
                [
                    ready(card("Marshland Bloodcaster").unwrap()),
                    ready(swamp()),
                    ready(swamp()),
                ],
            )
            .hand(
                0,
                vec![
                    creature("Ogre", 3, &[], 3, 3),
                    creature("Elk", 2, &[], 2, 2),
                ],
            ),
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

/// A planeswalker of the one type with three loyalty counters.
fn r300c_walker(name: &str, kind: &str) -> mtg_engine::setup::PresetPermanent {
    with_counters(
        ready(Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Planeswalker)
                .with_subtype(kind)
                .with_mana_cost(cost(3, &[])),
        )),
        CounterKind::Loyalty,
        3,
    )
}

#[rstest]
#[case::none(&[], 0)]
#[case::one(&["Jace"], 1)]
#[case::one_type_twice(&["Jace", "Jace"], 1)]
#[case::two_types(&["Jace", "Chandra"], 2)]
fn r300c_tam_proliferates_once_for_each_planeswalker_type(
    #[case] kinds: &[&str],
    #[case] times: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![
        ready(card("Tam, the Possibility").unwrap()),
        with_counters(ready(bear()), CounterKind::PlusOnePlusOne, 1),
        ready(plains()),
        ready(island()),
        ready(swamp()),
        ready(mountain()),
        ready(forest()),
    ];
    for (at, kind) in kinds.iter().enumerate() {
        mine.push(r300c_walker(&format!("Walker {at}"), kind));
    }
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, mine),
    )?;
    driver.begin_activation("Tam, the Possibility")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options) => {
                let bear = driver.id_of("Grizzly Bears")?;
                let chosen = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| *id == bear)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    assert_eq!(
        r300c_counters(&driver, "Grizzly Bears", &CounterKind::PlusOnePlusOne),
        1 + times
    );
    r300c_replays(&driver);
    Ok(())
}

#[rstest]
fn r300c_orcish_librarian_exiles_half_of_the_eight_at_random(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let library: Vec<_> = (0..12)
        .map(|at| creature(&format!("Card {at}"), 1, &[], 1, 1))
        .collect();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .library(0, library)
            .battlefield(
                0,
                [ready(card("Orcish Librarian").unwrap()), ready(mountain())],
            ),
    )?;
    driver.begin_activation("Orcish Librarian")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let exiled = driver
        .state()
        .objects()
        .filter(|object| object.zone() == mtg_engine::zones::Zone::Exile)
        .count();
    assert_eq!(exiled, 4);
    let library = driver
        .state()
        .objects()
        .filter(|object| object.zone() == mtg_engine::zones::Zone::Library(pid(0)))
        .count();
    assert_eq!(library, 8);
    r300c_replays(&driver);
    Ok(())
}
