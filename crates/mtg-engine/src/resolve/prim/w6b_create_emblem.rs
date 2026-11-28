//! `Effect::CreateEmblem`: each player named gets an emblem with the abilities (CR 114); `Effect::R6051PlayerGains`
//! gives them one that lasts until end of turn.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_core::ids::ObjectId;

use super::wrong_effect;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::{Cx, ResolveError, Run};
use crate::w6b_emblems::{EmblemCreated, w6b_definition};
use crate::zones::Zone;

/// The abilities with each value the resolution chose (`Pred::ChosenAs`) written in as what was chosen, so the emblem
/// keeps it after the resolution ("Choose a creature type. You get an emblem with "Creatures you control of the chosen
/// type get +3/+3 ...""); the abilities as they are when nothing was written.
fn r4702_fixed(cx: &Cx<'_>, abilities: &[Ability]) -> Vec<Ability> {
    let source = r6072_permanent(cx);
    abilities
        .iter()
        .map(|ability| {
            serde_json::to_value(ability)
                .ok()
                .and_then(|mut value| {
                    let fixed = r6072_fix(source, &mut value);
                    (super::rule::fix(&cx.scope, &mut value) || fixed).then_some(value)
                })
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or_else(|| ability.clone())
        })
        .collect()
}

/// The permanent that makes the emblem as it is on the battlefield: the source itself, or, as it enters (`AsEnters`:
/// "As Tibalt enters, you get an emblem"), the permanent it becomes (CR 400.7).
fn r6072_permanent(cx: &Cx<'_>) -> ObjectRef {
    let source = cx.scope.source;
    let entering = cx.state().object(source.id).is_some_and(|object| {
        object.counter() == source.counter && object.zone() != Zone::Battlefield
    });
    if entering {
        ObjectRef {
            id: source.id,
            counter: source.counter + 1,
        }
    } else {
        source
    }
}

/// Writes the permanent in for each card it exiled the emblem names (`ExiledWith(This)`: "You may play cards exiled with
/// Tibalt, Cosmic Impostor"), so the emblem goes on naming that permanent's cards after it left (CR 607.2a).
fn r6072_fix(source: ObjectRef, value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(fields) => {
            if fields.len() == 1
                && fields.get("ExiledWith").and_then(serde_json::Value::as_str) == Some("This")
            {
                fields.insert(
                    "ExiledWith".to_owned(),
                    serde_json::json!({ "R6072Source": { "id": source.id, "counter": source.counter } }),
                );
                return true;
            }
            let mut any = false;
            for inner in fields.values_mut() {
                any |= r6072_fix(source, inner);
            }
            any
        }
        serde_json::Value::Array(items) => {
            let mut any = false;
            for inner in items {
                any |= r6072_fix(source, inner);
            }
            any
        }
        _ => false,
    }
}

/// One `EmblemCreated` for each player the effect names who is still in the game, in turn order of the names.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let (who, abilities, this_turn) = match effect {
        Effect::CreateEmblem { who, abilities } => (who, abilities, false),
        Effect::R6051PlayerGains { who, abilities } => (who, abilities, true),
        _ => return Err(wrong_effect("CreateEmblem")),
    };
    let state = cx.state();
    let abilities = r4702_fixed(cx, abilities);
    let definition = Arc::new(w6b_definition(&abilities));
    let first = state.next_object().get();
    let events = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .zip(first..)
        .map(|(owner, id)| {
            Event::EmblemCreated(EmblemCreated {
                object: ObjectId(id),
                owner,
                definition: Arc::clone(&definition),
                this_turn,
            })
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: an emblem is one new object in the command zone of each player named who is still in the game, with the
    //! abilities working from there; the fold refuses an object that exists and an owner who left; another effect is
    //! refused; each `ExiledWith(This)` of the abilities, and only it, is written as the permanent that made the emblem.
    use mtg_cards::refs::ZoneKind;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::object::ObjectKind;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const ANTHEM: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":0}}]}}"#;

    fn game(seats: usize) -> GameState {
        let mut boards = vec![vec![]; seats];
        boards[0].push(creature("Source", 1, 1));
        state(vec![vec![]; seats], boards)
    }

    fn emblems(game: &GameState, who: &str) -> Vec<Event> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let json = format!(r#"{{"CreateEmblem":{{"who":"{who}","abilities":[{ANTHEM}]}}}}"#);
        let crate::resolve::Run::Done { events, .. } =
            run(&cx, &serde_json::from_str(&json).unwrap()).unwrap()
        else {
            panic!("an emblem asks nothing");
        };
        events
    }

    #[rstest]
    #[case::you(2, "You", vec![0])]
    #[case::each_opponent_of_three(3, "Opponents", vec![1, 2])]
    #[case::each_player_of_four(4, "EachPlayer", vec![0, 1, 2, 3])]
    fn w6b_each_player_named_gets_one(
        #[case] seats: usize,
        #[case] who: &str,
        #[case] owners: Vec<u64>,
    ) {
        let mut game = game(seats);
        let events = emblems(&game, who);
        assert_eq!(events.len(), owners.len());
        for (event, owner) in events.iter().zip(owners) {
            apply_event(&mut game, event).unwrap();
            let Event::EmblemCreated(made) = event else {
                panic!("{event:?}");
            };
            let emblem = game.object(made.object).unwrap();
            assert_eq!(emblem.zone(), Zone::Command(PlayerId(owner)));
            assert_eq!(emblem.kind(), ObjectKind::Emblem);
            let written = serde_json::to_value(&*made.definition).unwrap();
            assert_eq!(
                written["rules"][0]["Static"]["from"],
                serde_json::to_value(ZoneKind::Command).unwrap()
            );
            assert!(
                apply_event(&mut game, event).is_err(),
                "the same object twice"
            );
        }
    }

    #[test]
    fn w6b_another_effect_is_refused() {
        let game = game(2);
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        assert!(run(&cx, &serde_json::from_str(r#"{"Draw":{}}"#).unwrap()).is_err());
    }

    #[rstest]
    #[case::exiled_with_this(r#"{"ExiledWith":"This"}"#, true)]
    #[case::nested_in_a_permission(
        r#"{"MayCast":{"what":[{"In":"Exile"},{"ExiledWith":"This"}],"from":"Exile"}}"#,
        true
    )]
    #[case::exiled_with_another(r#"{"ExiledWith":{"Target":0}}"#, false)]
    #[case::this_alone(r#"{"Is":"This"}"#, false)]
    fn r6072_the_maker_is_written_in(#[case] json: &str, #[case] fixed: bool) {
        let maker = ObjectRef {
            id: ObjectId(7),
            counter: 3,
        };
        let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(r6072_fix(maker, &mut value), fixed);
        let written = value.to_string();
        let named = ["R6072Source", r#""counter":3"#, r#""id":7"#];
        assert_eq!(
            named.iter().all(|part| written.contains(part)),
            fixed,
            "{written}"
        );
        assert!(!fixed || !written.contains(r#""This""#), "{written}");
    }
}
