//! Goal: under Carth the Lion ("Planeswalkers' loyalty abilities you activate cost an additional [+1] to activate.") a
//! [+1] adds two loyalty counters and a [−3] removes two, so a planeswalker with two counters may activate it, while
//! without Carth it may not, at tables of two to four seats; the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::island;
use mtg_testkit::{Driver, Scenario, corpus, ready, with_counters};
use rstest::rstest;

/// A planeswalker with "[+1]: You gain 1 life." and "[−3]: You gain 3 life."
fn walker() -> Arc<CardDefinition> {
    let rule = |json: &str| serde_json::from_str(json).unwrap();
    Arc::new(
        CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_loyalty(2)
            .with_rules(rule(
                r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":1}}]}}"#,
            ))
            .with_rules(rule(
                r#"{"Loyalty":{"cost":{"Minus":3},"effects":[{"GainLife":{"amount":3}}]}}"#,
            )),
    )
}

fn loyalty(driver: &Driver) -> Option<u32> {
    let id = driver.id_of("Walker").ok()?;
    let permanent = driver.state().object(id)?.permanent()?;
    Some(permanent.counters_of(&CounterKind::Loyalty))
}

#[rstest]
#[case::plus_with_carth(true, 0, true, Some(4))]
#[case::plus_alone(false, 0, true, Some(3))]
#[case::minus_with_carth(true, 1, true, None)]
#[case::minus_alone(false, 1, false, Some(2))]
fn carth_adds_one_loyalty_counter_to_each_cost(
    #[values(2, 3, 4)] seats: usize,
    #[case] carth: bool,
    #[case] ability: usize,
    #[case] offered: bool,
    #[case] after: Option<u32>,
) {
    let mut own = vec![with_counters(ready(walker()), CounterKind::Loyalty, 2)];
    if carth {
        own.push(ready(corpus::card("Carth the Lion").unwrap()));
    }
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, own);
    let scenario = (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..10).map(|_| island()))
    });
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let abilities = driver.offered_abilities("Walker").unwrap();
    assert_eq!(abilities.contains(&ability), offered, "{abilities:?}");
    if offered {
        driver.begin_ability("Walker", ability).unwrap();
        for _ in 0..12 {
            if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
                break;
            }
            let decision = driver.decision().unwrap().clone();
            driver.answer(Driver::passive_answer(&decision)).unwrap();
        }
    }
    // With no counter left the planeswalker is put into its owner's graveyard (CR 704.5i).
    assert_eq!(loyalty(&driver), after);
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}
