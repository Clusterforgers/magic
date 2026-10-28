//! Goal: the Will of the Planeswalkers vote of Path of the Enigma plays at two, three and four seats: the spell's own effect happens, each seat votes in turn order from the caster, the vote is told as it is cast, planeswalking and chaos change nothing in a game without a planar deck, and the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{Choice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

#[rstest]
#[case::two_seats_planeswalk(2, &[0, 0])]
#[case::two_seats_tied(2, &[0, 1])]
#[case::three_seats_chaos(3, &[1, 1, 0])]
#[case::four_seats_tied(4, &[0, 1, 0, 1])]
#[case::four_seats_planeswalk(4, &[0, 0, 0, 1])]
fn c10_vp_path_of_the_enigma_draws_and_holds_the_planar_vote(
    #[case] seats: usize,
    #[case] votes: &[usize],
) -> Result<(), ScriptError> {
    let lands: Vec<_> = (0..5).map(|_| ready(island())).collect();
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Path of the Enigma"])
        .battlefield(0, lands);
    for seat in 0..seats {
        scenario = scenario.library(
            seat,
            (0..6).map(|i| free_creature(&format!("Card {seat}.{i}"), 1, 1)),
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_player(seats - 1);
    driver.cast_at("Path of the Enigma", &[target])?;
    let mut asked = Vec::new();
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseOption(options) => {
                assert_eq!(options.options.len(), 2, "planeswalk or chaos");
                asked.push(decision.player);
                driver.answer(Answer::ChooseOption(OptionChoice(
                    votes[decision.player.index()],
                )))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let order: Vec<_> = (0..seats).map(pid).collect();
    assert_eq!(asked, order, "the caster first, then turn order");
    let told: Vec<Choice> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::ChoiceMade { choice, .. } => Some(choice.clone()),
            _ => None,
        })
        .collect();
    let want: Vec<Choice> = votes.iter().map(|vote| Choice::Option(*vote)).collect();
    assert_eq!(told, want, "an open vote is told as it is cast");
    let hand = driver.state().zone(Zone::Hand(pid(seats - 1))).len();
    assert_eq!(hand, 4, "the target draws four cards");
    assert!(
        driver
            .state()
            .zone(Zone::Graveyard(pid(0)))
            .iter()
            .any(|id| driver.state().object(*id).unwrap().card().name == "Path of the Enigma"),
        "the spell resolves whole"
    );
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
