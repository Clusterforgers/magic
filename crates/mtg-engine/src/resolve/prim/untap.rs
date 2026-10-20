//! `Effect::Untap`: permanents are untapped (CR 701.26).

use mtg_cards::effect::Effect;

use super::{tap, wrong_effect};
use crate::resolve::{Cx, ResolveError, Run};

/// Each permanent the effect names that is still on the battlefield and tapped becomes untapped, all together, in
/// the order the reference lists them; what `Tap` passes over, this passes over too (a permanent that is untapped
/// already, a card in another zone, a player, an object that changed zones since it was named, a repeated name).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Untap { what } = effect else {
        return Err(wrong_effect("Untap"));
    };
    tap::set(cx, what, false)
}

#[cfg(test)]
mod tests {
    //! Goal: an untap untaps each tapped permanent it names, all together and in the order the reference lists them, and passes over permanents that are untapped already, cards in other zones, players, objects that changed zones since they were named and names that repeat; it binds nothing, moves no object and goes on to the next effect; and it refuses another effect by name.
    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;
    use std::sync::Arc;

    use super::*;
    use crate::events::Event;
    use crate::fixtures::{creature, setup};
    use crate::object::ObjectRef;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn relic() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact))
    }

    /// Seat 0 controls the Source, a tapped Wolf, a tapped Relic and a tapped Idle. Seat 1 controls a tapped Bear and
    /// an Ogre that is not tapped.
    fn game() -> GameState {
        let mut game = setup(
            vec![vec![], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    creature("Wolf", 2, 2),
                    relic(),
                    creature("Idle", 1, 1),
                ],
                vec![creature("Bear", 2, 2), creature("Ogre", 3, 4)],
            ],
        );
        for (seat, place) in [(0, 1), (0, 2), (0, 3), (1, 0)] {
            game.players[seat].battlefield[place].tapped = true;
        }
        GameState::from_setup(&game).unwrap()
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|o| o.card().name == name)
            .map(crate::object::GameObject::reference)
            .unwrap()
    }

    fn untaps(game: &GameState, json: &str) -> Result<Vec<String>, ResolveError> {
        let effect: Effect = serde_json::from_str(json).unwrap();
        let source = named(game, "Source");
        let resolving = Resolving::begin(source.id);
        let cx = Cx {
            scope: Scope::announcing(game, ME, source),
            resolving: &resolving,
        };
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run(&cx, &effect)?
        else {
            panic!("an untap needs no answer");
        };
        assert!(bound.is_empty() && snapshots.is_empty());
        assert_eq!(next, Next::Sibling);
        Ok(events
            .iter()
            .map(|event| match event {
                Event::PermanentUntapped { object } => {
                    game.object(*object).unwrap().card().name.clone()
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect())
    }

    #[rstest]
    #[case::a_tapped_name(r#"{"Untap":{"what":{"Each":[{"Named":"Wolf"}]}}}"#, &["Wolf"])]
    #[case::a_name_that_is_untapped(r#"{"Untap":{"what":"This"}}"#, &[])]
    #[case::an_untapped_name_of_another_seat(r#"{"Untap":{"what":{"Each":[{"Named":"Ogre"}]}}}"#, &[])]
    #[case::every_tapped_permanent_you_control(
        r#"{"Untap":{"what":{"Each":[{"ControlledBy":"You"}]}}}"#,
        &["Wolf", "Relic", "Idle"]
    )]
    #[case::every_tapped_creature(
        r#"{"Untap":{"what":{"Each":[{"Type":"Creature"}]}}}"#,
        &["Wolf", "Idle", "Bear"]
    )]
    #[case::of_one_opponent(
        r#"{"Untap":{"what":{"Each":[{"ControlledBy":"Opponents"}]}}}"#,
        &["Bear"]
    )]
    #[case::untapped_permanents_only(r#"{"Untap":{"what":{"Each":["Untapped"]}}}"#, &[])]
    #[case::a_player(r#"{"Untap":{"what":"You"}}"#, &[])]
    fn an_untap_untaps_the_tapped_permanents_it_names(#[case] json: &str, #[case] want: &[&str]) {
        assert_eq!(untaps(&game(), json).unwrap(), want);
    }

    #[test]
    fn a_name_that_changed_zones_or_repeats_untaps_once_at_most() {
        let game = game();
        let wolf = named(&game, "Wolf");
        let stale = ObjectRef {
            counter: wolf.counter + 1,
            ..wolf
        };
        // A bound name reaches the evaluator through the scope; build the scope with it.
        let source = named(&game, "Source");
        let resolving = Resolving::begin(source.id);
        let mut bindings = crate::resolve::Bindings::new();
        bindings.insert(
            "them".into(),
            vec![
                crate::choose::Target::Object(stale),
                crate::choose::Target::Object(wolf),
                crate::choose::Target::Object(wolf),
            ],
        );
        let mut scope = Scope::announcing(&game, ME, source);
        scope.bindings = &bindings;
        let cx = Cx {
            scope,
            resolving: &resolving,
        };
        let effect: Effect =
            serde_json::from_str(r#"{"Untap":{"what":{"Bound":"them"}}}"#).unwrap();
        let Run::Done { events, .. } = run(&cx, &effect).unwrap() else {
            panic!("done");
        };
        assert_eq!(events, [Event::PermanentUntapped { object: wolf.id }]);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert_eq!(
            untaps(&game(), r#"{"Tap":{"what":"This"}}"#).unwrap_err(),
            ResolveError::Invalid("the Untap primitive was given another effect".into())
        );
    }
}
