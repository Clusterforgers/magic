//! Goal: where teams share their turns (Teams with shared turns, Two-Headed Giant, a Delves party) every teammate's attacker deals its combat damage to the defender it attacks (CR 506.2, 510.1, 805.3): unblocked and blocked, first strike and trample, split over both defenders, while an effect sets a controller, and the live declaration shape of a 2v2 game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::common;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Duration;
use mtg_core::house::HouseRules;
use mtg_core::ids::ObjectId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::Event;
use mtg_engine::combat::DamageTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::effects::Modification;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, keyworded};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use proptest::prelude::*;
use rstest::rstest;

/// Teams of two that share their turns.
const PAIRS: &[u32] = &[0, 0, 1, 1];
/// Two-Headed Giant: the same seats with shared life.
const GIANT: &[u32] = &[0, 0, 1, 1];
/// A Delves party of three and its boss.
const PARTY: &[u32] = &[0, 0, 0, 1];

/// A table of `mode` at the main phase of the first team's turn, with `boards[seat]` on each seat's battlefield and a
/// creature of the last seat named `Hostage` that the first seat may take.
fn table(mode: TeamMode, seats: &[u32], boards: &[Vec<Arc<CardDefinition>>]) -> Scenario {
    let mut scenario = Scenario::new(seats.len())
        .house_rules(HouseRules {
            teams: Some(Teams {
                mode,
                seats: seats.to_vec(),
            }),
            ..HouseRules::default()
        })
        .at(1, 0, Step::Main1);
    for (seat, board) in boards.iter().enumerate() {
        let mut board: Vec<_> = board.iter().cloned().map(ready).collect();
        if seat + 1 == seats.len() {
            board.push(ready(free_creature("Hostage", 1, 1)));
        }
        scenario = scenario.battlefield(seat, board).life(seat, 100);
    }
    scenario
}

/// Gives the first seat control of the `Hostage` for the turn: a control effect is in force through combat.
fn take_hostage(driver: &mut Driver) -> Result<(), ScriptError> {
    let hostage = driver.id_of("Hostage")?;
    let on = driver.on(hostage);
    driver.add_effect(
        on,
        Modification::SetController(pid(0)),
        Duration::UntilEndOfTurn,
    )?;
    Ok(())
}

/// Plays the combat of the turn: each seat asked to attack sends its creatures named in `attacks` at their seat, each
/// defender asked to block declares its blocks from `blocks` (blocker, attacker), damage is divided by default.
fn fight(
    driver: &mut Driver,
    attacks: &[(&str, usize, usize)],
    blocks: &[(&str, &str, usize)],
) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    for _ in 0..200 {
        if driver.state().turn().step() == Step::Main2 {
            return Ok(());
        }
        let decision = driver.decision()?.clone();
        let seat = decision.player.index();
        match &decision.kind {
            DecisionKind::DeclareAttackers(_) => {
                let own: Vec<(&str, usize)> = attacks
                    .iter()
                    .filter(|(_, by, _)| *by == seat)
                    .map(|(name, _, at)| (*name, *at))
                    .collect();
                driver.attack(&own)?;
            }
            DecisionKind::DeclareBlockers(_) => {
                let own: Vec<(&str, &str)> = blocks
                    .iter()
                    .filter(|(_, _, by)| *by == seat)
                    .map(|(blocker, attacker, _)| (*blocker, *attacker))
                    .collect();
                driver.block(&own)?;
            }
            // Lethal damage to each blocker and the rest past them: the default division.
            DecisionKind::AssignDamage(options) => {
                let shares = options
                    .blockers
                    .iter()
                    .copied()
                    .zip(options.lethal.iter().copied());
                driver.answer(Answer::AssignDamage(shares.collect()))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("combat".to_owned()))
}

/// Combat damage each source dealt to each recipient over the whole log.
fn dealt(driver: &Driver) -> BTreeMap<(ObjectId, DamageTarget), u32> {
    let mut totals = BTreeMap::new();
    for event in driver.engine().events() {
        if let Event::DamageDealt {
            source,
            target,
            amount,
            ..
        } = event
        {
            *totals.entry((*source, *target)).or_insert(0) += amount;
        }
    }
    totals
}

/// The damage the creature named `name` dealt to the player at `seat`.
fn to_player(driver: &Driver, name: &str, seat: usize) -> u32 {
    let id = driver.id_of(name).unwrap();
    dealt(driver)
        .get(&(id, DamageTarget::Player(pid(seat))))
        .copied()
        .unwrap_or(0)
}

/// Each attacking seat's board: a plain attacker, a first striker and a trampler, named after the seat.
fn attackers(seat: usize) -> Vec<Arc<CardDefinition>> {
    vec![
        free_creature(&format!("Striker {seat}"), 3, 3),
        keyworded(&format!("Edge {seat}"), 2, 2, &[Keyword::FirstStrike]),
        keyworded(&format!("Trampler {seat}"), 4, 4, &[Keyword::Trample]),
    ]
}

#[rstest]
#[case::pairs(TeamMode::SharedTurns, PAIRS)]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, GIANT)]
#[case::party(TeamMode::SharedTurns, PARTY)]
fn every_teammates_attackers_deal_their_damage_to_the_defender_they_attack(
    #[case] mode: TeamMode,
    #[case] seats: &[u32],
    #[values(false, true)] control_effect: bool,
    #[values(false, true)] blocked: bool,
) -> Result<(), ScriptError> {
    let team: Vec<usize> = (0..seats.len()).filter(|s| seats[*s] == 0).collect();
    let foes: Vec<usize> = (0..seats.len()).filter(|s| seats[*s] != 0).collect();
    let (first_foe, last_foe) = (foes[0], foes[foes.len() - 1]);
    let boards: Vec<Vec<Arc<CardDefinition>>> = (0..seats.len())
        .map(|seat| {
            if team.contains(&seat) {
                attackers(seat)
            } else if seat == last_foe {
                vec![free_creature("Wall", 0, 2)]
            } else {
                Vec::new()
            }
        })
        .collect();
    let mut driver = Driver::from_scenario(table(mode, seats, &boards))?;
    if control_effect {
        take_hostage(&mut driver)?;
    }
    let names: Vec<(String, usize, usize)> = team
        .iter()
        .flat_map(|seat| {
            [
                (format!("Striker {seat}"), *seat, first_foe),
                (format!("Edge {seat}"), *seat, last_foe),
                (format!("Trampler {seat}"), *seat, last_foe),
            ]
        })
        .collect();
    let attacks: Vec<(&str, usize, usize)> = names
        .iter()
        .map(|(n, by, at)| (n.as_str(), *by, *at))
        .collect();
    // The last teammate's trampler meets the Wall: 2 to the Wall, 2 past it.
    let last = team[team.len() - 1];
    let trampler = format!("Trampler {last}");
    let blocks: Vec<(&str, &str, usize)> = if blocked {
        vec![("Wall", trampler.as_str(), last_foe)]
    } else {
        Vec::new()
    };
    let wall = driver.id_of("Wall")?;
    fight(&mut driver, &attacks, &blocks)?;
    for seat in &team {
        assert_eq!(
            to_player(&driver, &format!("Striker {seat}"), first_foe),
            3,
            "Striker {seat}"
        );
        assert_eq!(
            to_player(&driver, &format!("Edge {seat}"), last_foe),
            2,
            "Edge {seat}"
        );
        let trampled = if blocked && *seat == last { 2 } else { 4 };
        assert_eq!(
            to_player(&driver, &format!("Trampler {seat}"), last_foe),
            trampled,
            "Trampler {seat}"
        );
    }
    if blocked {
        let trampler = driver.id_of(&trampler)?;
        assert_eq!(
            dealt(&driver).get(&(trampler, DamageTarget::Object(wall))),
            Some(&2)
        );
    }
    Ok(())
}

#[rstest]
#[case::the_attacked_head(false)]
#[case::the_other_head(true)]
fn either_head_of_the_attacked_giant_blocks_a_teammates_attacker_while_an_effect_sets_a_controller(
    #[case] other_head: bool,
) -> Result<(), ScriptError> {
    let boards = vec![
        vec![free_creature("Ogre", 3, 3)],
        vec![free_creature("Brute", 5, 5)],
        if other_head {
            Vec::new()
        } else {
            vec![free_creature("Wall", 0, 6)]
        },
        if other_head {
            vec![free_creature("Wall", 0, 6)]
        } else {
            Vec::new()
        },
    ];
    let mut driver = Driver::from_scenario(table(TeamMode::TwoHeadedGiant, GIANT, &boards))?;
    take_hostage(&mut driver)?;
    let defender = if other_head { 3 } else { 2 };
    fight(
        &mut driver,
        &[("Ogre", 0, 2), ("Brute", 1, 2)],
        &[("Wall", "Brute", defender)],
    )?;
    let (brute, wall) = (driver.id_of("Brute")?, driver.id_of("Wall")?);
    assert_eq!(
        dealt(&driver).get(&(brute, DamageTarget::Object(wall))),
        Some(&5)
    );
    assert_eq!(to_player(&driver, "Ogre", 2), 3);
    assert_eq!(to_player(&driver, "Brute", 2), 0);
    Ok(())
}

/// The live 2v2 game: seat 0 sends its two creatures at seat 2 and its teammate's four at seat 3 while a Master Thief
/// holds a stolen artifact; nobody blocks.
#[test]
fn the_live_declaration_of_a_two_versus_two_turn_deals_every_attackers_damage()
-> Result<(), ScriptError> {
    let boards = vec![
        vec![
            free_creature("Lorcan", 7, 7),
            card("Dream Pillager").unwrap(),
        ],
        vec![
            free_creature("Blackguard", 1, 3),
            free_creature("Rogue", 2, 2),
            free_creature("Thopter", 1, 1),
            free_creature("Thief", 2, 2),
        ],
        Vec::new(),
        Vec::new(),
    ];
    let mut driver = Driver::from_scenario(table(TeamMode::SharedTurns, PAIRS, &boards))?;
    take_hostage(&mut driver)?;
    let attacks = [
        ("Lorcan", 0, 2),
        ("Dream Pillager", 0, 2),
        ("Blackguard", 1, 3),
        ("Rogue", 1, 3),
        ("Thopter", 1, 3),
        ("Thief", 1, 3),
    ];
    let life = |driver: &Driver, seat: usize| driver.state().player(pid(seat)).unwrap().life();
    let before = (life(&driver, 2), life(&driver, 3));
    fight(&mut driver, &attacks, &[])?;
    assert_eq!(to_player(&driver, "Lorcan", 2), 7);
    assert_eq!(to_player(&driver, "Dream Pillager", 2), 4);
    for (name, power) in [
        ("Blackguard", 1),
        ("Rogue", 2),
        ("Thopter", 1),
        ("Thief", 2),
    ] {
        assert_eq!(to_player(&driver, name, 3), power, "{name}");
    }
    assert_eq!(
        (life(&driver, 2), life(&driver, 3)),
        (before.0 - 11, before.1 - 6)
    );
    Ok(())
}

/// A teammate's creature with a combat damage trigger: Dream Pillager under seat 1 exiles as many cards as it dealt.
#[rstest]
#[case::pairs(TeamMode::SharedTurns, PAIRS, 3)]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, GIANT, 2)]
#[case::party(TeamMode::SharedTurns, PARTY, 3)]
fn a_teammates_combat_damage_trigger_fires(
    #[case] mode: TeamMode,
    #[case] seats: &[u32],
    #[case] foe: usize,
) -> Result<(), ScriptError> {
    let mut boards = vec![Vec::new(); seats.len()];
    boards[0].push(free_creature("Ogre", 3, 3));
    boards[1].push(card("Dream Pillager").unwrap());
    let scenario =
        table(mode, seats, &boards).library(1, (0..8).map(|_| free_creature("Bear", 2, 2)));
    let mut driver = Driver::from_scenario(scenario)?;
    take_hostage(&mut driver)?;
    fight(
        &mut driver,
        &[("Ogre", 0, foe), ("Dream Pillager", 1, foe)],
        &[],
    )?;
    let power = to_player(&driver, "Dream Pillager", foe);
    assert!(power > 0);
    assert_eq!(
        driver.state().zone(mtg_engine::zones::Zone::Exile).len(),
        usize::try_from(power).unwrap()
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Every unblocked attacker of the active team deals damage equal to its power to the defender it attacks, whoever
    /// of the team controls it and whatever control effect is in force.
    #[test]
    fn every_unblocked_attacker_of_the_team_deals_its_power(
        giant in any::<bool>(),
        control_effect in any::<bool>(),
        plan in prop::collection::vec((0usize..2, 1i32..6, any::<bool>()), 1..6),
    ) {
        let mode = if giant { TeamMode::TwoHeadedGiant } else { TeamMode::SharedTurns };
        let mut boards: Vec<Vec<Arc<CardDefinition>>> = vec![Vec::new(); 4];
        let mut attacks = Vec::new();
        let names: Vec<String> = (0..plan.len()).map(|at| format!("Raider {at}")).collect();
        for (at, (seat, power, far)) in plan.iter().enumerate() {
            boards[*seat].push(free_creature(&names[at], *power, 1));
            attacks.push((names[at].as_str(), *seat, if *far { 3 } else { 2 }));
        }
        let mut driver = Driver::from_scenario(table(mode, PAIRS, &boards)).unwrap();
        if control_effect {
            take_hostage(&mut driver).unwrap();
        }
        fight(&mut driver, &attacks, &[]).unwrap();
        for ((name, _, at), (_, power, _)) in attacks.iter().zip(&plan) {
            prop_assert_eq!(to_player(&driver, name, *at), u32::try_from(*power).unwrap(), "{}", name);
        }
    }
}
