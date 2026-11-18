//! Goal: a boss phase set to a round begins at the end step of that round's last turn, once, whatever the table: two
//! or three seats, teams sharing their turns, an extra turn of either seat (which never advances the round); a phase's
//! "when this phase begins" ability uses the stack once, and an older phase's emblem does not trigger again when the
//! next phase begins; the games replay from their logs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::refs::Ref;
use mtg_core::house::HouseRules;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_testkit::turn_cards::time_spell;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// The boss seat of every table.
const BOSS: usize = 1;

/// When this phase begins, you gain `n` life.
fn on_begin(n: i32) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"PhaseBegins":{{}}}},"effects":[{{"GainLife":{{"amount":{n}}}}}]}}}}"#
    )
}

fn phase(name: &str, round: u32, gain: i32) -> BossPhase {
    let ability: Ability = serde_json::from_str(&on_begin(gain)).unwrap();
    let emblem: Arc<CardDefinition> = Arc::new(emblem_definition(name, &[ability]));
    BossPhase {
        begins: PhaseStart::AtRound(round),
        emblem,
    }
}

/// Where a phase began: the round, the active seat, whether the turn was extra, and the step.
type Began = (u32, usize, bool, Step);

/// Casts `first` (if any), then plays passively until `turns` turns have begun; returns where each of the boss's
/// phases began.
fn play(
    scenario: Scenario,
    first: Option<&str>,
    turns: u32,
) -> Result<(Vec<Began>, Driver), ScriptError> {
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    if let Some(card) = first {
        driver.cast(card)?;
    }
    let mut began = Vec::new();
    for _ in 0..20_000 {
        let state = driver.state();
        let (_, now) = state.player(pid(BOSS)).unwrap().phases();
        if now > began.len() {
            let turn = state.turn();
            began.push((
                turn.round(),
                turn.active().index(),
                turn.is_extra(),
                turn.step(),
            ));
        }
        if turn_number(&driver) > turns {
            break;
        }
        driver.answer_passively()?;
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok((began, driver))
}

fn turn_number(driver: &Driver) -> u32 {
    driver.state().turn().number()
}

fn table(seats: usize, teams: Option<Vec<u32>>, rounds: [u32; 2]) -> Scenario {
    let house = HouseRules {
        teams: teams.map(|seats| Teams {
            mode: TeamMode::SharedTurns,
            seats,
        }),
        ..HouseRules::default()
    };
    Scenario::new(seats).house_rules(house).phases(
        BOSS,
        [
            phase("Phase 1", rounds[0], 5),
            phase("Phase 2", rounds[1], 3),
        ],
    )
}

#[rstest]
#[case::two_seats(2, None, 7, vec![(2, 1, false, Step::End), (3, 1, false, Step::End)])]
#[case::three_seats(3, None, 10, vec![(2, 2, false, Step::End), (3, 2, false, Step::End)])]
#[case::shared_team_turns(3, Some(vec![0, 1, 1]), 7, vec![(2, 1, false, Step::End), (3, 1, false, Step::End)])]
fn phases_begin_at_the_end_step_of_a_rounds_last_turn(
    #[case] seats: usize,
    #[case] teams: Option<Vec<u32>>,
    #[case] turns: u32,
    #[case] want: Vec<Began>,
) -> Result<(), ScriptError> {
    let (began, driver) = play(table(seats, teams, [2, 3]), None, turns)?;
    assert_eq!(began, want);
    // Each phase's "when it begins" ability resolved once: 5 and 3 life, the first emblem not again.
    assert_eq!(driver.state().player(pid(BOSS)).unwrap().life(), 20 + 5 + 3);
    Ok(())
}

#[rstest]
#[case::hero_extra_turn_mid_round(0, Ref::You, vec![(2, 1, false, Step::End), (3, 1, false, Step::End)])]
#[case::boss_extra_turn_ends_the_round(BOSS, Ref::You, vec![(2, 1, true, Step::End), (3, 1, false, Step::End)])]
fn an_extra_turn_does_not_advance_the_round(
    #[case] caster: usize,
    #[case] who: Ref,
    #[case] want: Vec<Began>,
) -> Result<(), ScriptError> {
    // The caster takes one extra turn after their turn of round 2.
    let walk = time_spell("Hourglass", who, 1, false);
    let turn = 3 + u32::try_from(caster).unwrap();
    let scenario = table(2, None, [2, 3])
        .at(turn, caster, Step::Main1)
        .hand(caster, [walk]);
    let (began, _) = play(scenario, Some("Hourglass"), 9)?;
    assert_eq!(began, want);
    Ok(())
}

#[rstest]
#[case::boss_life_does_not_start_it(1)]
#[case::round_one_ends_without_a_phase(20)]
fn no_phase_before_its_round(#[case] boss_life: i32) -> Result<(), ScriptError> {
    let (began, _) = play(table(2, None, [8, 14]).life(BOSS, boss_life), None, 4)?;
    assert!(began.is_empty(), "{began:?}");
    Ok(())
}
