//! Goal: the l291 cards whose rules join existing pieces play as printed at tables of two to four seats: Arcbound Tracker grows from the second spell of each turn on, Lich's Tomb sacrifices one permanent for each life lost, Kitnap stuns only without the promise, Longhorn Sharpshooter deals its damage as it becomes plotted, Underworld Cerberus exiles itself and returns every creature card to its owner's hand; and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::CostsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::object::PermanentState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::{bear, burn, free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid};

/// Answers passively until the stack and the waiting triggers are empty.
fn l291_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..300 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0's first main phase of turn 2 with `mine` on its battlefield and `hand` in its hand; every seat has a library and
/// every other seat a Grizzly Bears.
fn l291_table(
    seats: usize,
    mine: Vec<PresetPermanent>,
    hand: Vec<Arc<CardDefinition>>,
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, mine)
        .hand(0, hand);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
        if seat > 0 {
            scenario = scenario.battlefield(seat, [ready(bear())]);
        }
    }
    scenario
}

fn l291_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// The counters of `kind` on the permanent named `name`.
fn l291_counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| name_of(state, **id) == name)
        .and_then(|id| state.object(*id))
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(kind))
}

#[rstest]
fn l291_arcbound_tracker_grows_from_the_second_spell(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand: Vec<_> = (0..3)
        .map(|n| free_creature(&format!("Spell {n}"), 1, 1))
        .collect();
    let scenario = || {
        l291_table(
            seats,
            vec![PresetPermanent {
                counters: vec![(CounterKind::PlusOnePlusOne, 2)],
                ..ready(card("Arcbound Tracker").unwrap())
            }],
            hand.clone(),
        )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let plus = CounterKind::PlusOnePlusOne;
    let start = l291_counters(&driver, "Arcbound Tracker", &plus);
    for (n, grown) in [0, 1, 2].into_iter().enumerate() {
        driver.cast(&format!("Spell {n}"))?;
        l291_settle(&mut driver)?;
        assert_eq!(
            l291_counters(&driver, "Arcbound Tracker", &plus) - start,
            grown,
            "after spell {n}"
        );
    }
    l291_replays(&driver, scenario());
    Ok(())
}

#[rstest]
fn l291_lichs_tomb_sacrifices_one_permanent_for_each_life(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] damage: i32,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Lich's Tomb").unwrap()), ready(mountain())];
    mine.extend((0..4).map(|_| ready(bear())));
    let scenario = || l291_table(seats, mine.clone(), vec![burn("Self Burn", damage)]);
    let mut driver = Driver::from_scenario(scenario())?;
    let mine = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| driver.state().object(**id).unwrap().controller() == pid(0))
            .count()
    };
    let before = mine(&driver);
    let me = driver.target_player(0);
    driver.cast_at("Self Burn", &[me])?;
    l291_settle(&mut driver)?;
    assert_eq!(
        mine(&driver),
        before - usize::try_from(damage).unwrap(),
        "one permanent for each life lost"
    );
    l291_replays(&driver, scenario());
    Ok(())
}

#[rstest]
fn l291_kitnap_stuns_only_without_the_promise(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] promised: bool,
) -> Result<(), ScriptError> {
    let mine: Vec<_> = (0..4).map(|_| ready(island())).collect();
    let scenario = || l291_table(seats, mine.clone(), vec![card("Kitnap").unwrap()]);
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Kitnap")?;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseCosts(options) => {
                let costs = if promised {
                    vec![options.optional[0].index]
                } else {
                    Vec::new()
                };
                driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let bear = driver.target_named("Grizzly Bears")?;
                driver.choose_targets(&[bear])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
                break;
            }
            _ => panic!("unexpected {}", decision.kind.name()),
        }
    }
    l291_settle(&mut driver)?;
    let stun = CounterKind::Other("stun".to_owned());
    let state = driver.state();
    let bear = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            name_of(state, *id) == "Grizzly Bears"
                && mtg_engine::characteristics::controller_of(state, *id) == Some(pid(0))
        })
        .expect("the bear is seat 0's");
    let permanent = state.object(bear).unwrap().permanent().unwrap();
    assert!(permanent.tapped(), "tapped");
    assert_eq!(permanent.counters_of(&stun), if promised { 0 } else { 3 });
    l291_replays(&driver, scenario());
    Ok(())
}

#[rstest]
fn l291_longhorn_sharpshooter_deals_damage_as_it_becomes_plotted(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mine: Vec<_> = (0..4).map(|_| ready(mountain())).collect();
    let scenario = || {
        l291_table(
            seats,
            mine.clone(),
            vec![card("Longhorn Sharpshooter").unwrap()],
        )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let life = |driver: &Driver| -> i32 {
        (0..seats)
            .map(|seat| driver.state().player(pid(seat)).unwrap().life())
            .sum()
    };
    let damaged = |driver: &Driver| -> u32 {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| driver.state().object(*id)?.permanent())
            .map(PermanentState::damage)
            .sum()
    };
    let before = life(&driver);
    let id = driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == "Longhorn Sharpshooter")
        .expect("in hand");
    driver.answer(Answer::Priority(PriorityAction::Plot(id)))?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    l291_settle(&mut driver)?;
    assert!(
        names(driver.state(), Zone::Exile).contains(&"Longhorn Sharpshooter".to_owned()),
        "plotted"
    );
    let dealt = u32::try_from(before - life(&driver)).unwrap() + damaged(&driver);
    let died = (1..seats)
        .filter(|seat| {
            names(driver.state(), Zone::Graveyard(pid(*seat))).contains(&"Grizzly Bears".to_owned())
        })
        .count();
    assert_eq!(
        dealt + 2 * u32::try_from(died).unwrap(),
        2,
        "two damage once"
    );
    l291_replays(&driver, scenario());
    Ok(())
}

#[rstest]
fn l291_underworld_cerberus_returns_every_creature_card(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario_of = || {
        let mut scenario = l291_table(
            seats,
            vec![
                ready(card("Underworld Cerberus").unwrap()),
                ready(mountain()),
            ],
            vec![burn("Big Burn", 6)],
        );
        for seat in 0..seats {
            scenario = scenario.graveyard(seat, [bear(), island()]);
        }
        scenario
    };
    let mut driver = Driver::from_scenario(scenario_of())?;
    let cerberus = driver.target_named("Underworld Cerberus")?;
    driver.cast_at("Big Burn", &[cerberus])?;
    l291_settle(&mut driver)?;
    assert!(names(driver.state(), Zone::Exile).contains(&"Underworld Cerberus".to_owned()));
    for seat in 0..seats {
        let graveyard = names(driver.state(), Zone::Graveyard(pid(seat)));
        assert!(
            !graveyard.contains(&"Grizzly Bears".to_owned()),
            "seat {seat}"
        );
        assert!(graveyard.contains(&"Island".to_owned()), "seat {seat}");
        assert!(
            names(driver.state(), Zone::Hand(pid(seat))).contains(&"Grizzly Bears".to_owned()),
            "seat {seat}"
        );
    }
    l291_replays(&driver, scenario_of());
    Ok(())
}

/// An artifact with "{0}: Put a +1/+1 counter on target creature."
fn l291_pusher() -> Arc<CardDefinition> {
    let put: mtg_cards::ability::Ability = serde_json::from_str(
        r#"{"Activated":{"cost":"{0}","targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Counter Pusher")
            .with_type(mtg_core::card::CardType::Artifact)
            .with_rules(put),
    )
}

#[rstest]
fn l291_kros_taps_a_creature_you_do_not_control_that_gets_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] theirs: bool,
) -> Result<(), ScriptError> {
    let mine = vec![
        ready(card("Kros, Defense Contractor").unwrap()),
        ready(l291_pusher()),
        ready(free_creature("My Bear", 2, 2)),
    ];
    let scenario = || l291_table(seats, mine.clone(), Vec::new());
    let mut driver = Driver::from_scenario(scenario())?;
    let name = if theirs { "Grizzly Bears" } else { "My Bear" };
    driver.begin_activation("Counter Pusher")?;
    let target = driver.target_named(name)?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    l291_settle(&mut driver)?;
    let state = driver.state();
    let tapped = state
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| name_of(state, **id) == name)
        .and_then(|id| state.object(*id)?.permanent())
        .is_some_and(PermanentState::tapped);
    assert_eq!(tapped, theirs, "only a creature you don't control");
    l291_replays(&driver, scenario());
    Ok(())
}
