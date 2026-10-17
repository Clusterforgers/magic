//! Goal: on hand-built tables an event batch lists exactly the objects an answer changed, including
//! creatures changed by effects that no event names, and lists what a seat can no longer see in `gone`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::decision::{Answer, MulliganChoice};
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameState};
use mtg_protocol::{EventBatch, EventView, HiddenEvent, StateView, project_events};
use mtg_testkit::cards::{bear, elf, forest, free_creature, giant, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, fuzz_setup, ready};
use rstest::rstest;

/// What `gone` holds when the viewer lost sight of nothing.
const NONE: [ObjectId; 0] = [];

/// A scripted game whose seats each hold a view that is advanced only by folding batches.
struct Table {
    driver: Driver,
    views: Vec<StateView>,
}

/// One answer's events and the batch each seat was sent for them.
struct Run {
    events: Vec<Event>,
    batches: Vec<EventBatch>,
}

fn seats(count: usize) -> Vec<PlayerId> {
    (0..count).map(PlayerId::from_index).collect()
}

impl Table {
    /// Starts every seat from a snapshot of the scenario's opening state.
    fn new(scenario: Scenario) -> Table {
        let driver = Driver::from_scenario(scenario).unwrap();
        let seats = seats(driver.state().players().len());
        let views = seats
            .iter()
            .map(|seat| StateView::for_player(driver.state(), *seat))
            .collect();
        Table { driver, views }
    }

    /// Runs one scripted step, projects it for every seat and folds it into their views.
    ///
    /// Each folded view must equal the one built from the true state, or the run fails.
    fn run(&mut self, act: impl FnOnce(&mut Driver) -> Result<Vec<Event>, ScriptError>) -> Run {
        let before = self.driver.state().clone();
        let events = act(&mut self.driver).unwrap();
        let logged = self.driver.engine().events().len();
        let from_seq = u64::try_from(logged - events.len()).unwrap();
        let viewers = seats(self.views.len());
        let batches = project_events(&before, &events, &viewers, from_seq).unwrap();
        for (view, batch) in self.views.iter_mut().zip(&batches) {
            view.apply(batch);
            let truth = StateView::for_player(self.driver.state(), view.table.viewer);
            assert!(
                *view == truth,
                "seat {:?} drifted from the true state",
                truth.table.viewer
            );
        }
        Run { events, batches }
    }

    fn id(&self, name: &str) -> ObjectId {
        self.driver.id_of(name).unwrap()
    }
}

fn ids(batch: &EventBatch) -> Vec<ObjectId> {
    batch.objects.iter().map(|o| o.id).collect()
}

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

fn two_player_board() -> Scenario {
    Scenario::new(2)
        .battlefield(
            0,
            [
                ready(forest()),
                ready(forest()),
                ready(bear()),
                ready(elf()),
            ],
        )
        .battlefield(1, [ready(swamp()), ready(giant())])
        .hand(0, [free_creature("Spare Bear", 2, 2)])
        .hand(1, [free_creature("Spare Wurm", 4, 4)])
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
        d.add_effect(
            Selector::AllCreatures,
            boost,
            Duration::WhileSourceOnBattlefield(lord_id),
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

#[test]
fn a_drawn_card_reaches_only_the_seat_that_drew_it() {
    let mut table = Table::new(
        Scenario::new(2)
            .at(2, 1, Step::Upkeep)
            .library(1, [bear(), elf(), giant(), forest()]),
    );
    let mut drawn = None;
    for _ in 0..50 {
        let run = table.run(Driver::answer_passively);
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
