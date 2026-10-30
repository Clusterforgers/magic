//! `Effect::RemoveFromCombat`: creatures stop attacking or blocking (CR 506.4).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each creature the effect names that is still the object named, on the battlefield and in combat as an attacker or a
/// blocker leaves combat. A creature that is not in combat, an object that changed zones since it was named (CR 400.7)
/// and a name that repeats are passed over, and naming nothing removes nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::RemoveFromCombat { what } = effect else {
        return Err(wrong_effect("RemoveFromCombat"));
    };
    let combat = &cx.state().combat;
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield && combat.contains(object.id()))
        .map(|object| Event::RemovedFromCombat {
            object: object.id(),
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a removal from combat takes each creature it names that attacks or blocks out of the combat and no other: an attacker leaves and a blocker leaves its attacker's blockers while the attacker stays blocked, creatures outside combat, cards elsewhere and names that match nothing are passed over, and the effect refuses another effect by name.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::combat::{AttackTarget, Attacker};
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Seat 0 controls the Source and a Wolf (both attacking the foe) and an Idle creature; seat 1 controls a Bear that
    /// blocks the Source and an Ogre that blocks nothing.
    fn game() -> GameState {
        let setup = setup(
            vec![vec![creature("Spell", 1, 1)], vec![]],
            vec![
                vec![
                    creature("Source", 1, 1),
                    creature("Wolf", 2, 2),
                    creature("Idle", 1, 1),
                ],
                vec![creature("Bear", 2, 2), creature("Ogre", 3, 4)],
            ],
        );
        let mut game = GameState::from_setup(&setup).unwrap();
        let (source, wolf, bear) = (id(&game, "Source"), id(&game, "Wolf"), id(&game, "Bear"));
        for (object, blockers) in [(source, vec![bear]), (wolf, Vec::new())] {
            game.combat.attackers.push(Attacker {
                object,
                target: AttackTarget::Player(FOE),
                blocked: !blockers.is_empty(),
                blockers,
                assignment: None,
                target_left: false,
            });
        }
        game
    }

    fn id(game: &GameState, name: &str) -> ObjectId {
        game.objects()
            .find(|o| o.card().name == name)
            .map(crate::object::GameObject::id)
            .unwrap()
    }

    fn name(game: &GameState, object: ObjectId) -> String {
        game.object(object).unwrap().card().name.clone()
    }

    fn run_json(game: &GameState, json: &str) -> Result<Run, ResolveError> {
        let effect: Effect = serde_json::from_str(json).unwrap();
        let source = game
            .objects()
            .find(|o| o.card().name == "Source")
            .map(crate::object::GameObject::reference)
            .unwrap();
        let resolving = Resolving::begin(source.id);
        let cx = Cx {
            scope: Scope::announcing(game, ME, source),
            resolving: &resolving,
        };
        run(&cx, &effect)
    }

    /// The names of the creatures the effect removes, sorted.
    fn removed(game: &GameState, json: &str) -> Vec<String> {
        let Run::Done { events, next, .. } = run_json(game, json).unwrap() else {
            panic!("a removal needs no answer");
        };
        assert_eq!(next, Next::Sibling);
        let mut names: Vec<String> = events
            .iter()
            .map(|event| match event {
                Event::RemovedFromCombat { object } => name(game, *object),
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        names.sort();
        names
    }

    #[rstest]
    #[case::the_source(r#"{"RemoveFromCombat":{"what":"This"}}"#, &["Source"])]
    #[case::an_attacker_by_name(r#"{"RemoveFromCombat":{"what":{"Each":[{"Named":"Wolf"}]}}}"#, &["Wolf"])]
    #[case::a_blocker_by_name(r#"{"RemoveFromCombat":{"what":{"Each":[{"Named":"Bear"}]}}}"#, &["Bear"])]
    #[case::a_creature_outside_combat(r#"{"RemoveFromCombat":{"what":{"Each":[{"Named":"Idle"}]}}}"#, &[])]
    #[case::a_creature_of_the_foe_outside_combat(r#"{"RemoveFromCombat":{"what":{"Each":[{"Named":"Ogre"}]}}}"#, &[])]
    #[case::every_creature_in_combat(
        r#"{"RemoveFromCombat":{"what":{"Each":[{"Type":"Creature"}]}}}"#,
        &["Bear", "Source", "Wolf"]
    )]
    #[case::every_creature_of_one_seat(
        r#"{"RemoveFromCombat":{"what":{"Each":[{"ControlledBy":"Opponents"}]}}}"#,
        &["Bear"]
    )]
    #[case::a_card_in_a_hand(r#"{"RemoveFromCombat":{"what":{"Each":[{"Named":"Spell"},{"In":"Hand"}]}}}"#, &[])]
    #[case::a_player(r#"{"RemoveFromCombat":{"what":"You"}}"#, &[])]
    fn a_removal_names_the_creatures_that_are_in_combat(#[case] json: &str, #[case] want: &[&str]) {
        assert_eq!(removed(&game(), json), want);
    }

    #[rstest]
    #[case::an_attacker("Source", &["Wolf"], &[])]
    #[case::an_attacker_that_was_not_blocked("Wolf", &["Source"], &["Bear"])]
    #[case::a_blocker("Bear", &["Source", "Wolf"], &[])]
    #[case::a_creature_outside_combat("Idle", &["Source", "Wolf"], &["Bear"])]
    fn the_fold_takes_the_creature_out_of_the_combat_only(
        #[case] leaving: &str,
        #[case] attackers: &[&str],
        #[case] blockers_of_source: &[&str],
    ) {
        let mut game = game();
        let object = id(&game, leaving);
        if game.combat.contains(object) {
            apply_event(&mut game, &Event::RemovedFromCombat { object }).unwrap();
        }
        let left: Vec<String> = game
            .combat
            .attackers()
            .iter()
            .map(|a| name(&game, a.object()))
            .collect();
        assert_eq!(left, attackers);
        let source = game
            .combat
            .attackers()
            .iter()
            .find(|a| name(&game, a.object()) == "Source");
        if let Some(source) = source {
            let blockers: Vec<String> = source.blockers().iter().map(|b| name(&game, *b)).collect();
            assert_eq!(blockers, blockers_of_source);
            // A creature stays blocked when every creature blocking it leaves combat (CR 509.1h).
            assert!(source.blocked());
        }
    }

    #[test]
    fn it_refuses_another_effect_by_name() {
        assert!(run_json(&game(), r#"{"Tap":{"what":"This"}}"#).is_err());
    }
}
