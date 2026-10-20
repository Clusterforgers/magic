//! `Effect::Continuous`: a continuous effect begins (CR 611.2).

use mtg_cards::effect::Effect;
use mtg_cards::modify::Modification as Wanted;
use mtg_cards::value::Amount;
use mtg_core::ids::EffectId;

use super::wrong_effect;
use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each modification becomes one effect that applies to the permanents the reference names now and to no other object,
/// whatever happens to the set later (CR 611.2c). The effects take consecutive timestamps in the order of the card's
/// text (CR 613.7), and their amounts are worked out now. Naming no permanent begins nothing. A modification the engine
/// does not run refuses by name.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Continuous {
        affected,
        mods,
        duration,
    } = effect
    else {
        return Err(wrong_effect("Continuous"));
    };
    let state = cx.state();
    let named: Vec<ObjectRef> = cx
        .scope
        .live_objects(affected)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .map(GameObject::reference)
        .collect();
    if named.is_empty() {
        return Ok(Run::events(Vec::new()));
    }
    let mut events = Vec::new();
    for (made, wanted) in (0u64..).zip(mods) {
        let modification = lower(cx, wanted)?;
        events.push(Event::EffectCreated {
            effect: ContinuousEffect::new(
                EffectId(state.next_effect + made),
                Selector::Objects(named.clone()),
                modification,
                *duration,
                state.next_timestamp + made,
            )
            .from_source(cx.scope.source)
            .begun(cx.controller(), state.turn().number()),
        });
    }
    Ok(Run::events(events))
}

/// What the engine applies for a modification of the card, with its amounts worked out.
fn lower(cx: &Cx<'_>, wanted: &Wanted) -> Result<Modification, ResolveError> {
    let amount = |amount: &Amount| -> Result<i32, ResolveError> {
        i32::try_from(cx.scope.amount(amount)?).map_err(|_| {
            ResolveError::Invalid("a change of power or toughness is out of range".into())
        })
    };
    Ok(match wanted {
        Wanted::ModifyPT { power, toughness } => Modification::ModifyPowerToughness {
            power: amount(power)?,
            toughness: amount(toughness)?,
        },
        Wanted::AddKeyword(keyword) => Modification::AddKeyword(keyword.clone()),
        Wanted::SetPT { .. } => return Err(ResolveError::Unsupported("Mod.SetPT")),
        Wanted::SwitchPT => return Err(ResolveError::Unsupported("Mod.SwitchPT")),
        Wanted::AddType(_) => return Err(ResolveError::Unsupported("Mod.AddType")),
        Wanted::AddSubtype(_) => return Err(ResolveError::Unsupported("Mod.AddSubtype")),
        Wanted::SetColors(_) => return Err(ResolveError::Unsupported("Mod.SetColors")),
        Wanted::RemoveKeyword(_) => return Err(ResolveError::Unsupported("Mod.RemoveKeyword")),
        Wanted::SetController(_) => return Err(ResolveError::Unsupported("Mod.SetController")),
        Wanted::CantAttack => return Err(ResolveError::Unsupported("Mod.CantAttack")),
        Wanted::CantBlock => return Err(ResolveError::Unsupported("Mod.CantBlock")),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a continuous effect begins as the card says: one effect for each modification, in the order of the text, with the next ids and timestamps of the state, naming the permanents the reference names now and no object that has since changed zones or is not a permanent, with its source, controller, turn and duration, its amounts worked out at that moment, and the modifications the engine does not run refuse by their own names.
    use mtg_cards::refs::{Bounds, Selector as Pick, TargetSpec, Targetable};
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::{Choices, Target};
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game() -> GameState {
        state(
            vec![vec![creature("Pump", 0, 0)], vec![]],
            vec![
                vec![creature("Source", 1, 1), creature("Wolf", 2, 2)],
                vec![creature("Bear", 2, 2), creature("Ogre", 3, 4)],
            ],
        )
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|o| o.card().name == name)
            .map(GameObject::reference)
            .unwrap()
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn creature_spec() -> TargetSpec {
        TargetSpec {
            what: Targetable::Object(Pick(vec![mtg_cards::refs::Pred::Type(
                mtg_core::card::CardType::Creature,
            )])),
            bounds: Bounds::default(),
            divide: None,
        }
    }

    /// Runs `effect` as the Source's with `chosen` as the targets of one creature specification.
    fn run_with(
        game: &GameState,
        chosen: Vec<Target>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = named(game, "Source");
        let resolving = Resolving::begin(source.id);
        let choices = Choices {
            targets: vec![chosen],
            ..Choices::default()
        };
        let spec = creature_spec();
        let mut scope = Scope::announcing(game, ME, source);
        scope.choices = &choices;
        scope.specs = vec![&spec];
        let cx = Cx {
            scope,
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    fn begun(run: Run) -> Vec<ContinuousEffect> {
        let Run::Done {
            events,
            bound,
            snapshots,
            ..
        } = run
        else {
            panic!("the effect is done");
        };
        assert!(bound.is_empty() && snapshots.is_empty());
        events
            .into_iter()
            .map(|event| match event {
                Event::EffectCreated { effect } => effect,
                other => panic!("only effects begin, got {other:?}"),
            })
            .collect()
    }

    const GROWTH: &str = r#"{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":3,"toughness":3}}],"duration":"UntilEndOfTurn"}}"#;

    #[test]
    fn a_pump_names_what_was_targeted_and_begins_now() {
        let mut game = game();
        game.turn.number = 4;
        let bear = named(&game, "Bear");
        let made = begun(run_with(&game, vec![Target::Object(bear)], &effect(GROWTH)).unwrap());
        let source = named(&game, "Source");
        assert_eq!(
            made,
            [ContinuousEffect::new(
                EffectId(0),
                Selector::Objects(vec![bear]),
                Modification::ModifyPowerToughness {
                    power: 3,
                    toughness: 3
                },
                mtg_cards::modify::Duration::UntilEndOfTurn,
                0,
            )
            .from_source(source)
            .begun(ME, 4)]
        );
    }

    #[test]
    fn the_modifications_take_the_next_ids_and_timestamps_in_the_order_of_the_text() {
        let mut game = game();
        game.next_effect = 7;
        game.next_timestamp = 40;
        let rush = effect(
            r#"{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"},{"ModifyPT":{"power":1,"toughness":0}},{"AddKeyword":"Flying"}],"duration":"Forever"}}"#,
        );
        let wolf = named(&game, "Wolf");
        let made = begun(run_with(&game, vec![Target::Object(wolf)], &rush).unwrap());
        let seen: Vec<_> = made
            .iter()
            .map(|e| (e.id.0, e.timestamp, e.layer, e.selector.clone()))
            .collect();
        assert_eq!(
            seen,
            [
                (
                    7,
                    40,
                    crate::effects::Layer::Ability,
                    Selector::Objects(vec![wolf])
                ),
                (
                    8,
                    41,
                    crate::effects::Layer::PtModify,
                    Selector::Objects(vec![wolf])
                ),
                (
                    9,
                    42,
                    crate::effects::Layer::Ability,
                    Selector::Objects(vec![wolf])
                ),
            ]
        );
        assert!(
            made.iter()
                .all(|e| e.duration == mtg_cards::modify::Duration::Forever)
        );
    }

    #[rstest]
    #[case::my_creatures(r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#, &["Source", "Wolf"])]
    #[case::theirs(r#"[{"Type":"Creature"},{"ControlledBy":"Opponents"}]"#, &["Bear", "Ogre"])]
    #[case::the_big_ones(r#"[{"Compare":{"stat":"Power","op":"Ge","to":3}}]"#, &["Ogre"])]
    #[case::nothing_matches(r#"[{"Type":"Land"}]"#, &[])]
    #[case::cards_in_hand_are_not_permanents(r#"[{"In":"Hand"}]"#, &[])]
    fn a_selector_names_the_permanents_it_matches_now_and_all_are_one_effect(
        #[case] filter: &str,
        #[case] expected: &[&str],
    ) {
        let game = game();
        let all = effect(&format!(
            r#"{{"Continuous":{{"affected":{{"Each":{filter}}},"mods":[{{"ModifyPT":{{"power":1,"toughness":1}}}}],"duration":"UntilEndOfTurn"}}}}"#
        ));
        let made = begun(run_with(&game, Vec::new(), &all).unwrap());
        let Some(first) = made.first() else {
            assert!(expected.is_empty(), "something was named");
            return;
        };
        assert_eq!(made.len(), 1, "one modification is one effect");
        let Selector::Objects(objects) = &first.selector else {
            panic!("the objects are named");
        };
        let mut names: Vec<_> = objects
            .iter()
            .map(|o| game.object(o.id).unwrap().card().name.clone())
            .collect();
        names.sort();
        let mut want: Vec<_> = expected.iter().map(ToString::to_string).collect();
        want.sort();
        assert_eq!(names, want);
    }

    #[test]
    fn a_target_that_became_a_new_object_is_not_named() {
        let mut game = game();
        let bear = named(&game, "Bear");
        game.objects.get_mut(&bear.id).unwrap().counter += 1;
        let made = begun(run_with(&game, vec![Target::Object(bear)], &effect(GROWTH)).unwrap());
        assert!(made.is_empty(), "the creature that was targeted is gone");
    }

    #[test]
    fn the_amounts_are_worked_out_when_the_effect_begins() {
        let game = game();
        let growth = effect(
            r#"{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Creature"}]},"toughness":-2}}],"duration":"UntilEndOfTurn"}}"#,
        );
        let ogre = named(&game, "Ogre");
        let made = begun(run_with(&game, vec![Target::Object(ogre)], &growth).unwrap());
        assert_eq!(
            made[0].modification,
            Modification::ModifyPowerToughness {
                power: 4,
                toughness: -2
            }
        );
    }

    #[rstest]
    #[case(r#"{"SetPT":{"power":1,"toughness":1}}"#, "Mod.SetPT")]
    #[case(r#""SwitchPT""#, "Mod.SwitchPT")]
    #[case(r#"{"AddType":"Artifact"}"#, "Mod.AddType")]
    #[case(r#"{"AddSubtype":"Elf"}"#, "Mod.AddSubtype")]
    #[case(r#"{"SetColors":["Blue"]}"#, "Mod.SetColors")]
    #[case(r#"{"RemoveKeyword":"Flying"}"#, "Mod.RemoveKeyword")]
    #[case(r#"{"SetController":"You"}"#, "Mod.SetController")]
    #[case(r#""CantAttack""#, "Mod.CantAttack")]
    #[case(r#""CantBlock""#, "Mod.CantBlock")]
    fn a_modification_the_engine_does_not_run_refuses_by_its_name(
        #[case] modification: &str,
        #[case] name: &'static str,
    ) {
        let game = game();
        let asked = effect(&format!(
            r#"{{"Continuous":{{"affected":"This","mods":[{modification}],"duration":"UntilEndOfTurn"}}}}"#
        ));
        assert_eq!(
            run_with(&game, Vec::new(), &asked),
            Err(ResolveError::Unsupported(name))
        );
    }
}
