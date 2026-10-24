//! Goal: a keyword counter gives its permanent the keyword while it is there (CR 122.1b), at tables of two, three and four seats: each of the fourteen counters gives its own keyword and no other, two counters of one keyword give it once, a counter of another kind gives none, a permanent that comes back to the battlefield with a counter has the keyword (Unbreakable Bond), a copy of a permanent with a counter has no counter and so no keyword, a proliferate adds a counter and the keyword stays, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, pid};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::keywords_of;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, free_creature, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

fn counter(name: &str) -> CounterKind {
    CounterKind::Other(name.to_owned())
}

/// The keywords of the permanent called `name`, as words.
fn keywords(driver: &Driver, name: &str) -> Vec<String> {
    let id = driver.id_of(name).unwrap();
    keywords_of(driver.state(), id)
        .into_iter()
        .map(|keyword| format!("{keyword:?}"))
        .collect()
}

fn held(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(kind)
}

/// Seat 0 controls a Wolf with these counters and a plain Boar; every other seat has a library.
fn table(seats: usize, counters: &[(CounterKind, u32)]) -> Scenario {
    let wolf = counters
        .iter()
        .fold(ready(free_creature("Wolf", 2, 2)), |wolf, (kind, count)| {
            with_counters(wolf, kind.clone(), *count)
        });
    let mut scenario =
        Scenario::new(seats).battlefield(0, [wolf, ready(free_creature("Boar", 3, 3))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| island()));
    }
    scenario
}

#[rstest]
#[case::flying("flying", Keyword::Flying)]
#[case::first_strike("first strike", Keyword::FirstStrike)]
#[case::double_strike("double strike", Keyword::DoubleStrike)]
#[case::deathtouch("deathtouch", Keyword::Deathtouch)]
#[case::haste("haste", Keyword::Haste)]
#[case::hexproof("hexproof", Keyword::Hexproof)]
#[case::indestructible("indestructible", Keyword::Indestructible)]
#[case::lifelink("lifelink", Keyword::Lifelink)]
#[case::menace("menace", Keyword::Menace)]
#[case::reach("reach", Keyword::Reach)]
#[case::shadow("shadow", Keyword::Shadow)]
#[case::skulk("skulk", Keyword::Skulk)]
#[case::trample("trample", Keyword::Trample)]
#[case::vigilance("vigilance", Keyword::Vigilance)]
fn each_keyword_counter_gives_its_keyword_to_the_permanent_that_holds_it(
    #[case] name: &str,
    #[case] keyword: Keyword,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(seats, &[(counter(name), 1)]))?;
    assert_eq!(keywords(&driver, "Wolf"), [format!("{keyword:?}")]);
    assert!(keywords(&driver, "Boar").is_empty(), "only the Wolf has it");
    assert_eq!(counter(name).keyword(), Some(keyword));
    Ok(())
}

#[rstest]
#[case::two_of_one_keyword(&[("flying", 2)], &["Flying"])]
#[case::two_keywords(&[("flying", 1), ("lifelink", 1)], &["Flying", "Lifelink"])]
#[case::a_counter_the_rules_give_no_keyword(&[("shield", 1), ("charge", 2)], &[])]
#[case::a_counter_of_the_cards_kind(&[("quest", 3)], &[])]
fn counters_give_each_keyword_once_and_other_counters_none(
    #[case] counters: &[(&str, u32)],
    #[case] expected: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let counters: Vec<_> = counters
        .iter()
        .map(|(name, count)| (counter(name), *count))
        .collect();
    let driver = Driver::from_scenario(table(seats, &counters))?;
    assert_eq!(keywords(&driver, "Wolf"), expected);
    Ok(())
}

/// Seat 0 holds Unbreakable Bond over two Plains-less lands (the spell is free) and has a Bear and a Hero in its
/// graveyard; seat 1 has a Wolf on the battlefield.
fn bond_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card("Unbreakable Bond").unwrap()])
        .battlefield(0, (0..5).map(|_| ready(swamp())))
        .graveyard(
            0,
            [
                card("Grizzly Bears").unwrap(),
                card("Hero of Bladehold").unwrap(),
            ],
        )
        .battlefield(1, [ready(free_creature("Wolf", 2, 2))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| island()));
    }
    scenario
}

fn graveyard_target(driver: &Driver, name: &str) -> Target {
    let id = driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap();
    Target::Object(driver.state().object(id).unwrap().reference())
}

#[rstest]
fn a_creature_returned_with_a_lifelink_counter_has_lifelink(
    #[values(2, 3, 4)] seats: usize,
    #[values("Grizzly Bears", "Hero of Bladehold")] returned: &str,
) -> Result<(), ScriptError> {
    let setup = bond_table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(bond_table(seats))?;
    driver.begin_cast("Unbreakable Bond")?;
    let target = graveyard_target(&driver, returned);
    driver.choose_targets(&[target])?;
    driver.pay_tapping(&["Swamp"; 5], 0)?;
    for _ in 0..seats {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    assert!(driver.state().stack().is_empty());
    assert_eq!(held(&driver, returned, &counter("lifelink")), 1);
    assert!(keywords(&driver, returned).contains(&"Lifelink".to_owned()));
    // The other creature card stayed where it was and the Wolf has nothing.
    assert!(keywords(&driver, "Wolf").is_empty());
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(0))).len(), 2);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
fn a_copy_of_a_permanent_with_a_keyword_counter_has_no_counter_and_no_keyword(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [card("Cytoshape").unwrap()])
            .battlefield(
                0,
                [
                    ready(forest()),
                    ready(island()),
                    ready(island()),
                    ready(card("Grizzly Bears").unwrap()),
                ],
            )
            .battlefield(
                1,
                [with_counters(
                    ready(card("Hero of Bladehold").unwrap()),
                    counter("flying"),
                    1,
                )],
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let bears = driver.id_of("Grizzly Bears")?;
    let hero = driver.id_of("Hero of Bladehold")?;
    assert!(keywords(&driver, "Hero of Bladehold").contains(&"Flying".to_owned()));
    let target = Target::Object(driver.state().object(bears).unwrap().reference());
    driver.cast_at("Cytoshape", &[target])?;
    for _ in 0..seats {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)) {
            break;
        }
        driver.pass()?;
    }
    driver.answer(Answer::ChooseCards(CardsChoice(vec![hero])))?;
    for _ in 0..seats {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    let copy = driver.state().object(bears).unwrap();
    assert_eq!(copy.card().name, "Hero of Bladehold");
    let copy_keywords: Vec<String> = keywords_of(driver.state(), bears)
        .into_iter()
        .map(|keyword| format!("{keyword:?}"))
        .collect();
    assert!(
        !copy_keywords.contains(&"Flying".to_owned()),
        "{copy_keywords:?}"
    );
    assert!(keywords_of(driver.state(), hero).contains(&&Keyword::Flying));
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

fn passes_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        && !driver.state().stack().is_empty()
    {
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
fn a_proliferate_adds_a_keyword_counter_and_the_keyword_stays_once(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        let wolf = with_counters(ready(free_creature("Wolf", 2, 2)), counter("flying"), 1);
        Scenario::new(seats)
            .hand(0, [card("Contentious Plan").unwrap()])
            .battlefield(0, [wolf, ready(island()), ready(island())])
            .library(0, (0..8).map(|_| island()))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Contentious Plan")?;
    passes_until_asked(&mut driver)?;
    let wolf: ObjectId = driver.id_of("Wolf")?;
    driver.answer(Answer::ChooseCards(CardsChoice(vec![wolf])))?;
    passes_until_asked(&mut driver)?;
    assert_eq!(held(&driver, "Wolf", &counter("flying")), 2);
    assert_eq!(keywords(&driver, "Wolf"), ["Flying"]);
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::Proliferated { .. }))
    );
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
