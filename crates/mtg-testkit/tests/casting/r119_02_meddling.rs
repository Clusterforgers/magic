//! Goal: Minamo's Meddling counters a spell and its controller discards each card with the name of a card spliced onto
//! it (the spliced card and its namesakes) and keeps the rest of the hand; nothing is discarded for a spell nothing was
//! spliced onto; at two to four seats, and every game replays from its log.

#![allow(clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::names;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::{Keyword, SpliceOnto};
use mtg_core::ids::PlayerId;
use mtg_engine::choose::CostsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, replay};
use mtg_testkit::cards::{burn, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn arcane(name: &str) -> CardDefinition {
    (*burn(name, 2)).clone().with_subtype("Arcane")
}

/// An Arcane instant that deals 2 damage to any target, with splice onto Arcane for {1}{R}.
fn ray() -> Arc<CardDefinition> {
    Arc::new(
        arcane("Glacial Ray").with_rules(Ability::Keyword(Keyword::Splice {
            onto: SpliceOnto::Arcane,
            cost: Cost::mana("{1}{R}".parse().expect("a cost")),
        })),
    )
}

#[rstest]
fn meddling_discards_the_names_spliced(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] spliced: bool,
) {
    let setup = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(
            0,
            [
                Arc::new(arcane("Arcane Bolt")),
                ray(),
                ray(),
                Arc::new(arcane("Keeper")),
            ],
        )
        .battlefield(0, (0..3).map(|_| ready(mountain())))
        .hand(1, [card("Minamo's Meddling").expect("the corpus card")])
        .battlefield(1, (0..4).map(|_| ready(island())))
        .setup()
        .expect("the table is valid");
    let mut driver =
        Driver::new(Engine::new(setup.clone()).expect("the game starts")).expect("a driver");
    driver.begin_cast("Arcane Bolt").expect("the cast begins");
    assert!(matches!(
        driver.decision().expect("a decision").kind,
        DecisionKind::ChooseCosts(_)
    ));
    let picked = if spliced { vec![0] } else { Vec::new() };
    driver
        .answer(Answer::ChooseCosts(CostsChoice::Costs(picked)))
        .expect("the splice choice");
    driver
        .choose_targets(&[driver.target_player(1)])
        .expect("the bolt's target");
    if spliced {
        driver
            .choose_targets(&[driver.target_player(1)])
            .expect("the ray's target");
    }
    driver
        .answer(Answer::PayMana(PayChoice::Auto))
        .expect("the payment");
    let bolt = driver
        .state()
        .zone(Zone::Stack)
        .first()
        .and_then(|id| driver.state().object(*id))
        .map(|spell| mtg_engine::choose::Target::Object(spell.reference()))
        .expect("the bolt on the stack");
    driver.pass().expect("player 0 passes");
    driver
        .cast_at("Minamo's Meddling", &[bolt])
        .expect("the counter is cast");
    for _ in 0..20 {
        if driver.state().zone(Zone::Stack).is_empty() {
            break;
        }
        driver.pass().expect("a pass");
    }
    let kept = names(driver.state(), Zone::Hand(PlayerId(0)));
    if spliced {
        assert_eq!(kept, ["Keeper"]);
    } else {
        assert_eq!(kept.len(), 3);
    }
    assert!(
        names(driver.state(), Zone::Graveyard(PlayerId(0))).contains(&"Arcane Bolt".to_owned())
    );
    let replayed = replay(&setup, driver.engine().events()).expect("the log replays");
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}
