//! Goal: a chapter that names a card and searches the target opponent's graveyard, hand and library for up to four
//! cards with that name (The Rise of Sozin), at 2 to 4 seats: at most four go to exile from all three zones together,
//! a card of another name stays, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, NameChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, pid};

const SAGA: &str = "The Rise of Sozin // Fire Lord Sozin";

#[rstest]
#[case::five_copies_four_go(5, 4)]
#[case::three_copies_all_go(3, 3)]
fn the_rise_of_sozin_exiles_up_to_four_named_cards(
    #[values(2, 3, 4)] seats: usize,
    #[case] copies: usize,
    #[case] exiled: usize,
) -> Result<(), ScriptError> {
    let bears = || card("Grizzly Bears").unwrap();
    let scenario = || {
        Scenario::new(seats)
            .at(1, 0, Step::Upkeep)
            .battlefield(
                0,
                [with_counters(
                    ready(card(SAGA).unwrap()),
                    CounterKind::lore(),
                    1,
                )],
            )
            .graveyard(1, (0..copies.min(2)).map(|_| bears()).collect::<Vec<_>>())
            .hand(
                1,
                (2..copies.min(3))
                    .map(|_| bears())
                    .chain([forest()])
                    .collect::<Vec<_>>(),
            )
            .library(
                1,
                (3..copies)
                    .map(|_| bears())
                    .chain([forest(), forest()])
                    .collect::<Vec<_>>(),
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.idle_until(1, Step::Main1)?;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_player(1);
                driver.choose_targets(&[target])?;
            }
            DecisionKind::ChooseName(_) => {
                driver.answer(Answer::ChooseName(NameChoice("Grizzly Bears".to_owned())))?;
            }
            DecisionKind::ChooseCards(options) if decision.player == pid(0) => {
                let max = usize::try_from(options.max).unwrap();
                driver.answer(Answer::ChooseCards(CardsChoice(
                    options.from.iter().copied().take(max).collect(),
                )))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let gone = names(driver.state(), Zone::Exile)
        .iter()
        .filter(|name| *name == "Grizzly Bears")
        .count();
    assert_eq!(gone, exiled);
    let left: usize = [
        Zone::Graveyard(pid(1)),
        Zone::Hand(pid(1)),
        Zone::Library(pid(1)),
    ]
    .into_iter()
    .map(|zone| {
        names(driver.state(), zone)
            .iter()
            .filter(|n| *n == "Grizzly Bears")
            .count()
    })
    .sum();
    assert_eq!(left, copies - exiled);
    replays(&driver, &setup);
    Ok(())
}
