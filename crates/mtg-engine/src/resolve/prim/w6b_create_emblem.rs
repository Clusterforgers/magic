//! `Effect::CreateEmblem`: each player named gets an emblem with the abilities (CR 114).

use std::sync::Arc;

use mtg_cards::effect::Effect;
use mtg_core::ids::ObjectId;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::w6b_emblems::{EmblemCreated, w6b_definition};

/// One `EmblemCreated` for each player the effect names who is still in the game, in turn order of the names.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::CreateEmblem { who, abilities } = effect else {
        return Err(wrong_effect("CreateEmblem"));
    };
    let state = cx.state();
    let definition = Arc::new(w6b_definition(abilities));
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
            })
        })
        .collect();
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: an emblem is one new object in the command zone of each player named who is still in the game, with the
    //! abilities working from there; the fold refuses an object that exists and an owner who left; another effect is
    //! refused.
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
            assert!(emblem.def.rules.iter().all(|rule| matches!(
                **rule,
                mtg_cards::ability::Ability::Static {
                    from: ZoneKind::Command,
                    ..
                }
            )));
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
}
