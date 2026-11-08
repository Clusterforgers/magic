//! Goal: drafts that exile a found card and use it afterwards play it through: the card an exile names stays reachable by the steps after it (CR 400.7), and a delayed sacrifice takes every token it was set for.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::builtin;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, island, mountain, swamp};
use mtg_testkit::{Driver, Scenario, ready};

use crate::drafts::items;

fn draft(slug: &str) -> Arc<CardDefinition> {
    let (all, problems) = items();
    assert!(problems.is_empty(), "{problems:?}");
    let item = all.iter().find(|item| item.slug == slug).unwrap();
    let pairs = [(&item.printed, &item.card)];
    let defined =
        mtg_cli::drafts::read_definitions(builtin::library().unwrap(), &mtg_engine::caps(), &pairs);
    Arc::new(defined.into_iter().next().unwrap().unwrap())
}

/// Answers every question yes (the first card, the first option) and passes until the stack is empty again.
fn settle(driver: &mut Driver) {
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::YesNo(_) => Answer::YesNo(mtg_engine::choose::YesNoChoice(true)),
            DecisionKind::ChooseCards(o) => Answer::ChooseCards(mtg_engine::choose::CardsChoice(
                o.from.iter().copied().take(o.max as usize).collect(),
            )),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game does not settle");
}

fn named(driver: &Driver, zone: Zone, name: &str) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| state.object(*id).is_some_and(|o| o.card().name == name))
        .collect()
}

#[test]
fn fevered_suspicion_casts_the_card_it_exiled() {
    let spell = draft("fevered_suspicion");
    let name = spell.name.clone();
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(
            0,
            (0..5)
                .map(|_| ready(swamp()))
                .chain((0..5).map(|_| ready(mountain()))),
        )
        .library(1, [forest(), island(), bear(), forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();

    driver.cast(&name).unwrap();
    settle(&mut driver);

    assert_eq!(
        named(&driver, Zone::Battlefield, "Grizzly Bears").len(),
        1,
        "the exiled Grizzly Bears was cast free"
    );
}

#[test]
fn hoarding_dragon_remembers_the_artifact_it_exiled() {
    let dragon = draft("hoarding_dragon");
    let name = dragon.name.clone();
    let rock = mtg_testkit::corpus::card("Sol Ring").unwrap();
    let scenario = Scenario::new(2)
        .hand(
            0,
            [dragon, mtg_testkit::corpus::card("Doom Blade").unwrap()],
        )
        .battlefield(
            0,
            (0..5)
                .map(|_| ready(mountain()))
                .chain((0..3).map(|_| ready(swamp()))),
        )
        .library(0, [forest(), rock, forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();

    driver.cast(&name).unwrap();
    settle(&mut driver);

    assert_eq!(named(&driver, Zone::Exile, "Sol Ring").len(), 1);

    driver
        .cast_at(
            "Doom Blade",
            &[driver.target_named("Hoarding Dragon").unwrap()],
        )
        .unwrap();
    settle(&mut driver);

    assert_eq!(
        named(
            &driver,
            Zone::Hand(mtg_core::ids::PlayerId::from_index(0)),
            "Sol Ring"
        )
        .len(),
        1,
        "the exiled artifact returns to hand"
    );
}
