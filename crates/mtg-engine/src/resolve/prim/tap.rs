//! `Effect::Tap`: permanents are tapped (CR 701.26).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent the effect names that is still on the battlefield and untapped becomes tapped, all together, in
/// the order the reference lists them. A permanent that is already tapped, an object that is not a permanent or has
/// changed zones since it was named (CR 400.7), a player and a name that repeats are passed over, and naming nothing
/// taps nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Tap { what } = effect else {
        return Err(wrong_effect("Tap"));
    };
    set(cx, what, true)
}

/// Taps (`tapped`) or untaps the permanents `what` names that are not in that state yet; `Untap` is the same step in
/// the other direction.
pub(super) fn set(cx: &Cx<'_>, what: &Ref, tapped: bool) -> Result<Run, ResolveError> {
    let mut events = Vec::new();
    for object in cx.scope.live_objects(what)? {
        let changes = object.zone() == Zone::Battlefield
            && object
                .permanent()
                .is_some_and(|permanent| permanent.tapped() != tapped);
        if changes {
            let object = object.id();
            events.push(if tapped {
                Event::PermanentTapped { object }
            } else {
                Event::PermanentUntapped { object }
            });
        }
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a tap taps each untapped permanent it names, all together and in the order the reference lists them, whatever names them (the source, a name, a selector over the board, a chosen target, a bound name); permanents already tapped, cards in other zones, players, objects that changed zones since they were named and names that repeat are passed over; it changes nothing but the permanents, binds nothing and goes on to the next effect; and it refuses another effect by name.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::refs::{Bounds, Selector, TargetSpec, Targetable};
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::{Choices, Target};
    use crate::fixtures::{creature, setup};
    use crate::object::ObjectRef;
    use crate::resolve::{Bindings, Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn relic() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact))
    }

    /// Seat 0 controls the Source, a Wolf, a Relic and a tapped Idle and holds a Spell. Seat 1 controls a Bear and a
    /// tapped Ogre.
    fn game() -> GameState {
        let mut game = setup(
            vec![vec![creature("Spell", 1, 1)], vec![]],
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
        game.players[0].battlefield[3].tapped = true;
        game.players[1].battlefield[1].tapped = true;
        GameState::from_setup(&game).unwrap()
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|o| o.card().name == name)
            .map(crate::object::GameObject::reference)
            .unwrap()
    }

    fn a_creature() -> TargetSpec {
        TargetSpec {
            what: Targetable::Object(Selector(vec![mtg_cards::refs::Pred::Type(
                CardType::Creature,
            )])),
            bounds: Bounds::default(),
            divide: None,
        }
    }

    /// Runs `effect` as the Source's with the names bound and the target chosen, and says what it did.
    fn done(
        game: &GameState,
        bindings: &Bindings,
        chosen: Vec<Vec<Target>>,
        effect: &Effect,
        run: fn(&Cx<'_>, &Effect) -> Result<Run, ResolveError>,
    ) -> Result<Vec<String>, ResolveError> {
        let source = named(game, "Source");
        let resolving = Resolving::begin(source.id);
        let choices = Choices {
            targets: chosen,
            ..Choices::default()
        };
        let spec = a_creature();
        let mut scope = Scope::announcing(game, ME, source);
        scope.bindings = bindings;
        scope.choices = &choices;
        scope.specs = vec![&spec];
        let cx = Cx {
            scope,
            resolving: &resolving,
        };
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run(&cx, effect)?
        else {
            panic!("a tap needs no answer");
        };
        assert!(bound.is_empty() && snapshots.is_empty());
        assert_eq!(next, Next::Sibling);
        Ok(events
            .iter()
            .map(|event| match event {
                Event::PermanentTapped { object } => format!("tap {}", name(game, *object)),
                Event::PermanentUntapped { object } => format!("untap {}", name(game, *object)),
                other => panic!("unexpected {other:?}"),
            })
            .collect())
    }

    fn name(game: &GameState, id: mtg_core::ids::ObjectId) -> String {
        game.object(id).unwrap().card().name.clone()
    }

    fn taps(game: &GameState, json: &str) -> Vec<String> {
        done(game, &Bindings::new(), vec![], &effect(json), run).unwrap()
    }

    #[rstest]
    #[case::the_source(r#"{"Tap":{"what":"This"}}"#, &["tap Source"])]
    #[case::a_name(r#"{"Tap":{"what":{"Each":[{"Named":"Wolf"}]}}}"#, &["tap Wolf"])]
    #[case::a_name_that_is_tapped(r#"{"Tap":{"what":{"Each":[{"Named":"Idle"}]}}}"#, &[])]
    #[case::every_creature_that_is_untapped(
        r#"{"Tap":{"what":{"Each":[{"Type":"Creature"}]}}}"#,
        &["tap Source", "tap Wolf", "tap Bear"]
    )]
    #[case::every_untapped_permanent_of_one_seat(
        r#"{"Tap":{"what":{"Each":[{"ControlledBy":"Opponents"}]}}}"#,
        &["tap Bear"]
    )]
    #[case::every_permanent_you_control(
        r#"{"Tap":{"what":{"Each":[{"ControlledBy":"You"}]}}}"#,
        &["tap Source", "tap Wolf", "tap Relic"]
    )]
    #[case::permanents_that_are_tapped_already(
        r#"{"Tap":{"what":{"Each":["Tapped"]}}}"#,
        &[]
    )]
    #[case::a_card_in_a_hand(
        r#"{"Tap":{"what":{"Each":[{"Named":"Spell"},{"In":"Hand"}]}}}"#,
        &[]
    )]
    #[case::a_player(r#"{"Tap":{"what":"You"}}"#, &[])]
    #[case::the_players_of_the_table(r#"{"Tap":{"what":"EachPlayer"}}"#, &[])]
    fn a_tap_taps_the_untapped_permanents_it_names(#[case] json: &str, #[case] want: &[&str]) {
        assert_eq!(taps(&game(), json), want);
    }

    #[test]
    fn a_chosen_target_is_tapped_and_one_that_is_not_an_object_or_not_chosen_is_not() {
        let game = game();
        let wolf = Target::Object(named(&game, "Wolf"));
        let tap = effect(r#"{"Tap":{"what":{"Target":0}}}"#);
        let go = |chosen: Vec<Vec<Target>>| done(&game, &Bindings::new(), chosen, &tap, run);
        assert_eq!(go(vec![vec![wolf]]).unwrap(), ["tap Wolf"]);
        assert_eq!(
            go(vec![vec![Target::Player(ME)]]).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(go(vec![vec![]]).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_bound_name_is_tapped_unless_the_object_changed_zones_or_the_name_repeats() {
        let game = game();
        let wolf = named(&game, "Wolf");
        let bear = named(&game, "Bear");
        let stale = ObjectRef {
            counter: wolf.counter + 1,
            ..wolf
        };
        let tap = effect(r#"{"Tap":{"what":{"Bound":"them"}}}"#);
        let with = |names: Vec<Target>| {
            let mut bindings = Bindings::new();
            bindings.insert("them".into(), names);
            done(&game, &bindings, vec![], &tap, run).unwrap()
        };
        assert_eq!(
            with(vec![Target::Object(wolf), Target::Object(bear)]),
            ["tap Wolf", "tap Bear"]
        );
        assert_eq!(with(vec![Target::Object(stale)]), Vec::<String>::new());
        assert_eq!(
            with(vec![
                Target::Object(bear),
                Target::Object(bear),
                Target::Object(wolf)
            ]),
            ["tap Bear", "tap Wolf"]
        );
    }

    #[test]
    fn a_name_nothing_bound_is_an_error_not_an_empty_set() {
        let game = game();
        let tap = effect(r#"{"Tap":{"what":{"Bound":"ghost"}}}"#);
        assert!(matches!(
            done(&game, &Bindings::new(), vec![], &tap, run),
            Err(ResolveError::Invalid(_))
        ));
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let error = done(
            &game(),
            &Bindings::new(),
            vec![],
            &effect(r#"{"Draw":{}}"#),
            run,
        )
        .unwrap_err();
        assert_eq!(
            error,
            ResolveError::Invalid("the Tap primitive was given another effect".into())
        );
    }
}
