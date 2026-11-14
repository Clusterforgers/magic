//! Goal: in bot games at team tables a permanent is held back by summoning sickness until a turn of its controller begins and not a turn longer or shorter (CR 302.6): never free before that turn, never held back again after it, whoever plays the turns before it, in Teams with teammates apart or side by side and in Two-Headed Giant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::sync::Mutex;

use crate::common::seeds::{for_each_seed, fuzz_games};
use mtg_core::house::HouseRules;
use mtg_core::ids::ObjectId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::characteristics::controller_of;
use mtg_engine::fold::apply_event;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, GameState};
use mtg_testkit::{
    PlayOptions, copy_fuzz_setup, entered_fuzz_setup, fuzz_setup, keyword_fuzz_setup,
    play_game_with, spell_fuzz_setup, statics_fuzz_setup, trigger_fuzz_setup, turn_fuzz_setup,
};
use rstest::rstest;

/// Games per deck family and table; override with `MTG_FUZZ_GAMES`.
fn games() -> u64 {
    fuzz_games(6)
}

/// The decks of the campaign: plain play, spells, triggers, keywords, extra turns, statics (some take control of
/// creatures), permanents that enter in many ways and copies.
const FAMILIES: [fn(usize, u64) -> GameSetup; 8] = [
    fuzz_setup,
    spell_fuzz_setup,
    trigger_fuzz_setup,
    keyword_fuzz_setup,
    turn_fuzz_setup,
    |seats, seed| statics_fuzz_setup(seats, seed).unwrap(),
    |seats, seed| entered_fuzz_setup(seats, seed).unwrap(),
    |seats, seed| copy_fuzz_setup(seats, seed).unwrap(),
];

/// How many permanents were seen held back and how many free.
#[derive(Default)]
struct Seen {
    held: u64,
    free: u64,
}

/// Plays `setup` to its end and folds its log again, checking after every event that each permanent on the battlefield
/// is held back exactly while no turn of its controller has begun since it arrived. A permanent that ever changed
/// hands or phased out is left out: it follows other rules than its controller's turn.
fn watch(setup: &GameSetup, seed: u64, label: &str) -> Seen {
    let options = PlayOptions {
        check_invariants: false,
        ..PlayOptions::default()
    };
    let outcome = play_game_with(setup.clone(), seed, options)
        .unwrap_or_else(|failure| panic!("{label} seed {seed}: {}", failure.kind));
    let mut state = GameState::from_setup(setup).unwrap();
    // The permanents a turn of their controller has begun for since they arrived.
    let mut settled: BTreeSet<ObjectId> = BTreeSet::new();
    let mut apart: BTreeSet<ObjectId> = BTreeSet::new();
    let mut tally = Seen::default();
    for event in &outcome.events {
        apply_event(&mut state, event).unwrap();
        if matches!(event, Event::TurnBegan { .. }) {
            for &id in state.zone(Zone::Battlefield) {
                if controller_of(&state, id).is_some_and(|player| state.is_active(player)) {
                    settled.insert(id);
                }
            }
        }
        settled.retain(|id| state.zone(Zone::Battlefield).contains(id));
        for object in state.objects() {
            if object.zone() == Zone::PhasedOut
                || (object.zone() == Zone::Battlefield
                    && controller_of(&state, object.id()) != Some(object.owner()))
            {
                apart.insert(object.id());
            }
        }
        for &id in state.zone(Zone::Battlefield) {
            if apart.contains(&id) {
                continue;
            }
            let controller = controller_of(&state, id).unwrap();
            let held = state
                .object(id)
                .and_then(|object| object.permanent())
                .unwrap()
                .sick_for(controller);
            assert_eq!(
                held,
                !settled.contains(&id),
                "{label} seed {seed}, turn {} of {:?}: {id:?} of {controller:?} is {} after {event:.120?}",
                state.turn().number(),
                state.turn().active(),
                if held { "held back" } else { "free" },
            );
            if held {
                tally.held += 1;
            } else {
                tally.free += 1;
            }
        }
    }
    tally
}

#[rstest]
#[case::teams_apart_four(TeamMode::Teams, 4)]
#[case::teams_apart_six(TeamMode::Teams, 6)]
#[case::two_headed_giant(TeamMode::TwoHeadedGiant, 4)]
fn a_permanent_is_held_back_until_a_turn_of_its_controller_begins(
    #[case] mode: TeamMode,
    #[case] seats: usize,
) {
    let teams = Teams::even(mode, 2, u32::try_from(seats / 2).unwrap());
    let totals = Mutex::new(Seen::default());
    for (index, family) in FAMILIES.iter().enumerate() {
        for_each_seed(0..games(), |seed| {
            let setup = family(seats, seed).with_house_rules(HouseRules {
                teams: Some(teams.clone()),
                ..HouseRules::default()
            });
            let tally = watch(&setup, seed, &format!("family {index} {mode:?}/{seats}"));
            let mut totals = totals.lock().unwrap();
            totals.held += tally.held;
            totals.free += tally.free;
        });
    }
    let totals = totals.lock().unwrap();
    assert!(
        totals.held > 100 && totals.free > 100,
        "the games held {} permanents back and freed {}",
        totals.held,
        totals.free
    );
}
