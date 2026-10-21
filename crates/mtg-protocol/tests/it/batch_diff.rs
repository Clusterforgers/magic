//! Goal: on hand-built tables an event batch lists exactly the objects an answer changed, including
//! creatures changed by effects that no event names, and every seat's folded view stays true.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::table::{NONE, Run, Table, ids, two_player_board};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::Event;
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_protocol::EventBatch;
use mtg_testkit::cards::{bear, elf, free_creature, giant};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn names(batch: &EventBatch) -> Vec<&str> {
    let mut names: Vec<&str> = batch.objects.iter().map(|o| o.name.as_str()).collect();
    names.sort_unstable();
    names
}

fn stats(batch: &EventBatch, name: &str) -> (Option<i32>, Option<i32>) {
    let object = batch.objects.iter().find(|o| o.name == name).unwrap();
    (object.power, object.toughness)
}

/// True when no event in the run names `id`: the engine's own list of touched objects, which the
/// first version of the projection relied on, would have missed it.
fn unnamed_by_events(run: &Run, id: ObjectId) -> bool {
    run.events
        .iter()
        .all(|event| !event.touched_objects().contains(&id))
}

#[test]
fn an_answer_lists_only_the_objects_it_changed() {
    let mut table = Table::new(two_player_board());
    let before = table.views[0].table.priority.clone();

    let run = table.run(|d| d.tap_for_mana("Forest"));
    let tapped = run
        .events
        .iter()
        .find_map(|event| match event {
            Event::PermanentTapped { object } => Some(*object),
            _ => None,
        })
        .unwrap();
    for batch in &run.batches {
        assert_eq!(
            ids(batch),
            [tapped],
            "the other five permanents are omitted"
        );
        assert!(batch.objects[0].permanent.as_ref().unwrap().tapped);
        assert_eq!(batch.gone, NONE);
        assert_eq!(batch.table.players[0].mana.get(ManaType::Green), 1);
    }

    let run = table.run(Driver::pass);
    for batch in &run.batches {
        assert!(batch.objects.is_empty() && batch.gone.is_empty());
        assert_ne!(batch.table.priority, before, "priority moved on");
    }
}

#[rstest]
#[case::every_creature(Selector::AllCreatures, &["Craw Wurm", "Grizzly Bears", "Llanowar Elf"])]
#[case::one_seats_creatures(Selector::CreaturesControlledBy(PlayerId(0)), &["Grizzly Bears", "Llanowar Elf"])]
#[case::a_seat_with_one(Selector::CreaturesControlledBy(PlayerId(1)), &["Craw Wurm"])]
fn an_effect_that_names_no_object_lists_every_creature_it_changes(
    #[case] selector: Selector,
    #[case] changed: &[&str],
) {
    let mut table = Table::new(two_player_board());
    let buff = Modification::ModifyPowerToughness {
        power: 1,
        toughness: 2,
    };
    let run = table.run(|d| d.add_effect(selector, buff, Duration::UntilEndOfTurn));
    let base = [("Craw Wurm", 4), ("Grizzly Bears", 2), ("Llanowar Elf", 1)];
    for batch in &run.batches {
        assert_eq!(
            names(batch),
            changed,
            "lands and unaffected creatures are omitted"
        );
        for (name, power) in base.iter().filter(|(name, _)| changed.contains(name)) {
            assert_eq!(stats(batch, name), (Some(power + 1), Some(power + 2)));
            assert!(
                unnamed_by_events(&run, table.id(name)),
                "{name} is named by an event"
            );
        }
        assert_eq!(batch.gone, NONE);
    }
}

#[test]
fn an_effect_that_runs_out_lists_the_creatures_it_stops_changing() {
    let mut table = Table::new(two_player_board());
    let buff = Modification::ModifyPowerToughness {
        power: 2,
        toughness: 2,
    };
    table.run(|d| d.add_effect(Selector::AllCreatures, buff, Duration::UntilEndOfTurn));
    assert_eq!(
        table.views[1]
            .objects
            .iter()
            .find(|o| o.name == "Craw Wurm")
            .unwrap()
            .power,
        Some(6)
    );

    let mut expired = None;
    for _ in 0..200 {
        let run = table.run(Driver::answer_passively);
        if run
            .events
            .iter()
            .any(|e| matches!(e, Event::EffectExpired { .. }))
        {
            expired = Some(run);
            break;
        }
    }
    let run = expired.expect("the effect lasts until the end of the turn");
    for batch in &run.batches {
        assert_eq!(names(batch), ["Craw Wurm", "Grizzly Bears", "Llanowar Elf"]);
        assert_eq!(stats(batch, "Craw Wurm"), (Some(4), Some(4)));
        assert_eq!(stats(batch, "Llanowar Elf"), (Some(1), Some(1)));
    }
    assert!(unnamed_by_events(&run, table.id("Craw Wurm")));
}

#[test]
fn a_lord_leaving_changes_creatures_whose_events_never_name_them() {
    let lord = free_creature("Lord", 1, 1);
    let mut table = Table::new(
        Scenario::new(3)
            .at(1, 1, Step::Main1)
            .battlefield(0, [ready(lord)])
            .hand(0, [bear()])
            .battlefield(1, [ready(bear()), ready(elf())])
            .battlefield(2, [ready(giant())]),
    );
    let lord_id = table.id("Lord");
    let boost = Modification::ModifyPowerToughness {
        power: 1,
        toughness: 1,
    };
    let run = table.run(|d| {
        d.add_effect_from(
            lord_id,
            Selector::AllCreatures,
            boost,
            Duration::WhileSourceOnBattlefield,
        )
    });
    for batch in &run.batches {
        assert_eq!(stats(batch, "Craw Wurm"), (Some(5), Some(5)));
    }

    let hand_card = table.views[0]
        .objects
        .iter()
        .find(|o| o.zone == Zone::Hand(PlayerId(0)))
        .unwrap()
        .id;
    let run = table.run(|d| d.concede(PlayerId(0)));
    for (seat, batch) in run.batches.iter().enumerate() {
        assert_eq!(
            names(batch),
            ["Craw Wurm", "Grizzly Bears", "Llanowar Elf"],
            "seat {seat} is told the three creatures lost the bonus"
        );
        assert_eq!(stats(batch, "Craw Wurm"), (Some(4), Some(4)));
        assert_eq!(stats(batch, "Grizzly Bears"), (Some(2), Some(2)));
        assert!(
            batch.gone.contains(&lord_id),
            "the lord left the battlefield"
        );
    }
    for name in ["Craw Wurm", "Grizzly Bears", "Llanowar Elf"] {
        let id = table.id(name);
        assert!(unnamed_by_events(&run, id), "{name} is named by an event");
    }
    // The conceding seat loses sight of the card in its own hand; nobody else ever saw it.
    assert!(run.batches[0].gone.contains(&hand_card));
    assert!(!run.batches[1].gone.contains(&hand_card));
    assert!(!run.batches[2].gone.contains(&hand_card));
}
