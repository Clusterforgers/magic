//! `Effect::TakeInitiative`: a player takes the initiative (CR 725).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::w15a_initiative::{W15aHeld, w15a_take};

/// Each player the effect names who is in the game takes the initiative in turn, so the last one has it; a player who
/// already has it takes it again (CR 725.2), and each take triggers the venture into Undercity.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::TakeInitiative { who } = effect else {
        return Err(wrong_effect("TakeInitiative"));
    };
    let state = cx.state();
    let mut next_object = state.next_object;
    let mut held = state.w15a_initiative;
    let mut events = Vec::new();
    for player in cx.scope.players(who)? {
        if !state.is_alive(player) {
            continue;
        }
        let event = w15a_take(held, player, &mut next_object);
        if let Event::InitiativeTaken { object, .. } = event {
            held = Some(W15aHeld { player, object });
        }
        events.push(event);
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: taking the initiative logs `InitiativeTaken` for the player the effect names, again for a player who already has it (with the same object), with a new object for a new holder whose old object leaves the game, nothing for a player who has left, and refuses another effect.
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use mtg_core::ids::PlayerId;

    use super::*;
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::player::LossReason;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn take(game: &mut GameState, json: &str) -> Vec<Event> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        let Run::Done { events, .. } = run(&cx, &serde_json::from_str(json).unwrap()).unwrap()
        else {
            panic!("taking the initiative asks nothing");
        };
        for event in &events {
            apply_event(game, event).unwrap();
        }
        events
    }

    fn object_of(event: &Event) -> mtg_core::ids::ObjectId {
        let Event::InitiativeTaken { object, .. } = event else {
            panic!("{event:?}");
        };
        *object
    }

    #[test]
    fn the_initiative_moves_with_its_object_and_is_taken_again_by_its_holder() {
        let mut game = game();
        let first = take(&mut game, r#"{"TakeInitiative":{}}"#);
        assert_eq!(game.w15a_initiative(), Some(ME));
        assert_eq!(game.zone(Zone::Command(ME)).len(), 1);
        let again = take(&mut game, r#"{"TakeInitiative":{}}"#);
        assert_eq!(object_of(&again[0]), object_of(&first[0]));
        let taken = take(&mut game, r#"{"TakeInitiative":{"who":"Opponents"}}"#);
        assert_ne!(object_of(&taken[0]), object_of(&first[0]));
        assert_eq!(game.w15a_initiative(), Some(FOE));
        assert_eq!(
            game.zone(Zone::Command(ME)),
            &[] as &[mtg_core::ids::ObjectId]
        );
        assert_eq!(game.zone(Zone::Command(FOE)).len(), 1);
        let both = take(&mut game, r#"{"TakeInitiative":{"who":"EachPlayer"}}"#);
        assert_eq!(both.len(), 2);
        assert_ne!(object_of(&both[0]), object_of(&both[1]));
        assert_eq!(game.w15a_initiative(), Some(FOE));
        assert_eq!(game.zone(Zone::Command(FOE)).len(), 1);
    }

    #[test]
    fn a_player_who_left_takes_nothing_and_another_effect_is_refused() {
        let mut game = game();
        let lost = Event::PlayerLost {
            player: FOE,
            reason: LossReason::Conceded,
        };
        apply_event(&mut game, &lost).unwrap();
        assert_eq!(
            take(&mut game, r#"{"TakeInitiative":{"who":"Opponents"}}"#),
            []
        );
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        assert!(run(&cx, &serde_json::from_str(r#"{"Draw":{}}"#).unwrap()).is_err());
    }
}
