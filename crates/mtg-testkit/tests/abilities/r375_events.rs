//! Goal: the r37-5 trigger events play as printed at tables of two, three and four seats: Font of Agonies takes a
//! blood counter for each life its controller pays and none for an opponent's; Veteran Motorist pumps the Vehicle it
//! crews and not one another creature crews; Skull Skaab makes a Zombie when its exploit takes a nontoken creature
//! (itself included) and none when it takes nothing; Zidane makes a Treasure when an opponent gains control of a
//! permanent of its controller and none when its controller gains one; Nicanzil lets its controller put a land from
//! hand when a creature of theirs explores a land card and grows when one explores a nonland card; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{name_of, pid};
use mtg_cards::value::CounterKind;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers until the stack is empty: a pick of cards takes the ones named in `picks` (none when it allows none), a
/// yes-or-no question is answered `yes` and the rest passively.
fn r375_settle(driver: &mut Driver, picks: &[&str], yes: bool) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::ChooseCards(options) => {
                let chosen: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| picks.contains(&name_of(driver.state(), *id).as_str()))
                    .take(usize::try_from(options.max).unwrap())
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn r375_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0's first main phase; every seat has a library.
fn r375_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Main1);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| bear()));
    }
    scenario
}

/// How many permanents with the name the seat controls.
fn r375_count(driver: &Driver, seat: usize, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state.object(**id).is_some_and(|o| o.card().name == name)
                && mtg_engine::characteristics::controller_of(state, **id) == Some(pid(seat))
        })
        .count()
}

#[rstest]
fn r375_font_counts_the_life_its_controller_pays(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] mine: bool,
) -> Result<(), ScriptError> {
    let font_seat = usize::from(!mine);
    let scenario = r375_table(seats)
        .battlefield(font_seat, [ready(card("Font of Agonies").unwrap())])
        .hand(0, [card("Gitaxian Probe").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Gitaxian Probe")?;
    let them = driver.target_player(1);
    driver.choose_targets(&[them])?;
    driver.pay_tapping(&[], 2)?;
    r375_settle(&mut driver, &[], true)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 18);
    let font = driver.id_of("Font of Agonies")?;
    let blood = driver
        .state()
        .object(font)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Other("blood".to_owned()));
    assert_eq!(blood, if mine { 2 } else { 0 });
    r375_replays(&driver);
    Ok(())
}

#[rstest]
fn r375_motorist_pumps_the_vehicle_it_crews(
    #[values(2, 3, 4)] seats: usize,
    #[values("Veteran Motorist", "Grizzly Bears")] crew: &str,
) -> Result<(), ScriptError> {
    let scenario = r375_table(seats).battlefield(
        0,
        [
            ready(card("Veteran Motorist").unwrap()),
            ready(card("Smuggler's Copter").unwrap()),
            ready(bear()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Smuggler's Copter")?;
    r375_settle(&mut driver, &[crew], true)?;
    let copter = driver.id_of("Smuggler's Copter")?;
    let seen = characteristics(driver.state(), copter).unwrap();
    let bonus = i32::from(crew == "Veteran Motorist");
    assert_eq!(seen.power, Some(3 + bonus));
    assert_eq!(seen.toughness, Some(3 + bonus));
    r375_replays(&driver);
    Ok(())
}

#[rstest]
#[case::a_nontoken_creature(&["Grizzly Bears"], 1)]
#[case::itself(&["Skull Skaab"], 1)]
#[case::nothing(&[], 0)]
fn r375_skaab_makes_a_zombie_for_a_nontoken_exploited(
    #[values(2, 3, 4)] seats: usize,
    #[case] sacrificed: &[&str],
    #[case] zombies: usize,
) -> Result<(), ScriptError> {
    let scenario = r375_table(seats)
        .battlefield(0, [ready(bear()), ready(island()), ready(swamp())])
        .hand(0, [card("Skull Skaab").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Skull Skaab")?;
    r375_settle(&mut driver, sacrificed, !sacrificed.is_empty())?;
    assert_eq!(r375_count(&driver, 0, "Zombie"), zombies);
    r375_replays(&driver);
    Ok(())
}

#[rstest]
#[case::with_an_opponent(1, 1)]
#[case::between_my_own(0, 0)]
fn r375_zidane_makes_a_treasure_when_an_opponent_takes_a_permanent(
    #[values(2, 3, 4)] seats: usize,
    #[case] other: usize,
    #[case] treasures: usize,
) -> Result<(), ScriptError> {
    let elves = || ready(card("Llanowar Elves").unwrap());
    let mut mine = vec![
        ready(card("Zidane, Tantalus Thief").unwrap()),
        ready(bear()),
    ];
    mine.extend((0..5).map(|_| ready(island())));
    if other == 0 {
        mine.push(elves());
    }
    let mut scenario = r375_table(seats).battlefield(0, mine);
    if other != 0 {
        scenario = scenario.battlefield(other, [elves()]);
    }
    let scenario = scenario.hand(0, [card("Switcheroo").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Switcheroo")?;
    let targets = [
        driver.target_named("Grizzly Bears")?,
        driver.target_named("Llanowar Elves")?,
    ];
    driver.choose_targets(&targets)?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    r375_settle(&mut driver, &[], true)?;
    assert_eq!(r375_count(&driver, 0, "Treasure"), treasures);
    r375_replays(&driver);
    Ok(())
}

#[rstest]
#[case::a_land(true)]
#[case::a_nonland_card(false)]
fn r375_nicanzil_answers_what_an_explore_reveals(
    #[values(2, 3, 4)] seats: usize,
    #[case] land: bool,
) -> Result<(), ScriptError> {
    let top = if land { forest() } else { bear() };
    let scenario = r375_table(seats)
        .battlefield(
            0,
            [
                ready(card("Nicanzil, Current Conductor").unwrap()),
                ready(forest()),
                ready(forest()),
            ],
        )
        .hand(0, [card("Merfolk Branchwalker").unwrap(), island()])
        .library(0, [top, bear(), bear()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Merfolk Branchwalker")?;
    r375_settle(&mut driver, &["Island"], true)?;
    let nicanzil = driver.id_of("Nicanzil, Current Conductor")?;
    let grown = driver
        .state()
        .object(nicanzil)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(grown, u32::from(!land));
    let island_down = driver.id_of("Island").ok();
    assert_eq!(island_down.is_some(), land);
    if let Some(island) = island_down {
        assert!(
            driver
                .state()
                .object(island)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped()
        );
    }
    r375_replays(&driver);
    Ok(())
}
