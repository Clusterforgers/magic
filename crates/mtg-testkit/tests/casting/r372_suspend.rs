//! Goal: a card that gains suspend in exile (`Effect::GainSuspend`) plays as a suspended card at tables of two to four
//! seats: Epochrasite dies into exile with three time counters, loses one at each of its owner's upkeeps and no other,
//! and is cast free when the last is removed, entering with its counters and haste and without suspend; The Tenth Doctor
//! exiles cards until a nonland one and suspends only that one, and a card that has suspend already does not gain it again
//! (one counter goes at each upkeep, not two); The Parting of the Ways suspends each nonland card of the five it exiles
//! with a counter for each mana value; The Eleventh Doctor suspends the card its controller picks from their hand, or
//! none when they decline; Rory Williams exiles itself as it is cast from a hand, investigates, and is cast from exile at
//! last with haste and without the trigger; Delay exiles the spell it counters with three time counters and suspend;
//! Taigam copies the second spell of a turn and suspends the original, so only the copy resolves; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, burn, forest, giant, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid};

fn r372_time() -> CounterKind {
    CounterKind::Other("time".to_owned())
}

/// The id of the card called `name` in `zone`, if it is there.
fn r372_find(driver: &Driver, zone: Zone, name: &str) -> Option<ObjectId> {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
}

/// The time counters on the card called `name` in exile.
fn r372_counters(driver: &Driver, name: &str) -> u32 {
    let id = r372_find(driver, Zone::Exile, name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .card_counters_of(&r372_time())
}

/// How many instances of suspend the object has.
fn r372_suspends(driver: &Driver, id: ObjectId) -> usize {
    keywords_of(driver.state(), id)
        .iter()
        .filter(|keyword| matches!(keyword, Keyword::Suspend { .. }))
        .count()
}

/// Answers the questions until the stack is empty and the active seat has priority: casts are taken, targets the first
/// legal ones.
fn r372_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    r372_settle_picking(driver, None)
}

/// As `r372_settle`, with the card called `pick` chosen from a hand, or every "may" declined when `pick` is `Some("")`.
fn r372_settle_picking(driver: &mut Driver, pick: Option<&str>) -> Result<(), ScriptError> {
    for _ in 0..80 {
        match driver.decision()?.kind.name() {
            "yes-no" => {
                driver.answer(Answer::YesNo(YesNoChoice(pick != Some(""))))?;
            }
            "choose-cards" if pick.is_some() => {
                let picked = pick.and_then(|name| r372_find(driver, Zone::Hand(pid(0)), name));
                driver.answer(Answer::ChooseCards(CardsChoice(
                    picked.into_iter().collect(),
                )))?;
            }
            "pay-mana" => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            "priority"
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            "priority" => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn r372_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// Seat 0's first main phase with `battlefield` ready and `top` on top of its library; every seat has a library.
fn r372_table(
    seats: usize,
    battlefield: &[&str],
    top: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Shock", 2)])
        .battlefield(
            0,
            battlefield
                .iter()
                .map(|name| ready(card(name).unwrap()))
                .chain([ready(mountain())]),
        );
    for seat in 0..seats {
        let cards: Vec<_> = if seat == 0 {
            top.iter()
                .cloned()
                .chain((0..12).map(|_| forest()))
                .collect()
        } else {
            (0..12).map(|_| forest()).collect()
        };
        scenario = scenario.library(seat, cards);
    }
    scenario
}

#[rstest]
fn r372_epochrasite_dies_into_suspend_and_returns_with_haste(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = r372_table(seats, &["Epochrasite"], Vec::new());
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let target = driver.target_named("Epochrasite")?;
    driver.cast_at("Shock", &[target])?;
    r372_settle(&mut driver)?;
    let exiled = r372_find(&driver, Zone::Exile, "Epochrasite").expect("it is exiled");
    assert_eq!(r372_counters(&driver, "Epochrasite"), 3);
    assert_eq!(r372_suspends(&driver, exiled), 1, "it gained suspend");
    for (round, left) in (1u32..).zip([2, 1]) {
        let turn = 1 + round * u32::try_from(seats).unwrap();
        // The turns of the other seats remove nothing.
        driver.idle_until(turn - 1, Step::Upkeep)?;
        assert_eq!(r372_counters(&driver, "Epochrasite"), left + 1);
        driver.idle_until(turn, Step::Upkeep)?;
        r372_settle(&mut driver)?;
        assert_eq!(r372_counters(&driver, "Epochrasite"), left);
    }
    driver.idle_until(1 + 3 * u32::try_from(seats).unwrap(), Step::Upkeep)?;
    r372_settle(&mut driver)?;
    let back = r372_find(&driver, Zone::Battlefield, "Epochrasite").expect("it was cast");
    let permanent = driver.state().object(back).unwrap();
    assert_eq!(
        permanent
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne),
        3
    );
    assert!(keywords_of(driver.state(), back).contains(&&Keyword::Haste));
    assert_eq!(r372_suspends(&driver, back), 0, "suspend stayed in exile");
    assert_eq!(names(driver.state(), Zone::Exile), Vec::<String>::new());
    r372_replays(&driver, scenario);
    Ok(())
}

#[rstest]
#[case::a_plain_creature_gains_suspend("Grizzly Bears", 2)]
#[case::a_card_with_suspend_keeps_one("Rift Bolt", 2)]
fn r372_tenth_doctor_suspends_the_nonland_card_it_finds(
    #[values(2, 3, 4)] seats: usize,
    #[case] found: &str,
    #[case] after_one_upkeep: u32,
) -> Result<(), ScriptError> {
    let found_card = if found == "Grizzly Bears" {
        bear()
    } else {
        card(found).unwrap()
    };
    let scenario = r372_table(
        seats,
        &["The Tenth Doctor"],
        vec![mountain(), mountain(), found_card],
    );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("The Tenth Doctor", 1)])?;
    r372_settle(&mut driver)?;
    let mut exiled = names(driver.state(), Zone::Exile);
    exiled.sort();
    let mut expected = vec![
        "Mountain".to_owned(),
        "Mountain".to_owned(),
        found.to_owned(),
    ];
    expected.sort();
    assert_eq!(exiled, expected, "the lands stay exiled without counters");
    let id = r372_find(&driver, Zone::Exile, found).unwrap();
    assert_eq!(r372_counters(&driver, found), 3);
    assert_eq!(r372_counters(&driver, "Mountain"), 0);
    assert_eq!(r372_suspends(&driver, id), 1);
    let lands = driver
        .state()
        .zone(Zone::Exile)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == "Mountain")
        .filter(|id| r372_suspends(&driver, **id) > 0)
        .count();
    assert_eq!(lands, 0, "only the card found gains suspend");
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    r372_settle(&mut driver)?;
    assert_eq!(r372_counters(&driver, found), after_one_upkeep);
    r372_replays(&driver, scenario);
    Ok(())
}

/// The time counters and instances of suspend of each card in exile, by name, sorted.
fn r372_exiled(driver: &Driver) -> Vec<(String, u32, usize)> {
    let state = driver.state();
    let mut found: Vec<(String, u32, usize)> = state
        .zone(Zone::Exile)
        .iter()
        .map(|id| {
            (
                name_of(state, *id),
                state.object(*id).unwrap().card_counters_of(&r372_time()),
                r372_suspends(driver, *id),
            )
        })
        .collect();
    found.sort();
    found
}

#[rstest]
fn r372_parting_of_the_ways_suspends_each_nonland_card_by_its_mana_value(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let top = vec![
        mountain(),
        bear(),
        card("Rift Bolt").unwrap(),
        mountain(),
        giant(),
    ];
    let scenario = r372_table(seats, &[], top)
        .hand(0, [card("The Parting of the Ways").unwrap()])
        .battlefield(0, (0..6).map(|_| ready(mountain())));
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("The Parting of the Ways")?;
    r372_settle(&mut driver)?;
    assert_eq!(
        r372_exiled(&driver),
        [
            ("Craw Wurm".to_owned(), 5, 1),
            ("Grizzly Bears".to_owned(), 2, 1),
            ("Mountain".to_owned(), 0, 0),
            ("Mountain".to_owned(), 0, 0),
            ("Rift Bolt".to_owned(), 3, 1),
        ]
    );
    r372_replays(&driver, scenario);
    Ok(())
}

#[rstest]
#[case::a_creature_card("Grizzly Bears", &[("Grizzly Bears", 2, 1)])]
#[case::a_card_with_suspend("Rift Bolt", &[("Rift Bolt", 3, 1)])]
#[case::declined("", &[])]
fn r372_eleventh_doctor_suspends_the_card_picked_from_hand(
    #[values(2, 3, 4)] seats: usize,
    #[case] pick: &str,
    #[case] expected: &[(&str, u32, usize)],
) -> Result<(), ScriptError> {
    let scenario = r372_table(seats, &["The Eleventh Doctor"], Vec::new())
        .hand(0, [bear(), card("Rift Bolt").unwrap()]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("The Eleventh Doctor", 1)])?;
    driver.pass_until(Step::CombatDamage)?;
    r372_settle_picking(&mut driver, Some(pick))?;
    assert_eq!(r372_life(&driver, 1), 17, "the Doctor dealt its damage");
    let expected: Vec<(String, u32, usize)> = expected
        .iter()
        .map(|(name, counters, suspends)| ((*name).to_owned(), *counters, *suspends))
        .collect();
    assert_eq!(r372_exiled(&driver), expected);
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))).len(),
        2 - expected.len()
    );
    r372_replays(&driver, scenario);
    Ok(())
}

/// The life of the player of `seat`.
fn r372_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
fn r372_rory_williams_suspends_itself_and_returns_with_haste(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = r372_table(seats, &[], Vec::new())
        .hand(0, [card("Rory Williams").unwrap()])
        .battlefield(0, [ready(plains()), ready(island())]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Rory Williams")?;
    r372_settle(&mut driver)?;
    assert_eq!(r372_exiled(&driver), [("Rory Williams".to_owned(), 3, 1)]);
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|name| *name == "Clue")
            .count(),
        1
    );
    for round in 1..=3 {
        driver.idle_until(1 + round * u32::try_from(seats).unwrap(), Step::Upkeep)?;
        r372_settle(&mut driver)?;
    }
    let rory = r372_find(&driver, Zone::Battlefield, "Rory Williams").expect("cast from exile");
    assert!(keywords_of(driver.state(), rory).contains(&&Keyword::Haste));
    assert_eq!(
        r372_exiled(&driver),
        [],
        "a cast from exile does not trigger"
    );
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|name| *name == "Clue")
            .count(),
        1
    );
    r372_replays(&driver, scenario);
    Ok(())
}

#[rstest]
#[case::a_creature_spell("Grizzly Bears", ("Grizzly Bears".to_owned(), 3, 1))]
#[case::a_spell_with_suspend("Rift Bolt", ("Rift Bolt".to_owned(), 3, 1))]
fn r372_delay_suspends_the_spell_it_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] countered: &str,
    #[case] expected: (String, u32, usize),
) -> Result<(), ScriptError> {
    let spell = if countered == "Grizzly Bears" {
        bear()
    } else {
        card(countered).unwrap()
    };
    let scenario = r372_table(seats, &[], Vec::new())
        .hand(0, [spell, card("Delay").unwrap()])
        .battlefield(
            0,
            [forest(), forest(), island(), island(), mountain()]
                .into_iter()
                .map(ready),
        );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    if countered == "Grizzly Bears" {
        driver.cast(countered)?;
    } else {
        let at = driver.target_player(1);
        driver.cast_at(countered, &[at])?;
    }
    let spell = Target::Object(
        driver
            .state()
            .object(r372_find(&driver, Zone::Stack, countered).unwrap())
            .unwrap()
            .reference(),
    );
    driver.cast_at("Delay", &[spell])?;
    r372_settle(&mut driver)?;
    assert_eq!(r372_exiled(&driver), [expected]);
    assert_eq!(r372_life(&driver, 1), 20, "the countered spell did nothing");
    r372_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn r372_taigam_copies_the_second_spell_and_suspends_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = r372_table(seats, &["Taigam, Master Opportunist"], Vec::new())
        .hand(0, [burn("Shock", 2), burn("Spark", 1)])
        .battlefield(
            0,
            [
                ready(card("Taigam, Master Opportunist").unwrap()),
                ready(mountain()),
                ready(mountain()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let at = driver.target_player(1);
    driver.cast_at("Shock", &[at])?;
    r372_settle(&mut driver)?;
    assert_eq!(r372_exiled(&driver), [], "the first spell is not copied");
    driver.cast_at("Spark", &[at])?;
    r372_settle(&mut driver)?;
    assert_eq!(r372_life(&driver, 1), 17, "Shock and the copy of Spark");
    assert_eq!(r372_exiled(&driver), [("Spark".to_owned(), 4, 1)]);
    r372_replays(&driver, scenario);
    Ok(())
}
