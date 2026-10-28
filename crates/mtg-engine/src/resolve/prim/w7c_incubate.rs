//! `Effect::Incubate`: each player named incubates (CR 701.53).

use std::sync::Arc;

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;

use super::create_token::w7c_predefined_definition;
use super::put_counters::whole;
use super::wrong_effect;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::tokens::TokenCreated;

/// The name the Incubators of this step are bound to: the step is told apart by its position.
fn w7c_marker(cx: &Cx<'_>) -> String {
    format!("incubated {:?}", cx.resolving.position)
}

/// The first pass creates an Incubator token (a transforming artifact whose back face is a 0/0 Phyrexian artifact
/// creature) for each player named who is in the game, binds them and runs the effect again; the second pass puts the
/// +1/+1 counters on each of those tokens still on the battlefield, so a replacement effect sees them.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Incubate { who, count } = effect else {
        return Err(wrong_effect("Incubate"));
    };
    let marker = w7c_marker(cx);
    if let Some(made) = cx.resolving.bindings.get(&marker) {
        let delta = whole(cx.scope.amount(count)?);
        let state = cx.state();
        let events = if delta > 0 {
            made.iter()
                .filter_map(|made| match made {
                    Target::Object(token) => state
                        .object(token.id)
                        .filter(|object| object.reference() == *token)
                        .map(GameObject::reference),
                    Target::Player(_) => None,
                })
                .map(|token| {
                    Event::CountersChanged(CountersChanged {
                        on: Target::Object(token),
                        counter: CounterKind::PlusOnePlusOne,
                        delta,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        return Ok(Run::events(events));
    }
    let state = cx.state();
    let definition = Arc::new(w7c_predefined_definition("incubator")?);
    let first = state.next_object().get();
    let mut events = Vec::new();
    let mut made = Vec::new();
    let players = cx.scope.players(who)?;
    for (controller, id) in players
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .zip(first..)
    {
        events.push(Event::TokenCreated(TokenCreated {
            object: ObjectId(id),
            copy_of: None,
            controller,
            name: definition.name.clone(),
            tapped: false,
            attacking: None,
            definition: Arc::clone(&definition),
        }));
        // A token is a new object: its zone-change counter starts at 0.
        made.push(Target::Object(ObjectRef {
            id: ObjectId(id),
            counter: 0,
        }));
    }
    Ok(Run::Done {
        events,
        bound: vec![(marker, made)],
        snapshots: Vec::new(),
        next: Next::Again,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: an incubate creates one Incubator per player named, binds them and runs again; the rerun puts the counters
    //! on each token still there (none for a count of zero); the token transforms into a 0/0 Phyrexian; another effect
    //! is refused.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    fn game() -> GameState {
        GameState::from_setup(&setup(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        ))
        .unwrap()
    }

    fn cx_run(game: &GameState, resolving: &Resolving, json: &str) -> Result<Run, ResolveError> {
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(0), source.reference()),
            resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    #[rstest]
    #[case::you(r#"{"Incubate":{"count":3}}"#, 1)]
    #[case::each_player(r#"{"Incubate":{"who":"EachPlayer","count":3}}"#, 2)]
    fn the_first_pass_creates_and_binds(#[case] json: &str, #[case] tokens: usize) {
        let game = game();
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving::begin(source.id());
        let Run::Done {
            events,
            bound,
            next,
            ..
        } = cx_run(&game, &resolving, json).unwrap()
        else {
            panic!("the first pass asks nothing")
        };
        assert_eq!(next, Next::Again);
        assert_eq!(events.len(), tokens);
        assert_eq!(bound[0].1.len(), tokens);
        let Event::TokenCreated(made) = &events[0] else {
            panic!("a token is created")
        };
        assert_eq!(made.name, "Incubator");
        assert_eq!(made.definition.faces.len(), 2);
        assert_eq!(made.definition.faces[1].name, "Phyrexian Token");
        assert_eq!(made.definition.faces[1].power, Some(0));
    }

    #[rstest]
    #[case::three(3, 1)]
    #[case::none(0, 0)]
    fn the_rerun_puts_the_counters(#[case] count: i32, #[case] changes: usize) {
        let game = game();
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let mut resolving = Resolving::begin(source.id());
        let marker = format!("incubated {:?}", resolving.position);
        resolving
            .bindings
            .insert(marker, vec![Target::Object(source.reference())]);
        let json = format!(r#"{{"Incubate":{{"count":{count}}}}}"#);
        let run = cx_run(&game, &resolving, &json).unwrap();
        let Run::Done { events, .. } = run else {
            panic!("the rerun asks nothing")
        };
        assert_eq!(events.len(), changes);
    }

    #[test]
    fn another_effect_is_refused() {
        let game = game();
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving::begin(source.id());
        assert!(cx_run(&game, &resolving, r#"{"Draw":{}}"#).is_err());
    }
}
