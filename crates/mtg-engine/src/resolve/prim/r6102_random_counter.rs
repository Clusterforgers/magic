//! `Effect::PutRandomCounter`: "choose a kind of counter at random that this creature doesn't have on it from among ...
//! Put a counter of that kind on this creature." The pick spends the next substream of the generator and is logged as
//! `KindChosenAtRandom`, so a replay from the log gives the same game.

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;

use super::put_counters::holders;
use super::wrong_effect;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rng::GameRng;

/// Picks one of `among` that the object holds none of and puts one counter of it on the object; nothing is picked
/// (and no substream is spent) when it holds them all or has left the battlefield.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::PutRandomCounter { on, among } = effect else {
        return Err(wrong_effect("PutRandomCounter"));
    };
    let mut events = Vec::new();
    for holder in holders(cx, on)? {
        let Target::Object(object) = holder else {
            continue;
        };
        let Some(permanent) = cx.state().object(object.id).and_then(|o| o.permanent()) else {
            continue;
        };
        let open: Vec<&CounterKind> = among
            .iter()
            .filter(|kind| permanent.counters_of(kind) == 0)
            .collect();
        if open.is_empty() {
            continue;
        }
        let state = cx.state();
        // One substream for each pick: a second holder reads the next one.
        let index = state.rng_index + u64::try_from(events.len() / 2).unwrap_or(u64::MAX);
        let at = GameRng::substream(state.seed, index).index(open.len());
        let Some(kind) = open.get(at) else {
            continue;
        };
        events.push(Event::KindChosenAtRandom {
            object: object.id,
            kind: (*kind).clone(),
        });
        events.push(Event::CountersChanged(CountersChanged {
            on: holder,
            counter: (*kind).clone(),
            delta: 1,
        }));
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a counter chosen at random goes on the holder of a kind it lacks, never of one it has; every kind it lacks
    //! can come up over the seeds; it puts nothing when it holds every kind; the pick is logged before the counter.
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use std::collections::BTreeSet;

    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn kind(name: &str) -> CounterKind {
        match name {
            "+1/+1" => CounterKind::PlusOnePlusOne,
            other => CounterKind::Other(other.to_owned()),
        }
    }

    /// The Giant of seat 0 (of `seats`) holds these counters; the game uses `seed`.
    fn game(seats: usize, held: &[&str], seed: u64) -> GameState {
        let mut battlefields = vec![vec![creature("Giant", 3, 3)]];
        battlefields.extend((1..seats).map(|_| vec![creature("Bear", 2, 2)]));
        let mut game = setup(vec![vec![]; seats], battlefields);
        game.players[0].battlefield[0].counters = held.iter().map(|name| (kind(name), 1)).collect();
        game.seed = seed;
        GameState::from_setup(&game).unwrap()
    }

    fn put(game: &GameState, among: &[&str]) -> Vec<Event> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect = Effect::PutRandomCounter {
            on: mtg_cards::refs::Ref::This,
            among: among.iter().map(|name| kind(name)).collect(),
        };
        match run(&cx, &effect).unwrap() {
            Run::Done { events, .. } => events,
            other => panic!("{other:?}"),
        }
    }

    #[rstest]
    #[case::two_seats(2, &["flying"], &["flying", "reach", "+1/+1"])]
    #[case::three_seats(3, &["flying", "reach"], &["flying", "reach", "+1/+1"])]
    #[case::four_seats(4, &[], &["flying", "trample"])]
    fn the_kind_is_one_it_lacks(
        #[case] seats: usize,
        #[case] held: &[&str],
        #[case] among: &[&str],
    ) {
        let mut seen = BTreeSet::new();
        for seed in 0..64 {
            let events = put(&game(seats, held, seed), among);
            let [
                Event::KindChosenAtRandom { kind: picked, .. },
                Event::CountersChanged(changed),
            ] = events.as_slice()
            else {
                panic!("{events:?}");
            };
            assert_eq!(*picked, changed.counter);
            assert_eq!(changed.delta, 1);
            assert!(!held.contains(&picked.name()), "{picked:?}");
            seen.insert(picked.name().to_owned());
        }
        let lacking: BTreeSet<String> = among
            .iter()
            .filter(|name| !held.contains(name))
            .map(|name| (*name).to_owned())
            .collect();
        assert_eq!(seen, lacking);
    }

    #[rstest]
    #[case::all_held(&["flying", "reach"], &["flying", "reach"])]
    fn nothing_when_every_kind_is_held(#[case] held: &[&str], #[case] among: &[&str]) {
        assert!(put(&game(2, held, 7), among).is_empty());
    }
}
