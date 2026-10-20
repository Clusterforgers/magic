//! `Effect::Destroy`: permanents are destroyed (CR 701.8).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::{Event, ZoneCause};
use crate::keywords;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::snapshot::snapshot;
use crate::zones::Zone;

/// Each permanent the effect names that is still on the battlefield goes to its owner's graveyard, all together, unless
/// it is indestructible (CR 702.12b). Every move carries `ZoneCause::Destroyed` and the step carries what the permanent
/// was. An object that is not a permanent, has changed zones since it was named or is named twice is passed over, and
/// naming nothing destroys nothing. `regenerate` is read once an effect can make a regeneration shield.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Destroy { what, .. } = effect else {
        return Err(wrong_effect("Destroy"));
    };
    let state = cx.state();
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    for object in cx.scope.live_objects(what)? {
        if object.zone() != Zone::Battlefield || keywords::is_indestructible(state, object.id()) {
            continue;
        }
        snapshots.extend(snapshot(state, object.id()));
        events.push(Event::ZoneChanged {
            object: object.id(),
            from: Zone::Battlefield,
            to: Zone::Graveyard(object.owner()),
            cause: ZoneCause::Destroyed,
        });
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots,
        next: Next::Sibling,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a destroy moves each permanent it names that is still on the battlefield to its owner's graveyard, all together, with the cause and as the permanent was; indestructible permanents, objects that are not permanents, names of objects that have changed zones and names that repeat are passed over; a selector over everything, a filter and a chosen target name the same permanents the same way, and a target that is no longer legal is not destroyed.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_cards::keyword::Keyword;
    use mtg_cards::refs::{Bounds, Selector, TargetSpec, Targetable};
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::choose::{Choices, Target};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::object::ObjectRef;
    use crate::resolve::{Bindings, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn permanent(name: &str, card_type: CardType) -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new(name).with_type(card_type))
    }

    fn titan() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Titan")
                .with_type(CardType::Creature)
                .with_power_toughness(5, 5)
                .with_rules(Ability::Keyword(Keyword::Indestructible)),
        )
    }

    /// Seat 0 controls the Source, a Wolf and a Relic and holds a Spell. Seat 1 controls a Bear, an Ogre, a Walker and
    /// an indestructible Titan.
    fn game() -> GameState {
        state(
            vec![vec![creature("Spell", 1, 1)], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    creature("Wolf", 2, 2),
                    permanent("Relic", CardType::Artifact),
                ],
                vec![
                    creature("Bear", 2, 2),
                    creature("Ogre", 3, 4),
                    permanent("Walker", CardType::Planeswalker),
                    titan(),
                ],
            ],
        )
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

    fn on(spec: &str) -> TargetSpec {
        TargetSpec {
            what: Targetable::Object(Selector(serde_json::from_str(spec).unwrap())),
            bounds: Bounds::default(),
            divide: None,
        }
    }

    /// Runs `effect` as the Source's, with the names bound and the targets chosen for the specifications.
    fn run_with(
        game: &GameState,
        bindings: &Bindings,
        chosen: Vec<Vec<Target>>,
        specs: &[TargetSpec],
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = named(game, "Source");
        let resolving = Resolving::begin(source.id);
        let choices = Choices {
            targets: chosen,
            ..Choices::default()
        };
        let mut scope = Scope::announcing(game, ME, source);
        scope.bindings = bindings;
        scope.choices = &choices;
        scope.specs = specs.iter().collect();
        let cx = Cx {
            scope,
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    fn destroy_each(game: &GameState, filter: &str) -> Run {
        let destroy = effect(&format!(
            r#"{{"Destroy":{{"what":{{"Each":{filter}}},"regenerate":false}}}}"#
        ));
        run_with(game, &Bindings::new(), Vec::new(), &[], &destroy).unwrap()
    }

    /// The moves a finished run proposes, as (permanent, where it goes), and the objects its snapshots describe.
    fn destroyed(run: Run) -> (Vec<(ObjectId, Zone)>, Vec<ObjectId>) {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the effect is done");
        };
        assert!(bound.is_empty() && next == Next::Sibling);
        let moves = events
            .into_iter()
            .map(|event| match event {
                Event::ZoneChanged {
                    object,
                    from: Zone::Battlefield,
                    to,
                    cause: ZoneCause::Destroyed,
                } => (object, to),
                other => panic!("only destructions were expected, got {other:?}"),
            })
            .collect();
        (moves, snapshots.into_iter().map(|shot| shot.id).collect())
    }

    fn names(game: &GameState, ids: &[ObjectId]) -> Vec<String> {
        ids.iter()
            .map(|id| game.object(*id).unwrap().card().name.clone())
            .collect()
    }

    #[rstest]
    #[case::every_creature(
        r#"[{"Type":"Creature"}]"#,
        &["Source", "Wolf", "Bear", "Ogre"]
    )]
    #[case::theirs_only(r#"[{"ControlledBy":"Opponents"}]"#, &["Bear", "Ogre", "Walker"])]
    #[case::mine_only(r#"[{"ControlledBy":"You"}]"#, &["Source", "Wolf", "Relic"])]
    #[case::an_artifact(r#"[{"Type":"Artifact"}]"#, &["Relic"])]
    #[case::not_a_creature(r#"[{"Not":{"Type":"Creature"}}]"#, &["Relic", "Walker"])]
    #[case::by_name(r#"[{"Named":"Ogre"}]"#, &["Ogre"])]
    #[case::big_ones(r#"[{"Compare":{"stat":"Power","op":"Ge","to":3}}]"#, &["Ogre"])]
    #[case::nothing_matches(r#"[{"Type":"Land"}]"#, &[])]
    #[case::everything_on_the_battlefield("[]", &["Source", "Wolf", "Relic", "Bear", "Ogre", "Walker"])]
    #[case::cards_in_hand_are_not_permanents(r#"[{"In":"Hand"}]"#, &[])]
    fn all_selects_the_permanents_that_match_and_the_indestructible_one_stays(
        #[case] filter: &str,
        #[case] expected: &[&str],
    ) {
        let game = game();
        let (moves, shots) = destroyed(destroy_each(&game, filter));
        let goes: Vec<ObjectId> = moves.iter().map(|(object, _)| *object).collect();
        assert_eq!(names(&game, &goes), expected);
        assert_eq!(shots, goes, "each permanent is kept as it was");
        for (object, to) in moves {
            let owner = game.object(object).unwrap().owner();
            assert_eq!(to, Zone::Graveyard(owner), "to its owner's graveyard");
        }
    }

    #[test]
    fn the_titan_is_named_and_nothing_happens_to_it() {
        let game = game();
        let (moves, shots) = destroyed(destroy_each(&game, r#"[{"Named":"Titan"}]"#));
        assert_eq!((moves, shots), (Vec::new(), Vec::new()));
    }

    #[test]
    fn a_permanent_goes_to_its_owners_graveyard_not_its_controllers() {
        let mut game = game();
        let bear = named(&game, "Bear");
        game.objects.get_mut(&bear.id).unwrap().controller = ME;
        let (moves, _) = destroyed(destroy_each(&game, r#"[{"Named":"Bear"}]"#));
        assert_eq!(moves, [(bear.id, Zone::Graveyard(FOE))]);
    }

    #[test]
    fn the_snapshot_is_the_permanent_as_it_was_with_its_controller() {
        let mut game = game();
        let bear = named(&game, "Bear");
        game.objects.get_mut(&bear.id).unwrap().controller = ME;
        let run = destroy_each(&game, r#"[{"Named":"Bear"}]"#);
        let Run::Done { snapshots, .. } = run else {
            panic!("the effect is done");
        };
        let shot = &snapshots[0];
        assert_eq!((shot.reference(), shot.zone), (bear, Zone::Battlefield));
        assert_eq!((shot.controller, shot.owner), (ME, FOE));
        assert_eq!(shot.characteristics.power, Some(2));
    }

    #[test]
    fn a_name_of_an_object_that_changed_zones_or_repeats_is_passed_over() {
        let mut game = game();
        let (bear, ogre) = (named(&game, "Bear"), named(&game, "Ogre"));
        let bindings: Bindings = [
            (
                "gone".to_owned(),
                vec![
                    Target::Object(bear),
                    Target::Object(ogre),
                    Target::Object(ogre),
                ],
            ),
            ("players".to_owned(), vec![Target::Player(FOE)]),
        ]
        .into();
        // The Bear left before the effect ran: the name now describes an object that is somewhere else.
        apply_event(
            &mut game,
            &Event::ZoneChanged {
                object: bear.id,
                from: Zone::Battlefield,
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            },
        )
        .unwrap();
        let destroy = effect(r#"{"Destroy":{"what":{"Bound":"gone"}}}"#);
        let run = run_with(&game, &bindings, Vec::new(), &[], &destroy).unwrap();
        assert_eq!(destroyed(run).0, [(ogre.id, Zone::Graveyard(FOE))]);
        // A player is not an object, and naming only players destroys nothing.
        let players = effect(r#"{"Destroy":{"what":{"Bound":"players"}}}"#);
        let run = run_with(&game, &bindings, Vec::new(), &[], &players).unwrap();
        assert_eq!(destroyed(run).0, []);
    }

    #[rstest]
    #[case::a_legal_target(r#"[{"Type":"Creature"}]"#, true)]
    #[case::a_target_that_is_not_what_the_spec_asks_for(r#"[{"Type":"Land"}]"#, false)]
    fn a_target_is_destroyed_only_while_it_is_legal(
        #[case] spec: &str,
        #[case] destroyed_it: bool,
    ) {
        let game = game();
        let ogre = named(&game, "Ogre");
        let destroy = effect(r#"{"Destroy":{"what":{"Target":0}}}"#);
        let run = run_with(
            &game,
            &Bindings::new(),
            vec![vec![Target::Object(ogre)]],
            &[on(spec)],
            &destroy,
        )
        .unwrap();
        assert_eq!(
            destroyed(run).0,
            if destroyed_it {
                vec![(ogre.id, Zone::Graveyard(FOE))]
            } else {
                Vec::new()
            }
        );
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let game = game();
        let refused = run_with(
            &game,
            &Bindings::new(),
            Vec::new(),
            &[],
            &effect(r#"{"Draw":{}}"#),
        );
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
