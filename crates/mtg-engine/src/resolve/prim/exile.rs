//! `Effect::Exile`: objects are exiled (CR 406, 701.13).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::{Event, ZoneCause};
use crate::object::ObjectKind;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::snapshot::snapshot;
use crate::zones::Zone;

/// Each object the effect names that is still where it was named moves to exile, all together, from whichever zone it
/// is in. Every move carries `ZoneCause::Exiled` and the step carries what the object was. Indestructible does not
/// matter. An ability, which is not an object that changes zones, the spell that is resolving, an object already in
/// exile, one that has changed zones since it was named and a name that repeats are passed over.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Exile { what } = effect else {
        return Err(wrong_effect("Exile"));
    };
    let state = cx.state();
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    for object in cx.scope.live_objects(what)? {
        let resolving_spell = object.zone() == Zone::Stack && object.id() == cx.resolving.object();
        if object.kind() == ObjectKind::Ability || resolving_spell || object.zone() == Zone::Exile {
            continue;
        }
        snapshots.extend(snapshot(state, object.id()));
        events.push(Event::ZoneChanged {
            object: object.id(),
            from: object.zone(),
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
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
    //! Goal: an exile moves each object it names that is still where it was named to exile, from the battlefield, a graveyard, a hand, a library or the stack, all together, with the cause and as the object was, whether or not it is indestructible; abilities, the resolving spell, objects already exiled, names of objects that have changed zones and names that repeat are passed over; a chosen target is exiled while it is legal and the steps after it still read it as it was.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_cards::keyword::Keyword;
    use mtg_cards::refs::{Bounds, Selector, TargetSpec, Targetable};
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::abilities::PutOnStack;
    use crate::choose::{Choices, Target};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::object::ObjectRef;
    use crate::resolve::{Bindings, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn titan() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Titan")
                .with_type(CardType::Creature)
                .with_power_toughness(5, 5)
                .with_rules(Ability::Keyword(Keyword::Indestructible)),
        )
    }

    fn totem() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Totem")
                .with_type(CardType::Artifact)
                .with_rules(Ability::Activated {
                    cost: mtg_cards::cost::Cost(Vec::new()),
                    timing: mtg_cards::ability::Timing::Instant,
                    limit: None,
                    from: mtg_cards::refs::ZoneKind::Battlefield,
                    targets: Vec::new(),
                    effects: Vec::new(),
                }),
        )
    }

    /// Seat 0 controls the Source, a Wolf and a Totem and holds a Spell and a Ritual; seat 1 controls a Bear, an Ogre and
    /// an indestructible Titan and holds a Charm.
    fn game() -> GameState {
        state(
            vec![
                vec![creature("Spell", 1, 1), creature("Ritual", 1, 1)],
                vec![creature("Charm", 1, 1)],
            ],
            vec![
                vec![creature("Source", 1, 1), creature("Wolf", 2, 2), totem()],
                vec![creature("Bear", 2, 2), creature("Ogre", 3, 4), titan()],
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

    /// Runs `effect` as the Source's resolution, or as `resolving`'s when it is given.
    fn run_with(
        game: &GameState,
        resolving: Option<ObjectRef>,
        bindings: &Bindings,
        chosen: Vec<Vec<Target>>,
        specs: &[TargetSpec],
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = resolving.unwrap_or_else(|| named(game, "Source"));
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

    fn exile_each(game: &GameState, filter: &str) -> Run {
        let exile = effect(&format!(r#"{{"Exile":{{"what":{{"Each":{filter}}}}}}}"#));
        run_with(game, None, &Bindings::new(), Vec::new(), &[], &exile).unwrap()
    }

    /// The moves a finished run proposes, as (object, from, to), and the objects its snapshots describe.
    fn exiled(run: Run) -> (Vec<(ObjectId, Zone, Zone)>, Vec<ObjectId>) {
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
                    from,
                    to,
                    cause: ZoneCause::Exiled,
                } => (object, from, to),
                other => panic!("only exiles were expected, got {other:?}"),
            })
            .collect();
        (moves, snapshots.into_iter().map(|shot| shot.id).collect())
    }

    fn names(game: &GameState, moves: &[(ObjectId, Zone, Zone)]) -> Vec<String> {
        moves
            .iter()
            .map(|(id, ..)| game.object(*id).unwrap().card().name.clone())
            .collect()
    }

    #[rstest]
    #[case::every_creature(
        r#"[{"Type":"Creature"}]"#,
        &["Source", "Wolf", "Bear", "Ogre", "Titan"]
    )]
    #[case::theirs_only(r#"[{"ControlledBy":"Opponents"}]"#, &["Bear", "Ogre", "Titan"])]
    #[case::an_artifact(r#"[{"Type":"Artifact"}]"#, &["Totem"])]
    #[case::by_name(r#"[{"Named":"Titan"}]"#, &["Titan"])]
    #[case::nothing_matches(r#"[{"Type":"Land"}]"#, &[])]
    #[case::hands(r#"[{"In":"Hand"}]"#, &["Spell", "Ritual", "Charm"])]
    #[case::a_hand_of_one_owner(r#"[{"In":"Hand"},{"OwnedBy":"Opponents"}]"#, &["Charm"])]
    #[case::libraries_of_the_table(r#"[{"In":"Library"},{"Named":"Library 0"}]"#, &["Library 0", "Library 0"])]
    fn all_selects_what_matches_wherever_it_is_and_indestructible_does_not_matter(
        #[case] filter: &str,
        #[case] expected: &[&str],
    ) {
        let game = game();
        let (moves, shots) = exiled(exile_each(&game, filter));
        assert_eq!(names(&game, &moves), expected);
        for (object, from, to) in &moves {
            assert_eq!(*from, game.object(*object).unwrap().zone());
            assert_eq!(*to, Zone::Exile);
        }
        assert_eq!(
            shots,
            moves.iter().map(|(object, ..)| *object).collect::<Vec<_>>(),
            "each object is kept as it was"
        );
    }

    #[test]
    fn a_graveyard_card_goes_from_the_graveyard() {
        let mut game = game();
        let bear = named(&game, "Bear");
        apply_event(
            &mut game,
            &Event::CreatureDied {
                object: bear.id,
                cause: crate::events::DeathCause::LethalDamage,
            },
        )
        .unwrap();
        let (moves, _) = exiled(exile_each(&game, r#"[{"In":"Graveyard"}]"#));
        assert_eq!(moves, [(bear.id, Zone::Graveyard(FOE), Zone::Exile)]);
    }

    #[test]
    fn a_spell_on_the_stack_is_exiled_and_the_resolving_one_is_not() {
        let mut game = game();
        let (spell, ritual) = (named(&game, "Spell"), named(&game, "Ritual"));
        for object in [spell, ritual] {
            apply_event(
                &mut game,
                &Event::SpellCast {
                    player: ME,
                    object: object.id,
                    choices: Choices::default(),
                },
            )
            .unwrap();
        }
        // The Ritual is on top, so it is the one resolving and the one that names the spells on the stack.
        let ritual = game.object(ritual.id).unwrap().reference();
        let exile = effect(r#"{"Exile":{"what":{"Each":["Spell"]}}}"#);
        let run = run_with(
            &game,
            Some(ritual),
            &Bindings::new(),
            Vec::new(),
            &[],
            &exile,
        )
        .unwrap();
        let (moves, shots) = exiled(run);
        assert_eq!(moves, [(spell.id, Zone::Stack, Zone::Exile)]);
        assert_eq!(shots, [spell.id]);
    }

    #[test]
    fn an_ability_on_the_stack_does_not_change_zones() {
        let mut game = game();
        let totem = named(&game, "Totem");
        let ability = game.next_object();
        let put = PutOnStack {
            object: ability,
            source: totem,
            snapshot: crate::snapshot::snapshot(&game, totem.id).unwrap(),
            ability: 0,
            controller: ME,
            choices: Choices::default(),
            cause: None,
            delayed: None,
        };
        apply_event(&mut game, &Event::AbilityPutOnStack(Box::new(put))).unwrap();
        let named_ability: Bindings = [(
            "ability".to_owned(),
            vec![Target::Object(game.object(ability).unwrap().reference())],
        )]
        .into();
        let exile = effect(r#"{"Exile":{"what":{"Bound":"ability"}}}"#);
        let run = run_with(&game, None, &named_ability, Vec::new(), &[], &exile).unwrap();
        assert_eq!(exiled(run).0, []);
        let (moves, _) = exiled(exile_each(&game, r#"[{"In":"Stack"}]"#));
        assert_eq!(moves, []);
        let (moves, _) = exiled(exile_each(&game, r#"[{"Type":"Artifact"}]"#));
        assert_eq!(names(&game, &moves), ["Totem"], "its source is exiled");
    }

    #[test]
    fn what_is_already_exiled_or_has_changed_zones_or_repeats_is_passed_over() {
        let mut game = game();
        let (bear, ogre, wolf) = (
            named(&game, "Bear"),
            named(&game, "Ogre"),
            named(&game, "Wolf"),
        );
        let bindings: Bindings = [(
            "them".to_owned(),
            vec![
                Target::Object(bear),
                Target::Object(ogre),
                Target::Object(ogre),
                Target::Object(wolf),
                Target::Player(FOE),
            ],
        )]
        .into();
        for object in [bear, wolf] {
            apply_event(
                &mut game,
                &Event::ZoneChanged {
                    object: object.id,
                    from: Zone::Battlefield,
                    to: Zone::Exile,
                    cause: ZoneCause::Exiled,
                },
            )
            .unwrap();
        }
        // The Wolf is exiled again under its new counter: it is a new object that nothing named.
        let exile = effect(r#"{"Exile":{"what":{"Bound":"them"}}}"#);
        let run = run_with(&game, None, &bindings, Vec::new(), &[], &exile).unwrap();
        assert_eq!(
            exiled(run).0,
            [(ogre.id, Zone::Battlefield, Zone::Exile)],
            "the Ogre once; the Bear and the Wolf left; the player is no object"
        );
        // What is exiled stays: naming the exiled cards by selector moves nothing.
        let (moves, _) = exiled(exile_each(&game, r#"[{"In":"Exile"}]"#));
        assert_eq!(moves, []);
    }

    #[rstest]
    #[case::a_legal_target(r#"[{"Type":"Creature"}]"#, true)]
    #[case::a_target_that_is_not_what_the_spec_asks_for(r#"[{"Type":"Artifact"}]"#, false)]
    fn a_target_is_exiled_only_while_it_is_legal(#[case] spec: &str, #[case] exiled_it: bool) {
        let game = game();
        let ogre = named(&game, "Ogre");
        let exile = effect(r#"{"Exile":{"what":{"Target":0}}}"#);
        let run = run_with(
            &game,
            None,
            &Bindings::new(),
            vec![vec![Target::Object(ogre)]],
            &[on(spec)],
            &exile,
        )
        .unwrap();
        assert_eq!(
            exiled(run).0,
            if exiled_it {
                vec![(ogre.id, Zone::Battlefield, Zone::Exile)]
            } else {
                Vec::new()
            }
        );
    }

    #[test]
    fn the_snapshot_is_the_object_as_it_was_with_its_controller() {
        let mut game = game();
        let bear = named(&game, "Bear");
        game.objects.get_mut(&bear.id).unwrap().controller = ME;
        let Run::Done { snapshots, .. } = exile_each(&game, r#"[{"Named":"Bear"}]"#) else {
            panic!("the effect is done");
        };
        let shot = &snapshots[0];
        assert_eq!((shot.reference(), shot.zone), (bear, Zone::Battlefield));
        assert_eq!((shot.controller, shot.owner), (ME, FOE));
        assert_eq!(shot.characteristics.power, Some(2));
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let game = game();
        let refused = run_with(
            &game,
            None,
            &Bindings::new(),
            Vec::new(),
            &[],
            &effect(r#"{"Draw":{}}"#),
        );
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
