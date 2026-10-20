//! Goal: destroying, exiling and countering hold up on a busy stack at tables of two, three and four seats: a spell whose target was destroyed, exiled or countered first, or has stopped being a legal target, does nothing and ends fizzled; a countered spell has already set off what triggers on a cast and its leaving the stack sets off what watches the stack; a Wrath of God lets every creature that dies see every other die, triggers once per watcher and death and puts the triggers on the stack in turn order; state-based actions follow a resolution that ends what kept a creature alive; and the log of a game that casts all four cards replays to the live state, is rebuilt at every decision and after every step of a resolution, and goes on the same way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::removal::{
    Aim, Step, check_rebuilds, corpus, moves, owner_of, pass_to, play_script, resolve_the_stack,
    spell_named, table, titan,
};
use common::{name_of, names, pid};
use mtg_cards::refs::{Selector as Matching, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::ObjectId;
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::events::{DeathCause, ResolutionOutcome, ZoneCause};
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event, replay};
use mtg_testkit::cards::{burn, free_creature, gain_life, island, mountain, plains, swamp};
use mtg_testkit::trigger_cards::{artifact_costing, trigger, watcher, when};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// How each resolution since `from` ended: the object and the outcome, in the order they ended.
fn endings(driver: &Driver, from: usize) -> Vec<(ObjectId, ResolutionOutcome)> {
    driver.engine().events()[from..]
        .iter()
        .filter_map(|event| match event {
            Event::ResolutionEnded { object, outcome } => Some((*object, *outcome)),
            _ => None,
        })
        .collect()
}

/// The events of the resolution of `object`, from its beginning to its end.
fn resolution_of(driver: &Driver, from: usize, object: ObjectId) -> Vec<Event> {
    let events = &driver.engine().events()[from..];
    let start = events
        .iter()
        .position(|e| matches!(e, Event::ResolutionBegan { object: o } if *o == object))
        .unwrap();
    let end = events
        .iter()
        .position(|e| matches!(e, Event::ResolutionEnded { object: o, .. } if *o == object))
        .unwrap();
    events[start..=end].to_vec()
}

/// The seats' life totals.
fn lives(driver: &Driver, seats: usize) -> Vec<i32> {
    (0..seats)
        .map(|seat| driver.state().player(pid(seat)).unwrap().life())
        .collect()
}

// A target that is gone

#[rstest]
fn a_second_doom_blade_destroys_the_creature_and_the_first_fizzles(
    #[values((2, "Grizzly Bears"), (3, "Serra Angel"), (4, "Llanowar Elves"), (4, "Wolf"))] case: (
        usize,
        &str,
    ),
) -> Result<(), ScriptError> {
    let (seats, victim) = case;
    let mut driver = Driver::from_scenario(table(
        seats,
        &["Doom Blade", "Doom Blade"],
        vec![swamp(), swamp(), swamp(), swamp()],
        Vec::new(),
    ))?;
    let owner = owner_of(&driver, victim);
    let begun = driver.engine().events().len();
    let target = driver.target_named(victim)?;
    driver.cast_at("Doom Blade", &[target])?;
    driver.cast_at("Doom Blade", &[target])?;
    let (first, second) = (driver.state().stack()[0], driver.state().stack()[1]);
    resolve_the_stack(&mut driver)?;

    assert_eq!(
        endings(&driver, begun),
        [
            (second, ResolutionOutcome::Resolved),
            (first, ResolutionOutcome::Fizzled)
        ]
    );
    assert_eq!(
        moves(&driver, begun),
        [(
            victim.to_owned(),
            Zone::Battlefield,
            Zone::Graveyard(pid(owner)),
            ZoneCause::Destroyed
        )],
        "the creature is destroyed once"
    );
    // The fizzled spell began and ended and did nothing in between, not even the step.
    let fizzled = resolution_of(&driver, begun, first);
    assert!(matches!(
        fizzled.as_slice(),
        [Event::ResolutionBegan { .. }, Event::ResolutionEnded { .. }]
    ));
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(
        graveyard.iter().filter(|n| *n == "Doom Blade").count(),
        2,
        "both spells are in their owner's graveyard"
    );
    Ok(())
}

#[rstest]
fn a_creature_that_becomes_black_in_response_is_no_longer_a_legal_target(
    #[values((2, "Grizzly Bears"), (3, "Serra Angel"), (4, "Llanowar Elves"))] case: (usize, &str),
) -> Result<(), ScriptError> {
    let (seats, victim) = case;
    let mut driver = Driver::from_scenario(table(
        seats,
        &["Doom Blade"],
        vec![swamp(), swamp()],
        Vec::new(),
    ))?;
    let begun = driver.engine().events().len();
    let id = driver.id_of(victim)?;
    let target = driver.target_named(victim)?;
    driver.cast_at("Doom Blade", &[target])?;
    let blade = driver.state().stack()[0];
    driver.add_effect(
        Selector::Object(id),
        Modification::SetColors(ColorSet::EMPTY.with(Color::Black)),
        Duration::UntilEndOfTurn,
    )?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(
        endings(&driver, begun),
        [(blade, ResolutionOutcome::Fizzled)]
    );
    assert_eq!(moves(&driver, begun), [], "the creature is left alone");
    assert!(names(driver.state(), Zone::Battlefield).contains(&victim.to_owned()));
    Ok(())
}

#[rstest]
fn a_second_swords_exiles_the_creature_and_the_first_fizzles_and_gains_nothing(
    #[values((2, "Grizzly Bears", 2), (3, "Serra Angel", 4), (4, "Llanowar Elves", 1))] case: (
        usize,
        &str,
        u32,
    ),
) -> Result<(), ScriptError> {
    let (seats, victim, power) = case;
    let mut driver = Driver::from_scenario(table(
        seats,
        &["Swords to Plowshares", "Swords to Plowshares"],
        vec![plains(), plains()],
        Vec::new(),
    ))?;
    let owner = owner_of(&driver, victim);
    let begun = driver.engine().events().len();
    let target = driver.target_named(victim)?;
    driver.cast_at("Swords to Plowshares", &[target])?;
    driver.cast_at("Swords to Plowshares", &[target])?;
    let (first, second) = (driver.state().stack()[0], driver.state().stack()[1]);
    resolve_the_stack(&mut driver)?;

    assert_eq!(
        endings(&driver, begun),
        [
            (second, ResolutionOutcome::Resolved),
            (first, ResolutionOutcome::Fizzled)
        ]
    );
    let gains: Vec<(usize, u32)> = driver.engine().events()[begun..]
        .iter()
        .filter_map(|event| match event {
            Event::LifeGained { player, amount } => Some((player.index(), *amount)),
            _ => None,
        })
        .collect();
    assert_eq!(gains, [(owner, power)], "the life is gained once");
    assert_eq!(names(driver.state(), Zone::Exile), [victim]);
    let mut expected = vec![20; seats];
    expected[owner] += i32::try_from(power).unwrap();
    assert_eq!(lives(&driver, seats), expected);
    Ok(())
}

/// Seat 0 casts a Lightning Bolt at seat 1 and `countering` holds `counters` Counterspells over four Islands.
fn counter_table(seats: usize, countering: usize, counters: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [corpus("Lightning Bolt")])
        .battlefield(0, [ready(mountain())]);
    for seat in 1..seats {
        scenario = if seat == countering {
            scenario
                .hand(seat, (0..counters).map(|_| corpus("Counterspell")))
                .battlefield(seat, [island(), island(), island(), island()].map(ready))
        } else {
            scenario.battlefield(seat, [ready(free_creature("Bystander", 1, 1))])
        };
    }
    scenario
}

#[rstest]
fn a_second_counterspell_counters_the_spell_and_the_first_fizzles(
    #[values((2, 1), (3, 2), (4, 1), (4, 3))] seating: (usize, usize),
) -> Result<(), ScriptError> {
    let (seats, countering) = seating;
    let mut driver = Driver::from_scenario(counter_table(seats, countering, 2))?;
    let begun = driver.engine().events().len();
    driver.cast_at("Lightning Bolt", &[driver.target_player(1)])?;
    let bolt = driver.state().stack()[0];
    pass_to(&mut driver, countering)?;
    let target = spell_named(&driver, "Lightning Bolt");
    driver.cast_at("Counterspell", &[target])?;
    driver.cast_at("Counterspell", &[target])?;
    let (first, second) = (driver.state().stack()[1], driver.state().stack()[2]);
    resolve_the_stack(&mut driver)?;

    assert_eq!(
        endings(&driver, begun),
        [
            (bolt, ResolutionOutcome::Countered),
            (second, ResolutionOutcome::Resolved),
            (first, ResolutionOutcome::Fizzled)
        ],
        "the top one counters the Bolt; the one below finds nothing to counter"
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(countering))),
        ["Counterspell", "Counterspell"]
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Lightning Bolt"]
    );
    assert_eq!(
        lives(&driver, seats),
        vec![20; seats],
        "the Bolt did nothing"
    );
    Ok(())
}

// What a countered spell has already set off

fn ledger_pattern() -> TriggerPattern {
    TriggerPattern::ZoneChange {
        what: Matching::default(),
        from: Some(ZoneKind::Stack),
        to: Some(ZoneKind::Graveyard),
    }
}

/// Seat 0 casts a Bolt; `countering` has two Islands and a Counterspell, and an artifact that gains 1 life whenever an
/// opponent casts a spell and one that gains 1 life whenever a spell goes from the stack to a graveyard.
fn watched_table(seats: usize, countering: usize) -> Scenario {
    let mirror = artifact_costing(
        "Mirror",
        0,
        vec![trigger(when::opponent_casts(), vec![gain_life(1)])],
    );
    let ledger = artifact_costing(
        "Ledger",
        0,
        vec![trigger(ledger_pattern(), vec![gain_life(1)])],
    );
    let mut scenario = Scenario::new(seats)
        .hand(0, [corpus("Lightning Bolt")])
        .battlefield(0, [ready(mountain())]);
    for seat in 1..seats {
        scenario = if seat == countering {
            scenario.hand(seat, [corpus("Counterspell")]).battlefield(
                seat,
                [
                    ready(island()),
                    ready(island()),
                    ready(mirror.clone()),
                    ready(ledger.clone()),
                ],
            )
        } else {
            scenario.battlefield(seat, [ready(free_creature("Bystander", 1, 1))])
        };
    }
    scenario
}

#[rstest]
fn a_countered_spell_has_set_off_what_triggers_on_a_cast_and_its_leaving_the_stack_sets_off_the_rest(
    #[values((2, 1, 1), (3, 2, 1), (3, 1, 2), (4, 3, 2), (4, 1, 1))] seating: (usize, usize, usize),
) -> Result<(), ScriptError> {
    let (seats, countering, aimed) = seating;
    let mut driver = Driver::from_scenario(watched_table(seats, countering))?;
    let begun = driver.engine().events().len();
    driver.cast_at("Lightning Bolt", &[driver.target_player(aimed)])?;
    // The cast set off the Mirror: its trigger waits on the stack above the Bolt.
    assert_eq!(driver.state().stack().len(), 2);
    pass_to(&mut driver, countering)?;
    let target = spell_named(&driver, "Lightning Bolt");
    driver.cast_at("Counterspell", &[target])?;
    resolve_the_stack(&mut driver)?;

    let state = driver.state();
    let triggered: Vec<String> = driver.engine().events()[begun..]
        .iter()
        .filter_map(|event| match event {
            Event::AbilityTriggered(trigger) => Some(name_of(state, trigger.source.id)),
            _ => None,
        })
        .collect();
    assert_eq!(
        triggered,
        ["Mirror", "Ledger", "Ledger"],
        "the cast, then the Bolt countered, then the Counterspell resolved"
    );
    let events = &driver.engine().events()[begun..];
    let countered = events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::ResolutionEnded {
                    outcome: ResolutionOutcome::Countered,
                    ..
                }
            )
        })
        .unwrap();
    let first_ledger = events
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, Event::AbilityTriggered(t) if name_of(state, t.source.id) == "Ledger"))
        .map(|(at, _)| at)
        .next()
        .unwrap();
    assert!(countered < first_ledger, "the cause is seen with the move");
    let mut expected = vec![20; seats];
    expected[countering] += 3;
    assert_eq!(
        lives(&driver, seats),
        expected,
        "all three resolved; the Bolt did nothing"
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Lightning Bolt"]
    );
    Ok(())
}

// Many deaths at once

fn mourner(seat: usize) -> std::sync::Arc<mtg_cards::CardDefinition> {
    watcher(
        &format!("Mourner {seat}"),
        when::any_creature_dies(),
        vec![gain_life(1)],
    )
}

/// Wrath of God at a table where every seat has a Mourner and the last seat an indestructible Titan.
fn mourning(seats: usize) -> Scenario {
    let mut extra: Vec<_> = (0..seats).map(|seat| (seat, mourner(seat))).collect();
    extra.push((seats - 1, titan()));
    table(
        seats,
        &["Wrath of God"],
        vec![plains(), plains(), plains(), plains()],
        extra,
    )
}

#[rstest]
fn every_creature_that_dies_in_a_wrath_is_seen_by_every_watcher_and_the_triggers_go_on_in_turn_order(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(mourning(seats))?;
    let dying = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.card().power.is_some() && o.card().name != "Titan")
        .count();
    let begun = driver.engine().events().len();
    driver.cast("Wrath of God")?;
    while driver
        .state()
        .stack()
        .iter()
        .any(|id| name_of(driver.state(), *id) == "Wrath of God")
    {
        driver.pass()?;
    }

    // Each Mourner died with the rest and still saw them all, so each triggered once per death.
    let played = &driver.engine().events()[begun..];
    let last_move = played
        .iter()
        .rposition(|e| matches!(e, Event::ZoneChanged { .. }))
        .unwrap();
    let triggers: Vec<usize> = played
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, Event::AbilityTriggered(_)))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(triggers.len(), seats * dying);
    assert!(
        triggers.iter().all(|at| *at > last_move),
        "they are found once the batch of deaths is done"
    );
    assert_eq!(
        played
            .iter()
            .filter(|e| matches!(
                e,
                Event::ZoneChanged {
                    cause: ZoneCause::Destroyed,
                    ..
                }
            ))
            .count(),
        dying
    );
    // They go on the stack in turn order from the active player: seat 0's first, so they resolve last.
    let controllers: Vec<usize> = driver
        .state()
        .stack()
        .iter()
        .map(|id| driver.state().object(*id).unwrap().controller().index())
        .collect();
    let mut by_seat = controllers.clone();
    by_seat.sort_unstable();
    assert_eq!(controllers, by_seat, "bottom to top: seat 0, seat 1, ...");
    assert_eq!(controllers.len(), seats * dying);

    resolve_the_stack(&mut driver)?;
    let gained = i32::try_from(dying).unwrap();
    assert_eq!(lives(&driver, seats), vec![20 + gained; seats]);
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|n| n.as_str() == "Titan")
            .count(),
        1
    );
    Ok(())
}

// State-based actions after

#[rstest]
fn the_creature_the_destroyed_one_kept_alive_dies_to_state_based_actions_right_after(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let last = seats - 1;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [corpus("Doom Blade"), burn("Shock", 2)])
            .battlefield(
                0,
                [
                    ready(swamp()),
                    ready(swamp()),
                    ready(mountain()),
                    ready(mourner(0)),
                ],
            )
            .battlefield(
                last,
                [
                    ready(free_creature("Lord", 2, 2)),
                    ready(free_creature("Squire", 1, 1)),
                ],
            ),
    )?;
    let (lord, squire) = (driver.id_of("Lord")?, driver.id_of("Squire")?);
    // While the Lord is there the Squire is a 3/3; two damage does not kill it.
    driver.add_effect(
        Selector::Object(squire),
        Modification::ModifyPowerToughness {
            power: 2,
            toughness: 2,
        },
        Duration::WhileSourceOnBattlefield(lord),
    )?;
    let target = driver.target_named("Squire")?;
    driver.cast_at("Shock", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Squire".to_owned()));

    let begun = driver.engine().events().len();
    let target = driver.target_named("Lord")?;
    driver.cast_at("Doom Blade", &[target])?;
    resolve_the_stack(&mut driver)?;

    let played = &driver.engine().events()[begun..];
    let at = |wanted: &dyn Fn(&Event) -> bool| played.iter().position(wanted).unwrap();
    let destroyed = at(
        &|e| matches!(e, Event::ZoneChanged { object, cause: ZoneCause::Destroyed, .. } if *object == lord),
    );
    let resolved = at(&|e| {
        matches!(
            e,
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Resolved,
                ..
            }
        )
    });
    let died = at(
        &|e| matches!(e, Event::CreatureDied { object, cause: DeathCause::LethalDamage } if *object == squire),
    );
    assert!(
        destroyed < resolved && resolved < died,
        "the Lord is destroyed by the spell and the Squire dies after it, by the state-based action"
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(last))),
        ["Lord", "Squire"]
    );
    driver.expect_life(0, 22)?; // the Mourner saw both deaths
    Ok(())
}

// Whole games

/// Seat 0 holds all but the Counterspell, which the last seat holds; every seat has a Mourner and the last a Titan.
fn game(seats: usize) -> Scenario {
    let last = seats - 1;
    let mut scenario = Scenario::new(seats)
        .hand(
            0,
            ["Doom Blade", "Swords to Plowshares", "Wrath of God"].map(corpus),
        )
        .battlefield(
            0,
            [
                ready(swamp()),
                ready(swamp()),
                ready(plains()),
                ready(plains()),
                ready(plains()),
                ready(plains()),
                ready(plains()),
                ready(free_creature("Wolf", 3, 3)),
                ready(mourner(0)),
            ],
        );
    for seat in 1..seats {
        let mut board = vec![ready(mourner(seat))];
        if seat == 1 {
            board.extend([ready(corpus("Grizzly Bears")), ready(corpus("Serra Angel"))]);
        } else {
            board.push(ready(corpus("Llanowar Elves")));
        }
        if seat == last {
            board.extend([ready(titan()), ready(island()), ready(island())]);
            scenario = scenario.hand(seat, [corpus("Counterspell")]);
        }
        scenario = scenario.battlefield(seat, board);
    }
    scenario
}

fn casts(seats: usize) -> Vec<Step> {
    vec![
        Step {
            seat: 0,
            card: "Doom Blade",
            aim: Aim::Creature("Serra Angel"),
        },
        Step {
            seat: seats - 1,
            card: "Counterspell",
            aim: Aim::Spell("Doom Blade"),
        },
        Step {
            seat: 0,
            card: "Swords to Plowshares",
            aim: Aim::Creature("Grizzly Bears"),
        },
        Step {
            seat: 0,
            card: "Wrath of God",
            aim: Aim::Nothing,
        },
    ]
}

#[rstest]
fn a_game_that_casts_all_four_cards_is_rebuilt_at_every_decision_and_replays_to_the_live_state(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), EngineError> {
    let setup = game(seats).setup().unwrap();
    let played = play_script(setup.clone(), casts(seats))?;
    check_rebuilds(&played)?;

    // The game used what it was written to use.
    let log = &played.log;
    let count = |wanted: &dyn Fn(&Event) -> bool| log.iter().filter(|e| wanted(e)).count();
    assert_eq!(
        count(&|e| matches!(
            e,
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Countered,
                ..
            }
        )),
        1,
        "the Doom Blade was countered"
    );
    assert_eq!(
        count(&|e| matches!(
            e,
            Event::ZoneChanged {
                cause: ZoneCause::Exiled,
                ..
            }
        )),
        1,
        "the Bears were exiled"
    );
    // Each seat lost two creatures to the Wrath: the Wolf and a Mourner, the Angel and a Mourner, an Elves and a Mourner.
    let destroyed = count(&|e| {
        matches!(
            e,
            Event::ZoneChanged {
                cause: ZoneCause::Destroyed,
                ..
            }
        )
    });
    assert_eq!(destroyed, 2 * seats);
    // Every Mourner saw every death, and the Bears' controller gained their power from the Swords.
    let end = replay(&setup, log)?;
    let deaths = i32::try_from(destroyed).unwrap();
    for seat in 0..seats {
        let swords = if seat == 1 { 2 } else { 0 };
        assert_eq!(
            end.player(pid(seat)).unwrap().life(),
            20 + deaths + swords,
            "seat {seat}"
        );
    }
    assert!(names(&end, Zone::Battlefield).contains(&"Titan".to_owned()));
    Ok(())
}

#[test]
fn the_same_script_plays_the_same_game_every_time() -> Result<(), EngineError> {
    let setup = game(3).setup().unwrap();
    let first = play_script(setup.clone(), casts(3))?;
    let second = play_script(setup, casts(3))?;
    assert_eq!(first.log, second.log);
    Ok(())
}
