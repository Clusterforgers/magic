//! Goal: translated votes and clashes play out in a game: the winner of a vote runs and a tie runs its named choice, each vote counts once for a counted choice, and a clash runs its win part only when the clasher's card is greater.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_testkit::cards::{bear, forest, island};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::r3p3_runs::translated;

/// Plays the spell with the votes answered in order (the option number of each ballot), then passes until it is done.
fn play(
    script: &str,
    votes: &[usize],
    library: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
) -> Driver {
    let card = translated(script, "Poll", "Sorcery", "{G}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(forest())])
        .library(0, library.clone())
        .library(1, library);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Poll").unwrap();
    let mut ballots = votes.iter();
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return driver,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::ChooseOption(_) => {
                Answer::ChooseOption(OptionChoice(*ballots.next().expect("a ballot")))
            }
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game does not settle");
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(mtg_engine::zones::Zone::Hand(PlayerId::from_index(seat)))
        .len()
}

const TIED: &str = "Name:Poll\nManaCost:G\nTypes:Sorcery\nA:SP$ Vote | Defined$ Player | Choices$ DBGain,DBDraw | VoteTiedAbility$ DBDraw\nSVar:DBGain:DB$ GainLife | LifeAmount$ 5 | SpellDescription$ Gain\nSVar:DBDraw:DB$ Draw | NumCards$ 2 | SpellDescription$ Cards\n";
const COUNTED: &str = "Name:Poll\nManaCost:G\nTypes:Sorcery\nA:SP$ Vote | Defined$ Player | Choices$ DBGain,DBDraw | StoreVoteNum$ True\nSVar:DBGain:DB$ GainLife | LifeAmount$ VoteNum | SpellDescription$ Gain\nSVar:DBDraw:DB$ Draw | NumCards$ VoteNum | SpellDescription$ Cards\n";
const EACH: &str = "Name:Poll\nManaCost:G\nTypes:Sorcery\nA:SP$ Vote | Defined$ Player | Choices$ DBGain,DBDraw | EachVote$ True\nSVar:DBGain:DB$ GainLife | LifeAmount$ 2 | SpellDescription$ Gain\nSVar:DBDraw:DB$ Draw | SpellDescription$ Cards\n";

fn library() -> Vec<std::sync::Arc<mtg_cards::CardDefinition>> {
    (0..12).map(|_| forest()).collect()
}

// (votes of me then the foe, life gained, cards drawn)
#[rstest]
#[case::the_first_choice_wins_both(&[0, 0], 5, 0)]
#[case::a_tie_runs_the_named_choice(&[0, 1], 0, 2)]
#[case::the_named_choice_wins(&[1, 1], 0, 2)]
fn a_vote_with_a_tied_choice(#[case] votes: &[usize], #[case] gained: i32, #[case] drawn: usize) {
    let driver = play(TIED, votes, library());
    assert_eq!(life(&driver, 0), 20 + gained);
    assert_eq!(hand(&driver, 0), drawn);
}

#[rstest]
#[case::two_for_gain(&[0, 0], 2, 0)]
#[case::split(&[0, 1], 1, 1)]
#[case::two_for_cards(&[1, 1], 0, 2)]
fn a_counted_choice_runs_once_for_the_votes_it_got(
    #[case] votes: &[usize],
    #[case] gained: i32,
    #[case] drawn: usize,
) {
    let driver = play(COUNTED, votes, library());
    assert_eq!(life(&driver, 0), 20 + gained);
    assert_eq!(hand(&driver, 0), drawn);
}

#[rstest]
#[case::two_for_gain(&[0, 0], 4, 0)]
#[case::split(&[0, 1], 2, 1)]
#[case::two_for_cards(&[1, 1], 0, 2)]
fn each_vote_runs_its_choice(#[case] votes: &[usize], #[case] gained: i32, #[case] drawn: usize) {
    let driver = play(EACH, votes, library());
    assert_eq!(life(&driver, 0), 20 + gained);
    assert_eq!(hand(&driver, 0), drawn);
}

const CLASH: &str = "Name:Poll\nManaCost:G\nTypes:Sorcery\nA:SP$ Clash | WinSubAbility$ DBGain | OtherwiseSubAbility$ DBLose\nSVar:DBGain:DB$ GainLife | LifeAmount$ 3\nSVar:DBLose:DB$ LoseLife | LifeAmount$ 2\n";

#[rstest]
#[case::greater_card_wins(vec![bear()], vec![forest()], 23)]
#[case::lesser_card_loses(vec![forest()], vec![bear()], 18)]
#[case::a_tie_is_not_a_win(vec![forest()], vec![island()], 18)]
fn a_clash_runs_the_part_the_cards_decide(
    #[case] mine: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    #[case] theirs: Vec<std::sync::Arc<mtg_cards::CardDefinition>>,
    #[case] life_after: i32,
) {
    let card = translated(CLASH, "Poll", "Sorcery", "{G}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(forest())])
        .library(0, mine)
        .library(1, theirs);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Poll").unwrap();
    crate::r3p3_runs::settle(&mut driver);
    assert_eq!(life(&driver, 0), life_after);
}

const LURE: &str = "Name:Poll\nManaCost:G\nTypes:Creature Bear\nPT:1/1\nA:AB$ MustBlock | Cost$ G | ValidTgts$ Creature | TgtPrompt$ Select target creature to block this creature\n";

#[test]
fn a_translated_lure_ability_is_offered_and_resolves_on_a_target() {
    let card = translated(LURE, "Poll", "Creature — Bear", "{G}");
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(card), ready(forest())])
        .battlefield(1, [ready(bear())]);
    let driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(driver.offered_abilities("Poll").unwrap().len(), 1);
}
