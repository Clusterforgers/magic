//! Goal: "You choose how each player votes this turn." (Illusion of Choice, CR 701.38) has its controller cast every
//! ballot of a later vote that turn at two to four seats, each counted as its voter's: the ballots are asked of the
//! chooser in turn order from the vote's controller, the tally follows the chooser's answers, `VotesFinished` names each
//! voter, a chooser who is not the vote's controller also answers every ballot, and the game replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::effects::Duration;
use mtg_engine::rules::Rule;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{cost, forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A free sorcery: everyone votes for gift or life; if gift gets more votes, its caster draws a card, otherwise gains
/// one life.
fn ballot() -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[{"Vote":{"options":{"Named":["gift","life"]}}},{"If":{"condition":{"Compare":{"left":{"Players":{"Bound":"gift"}},"op":"Gt","right":{"Players":{"Bound":"life"}}}},"then":[{"Draw":{}}],"otherwise":[{"GainLife":{"amount":1}}]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Ballot")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(spell),
    )
}

/// Resolves the stack, answering each ballot with `vote` and noting who was asked.
fn resolve(driver: &mut Driver, vote: usize, asked: &mut Vec<usize>) -> Result<(), ScriptError> {
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::ChooseOption(_) => {
                asked.push(decision.player.index());
                driver.answer(Answer::ChooseOption(OptionChoice(vote)))?;
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    panic!("the stack resolves");
}

#[rstest]
#[case::illusion_cast_gift(None, 0, true)]
#[case::illusion_cast_life(None, 1, false)]
#[case::another_player_chooses(Some(1), 0, true)]
fn r6400_one_player_casts_every_ballot(
    #[values(2, 3, 4)] seats: usize,
    #[case] rule_for: Option<usize>,
    #[case] vote: usize,
    #[case] drew: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .seed(64)
        .hand(0, [ballot(), card("Illusion of Choice").unwrap()])
        .battlefield(0, [ready(island())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..6).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    let chooser = if let Some(seat) = rule_for {
        driver.add_rule(Rule::R6400ChoosesVotes(pid(seat)), Duration::UntilEndOfTurn)?;
        seat
    } else {
        driver.cast("Illusion of Choice")?;
        resolve(&mut driver, vote, &mut Vec::new())?;
        0
    };
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    let start = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Ballot")?;
    let mut asked = Vec::new();
    resolve(&mut driver, vote, &mut asked)?;
    assert_eq!(
        asked,
        vec![chooser; seats],
        "the chooser answers each ballot"
    );
    let ballots = driver
        .engine()
        .events()
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::VotesFinished { ballots } => Some(ballots.clone()),
            _ => None,
        })
        .unwrap();
    let voters: Vec<usize> = ballots.iter().map(|(voter, _)| voter.index()).collect();
    assert_eq!(
        voters,
        (0..seats).collect::<Vec<_>>(),
        "each ballot is its voter's"
    );
    // The ballot left the hand; a gift draws a card in its place.
    let now = driver.state().zone(Zone::Hand(pid(0))).len();
    assert_eq!(now + 1, hand + usize::from(drew));
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, start + i32::from(!drew));
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
