//! Goal: a spell, a triggered ability and an activated ability of a card that runs on a player's read, whose rules fail partway, keep the steps already done, log `AbilityLeftToTable` and end like any resolution (the cost paid, the stack and priority consistent, the game going on, a rebuilt engine in the same state) at two to four seats, while the same rules on a card that is not on a read still stop the game and undo the action that reached them; whole bot games full of failing read cards keep every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::read::ability_wording;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::value::Amount;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event};
use mtg_testkit::cards::{forest, free_creature, gain_life, instant};
use mtg_testkit::harness::{PlayOptions, bots_for, play_game_with_bots};
use mtg_testkit::smoke::smoke_setup;
use mtg_testkit::trigger_cards::{artifact_with, creature_with, trigger, when, with_rules};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::removal::resolve_the_stack;
use crate::common::{name_of, pid};

/// An effect no step prepared for: it names something that was never bound, so the engine cannot run it.
fn failing() -> Effect {
    Effect::GainLife {
        who: Ref::Bound("nothing".to_owned()),
        amount: Amount::Fixed(1),
    }
}

/// Gain 2 life, an effect that cannot run, and 5 life more that is never reached.
fn effects() -> Vec<Effect> {
    vec![gain_life(2), failing(), gain_life(5)]
}

/// An instant for `{G}` with those effects.
fn spell() -> Ability {
    Ability::Spell {
        targets: Vec::new(),
        effects: effects(),
    }
}

/// A free ability that taps its artifact, with those effects.
fn tapper() -> Ability {
    Ability::Activated {
        cost: Cost(vec![CostItem::Tap]),
        timing: Timing::Instant,
        condition: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: effects(),
    }
}

/// What the card does and how it is put to use.
#[derive(Debug, Clone, Copy)]
enum Kind {
    /// An instant cast from the hand.
    Spell,
    /// A creature whose enters trigger runs the effects.
    Trigger,
    /// An artifact on the battlefield, used by tapping it.
    Activated,
}

const NAME: &str = "Faulty";

/// The card of `kind`; `read` says whether it runs on a player's read.
fn card(kind: Kind, read: bool) -> Arc<CardDefinition> {
    let card = match kind {
        Kind::Spell => with_rules(&instant(NAME, 0, &[Color::Green]), vec![spell()]),
        Kind::Trigger => creature_with(NAME, vec![trigger(when::this_enters(), effects())]),
        Kind::Activated => artifact_with(NAME, vec![tapper()]),
    };
    let mut definition = (*card).clone();
    definition.read = read;
    Arc::new(definition)
}

/// The rules of the card's one ability, which the event words.
fn rules_of(kind: Kind) -> Ability {
    match kind {
        Kind::Spell => spell(),
        Kind::Trigger => trigger(when::this_enters(), effects()),
        Kind::Activated => tapper(),
    }
}

/// A table of `seats` where seat 0 holds the card (in the hand or on the battlefield by its kind) and two forests.
fn scenario(seats: usize, kind: Kind, read: bool) -> Scenario {
    let faulty = card(kind, read);
    let mut scenario = Scenario::new(seats);
    let mut board = vec![ready(forest()), ready(forest())];
    match kind {
        Kind::Spell | Kind::Trigger => scenario = scenario.hand(0, [faulty]),
        Kind::Activated => board.push(ready(faulty)),
    }
    scenario = scenario.battlefield(0, board);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 12]);
    }
    scenario.battlefield(1, [ready(free_creature("Theirs", 2, 2))])
}

/// Puts the card to use: casts it, or activates it.
fn use_card(driver: &mut Driver, kind: Kind) -> Result<(), ScriptError> {
    match kind {
        Kind::Spell | Kind::Trigger => driver.cast(NAME).map(drop),
        Kind::Activated => driver.begin_activation(NAME).map(drop),
    }
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn find(driver: &Driver, zone: Zone) -> Option<ObjectId> {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == NAME)
}

fn left_to_table(events: &[Event]) -> Vec<&mtg_engine::resolve::LeftToTable> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::AbilityLeftToTable(left) => Some(&**left),
            _ => None,
        })
        .collect()
}

#[rstest]
fn a_failing_ability_of_a_read_card_is_left_to_the_table(
    #[values(2, 3, 4)] seats: usize,
    #[values(Kind::Spell, Kind::Trigger, Kind::Activated)] kind: Kind,
) {
    let mut driver = Driver::from_scenario(scenario(seats, kind, true)).unwrap();
    let start = life(&driver, 0);
    use_card(&mut driver, kind).unwrap();
    // Two forests are on the battlefield for a card that costs nothing: the cost is what the card asks, paid in full.
    let mark = driver.engine().events().len();
    resolve_the_stack(&mut driver).unwrap();
    let events = driver.engine().events().to_vec();

    let left = left_to_table(&events);
    assert_eq!(left.len(), 1, "{kind:?}: one ability is left to the table");
    assert_eq!(left[0].controller, pid(0), "{kind:?}");
    assert_eq!(left[0].source.id, find_source(&driver, kind), "{kind:?}");
    assert_eq!(
        left[0].text,
        ability_wording(&rules_of(kind)).unwrap_or_default(),
        "{kind:?}: the words are the sheet's"
    );

    // The event comes while the resolution is under way and the resolution ends as a resolution that did what it said.
    let after = &events[mark..];
    let at = after
        .iter()
        .position(|event| matches!(event, Event::AbilityLeftToTable(_)))
        .unwrap();
    assert!(
        matches!(
            after[at + 1],
            Event::ResolutionEnded {
                outcome: ResolutionOutcome::Resolved,
                ..
            }
        ),
        "{kind:?}: {:?}",
        after[at + 1]
    );

    // The first step stands, the steps after the failure never ran.
    assert_eq!(life(&driver, 0), start + 2, "{kind:?}");
    assert!(driver.state().stack().is_empty(), "{kind:?}");
    assert!(driver.state().resolving().is_none(), "{kind:?}");
    assert_eq!(driver.decision().unwrap().player, pid(0), "{kind:?}");
    assert_eq!(driver.decision().unwrap().kind.name(), "priority");
    match kind {
        Kind::Spell => assert!(find(&driver, Zone::Graveyard(pid(0))).is_some()),
        Kind::Trigger | Kind::Activated => assert!(find(&driver, Zone::Battlefield).is_some()),
    }
    if matches!(kind, Kind::Activated) {
        let id = find(&driver, Zone::Battlefield).unwrap();
        assert!(
            driver
                .state()
                .object(id)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped()
        );
    }

    // The game goes on, every seat passing through the rest of the turn and into the next one.
    let turn = driver.state().turn().number();
    driver.idle_until(turn + 1, Step::Upkeep).unwrap();
    assert_eq!(left_to_table(driver.engine().events()).len(), 1, "{kind:?}");

    // An engine rebuilt from the log stands where this one does.
    let rebuilt = Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(
        rebuilt.state_hash(),
        driver.engine().state_hash(),
        "{kind:?}"
    );
    assert_eq!(rebuilt.decision(), driver.engine().decision(), "{kind:?}");
}

/// The object whose ability the event names: the card itself, which a spell, a trigger and an activation all come from.
fn find_source(driver: &Driver, kind: Kind) -> ObjectId {
    let zone = match kind {
        Kind::Spell => Zone::Graveyard(pid(0)),
        Kind::Trigger | Kind::Activated => Zone::Battlefield,
    };
    find(driver, zone).unwrap()
}

#[rstest]
fn a_log_cut_inside_the_failing_resolution_replays_to_the_state_it_was_played_to(
    #[values(2, 3, 4)] seats: usize,
    #[values(Kind::Spell, Kind::Trigger, Kind::Activated)] kind: Kind,
) {
    let mut driver = Driver::from_scenario(scenario(seats, kind, true)).unwrap();
    use_card(&mut driver, kind).unwrap();
    resolve_the_stack(&mut driver).unwrap();
    let setup = driver.engine().setup().clone();
    let log = driver.engine().events().to_vec();
    let at = log
        .iter()
        .position(|event| matches!(event, Event::AbilityLeftToTable(_)))
        .unwrap();
    // Every cut from just before the event to the end of the log rebuilds the engine in the state it had there.
    for cut in at.saturating_sub(2)..=log.len() {
        let mut played = mtg_engine::GameState::from_setup(&setup).unwrap();
        for event in &log[..cut] {
            mtg_engine::apply_event(&mut played, event).unwrap();
        }
        let replayed = mtg_engine::engine::replay(&setup, &log[..cut]).unwrap();
        assert_eq!(
            replayed.state_hash(),
            played.state_hash(),
            "{kind:?} at {cut}"
        );
    }
    let rebuilt = Engine::resume(setup, log).unwrap();
    assert_eq!(
        rebuilt.state_hash(),
        driver.engine().state_hash(),
        "{kind:?}"
    );
}

#[rstest]
fn the_same_rules_on_a_card_that_is_not_read_stop_the_game_and_undo_the_action(
    #[values(2, 3, 4)] seats: usize,
    #[values(Kind::Spell, Kind::Trigger, Kind::Activated)] kind: Kind,
) {
    let mut driver = Driver::from_scenario(scenario(seats, kind, false)).unwrap();
    use_card(&mut driver, kind).unwrap();
    let mut failure = None;
    for _ in 0..(3 * seats) {
        let before = driver.engine().state_hash();
        let mark = driver.engine().events().len();
        if let Err(error) = driver.pass() {
            failure = Some((error, before, mark));
            break;
        }
    }
    let (error, before, mark) = failure.expect("the ability fails when it resolves");
    assert!(
        matches!(&error, ScriptError::Engine(EngineError::Resolution(what)) if what.contains("nothing")),
        "{kind:?}: {error:?}"
    );
    assert_eq!(
        driver.engine().state_hash(),
        before,
        "{kind:?}: the pass is undone"
    );
    assert_eq!(driver.engine().events().len(), mark, "{kind:?}");
    assert!(
        left_to_table(driver.engine().events()).is_empty(),
        "{kind:?}: nothing is left to the table"
    );
}

#[rstest]
#[case::spell(Kind::Spell)]
#[case::trigger(Kind::Trigger)]
#[case::activated(Kind::Activated)]
fn whole_games_of_failing_read_cards_keep_every_invariant_at_every_table_size(#[case] kind: Kind) {
    let mut left = 0;
    for seats in [2, 3, 4] {
        for seed in 0..6 {
            let card = card(kind, true);
            let setup = smoke_setup(&card, seats, seed);
            let options = PlayOptions {
                max_answers: 20_000,
                runaway_fails: true,
                ..PlayOptions::default()
            };
            let bots = bots_for(seats, seed, options)
                .into_iter()
                .map(|bot| bot.favouring(NAME.to_owned()))
                .collect();
            let outcome =
                play_game_with_bots(setup, seed, options, bots).unwrap_or_else(|failure| {
                    panic!("{kind:?} at {seats} seats, seed {seed}: {failure}")
                });
            left += left_to_table(&outcome.events).len();
        }
    }
    assert!(left > 0, "{kind:?}: the bots met the failing ability");
}
