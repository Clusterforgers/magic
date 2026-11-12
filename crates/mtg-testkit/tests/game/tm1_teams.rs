//! Goal: teams decide who is an opponent at four and six seats, a team wins once all its opponents have lost (CR 808), and Two-Headed Giant shares the turn, the life total, the poison counters and the loss of a team (CR 810), each with the turn order, the draws, the sorcery timing, the land plays and the combat of a shared turn; bot games of every mode finish under every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use crate::common::seeds::{for_each_seed, fuzz_games};

use std::sync::Arc;

use mtg_cards::value::CounterKind;
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Commander;
use mtg_engine::player::Player;
use mtg_engine::state::Stage;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameResult, GameState};
use mtg_testkit::cards::{bear, elf, forest, giant, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{
    Driver, Scenario, ScriptError, commander_fuzz_setup, fuzz_setup, play_game, ready,
};
use rstest::rstest;

/// Two even teams over `seats` seats.
fn rules(mode: TeamMode, seats: usize) -> HouseRules {
    let size = u32::try_from(seats / 2).unwrap();
    HouseRules {
        teams: Some(Teams::even(mode, 2, size)),
        ..HouseRules::default()
    }
}

fn life(state: &GameState, seat: usize) -> i32 {
    state.player(pid(seat)).map_or(i32::MIN, Player::life)
}

fn poison(state: &GameState, seat: usize) -> u32 {
    state
        .player(pid(seat))
        .map_or(0, |p| p.counters_of(&CounterKind::Poison))
}

fn alive(state: &GameState) -> Vec<usize> {
    state.alive_players().map(PlayerId::index).collect()
}

/// Seat 0 holds Infectious Inquiry ({2}{B}: draw two, lose 2 life, each opponent gets a poison counter) and three
/// Swamps; seat 1 holds the same.
fn inquiry_table(seats: usize, house: HouseRules) -> Scenario {
    let swamps = || (0..3).map(|_| ready(swamp()));
    Scenario::new(seats)
        .hand(0, [card("Infectious Inquiry").unwrap()])
        .hand(1, [card("Infectious Inquiry").unwrap()])
        .battlefield(0, swamps())
        .battlefield(1, swamps())
        .house_rules(house)
}

#[rstest]
fn opponents_are_the_players_of_other_teams(
    #[values(4, 6)] seats: usize,
    #[values(TeamMode::Teams, TeamMode::TwoHeadedGiant)] mode: TeamMode,
) -> Result<(), ScriptError> {
    if mode == TeamMode::TwoHeadedGiant && seats != 4 {
        return Ok(());
    }
    let house = rules(mode, seats);
    let teams = house.teams.clone().unwrap();
    let mut driver = Driver::from_scenario(inquiry_table(seats, house))?;
    driver.cast("Infectious Inquiry")?;
    driver.pass()?;
    for _ in 0..seats {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    let state = driver.state();
    for seat in 0..seats {
        let foe = teams.team_of(pid(seat)) != teams.team_of(pid(0));
        assert_eq!(state.is_opponent(pid(0), pid(seat)), foe, "seat {seat}");
        assert_eq!(poison(state, seat), u32::from(foe), "seat {seat}");
    }
    assert_eq!(state.opponents_of(pid(0)).len(), seats / 2);
    Ok(())
}

#[rstest]
fn a_team_wins_when_all_its_opponents_have_lost(
    #[values(4, 6)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).house_rules(rules(TeamMode::Teams, seats)))?;
    // Teams alternate around the table: team 0 holds the even seats.
    driver.concede(pid(0))?;
    assert_eq!(driver.state().stage(), Stage::Turns, "a teammate plays on");
    let foes: Vec<usize> = (0..seats).filter(|seat| seat % 2 == 1).collect();
    for (index, seat) in foes.iter().enumerate() {
        assert_eq!(driver.state().result(), None);
        driver.concede(pid(*seat))?;
        for _ in 0..20 {
            if driver.state().stage() == Stage::Finished || index + 1 < foes.len() {
                break;
            }
            driver.answer_passively()?;
        }
    }
    assert_eq!(driver.state().result(), Some(GameResult::Team(0)));
    Ok(())
}

#[rstest]
#[case::normal(false, 30)]
#[case::commander(true, 60)]
fn a_two_headed_giant_team_starts_with_one_shared_total(
    #[case] commander: bool,
    #[case] total: i32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(4).house_rules(rules(TeamMode::TwoHeadedGiant, 4));
    if commander {
        scenario = scenario.format(Arc::new(Commander));
    }
    let driver = Driver::from_scenario(scenario)?;
    for seat in 0..4 {
        assert_eq!(life(driver.state(), seat), total);
    }
    Ok(())
}

#[test]
fn a_two_headed_giant_team_shares_life_poison_and_its_loss() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        inquiry_table(4, rules(TeamMode::TwoHeadedGiant, 4))
            .counters(2, CounterKind::Poison, 8)
            .counters(3, CounterKind::Poison, 6),
    )?;
    driver.cast("Infectious Inquiry")?;
    for _ in 0..8 {
        if driver.state().stage() == Stage::Finished {
            break;
        }
        driver.answer_passively()?;
    }
    let state = driver.state();
    // The 2 life seat 0 lost is the team's; the team of seats 2 and 3 holds 16 poison counters, past 15.
    assert_eq!((life(state, 0), life(state, 1)), (28, 28));
    assert_eq!((poison(state, 2), poison(state, 3)), (9, 7));
    assert_eq!(alive(state), [0, 1]);
    assert_eq!(state.result(), Some(GameResult::Team(0)));
    Ok(())
}

#[test]
fn a_two_headed_giant_player_who_concedes_takes_the_team_out() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(Scenario::new(4).house_rules(rules(TeamMode::TwoHeadedGiant, 4)))?;
    driver.concede(pid(2))?;
    for _ in 0..8 {
        if driver.state().stage() == Stage::Finished {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(alive(driver.state()), [0, 1]);
    assert_eq!(driver.state().result(), Some(GameResult::Team(0)));
    Ok(())
}

#[test]
fn a_two_headed_giant_turn_belongs_to_the_team() -> Result<(), ScriptError> {
    let hand = || [forest(), forest(), card("Infectious Inquiry").unwrap()];
    let mut driver = Driver::from_scenario(
        inquiry_table(4, rules(TeamMode::TwoHeadedGiant, 4))
            .hand(0, hand())
            .hand(1, hand())
            .at(1, 0, Step::Main1),
    )?;
    assert_eq!(driver.state().active_players(), [pid(0), pid(1)]);
    // Each player of the active team plays their own land.
    driver.play_land("Forest")?;
    assert!(driver.play_land("Forest").is_err());
    driver.pass()?;
    assert_eq!(driver.decision()?.player, pid(1));
    driver.play_land("Forest")?;
    // The teammate casts a sorcery on the team's turn.
    driver.cast("Infectious Inquiry")?;
    assert_eq!(life(driver.state(), 0), life(driver.state(), 1));
    // Turns go from team to team, and both players of the team draw.
    let mut seen = vec![driver.state().turn().active()];
    for _ in 0..400 {
        let turn = driver.state().turn();
        if turn.number() == 3 && turn.step() == Step::Main1 {
            break;
        }
        if seen.last() != Some(&turn.active()) {
            seen.push(turn.active());
        }
        driver.answer_passively()?;
    }
    assert_eq!(seen, [pid(0), pid(2), pid(0)]);
    let state = driver.state();
    // Seat 0 kept two cards and drew one; seat 1 kept one, drew two with the Inquiry and one in the draw step.
    assert_eq!(state.zone(Zone::Hand(pid(0))).len(), 3);
    assert_eq!(state.zone(Zone::Hand(pid(1))).len(), 4);
    Ok(())
}

#[test]
fn a_two_headed_giant_team_attacks_together_and_either_defender_blocks() -> Result<(), ScriptError>
{
    let mut driver = Driver::from_scenario(
        Scenario::new(4)
            .battlefield(0, [ready(bear())])
            .battlefield(1, [ready(elf())])
            .battlefield(3, [ready(giant())])
            .house_rules(rules(TeamMode::TwoHeadedGiant, 4))
            .at(1, 0, Step::Main1),
    )?;
    driver.pass_until(Step::DeclareAttackers)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("no attack");
    };
    assert_eq!(
        options.attackers.len(),
        2,
        "the team's creatures attack together"
    );
    driver.attack(&[("Grizzly Bears", 2), ("Llanowar Elf", 2)])?;
    while !matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
        driver.pass()?;
    }
    // Seat 3 blocks an attacker of seat 2's team though seat 2 alone was attacked.
    assert_eq!(driver.decision()?.player, pid(3));
    driver.block(&[("Craw Wurm", "Grizzly Bears")])?;
    driver.pass_until(Step::End)?;
    let state = driver.state();
    // The unblocked 1/1 hurts the team.
    assert_eq!((life(state, 2), life(state, 3)), (29, 29));
    Ok(())
}

#[rstest]
#[case::giant(TeamMode::TwoHeadedGiant, 4, false)]
#[case::giant_commander(TeamMode::TwoHeadedGiant, 4, true)]
#[case::two_v_two(TeamMode::Teams, 4, false)]
#[case::three_v_three(TeamMode::Teams, 6, false)]
fn seeded_team_games_finish_with_a_team_or_a_draw(
    #[case] mode: TeamMode,
    #[case] seats: usize,
    #[case] commander: bool,
) {
    for_each_seed(0..fuzz_games(2), |seed| {
        let mut setup = if commander {
            commander_fuzz_setup(seats, seed).unwrap()
        } else {
            fuzz_setup(seats, seed)
        };
        setup.house_rules = rules(mode, seats);
        let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
        assert!(
            matches!(outcome.result, GameResult::Team(0 | 1) | GameResult::Draw),
            "seed {seed}: {:?}",
            outcome.result
        );
    });
}
