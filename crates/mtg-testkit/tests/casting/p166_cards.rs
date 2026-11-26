//! Goal: the p166 cards play whole at two to four seats: Monitor Monitor lets its controller pay {1} once each turn to
//! reroll dice of any roll, the turn-based roll to visit Attractions and an effect's roll alike, and a declined reroll is
//! offered again on the next roll; Bamboozling Beeble adds a die to the target player's next roll and its controller
//! chooses the roll ignored, which is never logged; Camouflage has each defending player divide creatures into one pile
//! per attacker, the piles go to the attackers at random and each creature that can block its pile's attacker does;
//! each game replays and each card is Verified with the engine's capabilities.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::attraction::Lights;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::events::Event;
use mtg_engine::r706_dice::R706DicePurpose;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, forest, free_creature, island, un33_attraction, un33_opener};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

fn p166_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn p166_libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 12])
    })
}

/// Answers `pick` gives first, then mana automatically, the first legal targets and passes, `steps` times at most or
/// until `stop` holds.
fn p166_run(
    driver: &mut Driver,
    steps: usize,
    pick: &mut dyn FnMut(&Driver, &Decision) -> Option<Answer>,
    stop: &dyn Fn(&Driver) -> bool,
) {
    for _ in 0..steps {
        if stop(driver) {
            return;
        }
        let decision = driver.decision().unwrap().clone();
        if let Some(answer) = pick(driver, &decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseTargets(options) => Answer::ChooseTargets(TargetChoice::Targets(
                options
                    .legal
                    .iter()
                    .copied()
                    .take(options.min.max(1) as usize)
                    .collect(),
            )),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
}

fn p166_at(turn: u32, step: Step) -> impl Fn(&Driver) -> bool {
    move |driver: &Driver| {
        let now = driver.state().turn();
        (now.number() == turn && now.step() >= step) || now.number() > turn
    }
}

/// The dice logged since event `from`, as (result, natural).
fn p166_rolled(driver: &Driver, from: usize) -> Vec<u32> {
    driver.engine().events()[from..]
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled { result, .. } => Some(*result),
            _ => None,
        })
        .collect()
}

/// The rolls to visit logged since event `from`.
fn p166_visits(driver: &Driver, from: usize) -> Vec<u32> {
    driver.engine().events()[from..]
        .iter()
        .filter_map(|event| match event {
            Event::RolledToVisit { result, .. } => Some(*result),
            _ => None,
        })
        .collect()
}

fn p166_count(driver: &Driver, from: usize, wanted: fn(&Event) -> bool) -> usize {
    driver.engine().events()[from..]
        .iter()
        .filter(|event| wanted(event))
        .count()
}

#[rstest]
fn monitor_monitor_rerolls_the_turn_based_visit_roll_once_each_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] reroll: bool,
) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Upkeep)
        .attractions(0, [(un33_attraction("Ride"), Lights::of(1..=6))])
        .hand(
            0,
            [
                un33_opener("Open One", Some(1)),
                un33_opener("Roll Again", None),
            ],
        )
        .battlefield(
            0,
            [
                ready(card("Monitor Monitor").unwrap()),
                ready(island()),
                ready(island()),
            ],
        );
    let mut driver = Driver::from_scenario(p166_libraries(scenario, seats)).unwrap();
    driver.cast("Open One").unwrap();
    let start = driver.engine().events().len();
    let mut asked = Vec::new();
    p166_run(
        &mut driver,
        80,
        &mut |_, decision| match &decision.kind {
            DecisionKind::R706ChooseDice(options) => {
                assert_eq!(decision.player, pid(0));
                assert!(matches!(options.purpose, R706DicePurpose::Reroll(_)));
                assert_eq!((options.min, options.max, options.results.len()), (0, 1, 1));
                asked.push(options.results[0]);
                Some(Answer::R706ChooseDice(if reroll {
                    vec![0]
                } else {
                    Vec::new()
                }))
            }
            _ => None,
        },
        &|driver| {
            driver.state().turn().step() == Step::Main1
                && driver.state().priority().is_some()
                && driver.state().stack().is_empty()
        },
    );
    assert_eq!(asked.len(), 1, "the turn-based roll asks once");
    let rerolled = p166_count(&driver, start, |e| {
        matches!(e, Event::R706DiceRerolled { .. })
    });
    assert_eq!(rerolled, usize::from(reroll));
    let rolled = p166_rolled(&driver, start);
    assert_eq!(rolled.len(), 1, "only the result that counts is logged");
    assert_eq!(
        p166_visits(&driver, start),
        rolled,
        "the visit reads the roll's result"
    );
    let tapped = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.permanent().is_some_and(|p| p.tapped()))
        .count();
    assert_eq!(tapped, usize::from(reroll), "the reroll paid {{1}}");
    // The next roll: offered again only when the first was declined.
    let again = driver.engine().events().len();
    driver.cast("Roll Again").unwrap();
    let mut asked_again = 0;
    p166_run(
        &mut driver,
        40,
        &mut |_, decision| match &decision.kind {
            DecisionKind::R706ChooseDice(_) => {
                asked_again += 1;
                Some(Answer::R706ChooseDice(Vec::new()))
            }
            _ => None,
        },
        &|driver| driver.state().stack().is_empty() && driver.state().resolving().is_none(),
    );
    assert_eq!(asked_again, usize::from(!reroll));
    assert_eq!(p166_visits(&driver, again).len(), 1);
    p166_replays(&driver);
}

#[rstest]
fn bamboozling_beeble_adds_a_die_whose_ignored_roll_its_controller_chooses(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] ignored: usize,
) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Upkeep)
        .attractions(0, [(un33_attraction("Ride"), Lights::of(1..=6))])
        .hand(0, [un33_opener("Open One", Some(1))])
        .battlefield(
            1,
            [ready(card("Bamboozling Beeble").unwrap()), ready(island())],
        );
    let mut driver = Driver::from_scenario(p166_libraries(scenario, seats)).unwrap();
    driver.cast("Open One").unwrap();
    let start = driver.engine().events().len();
    let mut activated = false;
    let mut shown = Vec::new();
    p166_run(
        &mut driver,
        120,
        &mut |driver, decision| match &decision.kind {
            DecisionKind::Priority(options)
                if decision.player == pid(1)
                    && !activated
                    && driver.state().turn().step() == Step::Upkeep
                    && driver.state().stack().is_empty() =>
            {
                activated = true;
                let ability = options
                    .abilities
                    .iter()
                    .find(|a| driver.state().object(a.object).is_some())
                    .copied()?;
                Some(Answer::Priority(PriorityAction::Activate(ability)))
            }
            DecisionKind::ChooseTargets(options) if decision.player == pid(1) => {
                assert!(options.legal.contains(&Target::Player(pid(0))));
                Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
                    Target::Player(pid(0)),
                ])))
            }
            DecisionKind::R706ChooseDice(options) => {
                assert_eq!(decision.player, pid(1), "the Beeble's controller chooses");
                assert_eq!(options.roller, pid(0));
                assert_eq!((options.min, options.max, options.results.len()), (1, 1, 2));
                shown = options.results.clone();
                Some(Answer::R706ChooseDice(vec![ignored]))
            }
            _ => None,
        },
        &|driver| {
            driver.state().turn().step() == Step::Main1
                && driver.state().priority().is_some()
                && driver.state().stack().is_empty()
        },
    );
    assert!(activated);
    let rolled = p166_rolled(&driver, start);
    assert_eq!(rolled.len(), 1, "the ignored roll is never logged");
    assert_eq!(
        p166_count(&driver, start, |e| matches!(
            e,
            Event::R706DieIgnored { .. }
        )),
        1
    );
    if shown.len() == 2 {
        assert_eq!(
            rolled[0],
            shown[1 - ignored],
            "the roll kept is the other one"
        );
    }
    assert_eq!(p166_visits(&driver, start), rolled);
    assert!(
        driver.state().rule_effects().is_empty(),
        "the change ends with the roll it changed"
    );
    p166_replays(&driver);
}

/// Seat 0 attacks seat 1 with two Bears and casts Camouflage; seat 1 puts each of its creatures in its own pile.
#[rstest]
fn camouflage_blocks_with_piles_assigned_at_random(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] seed: u64,
) {
    let scenario = Scenario::new(seats)
        .seed(seed)
        .at(2, 0, Step::Main1)
        .hand(0, [card("Camouflage").unwrap()])
        .battlefield(
            0,
            [
                ready(bear()),
                ready(free_creature("Cub", 1, 1)),
                ready(forest()),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Wall", 0, 4)),
                ready(free_creature("Ogre", 3, 3)),
            ],
        );
    let mut driver = Driver::from_scenario(p166_libraries(scenario, seats)).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Grizzly Bears", 1), ("Cub", 1)]).unwrap();
    let start = driver.engine().events().len();
    let wall = driver.id_of("Wall").unwrap();
    let ogre = driver.id_of("Ogre").unwrap();
    let mut cast = false;
    let mut piles_asked = Vec::new();
    p166_run(
        &mut driver,
        80,
        &mut |driver, decision| match &decision.kind {
            DecisionKind::Priority(options)
                if decision.player == pid(0)
                    && !cast
                    && driver.state().turn().step() == Step::DeclareAttackers =>
            {
                cast = true;
                let spell = options
                    .castable
                    .iter()
                    .find(|id| driver.state().object(**id).is_some())
                    .copied()?;
                Some(Answer::Priority(PriorityAction::CastSpell(spell)))
            }
            DecisionKind::R509BlockPiles(options) => {
                piles_asked.push(decision.player);
                assert_eq!(options.attackers.len(), 2);
                assert_eq!(options.creatures.len(), 2);
                Some(Answer::R509BlockPiles(vec![vec![wall], vec![ogre]]))
            }
            DecisionKind::DeclareBlockers(_) => panic!("blockers are declared as piles"),
            _ => None,
        },
        &p166_at(2, Step::CombatDamage),
    );
    assert!(cast);
    assert_eq!(
        piles_asked,
        vec![pid(1)],
        "only the defending player divides"
    );
    let (assigned, blocks): (Vec<(ObjectId, Vec<ObjectId>)>, Vec<(ObjectId, ObjectId)>) = {
        let events = &driver.engine().events()[start..];
        let assigned = events
            .iter()
            .find_map(|event| match event {
                Event::R509PilesAssigned { piles, .. } => Some(piles.clone()),
                _ => None,
            })
            .unwrap();
        let blocks = events
            .iter()
            .find_map(|event| match event {
                Event::BlockersDeclared { blocks, .. } => Some(blocks.clone()),
                _ => None,
            })
            .unwrap();
        (assigned, blocks)
    };
    let mut expected: Vec<(ObjectId, ObjectId)> = assigned
        .iter()
        .flat_map(|(attacker, pile)| pile.iter().map(|creature| (*creature, *attacker)))
        .collect();
    let mut got = blocks;
    expected.sort_unstable();
    got.sort_unstable();
    assert_eq!(got, expected, "each creature blocks its pile's attacker");
    p166_replays(&driver);
}

#[test]
fn camouflage_piles_go_both_ways_across_seeds() {
    let mut orders = std::collections::BTreeSet::new();
    for seed in 0..16 {
        let scenario = Scenario::new(2)
            .seed(seed)
            .at(2, 0, Step::Main1)
            .hand(0, [card("Camouflage").unwrap()])
            .battlefield(
                0,
                [
                    ready(bear()),
                    ready(free_creature("Cub", 1, 1)),
                    ready(forest()),
                ],
            )
            .battlefield(1, [ready(free_creature("Wall", 0, 4))]);
        let mut driver = Driver::from_scenario(p166_libraries(scenario, 2)).unwrap();
        driver.pass_until(Step::DeclareAttackers).unwrap();
        driver.attack(&[("Grizzly Bears", 1), ("Cub", 1)]).unwrap();
        let wall = driver.id_of("Wall").unwrap();
        let mut cast = false;
        p166_run(
            &mut driver,
            60,
            &mut |_, decision| match &decision.kind {
                DecisionKind::Priority(options) if decision.player == pid(0) && !cast => {
                    cast = true;
                    options
                        .castable
                        .first()
                        .copied()
                        .map(|spell| Answer::Priority(PriorityAction::CastSpell(spell)))
                }
                DecisionKind::R509BlockPiles(_) => {
                    Some(Answer::R509BlockPiles(vec![vec![wall], vec![]]))
                }
                _ => None,
            },
            &p166_at(2, Step::CombatDamage),
        );
        let blocked = driver
            .engine()
            .events()
            .iter()
            .find_map(|event| match event {
                Event::BlockersDeclared { blocks, .. } => blocks.first().map(|(_, a)| *a),
                _ => None,
            });
        orders.insert(blocked);
    }
    assert_eq!(
        orders.len(),
        2,
        "the Wall's pile goes to either attacker: {orders:?}"
    );
}

#[rstest]
#[case("Monitor Monitor")]
#[case("Bamboozling Beeble")]
#[case("Camouflage")]
fn p166_the_cards_are_verified(#[case] name: &str) {
    let support = mtg_testkit::corpus::corpus()
        .unwrap()
        .support(name)
        .unwrap();
    assert_eq!(
        support.support,
        mtg_cards::support::Support::Verified,
        "{name}: {support:?}"
    );
}
