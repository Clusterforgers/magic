//! Goal: Done for the Day's end step trigger runs only while its controller has an Employee, a Performer or a Robot,
//! gives the chosen {TK} or Treasure, and offers a sticker only with all three, at two to four seats; the log replays
//! to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Sticker};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::Subtype;
use mtg_engine::choose::{CardsChoice, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{free_creature, un34_sheet};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A 1/1 creature of one creature type, named after it.
fn r5519_worker(subtype: &str) -> Arc<CardDefinition> {
    let mut made = (*free_creature(subtype, 1, 1)).clone();
    made.subtypes = vec![Subtype(subtype.into())];
    Arc::new(made)
}

#[rstest]
fn r5519_done_for_the_day_pays_its_staff(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] staff: usize,
    #[values(0, 1)] pick: usize,
) {
    let mut mine = vec![ready(card("Done for the Day").unwrap())];
    mine.extend(
        ["Employee", "Performer", "Robot"]
            .iter()
            .take(staff)
            .map(|subtype| ready(r5519_worker(subtype))),
    );
    let sheet = un34_sheet(
        "Sheet",
        vec![Sticker::Ability {
            ability: Box::new(Ability::Keyword(Keyword::Flying)),
            tickets: 0,
        }],
    );
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main2)
        .stickers(0, [sheet])
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.idle_until(3, Step::End).unwrap();
    for _ in 0..40 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                break;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            DecisionKind::ChooseOption(_) => {
                driver
                    .answer(Answer::ChooseOption(OptionChoice(pick)))
                    .unwrap();
            }
            DecisionKind::ChooseCards(options) => {
                let first = options.from.first().copied().into_iter().collect();
                driver
                    .answer(Answer::ChooseCards(CardsChoice(first)))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    let paid = staff > 0;
    let tickets = driver
        .state()
        .player(pid(0))
        .unwrap()
        .counters_of(&CounterKind::Other("ticket".to_owned()));
    let treasures = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Treasure")
        .count();
    assert_eq!(tickets, u32::from(paid && pick == 0), "tickets");
    assert_eq!(treasures, usize::from(paid && pick == 1), "treasures");
    let stickers = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::StickerPut { .. }))
        .count();
    assert_eq!(stickers, usize::from(staff == 3), "stickers");
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
