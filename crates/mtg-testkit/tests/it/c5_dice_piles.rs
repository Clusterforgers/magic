//! Goal: Fact or Fiction plays as printed at tables of two, three and four seats: its caster chooses the opponent, that opponent splits the five revealed cards into two piles (one may be empty), the caster puts the pile they pick into their hand and the other into their graveyard, and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::c4_control::replays;
use crate::common::{names, pid};
use mtg_engine::choose::{CardsChoice, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const TOP: [&str; 5] = ["Ant", "Bat", "Cat", "Dog", "Elk"];

/// Casts Fact or Fiction and answers its questions: the last opponent separates, `pile` is the first pile and `take`
/// is whether the caster takes it. Returns the seat that split the cards.
fn play(
    driver: &mut Driver,
    seats: usize,
    pile: &[&str],
    take: bool,
) -> Result<usize, ScriptError> {
    driver.cast("Fact or Fiction")?;
    let mut splitter = None;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseOption(options) => {
                assert_eq!(decision.player, pid(0), "the caster chooses the opponent");
                assert_eq!(options.options.len(), seats - 1);
                driver.answer(Answer::ChooseOption(OptionChoice(seats - 2)))?;
            }
            DecisionKind::ChooseCards(options) => {
                splitter = Some(decision.player.index());
                assert_eq!(options.from.len(), 5);
                let wanted: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| {
                        pile.contains(&driver.state().object(*id).unwrap().card().name.as_str())
                    })
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(wanted)))?;
            }
            DecisionKind::YesNo(_) => {
                assert_eq!(decision.player, pid(0), "the caster picks the pile");
                driver.answer(Answer::YesNo(YesNoChoice(take)))?;
            }
            _ if driver.state().stack().is_empty() && driver.state().resolving().is_none() => {
                return splitter.ok_or_else(|| ScriptError::Stuck("no split".into()));
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("Fact or Fiction".into()))
}

fn sorted(mut list: Vec<String>) -> Vec<String> {
    list.sort();
    list
}

#[rstest]
#[case::takes_two(&["Ant", "Bat"], true)]
#[case::takes_the_other_three(&["Ant", "Bat"], false)]
#[case::takes_all_five(&TOP, true)]
#[case::takes_the_empty_pile(&TOP, false)]
#[case::takes_five_from_an_empty_first_pile(&[], false)]
fn fact_or_fiction_puts_the_chosen_pile_into_the_hand(
    #[case] pile: &[&str],
    #[case] take: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut library: Vec<_> = TOP.iter().map(|name| free_creature(name, 1, 1)).collect();
    library.push(free_creature("Yak", 1, 1));
    let driver_scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Fact or Fiction").unwrap()])
        .library(0, library)
        .battlefield(0, (0..4).map(|_| ready(island())));
    let mut driver = Driver::from_scenario(driver_scenario)?;
    let splitter = play(&mut driver, seats, pile, take)?;
    assert_eq!(splitter, seats - 1, "the chosen opponent separates");
    let (first, second): (Vec<&str>, Vec<&str>) = TOP.iter().partition(|name| pile.contains(name));
    let (kept, lost) = if take {
        (first, second)
    } else {
        (second, first)
    };
    let as_strings = |list: Vec<&str>| sorted(list.into_iter().map(str::to_owned).collect());
    assert_eq!(
        sorted(names(driver.state(), Zone::Hand(pid(0)))),
        as_strings(kept)
    );
    let mut yard = lost;
    yard.push("Fact or Fiction");
    assert_eq!(
        sorted(names(driver.state(), Zone::Graveyard(pid(0)))),
        as_strings(yard)
    );
    assert_eq!(names(driver.state(), Zone::Library(pid(0))), ["Yak"]);
    replays(&driver);
    Ok(())
}
