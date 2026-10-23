//! Goal: Sagas play as the rules say (CR 714) at tables of two, three and four seats: a Saga enters with a lore counter and gets one as each of its controller's precombat main phases begins (never in another player's), a chapter ability triggers when the lore counters reach or pass its number (a line of several numbers is one ability for each, counters put on by an effect trigger every chapter they pass in the order their controller picks, Doubling Season doubles the lore counters and Hardened Scales does not), the Saga stays while a chapter ability of it waits or is on the stack and is sacrificed once the last has resolved, a Saga that leaves and returns starts over, and the log of whole games with Sagas replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::saga_cards::{lore_spell, recall_spell, saga};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// Chapter I gains 1 life, II gains 2 and III gains 4.
const THREE: &[(&[u32], i32)] = &[(&[1], 1), (&[2], 2), (&[3], 4)];

/// Chapters I and II gain 3 life and III gains 4.
const TWICE: &[(&[u32], i32)] = &[(&[1, 2], 3), (&[3], 4)];

/// The id of the card with this name wherever it lies.
fn anywhere(driver: &Driver, name: &str) -> ObjectId {
    let state = driver.state();
    let mut zones = vec![Zone::Battlefield, Zone::Exile, Zone::Stack];
    for seat in 0..state.players().len() {
        zones.extend([
            Zone::Hand(pid(seat)),
            Zone::Graveyard(pid(seat)),
            Zone::Library(pid(seat)),
        ]);
    }
    zones
        .into_iter()
        .flat_map(|zone| state.zone(zone).iter().copied())
        .find(|id| {
            state
                .object(*id)
                .is_some_and(|object| object.card().name == name)
        })
        .unwrap()
}

fn lore(driver: &Driver, name: &str) -> u32 {
    driver
        .state()
        .object(anywhere(driver, name))
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(&CounterKind::lore()))
}

fn on_battlefield(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .object(anywhere(driver, name))
        .is_some_and(|object| object.zone() == Zone::Battlefield)
}

/// The card with this name, wherever it lies, as a target.
fn target_of(driver: &Driver, name: &str) -> Target {
    let object = driver.state().object(anywhere(driver, name)).unwrap();
    Target::Object(object.reference())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Answers until nothing is on the stack, no trigger waits to be ordered and nothing resolves: triggers are put on the
/// stack in `order` (the order they are listed in when empty) and everything else is passed. The Saga `keep` must be on
/// the battlefield whenever a chapter ability waits or is on the stack. Returns the amounts of life gained, in order.
fn settle(driver: &mut Driver, order: &[usize], keep: &str) -> Result<Vec<u32>, ScriptError> {
    let mut gained = Vec::new();
    let mut entered = on_battlefield(driver, keep);
    for _ in 0..300 {
        let ordering = matches!(driver.decision()?.kind, DecisionKind::OrderTriggers(_));
        let busy = !driver.state().stack().is_empty() || driver.state().resolving().is_some();
        if !busy && !ordering {
            return Ok(gained);
        }
        // Before it enters the Saga is a spell or a card in a graveyard.
        let there = on_battlefield(driver, keep);
        entered |= there;
        assert!(
            there || !entered,
            "{keep} left with a chapter ability waiting or on the stack"
        );
        let events = if let DecisionKind::OrderTriggers(options) = &driver.decision()?.kind {
            let all: Vec<usize> = (0..options.triggers.len()).collect();
            let picked = if order.is_empty() {
                all
            } else {
                order.to_vec()
            };
            driver.order_triggers(&picked)?
        } else {
            driver.answer_passively()?
        };
        gained.extend(events.iter().filter_map(|event| match event {
            Event::LifeGained { amount, .. } => Some(*amount),
            _ => None,
        }));
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 with a Plains and these cards in hand and these permanents, at the beginning of its first main phase.
fn table(seats: usize, hand: Vec<std::sync::Arc<mtg_cards::CardDefinition>>) -> Scenario {
    Scenario::new(seats)
        .hand(0, hand)
        .battlefield(0, [ready(plains())])
        .at(1, 0, Step::Main2)
}

// Entering

#[rstest]
fn a_saga_enters_with_a_lore_counter_and_its_first_chapter_triggers(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, vec![saga("Tale", THREE)]))?;
    let before = life(&driver, 0);
    driver.cast("Tale")?;
    let gained = settle(&mut driver, &[], "Tale")?;
    assert_eq!(gained, [1]);
    assert_eq!(lore(&driver, "Tale"), 1);
    assert_eq!(life(&driver, 0), before + 1);
    assert!(on_battlefield(&driver, "Tale"));
    Ok(())
}

// A turn at a time

#[rstest]
fn lore_counters_come_as_the_precombat_main_phase_begins_and_the_saga_leaves_after_its_last_chapter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let rival = with_counters(ready(saga("Rival", THREE)), CounterKind::lore(), 1);
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [with_counters(
                ready(saga("Tale", THREE)),
                CounterKind::lore(),
                1,
            )],
        )
        .battlefield(1, [rival])
        .at(1, 0, Step::Upkeep);
    let mut driver = Driver::from_scenario(scenario)?;
    let start = life(&driver, 0);
    driver.pass_until(Step::Main1)?;
    // The counter is a turn-based action: chapter II waits on the stack and the other player's Saga is untouched.
    assert_eq!(lore(&driver, "Tale"), 2);
    assert_eq!(lore(&driver, "Rival"), 1);
    assert_eq!(driver.state().stack().len(), 1);
    assert_eq!(life(&driver, 0), start);
    assert_eq!(settle(&mut driver, &[], "Tale")?, [2]);
    // Nobody else's main phase gives it a counter.
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(lore(&driver, "Tale"), 2);
    assert_eq!(lore(&driver, "Rival"), 2);
    driver.idle_until(next, Step::Main1)?;
    assert_eq!(lore(&driver, "Tale"), 3);
    // The last chapter is on the stack, so the Saga stays until it has resolved.
    assert!(on_battlefield(&driver, "Tale"));
    assert_eq!(driver.state().stack().len(), 1);
    assert_eq!(settle(&mut driver, &[], "Tale")?, [4]);
    assert!(!on_battlefield(&driver, "Tale"));
    assert_eq!(life(&driver, 0), start + 2 + 4);
    Ok(())
}

#[rstest]
#[case::the_line_of_two(TWICE, &[3, 3, 4])]
#[case::one_line_each(THREE, &[1, 2, 4])]
fn every_chapter_runs_once_in_the_turn_its_number_is_reached(
    #[values(2, 3, 4)] seats: usize,
    #[case] chapters: &[(&[u32], i32)],
    #[case] expected: &[u32],
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(saga("Tale", chapters))])
        .at(1, 0, Step::Upkeep);
    let mut driver = Driver::from_scenario(scenario)?;
    let mut gained = Vec::new();
    for round in 0..3 {
        driver.idle_until(1 + round * u32::try_from(seats).unwrap(), Step::Main1)?;
        assert_eq!(lore(&driver, "Tale"), round + 1);
        gained.extend(settle(&mut driver, &[], "Tale")?);
    }
    assert_eq!(gained, expected);
    assert!(!on_battlefield(&driver, "Tale"));
    Ok(())
}

#[rstest]
fn a_saga_without_chapter_abilities_is_never_sacrificed(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let blank = with_counters(ready(saga("Blank", &[])), CounterKind::lore(), 5);
    let scenario = Scenario::new(seats)
        .battlefield(0, [blank])
        .at(1, 0, Step::Upkeep);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    assert_eq!(lore(&driver, "Blank"), 6);
    assert!(on_battlefield(&driver, "Blank"));
    Ok(())
}

// Counters from effects

#[rstest]
#[case::one_counter_passes_one_chapter(1, &[2], true)]
#[case::two_counters_pass_two_chapters(2, &[2, 4], false)]
fn counters_put_on_by_an_effect_trigger_the_chapters_they_pass(
    #[values(2, 3, 4)] seats: usize,
    #[case] count: u32,
    #[case] expected: &[u32],
    #[case] stays: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(0, [lore_spell("Surge", count)])
        .battlefield(
            0,
            [
                ready(plains()),
                with_counters(ready(saga("Tale", THREE)), CounterKind::lore(), 1),
            ],
        )
        .at(1, 0, Step::Main2);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = target_of(&driver, "Tale");
    driver.cast_at("Surge", &[target])?;
    // Chapters II and III are ordered by the player; the first listed goes on the stack first.
    assert_eq!(settle(&mut driver, &[1, 0], "Tale")?.len(), expected.len());
    assert_eq!(lore(&driver, "Tale") > 0, stays);
    assert_eq!(on_battlefield(&driver, "Tale"), stays);
    Ok(())
}

#[rstest]
#[case::in_the_order_they_are_listed(&[], &[4, 2, 1])]
#[case::chapter_one_first(&[2, 1, 0], &[1, 2, 4])]
fn chapters_triggered_together_resolve_in_the_order_their_controller_picks(
    #[values(2, 3, 4)] seats: usize,
    #[case] order: &[usize],
    #[case] expected: &[u32],
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(0, [recall_spell("Retell", 2)])
        .graveyard(0, [saga("Tale", THREE)])
        .battlefield(0, [ready(plains())])
        .at(1, 0, Step::Main2);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = target_of(&driver, "Tale");
    driver.cast_at("Retell", &[target])?;
    // The Saga enters with its own counter and two more: chapters I, II and III trigger at once.
    let gained = settle(&mut driver, order, "Tale")?;
    assert_eq!(gained, expected);
    assert!(!on_battlefield(&driver, "Tale"), "sacrificed after III");
    Ok(())
}

// Rules that change counters

#[rstest]
#[case::doubling_season("Doubling Season", 2, &[1, 2], true)]
#[case::hardened_scales("Hardened Scales", 1, &[1], true)]
fn a_replacement_of_counters_reaches_the_lore_counter_a_saga_enters_with(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] counters: u32,
    #[case] expected: &[u32],
    #[case] stays: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(0, [saga("Tale", THREE)])
        .battlefield(0, [ready(plains()), ready(card(name).unwrap())])
        .at(1, 0, Step::Main2);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Tale")?;
    let gained = settle(&mut driver, &[1, 0], "Tale")?;
    assert_eq!(lore(&driver, "Tale"), counters);
    assert_eq!(gained.iter().sum::<u32>(), expected.iter().sum::<u32>());
    assert_eq!(gained.len(), expected.len());
    assert_eq!(on_battlefield(&driver, "Tale"), stays);
    Ok(())
}

#[rstest]
fn doubling_season_doubles_the_lore_counter_of_the_turn_based_action(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Doubling Season").unwrap()),
                with_counters(ready(saga("Tale", THREE)), CounterKind::lore(), 1),
            ],
        )
        .at(1, 0, Step::Upkeep);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    // One counter becomes two: 1 to 3, so chapters II and III trigger, and the Saga leaves after both.
    assert_eq!(lore(&driver, "Tale"), 3);
    let gained = settle(&mut driver, &[], "Tale")?;
    assert_eq!(gained.iter().sum::<u32>(), 2 + 4);
    assert!(!on_battlefield(&driver, "Tale"));
    Ok(())
}

// Leaving and returning

#[rstest]
fn a_saga_that_leaves_and_returns_starts_over(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [with_counters(
                ready(saga("Tale", THREE)),
                CounterKind::lore(),
                2,
            )],
        )
        .battlefield(1, [ready(free_creature("Ogre", 3, 4))])
        .at(1, 0, Step::Main2);
    let mut driver = Driver::from_scenario(scenario)?;
    let id = anywhere(&driver, "Tale");
    driver.manual(ManualAction::Move {
        object: id,
        to: ManualZone::Exile,
    })?;
    assert!(!on_battlefield(&driver, "Tale"));
    driver.manual(ManualAction::Move {
        object: id,
        to: ManualZone::Battlefield,
    })?;
    // A new object with one lore counter: chapter I runs again, and the Saga has the rest of its chapters to go.
    assert_eq!(lore(&driver, "Tale"), 1);
    assert_eq!(settle(&mut driver, &[], "Tale")?, [1]);
    assert!(on_battlefield(&driver, "Tale"));
    Ok(())
}

// Whole games

#[test]
fn bot_games_with_sagas_keep_every_invariant_and_replay() {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use mtg_cards::ability::Ability;
    use mtg_cards::trigger::TriggerPattern;
    use mtg_engine::abilities::AbilityRef;
    use mtg_engine::ability_reader::ability;
    use mtg_engine::events::ZoneCause;
    use mtg_testkit::saga_cards::SAGA_CARDS;
    use mtg_testkit::{PlayOptions, play_game_with, saga_fuzz_setup};

    use crate::common::seeds::{for_each_seed, fuzz_games};

    // Lore counters put on, chapters triggered, chapters that came in order, Sagas that left after their last chapter, and games.
    let seen = Mutex::new([0u64; 5]);
    let games = fuzz_games(12);
    for_each_seed(0..games, |seed| {
        for players in [2, 3, 4] {
            let outcome =
                play_game_with(saga_fuzz_setup(players, seed), seed, PlayOptions::default())
                    .unwrap_or_else(|failure| panic!("{failure}"));
            let replayed = replay(&saga_fuzz_setup(players, seed), &outcome.events).unwrap();
            assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
            let is_saga = |id| {
                replayed
                    .object(id)
                    .is_some_and(|object| SAGA_CARDS.contains(&object.card().name.as_str()))
            };
            let chapter_of = |rules: &Ability| match rules {
                Ability::Triggered {
                    when: TriggerPattern::Chapter { number },
                    ..
                } => Some(*number),
                _ => None,
            };
            let finals: BTreeMap<_, _> = SAGA_CARDS
                .iter()
                .filter_map(|name| {
                    let card = mtg_testkit::corpus::card(name).ok()?;
                    let last = card
                        .rules
                        .iter()
                        .filter_map(|rules| chapter_of(rules))
                        .max()?;
                    Some(((*name).to_owned(), last))
                })
                .collect();
            // The highest chapter that triggered for each object a Saga card has been.
            let mut highest: BTreeMap<(u64, u32), u32> = BTreeMap::new();
            let mut seen = seen.lock().unwrap();
            for event in &outcome.events {
                match event {
                    Event::CountersChanged(changed)
                        if changed.delta > 0 && changed.counter == CounterKind::lore() =>
                    {
                        seen[0] += 1;
                    }
                    Event::AbilityTriggered(trigger) if is_saga(trigger.source.id) => {
                        let address = AbilityRef {
                            object: trigger.source.id,
                            ability: trigger.ability,
                            source: trigger.origin,
                        };
                        let Some(number) =
                            ability(&replayed, address).and_then(|rules| chapter_of(&rules))
                        else {
                            continue;
                        };
                        seen[1] += 1;
                        let key = (trigger.source.id.0, trigger.source.counter);
                        let before = highest.insert(key, number).unwrap_or(0);
                        // Lore counters come one at a time here, so chapters never skip back to an earlier number.
                        if number >= before {
                            seen[2] += 1;
                        }
                    }
                    Event::ZoneChanged {
                        object,
                        from: Zone::Battlefield,
                        cause: ZoneCause::Sacrificed,
                        ..
                    } if is_saga(*object) => {
                        let name = replayed.object(*object).unwrap().card().name.clone();
                        let key = highest
                            .iter()
                            .filter(|((id, _), _)| *id == object.0)
                            .map(|(_, number)| *number)
                            .max();
                        assert_eq!(key, finals.get(&name).copied(), "{name} seed {seed}");
                        seen[3] += 1;
                    }
                    _ => {}
                }
            }
            seen[4] += 1;
        }
    });
    let [counters, chapters, in_order, left, games_played] = seen.into_inner().unwrap();
    assert!(games_played > 0);
    assert!(counters > 0, "some Saga got lore counters");
    assert!(chapters > 0, "some chapter ability triggered");
    assert!(in_order > 0, "chapters triggered in order");
    assert!(left > 0, "some Saga left after its last chapter");
}
