//! Goal: a player under "While voting, you may vote an additional time." casts a second ballot right after the first, which they may leave blank, at tables of two, three and four seats; the votes are counted with the extra ballot (a tie goes to the other branch), a player who has left does not vote, and the game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A free sorcery: everyone votes for gift or life; if gift gets more votes, its caster draws a card, otherwise gains
/// one life.
fn w17c_ballot() -> Arc<CardDefinition> {
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

/// An artifact with "While voting, you may vote an additional time."
fn w17c_extra_voice() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(r#"{"Rule":{"rule":{"VoteAgain":"You"}}}"#).unwrap();
    Arc::new(
        CardDefinition::new("Extra Voice")
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rule),
    )
}

// `votes` are the answers in the order asked (0 gift, 1 life, 2 the blank ballot); seat `left` (if any) concedes
/// first; `drew` is whether gift won.
#[rstest]
#[case::two_seats_two_gifts_beat_a_life(2, None, &[0, 0, 1], true)]
#[case::two_seats_a_blank_ballot_ties(2, None, &[0, 2, 1], false)]
#[case::three_seats_tied_two_to_two(3, None, &[0, 0, 1, 1], false)]
#[case::four_seats_with_one_gone(4, Some(2), &[0, 1, 1, 0], false)]
#[case::four_seats_gift_wins(4, None, &[0, 0, 1, 1, 0], true)]
fn w17c_the_extra_ballot_counts(
    #[case] seats: usize,
    #[case] left: Option<usize>,
    #[case] votes: &[usize],
    #[case] drew: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .seed(17)
        .hand(0, [w17c_ballot()])
        .battlefield(0, [ready(w17c_extra_voice())]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..6).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    if let Some(seat) = left {
        driver.concede(pid(seat))?;
    }
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    let start = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Ballot")?;
    let mut asked = Vec::new();
    let mut answers = votes.iter();
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::ChooseOption(options) => {
                let blank = options.options.len() == 3;
                if blank {
                    assert_eq!(
                        options.options[2],
                        ChoiceOption::Label("No additional vote".to_owned())
                    );
                }
                asked.push((decision.player.index(), blank));
                let vote = *answers.next().expect("a ballot for each answer");
                driver.answer(Answer::ChooseOption(OptionChoice(vote)))?;
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    let want: Vec<(usize, bool)> = std::iter::once((0, false))
        .chain(std::iter::once((0, true)))
        .chain(
            (1..seats)
                .filter(|seat| Some(*seat) != left)
                .map(|seat| (seat, false)),
        )
        .collect();
    assert_eq!(asked, want, "the caster twice first, then turn order");
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    assert_eq!(hand, usize::from(drew));
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, start + i32::from(!drew));
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
