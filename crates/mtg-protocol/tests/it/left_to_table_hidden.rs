//! Goal: an ability the game could not run is told whole to every seat when its card is public (a spell, an ability on the stack), and told without the card's words to every seat that cannot see the card a trigger came from (a card drawn), at two, three and four seats, with the hidden form naming only the source and the seat that applies it by hand.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_cards::value::Amount;
use mtg_core::color::Color;
use mtg_engine::Event;
use mtg_protocol::{EventView, HiddenEvent};
use mtg_testkit::cards::{forest, gain_life, instant, island};
use mtg_testkit::trigger_cards::{creature_with, trigger, when, with_rules};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use std::sync::Arc;

use crate::common::table::Table;

/// An effect no step prepared for: the engine cannot run it.
fn failing() -> Effect {
    Effect::GainLife {
        who: Ref::Bound("nothing".to_owned()),
        amount: Amount::Fixed(1),
    }
}

fn on_a_read(card: Arc<CardDefinition>) -> Arc<CardDefinition> {
    let mut definition = (*card).clone();
    definition.read = true;
    Arc::new(definition)
}

/// What the failing ability is and what sets it off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// A read instant, cast for `{G}`.
    Spell,
    /// A read permanent whose trigger fires on the cards its controller draws.
    DrawTrigger,
}

/// Seat 0 holds the card (or a spell that draws) and lands; every other seat has a library to draw from.
fn table(seats: usize, source: Source) -> Table {
    let faulty = match source {
        Source::Spell => on_a_read(with_rules(
            &instant("Faulty", 0, &[Color::Green]),
            vec![Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(2), failing()],
            }],
        )),
        Source::DrawTrigger => on_a_read(creature_with(
            "Faulty",
            vec![trigger(when::you_draw(), vec![gain_life(2), failing()])],
        )),
    };
    let mut board = vec![ready(forest()), ready(forest()), ready(island())];
    let scenario = Scenario::new(seats);
    let mut scenario = match source {
        Source::Spell => scenario.hand(0, [faulty]),
        Source::DrawTrigger => {
            board.push(ready(faulty));
            scenario.hand(0, [mtg_testkit::cards::study("Study")])
        }
    }
    .battlefield(0, board);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 12]);
    }
    Table::new(scenario)
}

/// Plays the source out and returns, for each seat, the view of the one `AbilityLeftToTable` that the batches hold.
fn played(table: &mut Table, source: Source) -> Vec<EventView> {
    let seats = table.views.len();
    let mut found: Vec<Vec<EventView>> = vec![Vec::new(); seats];
    let mut collect = |run: crate::common::table::Run| {
        for (seat, batch) in run.batches.iter().enumerate() {
            found[seat].extend(batch.events.iter().filter(|view| is_left(view)).cloned());
        }
    };
    let name = match source {
        Source::Spell => "Faulty",
        Source::DrawTrigger => "Study",
    };
    collect(table.run(|driver: &mut Driver| driver.cast(name)));
    for _ in 0..(4 * seats) {
        if table.driver.state().stack().is_empty() {
            break;
        }
        collect(table.run(|driver: &mut Driver| driver.pass()));
    }
    found
        .into_iter()
        .map(|mut seen| {
            assert_eq!(seen.len(), 1, "{source:?}: one event for a seat");
            seen.remove(0)
        })
        .collect()
}

fn is_left(view: &EventView) -> bool {
    matches!(
        view,
        EventView::Public(Event::AbilityLeftToTable(_))
            | EventView::Hidden(HiddenEvent::AbilityLeftToTable { .. })
    )
}

#[rstest]
fn a_failing_spell_is_told_whole_to_every_seat(#[values(2, 3, 4)] seats: usize) {
    let mut table = table(seats, Source::Spell);
    for (seat, view) in played(&mut table, Source::Spell).into_iter().enumerate() {
        let EventView::Public(Event::AbilityLeftToTable(left)) = view else {
            panic!("seat {seat}: {view:?}");
        };
        assert_eq!(left.controller.index(), 0, "seat {seat}");
        assert_eq!(left.source.id, table.driver.id_of("Faulty").unwrap());
    }
}

#[rstest]
fn a_failing_trigger_on_a_card_drawn_is_hidden_from_the_seats_that_cannot_see_the_card(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut table = table(seats, Source::DrawTrigger);
    let views = played(&mut table, Source::DrawTrigger);
    let watcher = table.driver.id_of("Faulty").unwrap();
    for (seat, view) in views.into_iter().enumerate() {
        match (seat, view) {
            (0, EventView::Public(Event::AbilityLeftToTable(left))) => {
                assert_eq!(left.source.id, watcher);
            }
            (seat, EventView::Hidden(HiddenEvent::AbilityLeftToTable { source, controller }))
                if seat > 0 =>
            {
                assert_eq!((source.id, controller.index()), (watcher, 0), "seat {seat}");
                let json =
                    serde_json::to_string(&HiddenEvent::AbilityLeftToTable { source, controller })
                        .unwrap();
                assert!(!json.contains("text"), "seat {seat} reads no words: {json}");
            }
            (seat, view) => panic!("seat {seat}: {view:?}"),
        }
    }
}
