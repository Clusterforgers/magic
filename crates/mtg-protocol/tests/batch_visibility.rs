//! Goal: when an object comes into or goes out of a seat's sight a batch says so to exactly the seats
//! that can see it, the first batch is complete, and applying a batch is robust to sloppy input.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::table::{NONE, Table, ids, seats, two_player_board};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Answer, MulliganChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameState};
use mtg_protocol::{EventView, HiddenEvent, StateView, project_events};
use mtg_testkit::cards::{bear, elf, forest, giant};
use mtg_testkit::{Scenario, fuzz_setup, ready};

#[test]
fn a_drawn_card_reaches_only_the_seat_that_drew_it() {
    let mut table = Table::new(
        Scenario::new(2)
            .at(2, 1, Step::Upkeep)
            .library(1, [bear(), elf(), giant(), forest()]),
    );
    let mut drawn = None;
    for _ in 0..50 {
        let run = table.run(mtg_testkit::Driver::answer_passively);
        let draw = run.events.iter().find_map(|event| match event {
            Event::CardDrawn { player, object } => Some((*player, *object)),
            _ => None,
        });
        if let Some(found) = draw {
            drawn = Some((found, run));
            break;
        }
    }
    let ((player, object), run) = drawn.expect("the active player draws for the turn");
    assert_eq!(player, PlayerId(1));
    let drawer = &run.batches[1];
    let listed = drawer.objects.iter().find(|o| o.id == object).unwrap();
    assert_eq!(listed.zone, Zone::Hand(PlayerId(1)));
    assert_eq!(drawer.gone, NONE);

    let other = &run.batches[0];
    assert!(other.objects.is_empty() && other.gone.is_empty());
    assert!(other.events.iter().any(|e| matches!(
        e,
        EventView::Hidden(HiddenEvent::CardDrawn { player }) if *player == PlayerId(1)
    )));
    for batch in &run.batches {
        assert_eq!(
            batch.table.players[1].hand.count, 1,
            "everyone sees the count"
        );
    }
    assert!(other.table.players[1].hand.cards.is_none());
    assert_eq!(
        drawer.table.players[1].hand.cards.as_deref(),
        Some(&[object][..])
    );
}

#[test]
fn a_mulligan_hides_the_old_hand_and_reveals_the_new_one_to_its_owner_only() {
    let mut engine = Engine::new(fuzz_setup(2, 5)).unwrap();
    let owner = engine.decision().unwrap().player;
    engine
        .apply(Answer::Mulligan(MulliganChoice::Mulligan))
        .unwrap();
    // The hands are only redealt once the last seat has decided.
    let before = engine.state().clone();
    let old_hand: Vec<ObjectId> = before.zone(Zone::Hand(owner)).to_vec();
    assert_eq!(old_hand.len(), 7);
    let from_seq = u64::try_from(engine.events().len()).unwrap();

    let events = engine
        .apply(Answer::Mulligan(MulliganChoice::Keep))
        .unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::MulliganTaken { player, .. } if *player == owner))
    );
    let new_hand: Vec<ObjectId> = engine.state().zone(Zone::Hand(owner)).to_vec();
    let batches = project_events(&before, &events, &seats(2), from_seq).unwrap();

    let mine = &batches[owner.index()];
    let theirs = &batches[1 - owner.index()];
    let mut arrived: Vec<ObjectId> = new_hand
        .iter()
        .copied()
        .filter(|id| !old_hand.contains(id))
        .collect();
    let mut left: Vec<ObjectId> = old_hand
        .iter()
        .copied()
        .filter(|id| !new_hand.contains(id))
        .collect();
    arrived.sort();
    left.sort();
    assert!(!arrived.is_empty(), "a new hand brings new cards");
    assert_eq!(ids(mine), arrived, "cards drawn again are not repeated");
    assert_eq!(
        mine.gone, left,
        "cards shuffled away leave the owner's view"
    );
    assert!(theirs.objects.is_empty() && theirs.gone.is_empty());
    assert!(mine.objects.iter().all(|o| o.zone == Zone::Hand(owner)));
}

#[test]
fn the_first_batch_lists_every_object_including_the_ones_the_setup_placed() {
    let setup = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(forest()), ready(bear())])
        .graveyard(1, [giant()])
        .hand(0, [elf()])
        .setup()
        .unwrap();
    let before = GameState::from_setup(&setup).unwrap();
    let engine = Engine::new(setup).unwrap();
    let batches = project_events(&before, engine.events(), &seats(2), 0).unwrap();
    for (seat, batch) in batches.iter().enumerate() {
        let truth = StateView::for_player(engine.state(), PlayerId::from_index(seat));
        let started = StateView::from_first_batch(batch).unwrap();
        assert!(
            started == truth,
            "seat {seat} cannot start from its first batch"
        );
        assert_eq!(batch.objects, truth.objects);
        let mut snapped = StateView::for_player(&before, PlayerId::from_index(seat));
        snapped.apply(batch);
        assert!(
            snapped == truth,
            "seat {seat} cannot fold it onto a snapshot either"
        );
    }
    let later = project_events(&before, engine.events(), &seats(2), 9).unwrap();
    assert!(StateView::from_first_batch(&later[0]).is_none());
}

#[test]
fn applying_a_batch_keeps_objects_in_id_order_and_ignores_unknown_ids() {
    let mut table = Table::new(two_player_board());
    let mut view = table.views[0].clone();
    let run = table.run(|d| d.tap_for_mana("Forest"));
    let mut batch = run.batches[0].clone();

    let mut extra = view.objects[0].clone();
    extra.id = ObjectId(1_000);
    let mut early = view.objects[1].clone();
    early.power = Some(40);
    batch.objects = vec![extra.clone(), batch.objects[0].clone(), early.clone()];
    batch.gone = vec![ObjectId(999_999)];
    let before_len = view.objects.len();
    view.apply(&batch);

    assert_eq!(
        view.objects.len(),
        before_len + 1,
        "one new object, nothing dropped"
    );
    assert!(view.objects.windows(2).all(|pair| pair[0].id < pair[1].id));
    assert_eq!(view.objects.last(), Some(&extra));
    assert!(view.objects.contains(&early));
    assert_eq!(view.table, *batch.table);
}
