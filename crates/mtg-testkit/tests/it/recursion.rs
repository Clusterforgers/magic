//! Goal: scavenge, transmute, awaken, gravestorm and sunburst play as the rules say at two to four seats. Scavenge exiles the card from a graveyard for its cost and puts as many +1/+1 counters as its power on a creature, at sorcery speed only; transmute discards the card for its cost and finds a card of the same mana value, at sorcery speed only; awaken is an alternative cost that adds a land target and turns that land into a 0/0 Elemental creature with haste and counters, and a spell cast for its mana cost does neither; gravestorm copies the spell once for each permanent put into a graveyard this turn; sunburst gives one counter for each color of mana spent, +1/+1 on a creature and charge on anything else; each log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, lands, pay, ways};
use crate::common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameState, characteristics, replay};
use mtg_testkit::cards::{bear, creature, forest, island, mountain, swamp, wastes};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers the orders of triggers and passes priority until nothing is on the stack and a player holds priority.
fn settle_all(driver: &mut Driver) -> Result<(), ScriptError> {
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        || !driver.state().stack().is_empty()
    {
        driver.answer_passively()?;
    }
    Ok(())
}

/// The log replays to the state the game is in.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// The counters of a kind on a permanent.
fn counters(state: &GameState, id: ObjectId, kind: &CounterKind) -> u32 {
    state
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(kind))
}

/// The cards called `name` in `zone`.
fn count(state: &GameState, zone: Zone, name: &str) -> usize {
    state
        .zone(zone)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

/// Seat 0 activates the ability of `card` aimed at `at` (nothing when `None`), pays and lets everything resolve.
fn activate(driver: &mut Driver, card: &str, at: Option<&str>) -> Result<(), ScriptError> {
    driver.begin_activation(card)?;
    if let Some(at) = at {
        let target = driver.target_named(at)?;
        driver.choose_targets(&[target])?;
    }
    pay(driver)?;
    settle_all(driver)
}

/// The basic land called `name`.
fn basic(name: &str) -> Arc<CardDefinition> {
    match name {
        "Island" => island(),
        "Swamp" => swamp(),
        "Mountain" => mountain(),
        _ => wastes(),
    }
}

/// A basic land and `count` more of the same, untapped.
fn basics(
    count: usize,
    land: fn() -> Arc<CardDefinition>,
) -> impl Iterator<Item = mtg_engine::PresetPermanent> {
    lands(count, land).into_iter()
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn scavenge_exiles_the_card_and_puts_counters_equal_to_its_power(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .graveyard_named(0, ["Dreg Mangler"])
            .battlefield(
                0,
                basics(3, swamp)
                    .chain(basics(2, forest))
                    .chain([ready(bear())]),
            ),
    )?;
    activate(&mut driver, "Dreg Mangler", Some("Grizzly Bears"))?;
    let bears = driver.id_of("Grizzly Bears")?;
    assert_eq!(counters(driver.state(), bears, &CounterKind::PlusOnePlusOne), 3);
    assert_eq!(count(driver.state(), Zone::Graveyard(pid(0)), "Dreg Mangler"), 0);
    assert_eq!(count(driver.state(), Zone::Exile, "Dreg Mangler"), 1);
    let stats = characteristics(driver.state(), bears).unwrap();
    assert_eq!((stats.power, stats.toughness), (Some(5), Some(5)));
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::another_players_creature(1)]
#[case::the_last_seat(2)]
fn scavenge_may_target_a_creature_of_another_player(
    #[case] owner: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(3)
            .graveyard_named(0, ["Drudge Beetle"])
            .battlefield(0, basics(6, forest))
            .battlefield(owner, [ready(creature("Fodder", 1, &[], 1, 1))]),
    )?;
    activate(&mut driver, "Drudge Beetle", Some("Fodder"))?;
    let fodder = driver.id_of("Fodder")?;
    let power = characteristics(driver.state(), fodder).unwrap().power;
    assert_eq!(power, Some(1 + 2), "a 1/1 and the two counters of a 2/2 Beetle");
    Ok(())
}

#[rstest]
#[case::without_the_mana(Step::Main1, 4)]
#[case::outside_a_main_phase(Step::Upkeep, 5)]
fn scavenge_is_a_sorcery_that_needs_its_mana(
    #[case] step: Step,
    #[case] mana: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, step)
            .graveyard_named(0, ["Dreg Mangler"])
            .battlefield(
                0,
                basics(mana.min(3), swamp)
                    .chain(basics(mana - mana.min(3), forest))
                    .chain([ready(bear())]),
            ),
    )?;
    assert!(driver.begin_activation("Dreg Mangler").is_err());
    Ok(())
}

#[rstest]
#[case::same_value_two("Dimir Infiltrator", &["Suntouched Myr", "Muddle the Mixture", "Clutch of Currents"], &["Muddle the Mixture"], "Muddle the Mixture")]
#[case::two_of_that_value("Dimir Infiltrator", &["Dimir Infiltrator", "Muddle the Mixture", "Suntouched Myr"], &["Dimir Infiltrator", "Muddle the Mixture"], "Dimir Infiltrator")]
#[case::a_three_drop("Drift of Phantasms", &["Suntouched Myr", "Dimir Infiltrator", "Clutch of Currents"], &["Suntouched Myr"], "Suntouched Myr")]
#[case::nothing_of_that_value("Drift of Phantasms", &["Dimir Infiltrator", "Clutch of Currents"], &[], "")]
fn transmute_finds_a_card_with_the_same_mana_value(
    #[case] card: &str,
    #[case] library: &[&str],
    #[case] offered: &[&str],
    #[case] pick: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, [card])
            .library_named(0, library.iter().copied())
            .battlefield(0, basics(3, island).chain(basics(2, swamp))),
    )?;
    driver.begin_activation(card)?;
    pay(&mut driver)?;
    assert_eq!(
        count(driver.state(), Zone::Graveyard(pid(0)), card),
        1,
        "discarded as a cost"
    );
    let mut seen = Vec::new();
    for _ in 0..30 {
        if let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
            seen = options
                .from
                .iter()
                .map(|id| name_of(driver.state(), *id))
                .collect();
            seen.sort();
            let picked: Vec<ObjectId> = options
                .from
                .iter()
                .copied()
                .filter(|id| name_of(driver.state(), *id) == pick)
                .take(1)
                .collect();
            driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
        } else if driver.state().stack().is_empty() {
            break;
        } else {
            driver.pass()?;
        }
    }
    assert_eq!(seen, offered, "only cards of the same mana value are offered");
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(hand, if pick.is_empty() { vec![] } else { vec![pick.to_owned()] });
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::without_the_mana(Step::Main1, 2)]
#[case::outside_a_main_phase(Step::Upkeep, 3)]
fn transmute_is_a_sorcery_that_needs_its_mana(
    #[case] step: Step,
    #[case] mana: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, step)
            .hand_named(0, ["Drift of Phantasms"])
            .library_named(0, ["Suntouched Myr"])
            .battlefield(0, basics(mana, island)),
    )?;
    assert!(driver.begin_activation("Drift of Phantasms").is_err());
    Ok(())
}

/// Seat 0 holds `spell` with `mana` islands and one forest in play; seat 1 has a bear.
fn awaken_table(seats: usize, spell: &str, mana: usize) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, [spell])
            .battlefield(0, basics(mana, island).chain([ready(forest())]))
            .battlefield(1, [ready(bear())]),
    )
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn awaken_adds_a_land_that_becomes_an_elemental_with_counters_and_haste(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = awaken_table(seats, "Clutch of Currents", 5)?;
    driver.begin_cast("Clutch of Currents")?;
    assert_eq!(ways(&driver)?.len(), 2, "the mana cost and the awaken cost");
    choose_way(&mut driver, "Awaken")?;
    let (bears, land) = (driver.target_named("Grizzly Bears")?, driver.target_named("Forest")?);
    driver.choose_targets(&[bears])?;
    driver.choose_targets(&[land])?;
    pay(&mut driver)?;
    settle_all(&mut driver)?;
    let forest = driver.id_of("Forest")?;
    assert_eq!(counters(driver.state(), forest, &CounterKind::PlusOnePlusOne), 3);
    let chars = characteristics(driver.state(), forest).unwrap();
    assert!(chars.types.contains(CardType::Land) && chars.is_creature());
    assert_eq!((chars.power, chars.toughness), (Some(3), Some(3)));
    assert!(chars.subtypes.contains(&Subtype("Elemental".to_owned())));
    assert!(has_keyword(driver.state(), forest, &Keyword::Haste));
    assert_eq!(names(driver.state(), Zone::Hand(pid(1))), ["Grizzly Bears"]);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn a_spell_cast_for_its_mana_cost_asks_for_no_land_and_changes_none(
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = awaken_table(seats, "Clutch of Currents", 5)?;
    driver.begin_cast("Clutch of Currents")?;
    choose_way(&mut driver, "Cast for")?;
    let bears = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bears])?;
    pay(&mut driver)?;
    settle_all(&mut driver)?;
    let forest = driver.id_of("Forest")?;
    assert_eq!(counters(driver.state(), forest, &CounterKind::PlusOnePlusOne), 0);
    assert!(!characteristics(driver.state(), forest).unwrap().is_creature());
    assert_eq!(names(driver.state(), Zone::Hand(pid(1))), ["Grizzly Bears"]);
    Ok(())
}

#[test]
fn the_awaken_cost_is_offered_only_with_the_mana_for_it() -> Result<(), ScriptError> {
    let mut driver = awaken_table(2, "Clutch of Currents", 3)?;
    driver.begin_cast("Clutch of Currents")?;
    assert!(
        matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)),
        "one way is paid, so no way is asked"
    );
    Ok(())
}

#[rstest]
#[case::nothing_dies(0)]
#[case::one_permanent(1)]
#[case::two_permanents(2)]
#[case::three_permanents(3)]
fn gravestorm_copies_the_spell_for_each_permanent_put_into_a_graveyard(
    #[case] dying: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Rising Miasma", "Ominous Harvest"])
        .battlefield(0, basics(7, swamp));
    for seat in 1..seats {
        scenario = scenario.library(seat, (0..12).map(|_| mountain()));
    }
    let mut doomed: Vec<Vec<_>> = (0..seats).map(|_| Vec::new()).collect();
    for index in 0..dying {
        let creature = creature(&format!("Doomed {index}"), 1, &[], 1, 1);
        doomed[1 + index % (seats - 1)].push(ready(creature));
    }
    for (seat, creatures) in doomed.into_iter().enumerate().skip(1) {
        scenario = scenario.battlefield(seat, creatures);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Rising Miasma")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(&mut driver, "Cast for")?;
    }
    pay(&mut driver)?;
    settle_all(&mut driver)?;
    let target = driver.target_player(1);
    driver.begin_cast("Ominous Harvest")?;
    driver.choose_targets(&[target])?;
    pay(&mut driver)?;
    settle_all(&mut driver)?;
    let opponent = driver.state().player(pid(1)).unwrap();
    assert_eq!(opponent.life(), 20 - 1 - i32::try_from(dying).unwrap());
    assert_eq!(driver.state().zone(Zone::Hand(pid(1))).len(), 1 + dying);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::one_color(&["Island", "Island", "Island"], 1)]
#[case::two_colors(&["Island", "Swamp", "Island"], 2)]
#[case::three_colors(&["Island", "Swamp", "Mountain"], 3)]
#[case::no_color(&["Wastes", "Wastes", "Wastes"], 0)]
fn sunburst_counts_the_colors_of_mana_spent_on_a_creature(
    #[case] paying: &[&str],
    #[case] colors: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Suntouched Myr"])
            .battlefield(0, paying.iter().map(|name| ready(basic(name)))),
    )?;
    driver.cast("Suntouched Myr")?;
    settle_all(&mut driver)?;
    match driver.id_of("Suntouched Myr") {
        Ok(myr) if colors > 0 => {
            assert_eq!(
                counters(driver.state(), myr, &CounterKind::PlusOnePlusOne),
                colors
            );
            let stats = characteristics(driver.state(), myr).unwrap();
            assert_eq!(stats.power, Some(i32::try_from(colors).unwrap()));
        }
        _ => {
            assert_eq!(colors, 0, "a Myr without a counter is a 0/0 and dies");
            assert_eq!(count(driver.state(), Zone::Graveyard(pid(0)), "Suntouched Myr"), 1);
        }
    }
    assert_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_colors(&["Island", "Swamp"], 2)]
#[case::one_color(&["Mountain", "Mountain"], 1)]
fn sunburst_gives_charge_counters_to_a_permanent_that_is_no_creature(
    #[case] paying: &[&str],
    #[case] colors: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand_named(0, ["Pentad Prism"])
            .battlefield(0, paying.iter().map(|name| ready(basic(name)))),
    )?;
    driver.cast("Pentad Prism")?;
    settle_all(&mut driver)?;
    let prism = driver.id_of("Pentad Prism")?;
    assert_eq!(counters(driver.state(), prism, &CounterKind::Charge), colors);
    assert_eq!(counters(driver.state(), prism, &CounterKind::PlusOnePlusOne), 0);
    Ok(())
}
