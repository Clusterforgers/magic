//! Goal: a spell whose required additional cost discards X cards keeps the hand hidden: X and the picks are asked of the casting seat alone and log nothing until the spell is paid for, the X cards discarded are named to every seat once they are in the graveyard, and no card that stays in a hand is named to a seat that may not see it, in the events, the table, the objects or the snapshot.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{CardsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventBatch, EventView, StateView, project_events};
use mtg_testkit::cards::{forest, island};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::Value;

use crate::hidden_info::{assert_clean, hidden_across};

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// A blue instant: "As an additional cost to cast this spell, discard X cards. Draw X cards."
fn pitcher() -> Arc<CardDefinition> {
    let additional = r#"{"AdditionalCost":{"cost":[{"Discard":{"count":"X"}}],"required":true}}"#;
    let spell = r#"{"Spell":{"effects":[{"Draw":{"count":"X"}}]}}"#;
    Arc::new(
        CardDefinition::new("Pitcher")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{U}").unwrap())
            .with_rules(serde_json::from_str(additional).unwrap())
            .with_rules(serde_json::from_str(spell).unwrap()),
    )
}

/// Seat 0 holds the Pitcher and three cards that stay hidden unless discarded; every other seat has a hand of its own.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 8])
        .hand(
            0,
            std::iter::once(pitcher()).chain(
                ["Counterspell", "Wrath of God", "Fireball"]
                    .map(|name| mtg_testkit::corpus::card(name).unwrap()),
            ),
        )
        .battlefield(0, [ready(island())]);
    let others = [
        ["Doom Blade", "Opt"],
        ["Giant Growth", "Brainstorm"],
        ["Lightning Bolt", "Ancestral Vision"],
    ];
    for other in 1..seats {
        scenario = scenario
            .library(other, vec![forest(); 8])
            .hand_named(other, others[other - 1]);
    }
    scenario
}

/// Whether the text of `value` holds `name` anywhere.
fn names(value: &Value, name: &str) -> bool {
    match value {
        Value::String(text) => text == name,
        Value::Array(items) => items.iter().any(|item| names(item, name)),
        Value::Object(map) => map.values().any(|item| names(item, name)),
        _ => false,
    }
}

/// Whether any part of the batch holds the card's name.
fn batch_names(batch: &EventBatch, name: &str) -> bool {
    [
        serde_json::to_value(&batch.events).unwrap(),
        serde_json::to_value(&batch.table).unwrap(),
        serde_json::to_value(&batch.objects).unwrap(),
        serde_json::to_value(&batch.gone).unwrap(),
    ]
    .iter()
    .any(|part| names(part, name))
}

fn card_in(state: &GameState, zone: Zone, name: &str) -> ObjectId {
    state
        .zone(zone)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == name)
        .unwrap()
}

#[rstest]
fn x_and_the_picks_are_asked_of_the_caster_alone_and_the_discarded_cards_are_named_to_every_seat_and_the_rest_of_the_hand_to_none_but_its_owner(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let before = driver.state().clone();
    let logged = driver.engine().events().len();
    driver.begin_cast("Pitcher").unwrap();
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, seat(0), "the casting seat is asked for X");
    let DecisionKind::ChooseNumber(number) = &decision.kind else {
        panic!("X is asked, not {}", decision.kind.name());
    };
    assert_eq!(
        number.max,
        Some(3),
        "the spell is not offered for its own cost"
    );
    assert_eq!(
        driver.engine().events().len(),
        logged,
        "X leaves no trace in the log"
    );
    driver
        .answer(Answer::ChooseNumber(NumberChoice::Number(2)))
        .unwrap();
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, seat(0), "the casting seat picks");
    let DecisionKind::ChooseCards(pick) = &decision.kind else {
        panic!("the discards are picked, not {}", decision.kind.name());
    };
    assert_eq!((pick.min, pick.max), (2, 2));
    assert_eq!(
        driver.engine().events().len(),
        logged,
        "the picks leave no trace"
    );
    let counterspell = card_in(driver.state(), Zone::Hand(seat(0)), "Counterspell");
    let fireball = card_in(driver.state(), Zone::Hand(seat(0)), "Fireball");
    driver
        .answer(Answer::ChooseCards(CardsChoice(vec![
            counterspell,
            fireball,
        ])))
        .unwrap();
    driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    let after = driver.state().clone();
    let events = driver.engine().events()[logged..].to_vec();
    for card in [counterspell, fireball] {
        assert!(events.iter().any(|event| matches!(
            event,
            Event::CardDiscarded { object, .. } if *object == card
        )));
    }
    let all: Vec<PlayerId> = (0..=seats).map(seat).collect();
    let batches = project_events(&before, &events, &all, u64::try_from(logged).unwrap()).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        for (card, name) in [(counterspell, "Counterspell"), (fireball, "Fireball")] {
            assert!(
                batch.events.iter().any(|view| matches!(
                    view,
                    EventView::Public(Event::CardDiscarded { object, .. }) if *object == card
                )),
                "{viewer:?} sees the discard of {name}"
            );
            assert!(batch_names(batch, name), "{viewer:?} reads {name}");
        }
        let mut hidden = hidden_across(&before, &after, *viewer);
        hidden.remove(&counterspell);
        hidden.remove(&fireball);
        assert_clean(batch, &hidden);
        if *viewer != seat(0) {
            assert!(
                !batch_names(batch, "Wrath of God"),
                "{viewer:?} reads the card that stayed in the hand"
            );
        }
        for name in [
            "Doom Blade",
            "Opt",
            "Giant Growth",
            "Brainstorm",
            "Lightning Bolt",
            "Ancestral Vision",
        ] {
            assert!(
                !batch_names(batch, name),
                "{viewer:?} reads {name} of another hand"
            );
        }
    }
    for viewer in &all {
        let value = serde_json::to_value(StateView::for_player(&after, *viewer)).unwrap();
        assert!(
            names(&value, "Counterspell"),
            "{viewer:?} reads the graveyard"
        );
        assert_eq!(names(&value, "Wrath of God"), *viewer == seat(0));
    }
}

#[rstest]
fn backing_out_at_x_or_at_the_picks_logs_nothing_and_names_no_card(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let logged = driver.engine().events().len();
    driver.begin_cast("Pitcher").unwrap();
    driver
        .answer(Answer::ChooseNumber(NumberChoice::Cancel))
        .unwrap();
    assert_eq!(driver.engine().events().len(), logged);
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::Priority(_)
    ));
    driver.begin_cast("Pitcher").unwrap();
    driver
        .answer(Answer::ChooseNumber(NumberChoice::Number(1)))
        .unwrap();
    driver
        .answer(Answer::ChooseCards(CardsChoice(Vec::new())))
        .unwrap();
    assert_eq!(driver.engine().events().len(), logged);
    assert_eq!(driver.state().zone(Zone::Hand(seat(0))).len(), 4);
}
