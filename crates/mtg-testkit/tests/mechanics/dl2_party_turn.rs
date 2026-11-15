//! Goal: under shared team turns (CR 805) a party of one to six heroes takes one turn against a boss and its minion, who take theirs: the turn order goes from side to side, every hero untaps, draws, may play a land and cast a sorcery and receives priority in turn order, the creatures of every hero attack in one combat while a creature that came under a hero's control stays sick until the party's next turn, each hero keeps their own life total, and a Spirit keeps sharing the party's turns; a side's turn is led by its first seat, the boss and not its minion.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::{BTreeMap, BTreeSet};

use common::pid;
use mtg_core::house::HouseRules;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::Event;
use mtg_engine::PresetPermanent;
use mtg_engine::characteristics::controller_of;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_testkit::cards::{forest, free_creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The boss's life: enough to take every attack of the test.
const BOSS_LIFE: i32 = 100;

/// `heroes` heroes on team 0, then the boss and a minion on team 1, under shared team turns. Every seat holds a Forest
/// and Infectious Inquiry (a sorcery for {2}{B}) with three Swamps and a ready Bear out; each hero also has a Pup that
/// came under their control this turn.
fn party(heroes: usize) -> Scenario {
    let mut seats = vec![0; heroes];
    seats.extend([1, 1]);
    let mut scenario = Scenario::new(heroes + 2)
        .house_rules(HouseRules {
            teams: Some(Teams {
                mode: TeamMode::SharedTurns,
                seats,
            }),
            ..HouseRules::default()
        })
        .at(1, 0, Step::Upkeep);
    for seat in 0..heroes + 2 {
        let mut board = vec![
            ready(swamp()),
            ready(swamp()),
            ready(swamp()),
            ready(free_creature(&format!("Bear {seat}"), 2, 2)),
        ];
        if seat < heroes {
            board.push(PresetPermanent {
                summoning_sick: true,
                ..ready(free_creature(&format!("Pup {seat}"), 1, 1))
            });
        }
        scenario = scenario
            .hand(seat, [forest(), card("Infectious Inquiry").unwrap()])
            .battlefield(seat, board);
        if seat >= heroes {
            scenario = scenario.life(seat, BOSS_LIFE);
        }
    }
    scenario
}

/// What a turn showed: its active seat, the priority holders of its precombat main phase with whether each could play
/// a land and cast the sorcery, the players who drew, the attackers offered with their controllers, and the players
/// they could attack.
#[derive(Debug, Default)]
struct TurnSeen {
    active: usize,
    main: Vec<(usize, bool, bool)>,
    drew: BTreeSet<usize>,
    attackers: BTreeSet<usize>,
    attackable: BTreeSet<usize>,
    offered: usize,
}

/// Plays `turns` turns; every offered attacker attacks the first player it may attack, and everything else is passive.
fn watch(driver: &mut Driver, turns: u32) -> Result<BTreeMap<u32, TurnSeen>, ScriptError> {
    let mut seen: BTreeMap<u32, TurnSeen> = BTreeMap::new();
    for _ in 0..4000 {
        let state = driver.state();
        let turn = state.turn();
        if turn.number() > turns || state.result().is_some() {
            return Ok(seen);
        }
        let entry = seen.entry(turn.number()).or_default();
        entry.active = turn.active().index();
        let decision = driver.decision()?.clone();
        let seat = decision.player.index();
        let mut answer = Driver::passive_answer(&decision);
        match &decision.kind {
            DecisionKind::Priority(options)
                if turn.step() == Step::Main1 && state.stack().is_empty() =>
            {
                entry.main.push((
                    seat,
                    !options.playable_lands.is_empty(),
                    !options.castable.is_empty(),
                ));
            }
            DecisionKind::DeclareAttackers(options) => {
                entry.offered += options.attackers.len();
                let mut chosen = Vec::new();
                for option in &options.attackers {
                    let controller = controller_of(state, option.object).unwrap();
                    // Each player chooses the attacks of their own creatures only.
                    assert_eq!(
                        controller, decision.player,
                        "{seat} is offered another's creature"
                    );
                    entry.attackers.insert(controller.index());
                    for target in &option.defenders {
                        if let AttackTarget::Player(player) = target {
                            entry.attackable.insert(player.index());
                        }
                    }
                    if let Some(first) = option.defenders.first() {
                        chosen.push((option.object, *first));
                    }
                }
                answer = Answer::DeclareAttackers(chosen);
            }
            _ => {}
        }
        let number = turn.number();
        for event in driver.answer(answer)? {
            if let Event::CardDrawn { player, .. } = event
                && driver.state().turn().step() == Step::Draw
                && driver.state().turn().number() == number
            {
                seen.entry(number).or_default().drew.insert(player.index());
            }
        }
    }
    panic!("the game did not reach turn {turns}");
}

#[rstest]
#[case::one_hero(1)]
#[case::two_heroes(2)]
#[case::four_heroes(4)]
#[case::six_heroes(6)]
fn the_party_shares_one_turn_against_the_boss(#[case] heroes: usize) -> Result<(), ScriptError> {
    let seats = heroes + 2;
    let (boss, minion) = (heroes, heroes + 1);
    let mut driver = Driver::from_scenario(party(heroes))?;
    let seen = watch(&mut driver, 4)?;

    let order: Vec<usize> = seen.values().map(|turn| turn.active).collect();
    assert_eq!(order, [0, boss, 0, boss], "turn order");

    let party: BTreeSet<usize> = (0..heroes).collect();
    let boss_side = BTreeSet::from([boss, minion]);
    for (number, turn) in &seen {
        let ours = if turn.active == boss {
            &boss_side
        } else {
            &party
        };
        let round: Vec<usize> = (0..seats).map(|n| (turn.active + n) % seats).collect();
        let holders: Vec<usize> = turn.main.iter().map(|(seat, _, _)| *seat).collect();
        assert_eq!(holders, round, "turn {number}: priority order");
        for (seat, land, sorcery) in &turn.main {
            let own = ours.contains(seat);
            assert_eq!((*land, *sorcery), (own, own), "turn {number}: seat {seat}");
        }
        // Only a game of two skips the first draw (CR 103.8a, CR 103.8c).
        let drew = if *number == 1 && seats == 2 {
            BTreeSet::new()
        } else {
            ours.clone()
        };
        assert_eq!(&turn.drew, &drew, "turn {number}: who draws");
        assert_eq!(
            &turn.attackers, ours,
            "turn {number}: whose creatures attack"
        );
        let foes = if turn.active == boss {
            &party
        } else {
            &boss_side
        };
        assert_eq!(&turn.attackable, foes, "turn {number}: attackable");
    }
    // The Bears that attacked on turn 1 untapped for turn 3, and the Pups were sick until the party's next turn.
    assert_eq!(seen[&1].offered, heroes, "turn 1: the Bears");
    assert_eq!(
        seen[&3].offered,
        2 * heroes,
        "turn 3: the Bears and the Pups"
    );

    // Each hero keeps their own life: the boss side's attacks spread over the heroes it chose, and the party's hit
    // the boss and its minion.
    let state = driver.state();
    let lost: i32 = (0..heroes)
        .map(|hero| 20 - state.player(pid(hero)).unwrap().life())
        .sum();
    assert_eq!(lost, 2 * 2 * 2, "the boss's and the minion's Bears, twice");
    let dealt: i32 = [boss, minion]
        .iter()
        .map(|seat| BOSS_LIFE - state.player(pid(*seat)).unwrap().life())
        .sum();
    let party_power = i32::try_from(heroes).unwrap();
    assert_eq!(
        dealt,
        2 * party_power + 3 * party_power,
        "Bears twice, Pups once"
    );
    Ok(())
}

#[rstest]
#[case::two_heroes(2)]
#[case::four_heroes(4)]
#[case::six_heroes(6)]
fn a_spirit_keeps_sharing_the_partys_turns(#[case] heroes: usize) -> Result<(), ScriptError> {
    let spirit = heroes - 1;
    let scenario = party(heroes)
        .life(spirit, 0)
        .spirit(spirit, std::sync::Arc::new(emblem_definition("Haunt", &[])));
    let mut driver = Driver::from_scenario(scenario)?;
    let seen = watch(&mut driver, 4)?;
    let state = driver.state();
    assert!(state.player(pid(spirit)).unwrap().is_spirit());
    let order: Vec<usize> = seen.values().map(|turn| turn.active).collect();
    assert_eq!(order, [0, heroes, 0, heroes], "turn order");
    for (number, turn) in &seen {
        let holds = turn.main.iter().any(|(seat, _, _)| *seat == spirit);
        assert!(holds, "turn {number}: the Spirit gets priority");
        if turn.active == 0 {
            assert!(
                turn.drew.contains(&spirit),
                "turn {number}: the Spirit draws"
            );
            // Its cards left the game, so no creature of its attacks.
            assert!(!turn.attackers.contains(&spirit), "turn {number}");
        } else {
            assert!(
                !turn.attackable.contains(&spirit),
                "turn {number}: out of reach"
            );
        }
    }
    Ok(())
}

#[rstest]
#[case::a_later_hero(3, 0)]
#[case::the_first_hero(0, 0)]
#[case::the_minion(5, 4)]
#[case::the_boss(4, 4)]
fn a_sides_turn_is_led_by_its_first_seat(
    #[case] seat: usize,
    #[case] lead: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(party(4))?;
    let state = driver.state();
    assert_eq!(state.turn_lead(pid(seat)), pid(lead));
    // A side's turn passes to the other side's first seat: the boss, never its minion, or the first hero.
    let next = if lead == 0 { 4 } else { 0 };
    assert_eq!(state.next_turn_seat(pid(seat)), Some(pid(next)));
    Ok(())
}
