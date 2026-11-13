//! Goal: "Ward—Get five poison counters." (The Serpent Society) gives the player whose ability targets it five poison
//! counters when they pay, and the ability resolves, or counters the ability when they decline; a player who reaches ten
//! poison counters this way loses the game; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, island};
use mtg_testkit::{Driver, Scenario, corpus, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

const SOCIETY: &str = "The Serpent Society";

/// "{T}: Tap target creature."
fn tapper() -> Arc<CardDefinition> {
    let rule = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#;
    Arc::new(
        CardDefinition::new("Tapper")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(rule).unwrap()),
    )
}

#[rstest]
fn the_serpent_society_ward_gives_poison_or_counters(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] pays: bool,
    #[values(0, 5)] poisoned: u32,
) {
    let mut scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(corpus::card(SOCIETY).unwrap())])
        .battlefield(1, [ready(tapper()), ready(creature("Bear", 0, &[], 3, 3))])
        .counters(1, CounterKind::Poison, poisoned);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Tapper").unwrap();
    let target = driver.target_named(SOCIETY).unwrap();
    driver.choose_targets(&[target]).unwrap();
    let mut asked = false;
    for _ in 0..40 {
        if (asked && driver.state().stack().is_empty()) || driver.decision().is_err() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) if decision.player == pid(1) => {
                asked = true;
                Answer::YesNo(YesNoChoice(pays))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    assert!(asked, "ward asks for its cost");
    let state = driver.state();
    let poison = state.player(pid(1)).unwrap().counters_of(&CounterKind::Poison);
    let expected = if pays { poisoned + 5 } else { poisoned };
    assert_eq!(poison, expected);
    let society = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(state, *id) == SOCIETY)
        .unwrap();
    let tapped = state.object(society).unwrap().permanent().unwrap().tapped();
    let lost = !state.is_alive(pid(1));
    assert_eq!(lost, expected >= 10, "ten poison counters lose the game");
    if !lost {
        assert_eq!(tapped, pays, "the ability resolves only when the cost is paid");
    }
}
