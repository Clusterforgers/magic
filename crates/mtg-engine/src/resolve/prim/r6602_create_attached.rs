//! `Effect::R6602CreateAttached`: an Aura token is created attached to a permanent or a player (CR 303.7, 111.1); when
//! the host is gone or the token could not enchant it, no token is created (CR 303.4i).

use mtg_cards::effect::Effect;
use mtg_core::ids::ObjectId;

use super::create_token::made_from;
use super::wrong_effect;
use crate::attach::Host;
use crate::events::Event;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::tokens::TokenCreated;
use crate::zones::Zone;

/// Runs the effect.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R6602CreateAttached { who, token, to } = effect else {
        return Err(wrong_effect("R6602CreateAttached"));
    };
    let state = cx.state();
    let done = |events: Vec<Event>| Run::Done {
        events,
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    };
    // A permanent it names on the battlefield, else a player it names who is still in the game ("attached to target
    // opponent").
    let host = match cx
        .scope
        .live_objects(to)?
        .into_iter()
        .find(|object| object.zone() == Zone::Battlefield)
    {
        Some(object) => Some(Host::Object(object.id())),
        None => cx
            .scope
            .players(to)?
            .into_iter()
            .find(|player| state.is_alive(*player))
            .map(Host::Player),
    };
    let controller = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player));
    let (Some(host), Some(controller)) = (host, controller) else {
        return Ok(done(Vec::new()));
    };
    let mut events = Vec::new();
    let mut next = state.next_object().get();
    for (definition, copy_of) in made_from(cx, token)? {
        let object = ObjectId(next);
        let created = Event::TokenCreated(TokenCreated {
            object,
            copy_of,
            controller,
            name: definition.name.clone(),
            tapped: false,
            attacking: None,
            blocking: None,
            definition,
        });
        // The token is judged as it would stand on the battlefield: its enchant ability and the host's protection.
        let mut judged = state.clone();
        let legal = crate::fold::apply_event(&mut judged, &created).is_ok()
            && match host {
                Host::Object(on) => crate::attach::legal_host(&judged, object, on),
                Host::Player(player) => crate::attach::legal_player_host(&judged, object, player),
            };
        if legal {
            events.push(created);
            events.push(host.attach(object));
            next += 1;
        }
    }
    Ok(done(events))
}

#[cfg(test)]
mod tests {
    //! Goal: an Aura token enters attached to the named permanent of any player at two to four seats, under the player
    //! the effect names; a host that is gone, one the token's enchant ability does not allow and one with protection
    //! from it get no token at all (CR 303.4i).
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_core::card::CardType;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::choose::Target;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    const ROLE: &str = r#"{"Custom":{"name":"Test Role","colors":[],"types":["Enchantment"],"subtypes":["Aura","Role"],
        "abilities":[{"Keyword":{"Enchant":[{"Type":"Creature"}]}}]}}"#;

    /// Seat 0 has a Bear, a Land and a Warded creature (protection from Auras); every other seat has a Bear of its own.
    fn game(seats: usize) -> GameState {
        let mut battlefields: Vec<Vec<Arc<CardDefinition>>> = (0..seats)
            .map(|seat| vec![creature(&format!("Bear{seat}"), 2, 2)])
            .collect();
        battlefields[0].push(Arc::new(
            CardDefinition::new("Land").with_type(CardType::Land),
        ));
        let warded: Ability =
            serde_json::from_str(r#"{"Keyword":{"Protection":[{"Subtype":"Aura"}]}}"#).unwrap();
        battlefields[0].push(Arc::new(
            CardDefinition::new("Warded")
                .with_type(CardType::Creature)
                .with_rules(warded),
        ));
        state(vec![Vec::new(); seats], battlefields)
    }

    fn id(game: &GameState, name: &str) -> ObjectId {
        game.objects().find(|o| o.card().name == name).unwrap().id()
    }

    /// The events of creating the role, for `who`, attached to the permanent named `host` (bound as "h"; none when
    /// empty).
    fn created(game: &GameState, who: &str, host: &str) -> Vec<Event> {
        let source = game.object(id(game, "Bear0")).unwrap();
        let mut resolving = Resolving::begin(source.id());
        let named = if host.is_empty() {
            Vec::new()
        } else {
            vec![Target::Object(
                game.object(id(game, host)).unwrap().reference(),
            )]
        };
        resolving.bindings.insert("h".into(), named);
        let cx = Cx {
            scope: Scope {
                bindings: &resolving.bindings,
                ..Scope::announcing(game, ME, source.reference())
            },
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(&format!(
            r#"{{"R6602CreateAttached":{{"who":{who},"token":{ROLE},"to":{{"Bound":"h"}}}}}}"#
        ))
        .unwrap();
        let Ok(Run::Done { events, .. }) = run(&cx, &effect) else {
            panic!("the effect is done");
        };
        events
    }

    #[rstest]
    #[case::my_creature_two_seats(2, "Bear0")]
    #[case::an_opponents_creature_three_seats(3, "Bear2")]
    #[case::the_last_seat_four_seats(4, "Bear3")]
    fn the_token_enters_attached_to_the_host(#[case] seats: usize, #[case] host: &str) {
        let game = game(seats);
        let events = created(&game, r#""You""#, host);
        let [Event::TokenCreated(token), Event::Attached { object, to }] = events.as_slice() else {
            panic!("a token and its attachment, got {events:?}");
        };
        assert_eq!((token.controller, token.name.as_str()), (ME, "Test Role"));
        assert_eq!((*object, *to), (token.object, id(&game, host)));
        let mut folded = game.clone();
        for event in &events {
            crate::fold::apply_event(&mut folded, event).unwrap();
        }
        assert_eq!(folded.host_of(token.object), Some(id(&game, host)));
    }

    #[rstest]
    #[case::two_seats(2)]
    #[case::four_seats(4)]
    fn the_token_is_controlled_by_the_player_named(#[case] seats: usize) {
        let game = game(seats);
        let events = created(&game, r#"{"ControllerOf":{"Bound":"h"}}"#, "Bear1");
        let Some(Event::TokenCreated(token)) = events.first() else {
            panic!("a token, got {events:?}");
        };
        assert_eq!(token.controller, PlayerId(1));
    }

    #[rstest]
    #[case::the_host_is_gone("")]
    #[case::a_host_the_token_cannot_enchant("Land")]
    #[case::a_host_with_protection_from_auras("Warded")]
    fn no_token_is_created_without_a_legal_host(#[case] host: &str) {
        for seats in [2, 3, 4] {
            assert!(created(&game(seats), r#""You""#, host).is_empty(), "{host}");
        }
    }
}
