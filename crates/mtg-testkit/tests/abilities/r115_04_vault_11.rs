//! Goal: a secret vote for up to one creature (`Vote.up_to`, CR 701.38) plays Vault 11: Voter's Dilemma's second chapter at tables of two to four seats: every ballot may be left blank, each player draws a card when no creature got a vote, otherwise each creature with the most votes or tied for most is destroyed and no one draws, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 has Vault 11 at one lore counter and "Bear 0"; every other seat has its own "Bear N"; each library holds
/// plains to draw.
fn r115_table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats).at(2, 0, Step::Upkeep).battlefield(
        0,
        [
            with_counters(
                ready(card("Vault 11: Voter's Dilemma").unwrap()),
                CounterKind::lore(),
                1,
            ),
            ready(creature("Bear 0", 1, &[Color::Green], 2, 2)),
        ],
    );
    for seat in 0..seats {
        if seat > 0 {
            scenario = scenario.battlefield(
                seat,
                [ready(creature(
                    &format!("Bear {seat}"),
                    1,
                    &[Color::Green],
                    2,
                    2,
                ))],
            );
        }
        scenario = scenario.library(seat, (0..5).map(|_| plains()).collect::<Vec<_>>());
    }
    scenario
}

/// Plays to the end of chapter II: seat `n` votes for "Bear {votes[n]}", or casts no vote for `None`.
fn r115_play(driver: &mut Driver, votes: &[Option<usize>]) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..300 {
        let decision = driver.decision()?.clone();
        if driver.state().turn().step() == Step::Main1
            && driver.state().zone(Zone::Stack).is_empty()
            && driver.state().pending_triggers().is_empty()
            && asked > 0
        {
            return Ok(asked);
        }
        match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                let seat = (0..votes.len())
                    .find(|seat| pid(*seat) == decision.player)
                    .unwrap();
                let wanted = votes[seat].map(|bear| driver.id_of(&format!("Bear {bear}")).unwrap());
                let at = options
                    .options
                    .iter()
                    .position(|option| match (option, wanted) {
                        (ChoiceOption::SecretObject(id), Some(want)) => *id == want,
                        (ChoiceOption::Secret(word), None) => word == "No vote",
                        _ => false,
                    })
                    .expect("the ballot offers the creature and a blank");
                asked += 1;
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("chapter II did not resolve".to_owned()))
}

#[rstest]
#[case::nobody_votes(&[None, None, None, None], &[])]
#[case::one_leader(&[Some(1), Some(1), Some(0), Some(1)], &[1])]
#[case::a_lone_vote(&[None, Some(0), None, None], &[0])]
#[case::tied(&[Some(0), Some(1), None, None], &[0, 1])]
fn r115_vault_destroys_the_most_voted_or_draws(
    #[values(2, 3, 4)] seats: usize,
    #[case] votes: &[Option<usize>],
    #[case] destroyed: &[usize],
) -> Result<(), ScriptError> {
    let votes = &votes[..seats];
    let destroyed: Vec<usize> = destroyed
        .iter()
        .copied()
        .filter(|bear| *bear < seats)
        .collect();
    // Each seat's vote counts only where the seat plays; a tie of seats left out of a smaller table may become a lead.
    let mut counts = vec![0usize; seats];
    for bear in votes.iter().flatten() {
        counts[*bear] += 1;
    }
    let most = counts.iter().copied().max().unwrap_or(0);
    let leaders: Vec<usize> = (0..seats)
        .filter(|bear| most > 0 && counts[*bear] == most)
        .collect();
    assert_eq!(
        leaders, destroyed,
        "the case names the leaders of this table"
    );
    let setup = r115_table(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(r115_table(seats))?;
    let hands: Vec<usize> = (0..seats)
        .map(|seat| driver.state().zone(Zone::Hand(pid(seat))).len())
        .collect();
    assert_eq!(
        r115_play(&mut driver, votes)?,
        seats,
        "every player casts one ballot"
    );
    for (seat, before) in hands.iter().enumerate() {
        let gone = matches!(
            driver
                .state()
                .objects()
                .find(|object| object.card().name == format!("Bear {seat}"))
                .map(mtg_engine::object::GameObject::zone),
            Some(Zone::Graveyard(_))
        );
        assert_eq!(gone, destroyed.contains(&seat), "Bear {seat}");
        // The active player drew for the turn as well.
        let drawn = usize::from(seat == 0) + usize::from(destroyed.is_empty());
        assert_eq!(
            driver.state().zone(Zone::Hand(pid(seat))).len(),
            before + drawn,
            "seat {seat}"
        );
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
