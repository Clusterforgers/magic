//! `Effect::ExchangeControl`: two permanents exchange controllers (CR 701.12b).

use mtg_cards::effect::Effect;
use mtg_core::ids::EffectId;

use super::wrong_effect;
use crate::characteristics::controller_of_object;
use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::events::Event;
use crate::object::GameObject;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The references name the two permanents still on the battlefield; each takes the controller the other has now, as two
/// control effects that begin together with consecutive timestamps. Nothing happens unless exactly two permanents are
/// named and two players control them (CR 701.12b), so one target that became illegal stops the whole exchange.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ExchangeControl { what, duration } = effect else {
        return Err(wrong_effect("ExchangeControl"));
    };
    let state = cx.state();
    let mut named: Vec<&GameObject> = Vec::new();
    for reference in what {
        for object in cx.scope.live_objects(reference)? {
            if object.zone() == Zone::Battlefield
                && named.iter().all(|seen| seen.id() != object.id())
            {
                named.push(object);
            }
        }
    }
    let [first, second] = named.as_slice() else {
        return Ok(Run::events(Vec::new()));
    };
    let (one, other) = (
        controller_of_object(state, first),
        controller_of_object(state, second),
    );
    if one == other {
        return Ok(Run::events(Vec::new()));
    }
    let events = [(first, other), (second, one)]
        .into_iter()
        .zip(0u64..)
        .map(|((object, player), made)| Event::EffectCreated {
            effect: ContinuousEffect::new(
                EffectId(state.next_effect + made),
                Selector::Objects(vec![object.reference()]),
                Modification::SetController(player),
                *duration,
                state.next_timestamp + made,
            )
            .from_source(cx.scope.source)
            .begun(cx.controller(), state.turn().number()),
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: two permanents of two players exchange controllers as one pair of effects with consecutive timestamps, an
    //! exchange of the pair again gives each back, and nothing happens when the references name another number of
    //! permanents or one player controls both.
    use mtg_cards::modify::Duration;
    use mtg_cards::refs::Ref;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::characteristics::controller_of;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    /// Seat 0 has a Bear and a Cub, seat 1 an Ogre, seat 2 an Elf.
    fn game() -> GameState {
        state(
            vec![vec![], vec![], vec![]],
            vec![
                vec![creature("Bear", 2, 2), creature("Cub", 1, 1)],
                vec![creature("Ogre", 3, 3)],
                vec![creature("Elf", 1, 1)],
            ],
        )
    }

    fn controls(game: &GameState, name: &str) -> PlayerId {
        let object = game.objects().find(|o| o.card().name == name).unwrap();
        controller_of(game, object.id()).unwrap()
    }

    /// Runs the exchange of the creatures the names list, as seat 0, and folds what it proposes.
    fn exchange(game: &mut GameState, names: &[&str]) -> Vec<Event> {
        let what: Vec<Ref> = names
            .iter()
            .map(|name| {
                serde_json::from_str(&format!(r#"{{"Each":[{{"Named":"{name}"}}]}}"#)).unwrap()
            })
            .collect();
        let effect = Effect::ExchangeControl {
            what,
            duration: Duration::Forever,
        };
        let source = game.objects().next().unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(0), source.reference()),
            resolving: &resolving,
        };
        let Ok(Run::Done { events, .. }) = run(&cx, &effect) else {
            panic!("the exchange did not run");
        };
        for event in &events {
            apply_event(game, event).unwrap();
        }
        events
    }

    #[test]
    fn w6e_two_players_exchange_their_permanents() {
        let mut game = game();
        let events = exchange(&mut game, &["Bear", "Ogre"]);
        assert_eq!(controls(&game, "Bear"), PlayerId(1));
        assert_eq!(controls(&game, "Ogre"), PlayerId(0));
        assert_eq!(controls(&game, "Elf"), PlayerId(2));
        let stamps: Vec<u64> = events
            .iter()
            .filter_map(|event| match event {
                Event::EffectCreated { effect } => Some(effect.timestamp),
                _ => None,
            })
            .collect();
        assert_eq!(stamps.len(), 2);
        assert_eq!(stamps[1], stamps[0] + 1);
        exchange(&mut game, &["Ogre", "Bear"]);
        assert_eq!(controls(&game, "Bear"), PlayerId(0));
        assert_eq!(controls(&game, "Ogre"), PlayerId(1));
    }

    #[rstest]
    #[case::one(&["Bear"])]
    #[case::three(&["Bear", "Ogre", "Elf"])]
    #[case::the_same_twice(&["Ogre", "Ogre"])]
    #[case::one_controller_of_both(&["Bear", "Cub"])]
    fn w6e_anything_but_a_pair_of_two_players_exchanges_nothing(#[case] names: &[&str]) {
        let mut game = game();
        assert_eq!(exchange(&mut game, names), Vec::new());
        assert_eq!(controls(&game, "Bear"), PlayerId(0));
        assert_eq!(controls(&game, "Ogre"), PlayerId(1));
    }
}
