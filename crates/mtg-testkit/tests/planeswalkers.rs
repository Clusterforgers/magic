//! Goal: planeswalkers play as the rules say, at tables of two, three and four seats: one cast from a hand enters with its printed loyalty in the batch that brings it in; each loyalty cost (plus, zero and minus, down to nothing) changes the counters when the ability is activated and the ability resolves when its planeswalker has gone; a loyalty ability is offered only to the active player in a main step with an empty stack, only with the counters its cost removes, and once for each planeswalker each turn; damage to a planeswalker removes that many loyalty counters and marks none (unblocked, past a blocker with trample, more than it holds, with lifelink), and a planeswalker with no loyalty goes to the graveyard; a creature goes on attacking a planeswalker that left, is not redirected, cannot be blocked and deals no combat damage to it, nor to anything else when it was not blocked; and Jace the Mind Sculptor, Jace Beleren, Ajani and Garruk do what their cards say; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::pid;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::Target;
use mtg_engine::events::ZoneCause;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{bear, free_creature, keyworded, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::loyalty_cards::{dawn_walker, pyre_walker};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

const LOYALTY: CounterKind = CounterKind::Loyalty;

fn loyalty_of(driver: &Driver, name: &str) -> Option<u32> {
    let id = driver.id_of(name).ok()?;
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(&LOYALTY))
}

/// Answers whatever is asked passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn in_zone(driver: &Driver, zone: Zone) -> Vec<String> {
    common::names(driver.state(), zone)
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

// Entering

#[rstest]
fn a_planeswalker_cast_from_a_hand_enters_with_its_printed_loyalty_in_the_batch_that_brings_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [dawn_walker().unwrap()])
            .battlefield(0, [ready(plains()), ready(plains()), ready(plains())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Dawn Walker")?;
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    let at = events
        .iter()
        .position(|event| matches!(event, Event::SpellResolved { .. }))
        .unwrap();
    let Event::SpellResolved { object } = events[at] else {
        unreachable!()
    };
    let Event::CountersChanged(changed) = &events[at + 1] else {
        panic!("the loyalty counters follow the spell in the same batch: {events:?}");
    };
    assert_eq!(
        (
            changed.on.object().map(|o| o.id),
            &changed.counter,
            changed.delta
        ),
        (Some(object), &LOYALTY, 4)
    );
    assert_eq!(loyalty_of(&driver, "Dawn Walker"), Some(4));
    // A planeswalker with its loyalty stays: no state-based action took it.
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(0))),
        Vec::<String>::new()
    );
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_planeswalker_that_starts_in_play_has_its_printed_loyalty_unless_the_setup_says_otherwise()
-> Result<(), ScriptError> {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(dawn_walker().unwrap()),
            with_counters(ready(pyre_walker().unwrap()), LOYALTY, 1),
        ],
    ))?;
    assert_eq!(loyalty_of(&driver, "Dawn Walker"), Some(4));
    assert_eq!(loyalty_of(&driver, "Pyre Walker"), Some(1));
    Ok(())
}

// The cost of a loyalty ability

/// Dawn Walker (+1: gain 2 life; 0: a Soldier; -3: destroy target creature) with `loyalty` counters, and a Bear of the
/// other seat to destroy.
fn dawn(seats: usize, loyalty: u32) -> Scenario {
    Scenario::new(seats)
        .battlefield(
            0,
            [with_counters(
                ready(dawn_walker().unwrap()),
                LOYALTY,
                loyalty,
            )],
        )
        .battlefield(1, [ready(bear())])
}

#[rstest]
#[case::plus(0, 4, Some(5), 22, false)]
#[case::zero(1, 4, Some(4), 20, false)]
#[case::minus(2, 4, Some(1), 20, true)]
#[case::minus_to_exactly_nothing(2, 3, None, 20, true)]
fn each_cost_changes_the_loyalty_counters_when_the_ability_is_activated(
    #[values(2, 3, 4)] seats: usize,
    #[case] ability: usize,
    #[case] start: u32,
    #[case] after: Option<u32>,
    #[case] life_after: i32,
    #[case] destroys: bool,
) -> Result<(), ScriptError> {
    let setup = dawn(seats, start).setup().unwrap();
    let mut driver = Driver::from_scenario(dawn(seats, start))?;
    let begun = driver.engine().events().len();
    driver.begin_ability("Dawn Walker", ability)?;
    if destroys {
        let bear = driver.target_named("Grizzly Bears")?;
        driver.choose_targets(&[bear])?;
    }
    // The cost is paid before the ability goes on the stack, and a planeswalker left with none is gone before anyone
    // receives priority.
    let after_the_cost = i64::from(start) + i64::from([1, 0, -3][ability]);
    assert_eq!(
        loyalty_of(&driver, "Dawn Walker"),
        u32::try_from(after_the_cost).ok().filter(|left| *left > 0)
    );
    let paid: Vec<i32> = driver.engine().events()[begun..]
        .iter()
        .filter_map(|event| match event {
            Event::CountersChanged(changed) => Some(changed.delta),
            _ => None,
        })
        .collect();
    let expected_cost = match ability {
        0 => vec![1],
        1 => vec![],
        _ => vec![-3],
    };
    assert_eq!(paid, expected_cost);
    assert_eq!(driver.state().stack().len(), 1);
    settle(&mut driver)?;
    assert_eq!(loyalty_of(&driver, "Dawn Walker"), after);
    assert_eq!(life(&driver, 0), life_after);
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(0))).contains(&"Dawn Walker".to_owned()),
        after.is_none(),
        "a planeswalker without loyalty is put into its owner's graveyard"
    );
    assert_eq!(
        in_zone(&driver, Zone::Graveyard(pid(1))).contains(&"Grizzly Bears".to_owned()),
        destroys,
        "the ability resolves whether or not its planeswalker is still there"
    );
    if ability == 1 {
        assert!(in_zone(&driver, Zone::Battlefield).contains(&"Soldier".to_owned()));
    }
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_planeswalker_that_paid_its_last_loyalty_goes_to_the_graveyard_by_a_state_based_action()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(dawn(2, 3))?;
    driver.begin_ability("Dawn Walker", 2)?;
    let bear = driver.target_named("Grizzly Bears")?;
    let begun = driver.engine().events().len();
    driver.choose_targets(&[bear])?;
    let events = &driver.engine().events()[begun..];
    let gone = events.iter().any(|event| {
        matches!(
            event,
            Event::ZoneChanged {
                to: Zone::Graveyard(_),
                cause: ZoneCause::StateBased,
                ..
            }
        )
    });
    // The state-based action is found before anyone receives priority, with the ability on the stack.
    assert!(gone || in_zone(&driver, Zone::Graveyard(pid(0))).contains(&"Dawn Walker".to_owned()));
    assert_eq!(driver.state().stack().len(), 1);
    Ok(())
}

// When it is offered

#[rstest]
#[case::all_three(4, 1, 0, Step::Main1, &[0, 1, 2])]
#[case::exactly_enough_for_the_minus(3, 1, 0, Step::Main1, &[0, 1, 2])]
#[case::one_short_of_the_minus(2, 1, 0, Step::Main1, &[0, 1])]
#[case::the_second_main_step(4, 1, 0, Step::Main2, &[0, 1, 2])]
#[case::the_upkeep(4, 1, 0, Step::Upkeep, &[])]
#[case::the_start_of_combat(4, 1, 0, Step::BeginCombat, &[])]
#[case::the_end_of_combat(4, 1, 0, Step::EndCombat, &[])]
#[case::the_end_step(4, 1, 0, Step::End, &[])]
fn a_loyalty_ability_is_offered_at_sorcery_timing_with_the_counters_its_cost_takes(
    #[case] loyalty: u32,
    #[case] turn: u32,
    #[case] active: usize,
    #[case] step: Step,
    #[case] offered: &[usize],
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(dawn(2, loyalty).at(turn, active, step))?;
    assert_eq!(driver.offered_abilities("Dawn Walker")?, offered);
    Ok(())
}

#[test]
fn an_opponent_is_not_offered_the_loyalty_abilities_of_a_planeswalker_it_does_not_control_nor_in_its_own_turn()
-> Result<(), ScriptError> {
    // Seat 1 has priority in its own main step; the planeswalker is seat 0's.
    let driver = Driver::from_scenario(dawn(2, 4).at(2, 1, Step::Main1))?;
    assert_eq!(driver.decision()?.player, pid(1));
    assert_eq!(
        driver.offered_abilities("Dawn Walker")?,
        Vec::<usize>::new()
    );
    // And seat 0 does not get them in seat 1's turn when it is handed priority.
    let mut driver = Driver::from_scenario(dawn(2, 4).at(2, 1, Step::Main1))?;
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(0));
    assert_eq!(
        driver.offered_abilities("Dawn Walker")?,
        Vec::<usize>::new()
    );
    Ok(())
}

#[test]
fn a_loyalty_ability_is_activated_once_for_each_planeswalker_each_turn() -> Result<(), ScriptError>
{
    let scenario = || {
        Scenario::new(2).battlefield(
            0,
            [
                ready(dawn_walker().unwrap()),
                ready(pyre_walker().unwrap()),
                ready(bear()),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    assert_eq!(driver.offered_abilities("Dawn Walker")?, [0, 1, 2]);
    assert_eq!(driver.offered_abilities("Pyre Walker")?, [0, 1]);
    // Not while one is on the stack: only an empty stack allows a sorcery.
    driver.begin_ability("Dawn Walker", 0)?;
    assert_eq!(
        driver.offered_abilities("Pyre Walker")?,
        Vec::<usize>::new()
    );
    settle(&mut driver)?;
    // Another planeswalker may, this one may not again.
    assert_eq!(
        driver.offered_abilities("Dawn Walker")?,
        Vec::<usize>::new()
    );
    assert_eq!(driver.offered_abilities("Pyre Walker")?, [0, 1]);
    driver.begin_ability("Pyre Walker", 0)?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 1), 19);
    assert_eq!(
        driver.offered_abilities("Pyre Walker")?,
        Vec::<usize>::new()
    );
    assert!(driver.decision().is_ok());
    // A new turn of the same player, the same planeswalkers: each has one more.
    driver.idle_until(3, Step::Main1)?;
    assert_eq!(driver.offered_abilities("Dawn Walker")?, [0, 1, 2]);
    assert_eq!(driver.offered_abilities("Pyre Walker")?, [0, 1]);
    replays(&driver, &setup);
    Ok(())
}

// Damage to a planeswalker

/// Seat 0 attacks Dawn Walker (`loyalty` counters) of seat 1 with `A`, which `blocks` blocks when seat 1 has a `B`.
fn at_the_walker(
    attacker: std::sync::Arc<mtg_cards::CardDefinition>,
    loyalty: u32,
    blocker: bool,
) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(attacker)])
            .battlefield(
                1,
                std::iter::once(with_counters(
                    ready(dawn_walker().unwrap()),
                    LOYALTY,
                    loyalty,
                ))
                .chain(blocker.then(|| ready(free_creature("B", 1, 1)))),
            ),
    )?;
    driver.attack_planeswalkers(&[("A", "Dawn Walker")])?;
    Ok(driver)
}

#[rstest]
#[case::less_than_it_holds(2, 4, false, Some(2), 0)]
#[case::exactly_what_it_holds(4, 4, false, None, 0)]
#[case::more_than_it_holds(7, 4, false, None, 0)]
#[case::blocked_by_a_chump(5, 4, true, Some(4), 0)]
fn combat_damage_to_a_planeswalker_removes_that_many_loyalty_counters_and_marks_none(
    #[case] power: i32,
    #[case] loyalty: u32,
    #[case] blocked: bool,
    #[case] left: Option<u32>,
    #[case] foe_life_lost: i32,
) -> Result<(), ScriptError> {
    let mut driver = at_the_walker(free_creature("A", power, power), loyalty, blocked)?;
    if blocked {
        common::combat::to_blockers(&mut driver, 1)?;
        driver.block(&[("B", "A")])?;
    }
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(loyalty_of(&driver, "Dawn Walker"), left);
    if left.is_none() {
        assert!(in_zone(&driver, Zone::Graveyard(pid(1))).contains(&"Dawn Walker".to_owned()));
    } else {
        let id = driver.id_of("Dawn Walker")?;
        let marked = driver
            .state()
            .object(id)
            .and_then(|o| o.permanent())
            .map(mtg_engine::object::PermanentState::damage);
        assert_eq!(marked, Some(0), "damage to a planeswalker is not marked");
    }
    assert_eq!(life(&driver, 1), 20 - foe_life_lost);
    Ok(())
}

#[test]
fn damage_past_the_loyalty_of_a_planeswalker_is_still_damage_dealt_so_lifelink_gains_all_of_it()
-> Result<(), ScriptError> {
    let mut driver = at_the_walker(keyworded("A", 5, 5, &[Keyword::Lifelink]), 2, false)?;
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(
        life(&driver, 0),
        25,
        "5 damage was dealt to a planeswalker with 2 loyalty"
    );
    assert!(in_zone(&driver, Zone::Graveyard(pid(1))).contains(&"Dawn Walker".to_owned()));
    Ok(())
}

#[test]
fn trample_puts_the_damage_beyond_the_blocker_on_the_planeswalker_and_none_on_the_player()
-> Result<(), ScriptError> {
    let mut driver = at_the_walker(keyworded("A", 4, 4, &[Keyword::Trample]), 3, true)?;
    common::combat::to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "A")])?;
    driver.pass()?;
    driver.pass()?;
    driver.assign_damage(&[("B", 1)])?;
    assert_eq!(
        loyalty_of(&driver, "Dawn Walker"),
        None,
        "3 of the 4 reached it"
    );
    assert_eq!(life(&driver, 1), 20);
    Ok(())
}

#[rstest]
fn only_the_opponents_of_its_controller_may_attack_a_planeswalker(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(free_creature("A", 2, 2)),
                ready(pyre_walker().unwrap()),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(dawn_walker().unwrap())]);
    }
    let driver = Driver::from_scenario(scenario)?;
    let mine = driver.id_of("Pyre Walker")?;
    let mtg_engine::DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("attackers were expected");
    };
    let defenders = &options.attackers[0].defenders;
    assert!(
        !defenders.contains(&mtg_engine::combat::AttackTarget::Planeswalker(mine)),
        "a player does not attack their own planeswalker"
    );
    let theirs = defenders
        .iter()
        .filter(|defender| matches!(defender, mtg_engine::combat::AttackTarget::Planeswalker(_)))
        .count();
    assert_eq!(
        theirs,
        seats - 1,
        "each opponent's planeswalker is a defender"
    );
    Ok(())
}

// A planeswalker that leaves during combat

/// Seat 0 attacks the Pyre Walker of seat 1 with `A`, and holds a Lightning Bolt and a Mountain; seat 1 has a Bear to
/// block with when `blocker`.
fn bolt_the_walker(blocker: bool) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .hand(0, [card("Lightning Bolt").unwrap()])
            .battlefield(0, [ready(free_creature("A", 3, 3)), ready(mountain())])
            .battlefield(
                1,
                std::iter::once(with_counters(ready(pyre_walker().unwrap()), LOYALTY, 3))
                    .chain(blocker.then(|| ready(free_creature("B", 1, 1)))),
            ),
    )?;
    driver.attack_planeswalkers(&[("A", "Pyre Walker")])?;
    Ok(driver)
}

fn bolt(driver: &mut Driver, target: &str) -> Result<(), ScriptError> {
    driver.begin_cast("Lightning Bolt")?;
    let target = driver.target_named(target)?;
    driver.choose_targets(&[target])?;
    driver.answer(mtg_engine::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    settle(driver)
}

#[test]
fn an_unblocked_creature_goes_on_attacking_a_planeswalker_that_left_and_deals_no_damage()
-> Result<(), ScriptError> {
    let mut driver = bolt_the_walker(false)?;
    bolt(&mut driver, "Pyre Walker")?;
    assert!(in_zone(&driver, Zone::Graveyard(pid(1))).contains(&"Pyre Walker".to_owned()));
    let attackers = driver.state().combat().attackers();
    assert_eq!(attackers.len(), 1, "it is still attacking");
    assert!(attackers[0].attacks_nothing());
    assert_eq!(
        attackers[0].defender(driver.state()),
        None,
        "nobody may block it"
    );
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(life(&driver, 1), 20, "it was not redirected to the player");
    assert_eq!(life(&driver, 0), 20);
    let dealt = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::DamageDealt { source, .. } if *source == driver.id_of("A").unwrap()))
        .count();
    assert_eq!(dealt, 0, "it dealt no combat damage");
    Ok(())
}

#[test]
fn a_creature_blocked_before_the_planeswalker_left_still_fights_its_blocker()
-> Result<(), ScriptError> {
    let mut driver = bolt_the_walker(true)?;
    common::combat::to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "A")])?;
    bolt(&mut driver, "Pyre Walker")?;
    assert!(driver.state().combat().attackers()[0].attacks_nothing());
    driver.idle_until(2, Step::EndCombat)?;
    assert!(
        in_zone(&driver, Zone::Graveyard(pid(1))).contains(&"B".to_owned()),
        "the blocker was dealt the damage"
    );
    assert_eq!(life(&driver, 1), 20);
    Ok(())
}

#[test]
fn a_planeswalker_that_left_is_not_a_target_of_an_attacker_that_attacked_another()
-> Result<(), ScriptError> {
    // The other planeswalker (and the player) are still attacked as they were declared.
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .hand(0, [card("Lightning Bolt").unwrap()])
            .battlefield(
                0,
                [
                    ready(free_creature("A", 2, 2)),
                    ready(free_creature("C", 2, 2)),
                    ready(mountain()),
                ],
            )
            .battlefield(
                1,
                [
                    with_counters(ready(pyre_walker().unwrap()), LOYALTY, 3),
                    with_counters(ready(dawn_walker().unwrap()), LOYALTY, 4),
                ],
            ),
    )?;
    driver.attack_planeswalkers(&[("A", "Pyre Walker"), ("C", "Dawn Walker")])?;
    bolt(&mut driver, "Pyre Walker")?;
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(
        loyalty_of(&driver, "Dawn Walker"),
        Some(2),
        "C's 2 damage reached its planeswalker"
    );
    assert_eq!(life(&driver, 1), 20);
    Ok(())
}

// The corpus planeswalkers

fn hand_size(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

#[rstest]
fn jace_beleren_draws_for_each_player_for_one_and_mills_twenty(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .battlefield(
                0,
                [with_counters(
                    ready(card("Jace Beleren").unwrap()),
                    LOYALTY,
                    10,
                )],
            )
            .library(0, std::iter::repeat_n(bear(), 30))
            .library(1, std::iter::repeat_n(bear(), 30))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_ability("Jace Beleren", 0)?;
    settle(&mut driver)?;
    assert_eq!(loyalty_of(&driver, "Jace Beleren"), Some(12));
    for seat in 0..seats {
        assert_eq!(hand_size(&driver, seat), 1, "seat {seat} drew a card");
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn jace_beleren_makes_a_chosen_player_draw_for_one_loyalty(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] chosen: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).battlefield(0, [ready(card("Jace Beleren").unwrap())]),
    )?;
    driver.begin_ability("Jace Beleren", 1)?;
    let target = driver.target_player(chosen);
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    assert_eq!(loyalty_of(&driver, "Jace Beleren"), Some(2));
    for seat in 0..seats {
        assert_eq!(
            hand_size(&driver, seat),
            usize::from(seat == chosen),
            "seat {seat}"
        );
    }
    Ok(())
}

#[test]
fn jace_beleren_mills_twenty_cards_of_a_player_for_ten_loyalty() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                [with_counters(
                    ready(card("Jace Beleren").unwrap()),
                    LOYALTY,
                    10,
                )],
            )
            .library(1, std::iter::repeat_n(bear(), 30)),
    )?;
    driver.begin_ability("Jace Beleren", 2)?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    assert_eq!(
        loyalty_of(&driver, "Jace Beleren"),
        None,
        "ten loyalty paid, none left"
    );
    settle(&mut driver)?;
    assert_eq!(driver.state().zone(Zone::Library(pid(1))).len(), 10);
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(1))).len(), 20);
    Ok(())
}

#[test]
fn ajani_puts_a_counter_on_up_to_one_creature_and_gives_flying_and_double_strike()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(card("Ajani, Caller of the Pride").unwrap()),
            ready(bear()),
        ],
    ))?;
    // "Up to one": with none chosen nothing gets a counter, and the loyalty is still paid.
    driver.begin_ability("Ajani, Caller of the Pride", 0)?;
    driver.choose_targets(&[])?;
    settle(&mut driver)?;
    assert_eq!(loyalty_of(&driver, "Ajani, Caller of the Pride"), Some(5));
    let bears = driver.id_of("Grizzly Bears")?;
    assert!(
        driver
            .state()
            .object(bears)
            .unwrap()
            .permanent()
            .unwrap()
            .counters()
            .next()
            .is_none()
    );
    driver.idle_until(3, Step::Main1)?;
    driver.begin_ability("Ajani, Caller of the Pride", 0)?;
    let target = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    let held = driver
        .state()
        .object(bears)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(held, 1);
    driver.idle_until(5, Step::Main1)?;
    driver.begin_ability("Ajani, Caller of the Pride", 1)?;
    let target = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    assert!(has_keyword(driver.state(), bears, &Keyword::Flying));
    assert!(has_keyword(driver.state(), bears, &Keyword::DoubleStrike));
    assert_eq!(loyalty_of(&driver, "Ajani, Caller of the Pride"), Some(3));
    Ok(())
}

#[test]
fn ajani_makes_a_cat_for_each_life_he_is_worth_at_eight_loyalty() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(
                0,
                [with_counters(
                    ready(card("Ajani, Caller of the Pride").unwrap()),
                    LOYALTY,
                    8,
                )],
            )
            .life(0, 3),
    )?;
    driver.begin_ability("Ajani, Caller of the Pride", 2)?;
    settle(&mut driver)?;
    let cats = in_zone(&driver, Zone::Battlefield)
        .iter()
        .filter(|name| *name == "Cat")
        .count();
    assert_eq!(cats, 3);
    assert_eq!(loyalty_of(&driver, "Ajani, Caller of the Pride"), None);
    Ok(())
}

#[test]
fn garruk_untaps_two_lands_makes_a_beast_and_gives_the_creatures_you_control_trample()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            with_counters(ready(card("Garruk Wildspeaker").unwrap()), LOYALTY, 5),
            mtg_testkit::tapped(mountain()),
            mtg_testkit::tapped(mountain()),
            ready(bear()),
        ],
    ))?;
    driver.begin_ability("Garruk Wildspeaker", 0)?;
    let lands: Vec<Target> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Mountain")
        .map(|id| Target::Object(driver.state().object(*id).unwrap().reference()))
        .collect();
    driver.choose_targets(&lands)?;
    settle(&mut driver)?;
    let tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = driver.state().object(**id).unwrap();
            object.card().name == "Mountain" && object.permanent().unwrap().tapped()
        })
        .count();
    assert_eq!(tapped, 0);
    driver.idle_until(3, Step::Main1)?;
    driver.begin_ability("Garruk Wildspeaker", 1)?;
    settle(&mut driver)?;
    assert!(in_zone(&driver, Zone::Battlefield).contains(&"Beast".to_owned()));
    driver.idle_until(5, Step::Main1)?;
    driver.begin_ability("Garruk Wildspeaker", 2)?;
    settle(&mut driver)?;
    let bears = driver.id_of("Grizzly Bears")?;
    let chars = mtg_engine::characteristics(driver.state(), bears).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(5), Some(5)));
    assert!(has_keyword(driver.state(), bears, &Keyword::Trample));
    Ok(())
}

#[test]
fn jace_the_mind_sculptor_bounces_a_creature_looks_at_a_card_and_exiles_a_library()
-> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .battlefield(
                0,
                [with_counters(
                    ready(card("Jace, the Mind Sculptor").unwrap()),
                    LOYALTY,
                    12,
                )],
            )
            .battlefield(1, [ready(bear())])
            .library(0, std::iter::repeat_n(mountain(), 5))
            .library(1, (0..6).map(|n| free_creature(&format!("Card {n}"), 1, 1)))
            .hand(1, [plains(), plains()])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    // -12: the library of seat 1 is exiled and its hand shuffled into a library of its own.
    driver.begin_ability("Jace, the Mind Sculptor", 3)?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    assert_eq!(loyalty_of(&driver, "Jace, the Mind Sculptor"), None);
    assert_eq!(
        driver.state().zone(Zone::Exile).len(),
        6,
        "the library was exiled"
    );
    assert_eq!(
        driver.state().zone(Zone::Library(pid(1))).len(),
        2,
        "the hand is the library now"
    );
    assert_eq!(hand_size(&driver, 1), 0);
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn jace_the_mind_sculptor_bounces_a_creature_for_one_and_draws_three_for_nothing()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(card("Jace, the Mind Sculptor").unwrap())])
            .battlefield(1, [ready(bear())])
            .library(0, std::iter::repeat_n(mountain(), 6)),
    )?;
    driver.begin_ability("Jace, the Mind Sculptor", 2)?;
    let target = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    assert_eq!(in_zone(&driver, Zone::Hand(pid(1))), ["Grizzly Bears"]);
    assert_eq!(loyalty_of(&driver, "Jace, the Mind Sculptor"), Some(2));
    driver.idle_until(3, Step::Main1)?;
    let before = hand_size(&driver, 0);
    driver.begin_ability("Jace, the Mind Sculptor", 1)?;
    settle(&mut driver)?;
    assert_eq!(
        hand_size(&driver, 0),
        before + 1,
        "three drawn, two put back"
    );
    assert_eq!(loyalty_of(&driver, "Jace, the Mind Sculptor"), Some(2));
    Ok(())
}
