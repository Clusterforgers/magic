//! Goal: the search of Invasion of Theros shows the searching seat the cards it may take and every other seat nothing of the library it searched: no batch or snapshot of theirs names a card left behind, while the card found reaches the searcher's hand.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_protocol::StateView;
use mtg_testkit::cards::plains;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::table::Table;
use crate::hidden_info::{assert_clean, leaks};

const CARD: &str = "Invasion of Theros // Ephara, Ever-Sheltering";
const LIBRARY: [&str; 4] = [
    "Grizzly Bears",
    "All That Glitters",
    "Heliod, God of the Sun",
    "Hercules, Olympian Hero",
];

/// Seat 0 holds the Siege over three Plains in its first main phase and owns `LIBRARY`.
fn table(seats: usize) -> Scenario {
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, [CARD])
        .library_named(0, LIBRARY)
        .battlefield(0, (0..3).map(|_| ready(plains())))
}

fn name_of(driver: &Driver, id: ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

#[rstest]
fn the_other_seats_never_see_the_cards_the_search_leaves_behind(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        "All That Glitters",
        "Heliod, God of the Sun",
        "Hercules, Olympian Hero"
    )]
    found: &'static str,
) {
    let mut table = Table::new(table(seats));
    let left: BTreeMap<ObjectId, String> = table
        .driver
        .state()
        .zone(Zone::Library(PlayerId::from_index(0)))
        .iter()
        .map(|id| (*id, name_of(&table.driver, *id)))
        .filter(|(_, name)| name != found)
        .collect();
    assert_eq!(left.len(), 3, "the Bears and the two cards not taken");
    let mut offered = Vec::new();
    let mut run = table.run(|driver| driver.cast("Invasion of Theros"));
    for _ in 0..200 {
        for batch in &run.batches[1..] {
            assert_clean(batch, &left);
        }
        for seat in 1..seats {
            let view = StateView::for_player(table.driver.state(), PlayerId::from_index(seat));
            let found_leaks = leaks(&serde_json::to_value(&view).unwrap(), &left);
            assert!(
                found_leaks.is_empty(),
                "seat {seat}'s snapshot leaks {found_leaks:?}"
            );
        }
        let decision = table.driver.decision().unwrap().clone();
        if table.driver.state().stack().is_empty()
            && matches!(decision.kind, DecisionKind::Priority(_))
        {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| matches!(option, ChoiceOption::Player(_)));
                Answer::ChooseOption(OptionChoice(at.unwrap_or(0)))
            }
            DecisionKind::ChooseCards(options) => {
                assert_eq!(decision.player.index(), 0, "the searcher chooses");
                offered = options
                    .from
                    .iter()
                    .map(|id| name_of(&table.driver, *id))
                    .collect();
                Answer::ChooseCards(CardsChoice(
                    options
                        .from
                        .iter()
                        .copied()
                        .filter(|id| name_of(&table.driver, *id) == found)
                        .collect(),
                ))
            }
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            _ => Driver::passive_answer(&decision),
        };
        run = table.run(|driver| driver.answer(answer));
    }
    offered.sort();
    assert_eq!(
        offered,
        [
            "All That Glitters",
            "Heliod, God of the Sun",
            "Hercules, Olympian Hero"
        ],
        "the searcher is offered the Aura, the God and the Demigod"
    );
    let hand = table
        .driver
        .state()
        .zone(Zone::Hand(PlayerId::from_index(0)));
    assert!(
        hand.iter().any(|id| name_of(&table.driver, *id) == found),
        "{found} is in the searcher's hand"
    );
    let mine = serde_json::to_string(&table.views[0]).unwrap();
    assert!(mine.contains(found), "the searcher's view names {found}");
}
