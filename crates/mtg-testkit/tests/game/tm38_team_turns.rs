//! Goal: at four and six seats, Two-Headed Giant gives the turn to a team and Teams to one player (CR 805, CR 808): the turn order, the players who draw, the players who may play lands and cast sorceries, the priority order and the players a creature may attack all follow the mode.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use crate::common::pid;

use mtg_core::house::HouseRules;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::Event;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, forest, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Every seat holds a Forest and Infectious Inquiry (a sorcery for {2}{B}) with three Swamps and a ready bear out.
fn table(mode: TeamMode, seats: usize) -> (Scenario, Teams) {
    let teams = Teams::even(mode, 2, u32::try_from(seats / 2).unwrap());
    let mut scenario = Scenario::new(seats)
        .house_rules(HouseRules {
            teams: Some(teams.clone()),
            ..HouseRules::default()
        })
        .at(1, 0, Step::Upkeep);
    for seat in 0..seats {
        scenario = scenario
            .hand(seat, [forest(), card("Infectious Inquiry").unwrap()])
            .battlefield(
                seat,
                [
                    ready(swamp()),
                    ready(swamp()),
                    ready(swamp()),
                    ready(bear()),
                ],
            );
    }
    (scenario, teams)
}

/// Whether it is `seat`'s turn while `active` is the active player under `teams`.
fn turn_of(teams: &Teams, seat: usize, active: usize) -> bool {
    seat == active || (teams.shared() && teams.team_of(pid(seat)) == teams.team_of(pid(active)))
}

/// What a turn showed: the priority holders of its precombat main phase in order, with whether each could play a land
/// and cast the sorcery; the players who drew in its draw step; and the players its attackers could attack.
#[derive(Debug, Default)]
struct TurnSeen {
    active: usize,
    main: Vec<(usize, bool, bool)>,
    drew: BTreeSet<usize>,
    attackable: BTreeSet<usize>,
    attackers: usize,
}

fn watch(driver: &mut Driver, turns: u32) -> Result<BTreeMap<u32, TurnSeen>, ScriptError> {
    let mut seen: BTreeMap<u32, TurnSeen> = BTreeMap::new();
    for _ in 0..2000 {
        let state = driver.state();
        let turn = state.turn();
        if turn.number() > turns {
            return Ok(seen);
        }
        let entry = seen.entry(turn.number()).or_default();
        entry.active = turn.active().index();
        let decision = driver.decision()?.clone();
        let seat = decision.player.index();
        match &decision.kind {
            DecisionKind::Priority(options)
                if turn.step() == Step::Main1 && state.stack().is_empty() =>
            {
                // The hand holds a Forest and the sorcery alone.
                let castable = !options.castable.is_empty();
                entry
                    .main
                    .push((seat, !options.playable_lands.is_empty(), castable));
            }
            DecisionKind::DeclareAttackers(options) => {
                entry.attackers = options.attackers.len();
                for option in &options.attackers {
                    for target in &option.defenders {
                        if let AttackTarget::Player(player) = target {
                            entry.attackable.insert(player.index());
                        }
                    }
                }
            }
            _ => {}
        }
        let drawing = turn.step() == Step::Draw;
        let number = turn.number();
        let events = driver.answer_passively()?;
        for event in events {
            if let Event::CardDrawn { player, .. } = event
                && (drawing || driver.state().turn().step() == Step::Draw)
                && driver.state().turn().number() == number
            {
                seen.entry(number).or_default().drew.insert(player.index());
            }
        }
    }
    panic!("the game did not reach turn {turns}");
}

#[rstest]
#[case::giant_four(TeamMode::TwoHeadedGiant, 4)]
#[case::teams_four(TeamMode::Teams, 4)]
#[case::teams_six(TeamMode::Teams, 6)]
fn a_turn_belongs_to_the_team_in_two_headed_giant_and_to_one_player_in_teams(
    #[case] mode: TeamMode,
    #[case] seats: usize,
) -> Result<(), ScriptError> {
    let (scenario, teams) = table(mode, seats);
    let mut driver = Driver::from_scenario(scenario)?;
    let rounds = if teams.shared() {
        4
    } else {
        u32::try_from(seats).unwrap() + 1
    };
    let seen = watch(&mut driver, rounds)?;

    // Turn order: Two-Headed Giant goes from team to team, Teams from seat to seat.
    let order: Vec<usize> = seen.values().map(|turn| turn.active).collect();
    let want: Vec<usize> = (0..rounds as usize)
        .map(|n| {
            if teams.shared() {
                (n % 2) * 2
            } else {
                n % seats
            }
        })
        .collect();
    assert_eq!(order, want, "turn order");

    for (number, turn) in &seen {
        let mine: BTreeSet<usize> = (0..seats)
            .filter(|seat| turn_of(&teams, *seat, turn.active))
            .collect();
        // Priority goes round the table in turn order from the active player.
        let holders: Vec<usize> = turn.main.iter().map(|(seat, _, _)| *seat).collect();
        let round: Vec<usize> = (0..seats).map(|n| (turn.active + n) % seats).collect();
        assert_eq!(holders, round, "turn {number}: priority order");
        // Lands and sorceries are for the players whose turn it is, and for them all.
        for (seat, land, sorcery) in &turn.main {
            let own = mine.contains(seat);
            assert_eq!((*land, *sorcery), (own, own), "turn {number}: seat {seat}");
        }
        // The players whose turn it is draw, and nobody else; the first team skips its first draw in Two-Headed Giant
        // (CR 103.8b), while a multiplayer Teams game skips none (CR 103.8a).
        let drew = if *number == 1 && teams.shared() {
            BTreeSet::new()
        } else {
            mine.clone()
        };
        assert_eq!(turn.drew, drew, "turn {number}: who draws");
        // The creatures of the players whose turn it is attack together, and only the other team.
        assert_eq!(turn.attackers, mine.len(), "turn {number}: attackers");
        let foes: BTreeSet<usize> = (0..seats)
            .filter(|seat| teams.team_of(pid(*seat)) != teams.team_of(pid(turn.active)))
            .collect();
        assert_eq!(turn.attackable, foes, "turn {number}: attackable");
    }
    Ok(())
}
