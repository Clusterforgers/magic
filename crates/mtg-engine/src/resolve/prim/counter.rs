//! `Effect::Counter`: spells and abilities on the stack are countered (CR 701.6).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::{Event, ResolutionOutcome};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::snapshot::snapshot;
use crate::zones::Zone;

/// Each spell or ability on the stack the effect names is countered, all together: it leaves the stack and does
/// nothing. The fold sends a spell to its owner's graveyard and an ability ceases to exist, and triggers see the end of
/// a countered spell as a move from the stack with `ZoneCause::Countered`. The step carries what each was. The
/// resolving spell, an object that is not on the stack, one that has left it since it was named and a name that
/// repeats are passed over.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Counter { what } = effect else {
        return Err(wrong_effect("Counter"));
    };
    let state = cx.state();
    let mut events = Vec::new();
    let mut snapshots = Vec::new();
    for object in cx.scope.live_objects(what)? {
        if object.zone() != Zone::Stack || object.id() == cx.resolving.object() {
            continue;
        }
        snapshots.extend(snapshot(state, object.id()));
        events.push(Event::ResolutionEnded {
            object: object.id(),
            outcome: ResolutionOutcome::Countered,
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
    //! Goal: a counter ends each spell or ability on the stack that it names, all together, as countered and as it was; the resolving spell, objects that are not on the stack, names of objects that have left it and names that repeat are passed over; a chosen target is countered while it is a legal target, and once the events are folded a spell is in its owner's graveyard, an ability has ceased to exist and nothing else on the stack is disturbed.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::{Ability, Timing};
    use mtg_cards::cost::Cost;
    use mtg_cards::refs::{Bounds, Selector, TargetSpec, Targetable, ZoneKind};
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

    fn instant(name: &str) -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new(name).with_type(CardType::Instant))
    }

    fn totem() -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new("Totem")
                .with_type(CardType::Artifact)
                .with_rules(Ability::Activated {
                    cost: Cost(Vec::new()),
                    timing: Timing::Instant,
                    limit: None,
                    from: ZoneKind::Battlefield,
                    targets: Vec::new(),
                    effects: Vec::new(),
                }),
        )
    }

    /// Seat 0 holds Bolt, Shock and the Counter and controls a Totem; seat 1 holds a Charm and controls a Bear. Bolt
    /// (seat 0), then the Charm (seat 1), then the Totem's ability (seat 0), then Shock (seat 0) and last the Counter
    /// (seat 0) are on the stack.
    fn stacked() -> GameState {
        let mut game = state(
            vec![
                vec![instant("Bolt"), instant("Shock"), instant("Counter")],
                vec![instant("Charm")],
            ],
            vec![vec![totem()], vec![creature("Bear", 2, 2)]],
        );
        let cast = |game: &mut GameState, player: PlayerId, name: &str| {
            let object = named(game, name);
            apply_event(
                game,
                &Event::SpellCast {
                    player,
                    object: object.id,
                    choices: Choices::default(),
                },
            )
            .unwrap();
        };
        cast(&mut game, ME, "Bolt");
        cast(&mut game, FOE, "Charm");
        let totem = named(&game, "Totem");
        let put = PutOnStack {
            object: game.next_object(),
            source: totem,
            snapshot: crate::snapshot::snapshot(&game, totem.id).unwrap(),
            ability: 0,
            controller: ME,
            choices: Choices::default(),
            cause: None,
            delayed: None,
        };
        apply_event(&mut game, &Event::AbilityPutOnStack(Box::new(put))).unwrap();
        cast(&mut game, ME, "Shock");
        cast(&mut game, ME, "Counter");
        game
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

    /// The ability of the Totem, which is the only ability object.
    fn ability(game: &GameState) -> ObjectRef {
        game.objects()
            .find(|o| o.ability().is_some())
            .map(crate::object::GameObject::reference)
            .unwrap()
    }

    /// Runs `effect` as the Counter's resolution.
    fn run_with(
        game: &GameState,
        bindings: &Bindings,
        chosen: Vec<Vec<Target>>,
        specs: &[TargetSpec],
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let counter = named(game, "Counter");
        let resolving = Resolving::begin(counter.id);
        let choices = Choices {
            targets: chosen,
            ..Choices::default()
        };
        let mut scope = Scope::announcing(game, ME, counter);
        scope.bindings = bindings;
        scope.choices = &choices;
        scope.specs = specs.iter().collect();
        let cx = Cx {
            scope,
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    fn counter_each(game: &GameState, filter: &str) -> Run {
        let counter = effect(&format!(r#"{{"Counter":{{"what":{{"Each":{filter}}}}}}}"#));
        run_with(game, &Bindings::new(), Vec::new(), &[], &counter).unwrap()
    }

    /// The objects a finished run counters, and the objects its snapshots describe.
    fn countered(run: Run) -> (Vec<ObjectId>, Vec<ObjectId>) {
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
        let ended = events
            .into_iter()
            .map(|event| match event {
                Event::ResolutionEnded {
                    object,
                    outcome: ResolutionOutcome::Countered,
                } => object,
                other => panic!("only counters were expected, got {other:?}"),
            })
            .collect();
        (ended, snapshots.into_iter().map(|shot| shot.id).collect())
    }

    fn names(game: &GameState, ids: &[ObjectId]) -> Vec<String> {
        ids.iter()
            .map(|id| game.object(*id).unwrap().card().name.clone())
            .collect()
    }

    #[rstest]
    #[case::every_spell(r#"["Spell"]"#, &["Bolt", "Shock", "Charm"])]
    #[case::their_spells(r#"["Spell",{"ControlledBy":"Opponents"}]"#, &["Charm"])]
    #[case::my_spells(r#"["Spell",{"ControlledBy":"You"}]"#, &["Bolt", "Shock"])]
    #[case::a_spell_by_name(r#"["Spell",{"Named":"Shock"}]"#, &["Shock"])]
    #[case::everything_on_the_stack_but_the_one_resolving(
        r#"[{"In":"Stack"}]"#,
        &["Bolt", "Shock", "Charm", "Totem"]
    )]
    #[case::no_such_spell(r#"["Spell",{"Named":"Bear"}]"#, &[])]
    #[case::creatures_are_not_on_the_stack(r#"[{"Type":"Creature"}]"#, &[])]
    fn all_counters_the_spells_that_match_but_not_the_one_resolving(
        #[case] filter: &str,
        #[case] expected: &[&str],
    ) {
        let game = stacked();
        let (ended, shots) = countered(counter_each(&game, filter));
        assert_eq!(names(&game, &ended), expected);
        assert_eq!(shots.len(), ended.len(), "each is kept as it was");
        let counter = named(&game, "Counter");
        assert!(
            !ended.contains(&counter.id),
            "the Counter does not counter itself"
        );
    }

    #[test]
    fn the_snapshot_is_the_spell_as_it_was_on_the_stack_with_its_controller() {
        let game = stacked();
        let Run::Done { snapshots, .. } = counter_each(&game, r#"["Spell",{"Named":"Charm"}]"#)
        else {
            panic!("the effect is done");
        };
        let charm = named(&game, "Charm");
        let shot = &snapshots[0];
        assert_eq!((shot.reference(), shot.zone), (charm, Zone::Stack));
        assert_eq!((shot.controller, shot.owner), (FOE, FOE));
    }

    #[test]
    fn a_counter_by_a_chosen_target_ends_it_and_folding_sends_the_spell_to_its_graveyard() {
        let mut game = stacked();
        let (bolt, charm) = (named(&game, "Bolt"), named(&game, "Charm"));
        let spec = TargetSpec {
            what: Targetable::Object(Selector(vec![mtg_cards::refs::Pred::Spell])),
            bounds: Bounds::default(),
            divide: None,
        };
        let counter = effect(r#"{"Counter":{"what":{"Target":0}}}"#);
        let run = run_with(
            &game,
            &Bindings::new(),
            vec![vec![Target::Object(charm)]],
            &[spec],
            &counter,
        )
        .unwrap();
        let Run::Done { events, .. } = run else {
            panic!("the effect is done");
        };
        let before = game.stack().to_vec();
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        assert_eq!(game.object(charm.id).unwrap().zone(), Zone::Graveyard(FOE));
        assert_eq!(game.object(charm.id).unwrap().counter(), charm.counter + 1);
        let expected: Vec<ObjectId> = before.into_iter().filter(|id| *id != charm.id).collect();
        assert_eq!(game.stack(), expected, "the rest of the stack is as it was");
        assert!(game.stack().contains(&bolt.id));
    }

    #[test]
    fn countering_an_ability_makes_it_cease_to_exist() {
        let mut game = stacked();
        let totem_ability = ability(&game);
        let bindings: Bindings =
            [("ability".to_owned(), vec![Target::Object(totem_ability)])].into();
        let counter = effect(r#"{"Counter":{"what":{"Bound":"ability"}}}"#);
        let run = run_with(&game, &bindings, Vec::new(), &[], &counter).unwrap();
        let Run::Done { events, .. } = run else {
            panic!("the effect is done");
        };
        assert_eq!(
            events,
            [Event::ResolutionEnded {
                object: totem_ability.id,
                outcome: ResolutionOutcome::Countered
            }]
        );
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        assert!(game.object(totem_ability.id).is_none());
        assert!(!game.stack().contains(&totem_ability.id));
    }

    #[test]
    fn what_has_left_the_stack_or_never_was_on_it_or_repeats_is_passed_over() {
        let mut game = stacked();
        let (bolt, shock, bear, charm) = (
            named(&game, "Bolt"),
            named(&game, "Shock"),
            named(&game, "Bear"),
            named(&game, "Charm"),
        );
        let bindings: Bindings = [(
            "them".to_owned(),
            vec![
                Target::Object(bolt),
                Target::Object(shock),
                Target::Object(shock),
                Target::Object(bear),
                Target::Object(charm),
                Target::Player(FOE),
            ],
        )]
        .into();
        // The Charm was countered by something else before the effect ran: its name describes a card in a graveyard.
        apply_event(
            &mut game,
            &Event::ResolutionEnded {
                object: charm.id,
                outcome: ResolutionOutcome::Countered,
            },
        )
        .unwrap();
        let counter = effect(r#"{"Counter":{"what":{"Bound":"them"}}}"#);
        let run = run_with(&game, &bindings, Vec::new(), &[], &counter).unwrap();
        assert_eq!(
            countered(run).0,
            [bolt.id, shock.id],
            "Shock once; the Bear is no spell; the Charm left; a player is no object"
        );
    }

    #[rstest]
    #[case::a_legal_target(r#"["Spell"]"#, true)]
    #[case::a_target_that_is_not_what_the_spec_asks_for(r#"["Spell",{"Named":"Bolt"}]"#, false)]
    fn a_target_is_countered_only_while_it_is_legal(
        #[case] spec: &str,
        #[case] countered_it: bool,
    ) {
        let game = stacked();
        let charm = named(&game, "Charm");
        let spec = TargetSpec {
            what: Targetable::Object(Selector(serde_json::from_str(spec).unwrap())),
            bounds: Bounds::default(),
            divide: None,
        };
        let counter = effect(r#"{"Counter":{"what":{"Target":0}}}"#);
        let run = run_with(
            &game,
            &Bindings::new(),
            vec![vec![Target::Object(charm)]],
            &[spec],
            &counter,
        )
        .unwrap();
        assert_eq!(
            countered(run).0,
            if countered_it {
                vec![charm.id]
            } else {
                Vec::new()
            }
        );
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let game = stacked();
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
