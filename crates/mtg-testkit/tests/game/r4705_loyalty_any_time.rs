//! Goal: the emblem of Teferi's Talent ("You may activate loyalty abilities of planeswalkers you control on any player's
//! turn any time you could cast an instant.") lets its owner activate a loyalty ability on another player's turn, and
//! without it no loyalty ability is offered there, at tables of two to four seats; the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::island;
use mtg_testkit::{Driver, Scenario, attached_to, corpus, ready, with_counters};
use rstest::rstest;

const TALENT: &str = "Teferi's Talent";

/// A planeswalker with "[+1]: You gain 1 life."
fn walker() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_loyalty(13)
            .with_rules(
                serde_json::from_str(
                    r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":1}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

#[rstest]
fn the_emblem_opens_loyalty_abilities_on_other_turns(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] emblem: bool,
) {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                with_counters(ready(walker()), CounterKind::Loyalty, 13),
                attached_to(ready(corpus::card(TALENT).unwrap()), 0, 0),
            ],
        );
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..10).map(|_| island()))
    });
    let mut driver = Driver::from_scenario(scenario).unwrap();
    if emblem {
        let walker = driver.id_of("Walker").unwrap();
        let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
            panic!("seat 0 holds priority");
        };
        let granted = *options
            .abilities
            .iter()
            .find(|ability| ability.object == walker && !ability.source.is_printed())
            .expect("the granted −12");
        driver
            .answer(Answer::Priority(PriorityAction::Activate(granted)))
            .unwrap();
        for _ in 0..12 {
            if driver.state().stack().is_empty() {
                break;
            }
            let decision = driver.decision().unwrap().clone();
            driver.answer(Driver::passive_answer(&decision)).unwrap();
        }
    }
    // Seat 1's upkeep: the players before seat 0 pass, and seat 0 holds priority on another player's turn.
    driver.idle_until(2, Step::Upkeep).unwrap();
    assert_eq!(driver.decision().unwrap().player, PlayerId(1));
    while driver.decision().unwrap().player != PlayerId(0) {
        driver.pass().unwrap();
    }
    let offered = driver.offered_abilities("Walker").unwrap();
    assert_eq!(offered.contains(&0), emblem, "{offered:?}");
    if emblem {
        let life = driver.state().player(PlayerId(0)).unwrap().life();
        driver.begin_ability("Walker", 0).unwrap();
        for _ in 0..12 {
            if driver.state().stack().is_empty() {
                break;
            }
            let decision = driver.decision().unwrap().clone();
            driver.answer(Driver::passive_answer(&decision)).unwrap();
        }
        assert_eq!(driver.state().player(PlayerId(0)).unwrap().life(), life + 1);
    }
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}
