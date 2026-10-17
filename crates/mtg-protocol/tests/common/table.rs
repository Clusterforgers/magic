//! Goal: a scripted table whose seats hold views that only ever advance by folding event batches.

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::Event;
use mtg_protocol::{EventBatch, StateView, project_events};
use mtg_testkit::cards::{bear, elf, forest, free_creature, giant, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

/// What `gone` holds when the viewer lost sight of nothing.
pub const NONE: [ObjectId; 0] = [];

/// A scripted game whose seats each hold a view that is advanced only by folding batches.
pub struct Table {
    /// The scripted engine.
    pub driver: Driver,
    /// Each seat's view, in seat order.
    pub views: Vec<StateView>,
}

/// One answer's events and the batch each seat was sent for them.
pub struct Run {
    /// The events the answer produced.
    pub events: Vec<Event>,
    /// One batch per seat, in seat order.
    pub batches: Vec<EventBatch>,
}

/// The first `count` seats.
pub fn seats(count: usize) -> Vec<PlayerId> {
    (0..count).map(PlayerId::from_index).collect()
}

/// The ids a batch lists as new or changed.
pub fn ids(batch: &EventBatch) -> Vec<ObjectId> {
    batch.objects.iter().map(|o| o.id).collect()
}

/// A board with lands and creatures on both sides and a spare creature in each hand.
pub fn two_player_board() -> Scenario {
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

impl Table {
    /// Starts every seat from a snapshot of the scenario's opening state.
    pub fn new(scenario: Scenario) -> Table {
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
    pub fn run(&mut self, act: impl FnOnce(&mut Driver) -> Result<Vec<Event>, ScriptError>) -> Run {
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

    /// The battlefield permanent with this name.
    pub fn id(&self, name: &str) -> ObjectId {
        self.driver.id_of(name).unwrap()
    }
}
