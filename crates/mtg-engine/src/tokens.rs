//! Tokens: the `TokenCreated` event, the object it makes and the rule that a token exists only on the battlefield
//! (CR 111.7).

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::combat::{AttackTarget, Attacker};
use crate::error::EngineError;
use crate::fold::{invalid, reset_passes};
use crate::object::{Designations, GameObject, ObjectKind, ObjectRef, PermanentState};
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// Whether a flag is off, so serialization can leave it out (serde hands over a reference).
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(flag: &bool) -> bool {
    !*flag
}

/// A token that was created: it enters the battlefield under its controller's control, who is also its owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TokenCreated {
    /// The token's object.
    pub object: ObjectId,
    /// Who controls it.
    pub controller: PlayerId,
    /// What it is called.
    pub name: String,
    /// It enters tapped.
    #[serde(default, skip_serializing_if = "is_false")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub tapped: bool,
    /// It enters attacking this player or planeswalker: it was never declared as an attacker (CR 508.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub attacking: Option<AttackTarget>,
    /// Its characteristics and abilities, so a log folds to the same game with no card data at hand.
    #[cfg_attr(feature = "ts", ts(skip))]
    pub definition: Arc<CardDefinition>,
    /// The object it is a copy of, when the definition is the copiable values of that object (CR 111.10, 707.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub copy_of: Option<ObjectRef>,
}

/// Folds the event: the token is a new object on the battlefield, tapped and attacking as the event says.
pub(crate) fn fold(state: &mut GameState, event: &TokenCreated) -> Result<(), EngineError> {
    let id = event.object;
    if state.objects.contains_key(&id) || state.ceased.contains_key(&id) {
        return Err(invalid(format!("{id:?} already exists")));
    }
    if !state.is_alive(event.controller) {
        return Err(invalid(format!(
            "{:?} is out of the game and cannot control a token",
            event.controller
        )));
    }
    if event.attacking.is_some() && !state.combat.attackers_declared {
        return Err(invalid(
            "a token cannot enter attacking before attackers are declared",
        ));
    }
    let mut token = GameObject::new(
        id,
        event.controller,
        Arc::clone(&event.definition),
        Zone::Battlefield,
    );
    token.kind = ObjectKind::Token;
    token.permanent = Some(PermanentState {
        tapped: event.tapped,
        designations: Designations::default(),
        damage: 0,
        deathtouched: false,
        summoning_sick: true,
        new_to: None,
        counters: BTreeMap::new(),
        attached_to: None,
        attached_player: None,
        timestamp: state.next_timestamp,
        observed: None,
        chosen: None,
        paid: Vec::new(),
        way: None,
        x: None,
        face: None,
        copy: event.copy_of.map(|of| {
            crate::copies::CopyEffect::new(of, Arc::clone(&event.definition), state.next_timestamp)
        }),
        merged: None,
    });
    state.next_timestamp += 1;
    // A token can bring what no card of the game had: a triggered ability, a static ability, a legend.
    crate::copies::note_definition(state, &event.definition);
    state.objects.insert(id, token);
    state.zones.insert(Zone::Battlefield, id, Placement::Top);
    state.next_object = state.next_object.max(id.get() + 1);
    if let Some(target) = event.attacking {
        state.combat.attackers.push(Attacker {
            object: id,
            target,
            blocked: false,
            blockers: Vec::new(),
            assignment: None,
            target_left: false,
        });
    }
    reset_passes(state);
    Ok(())
}

/// Ends a token that has left the battlefield (CR 111.7, 704.5d).
///
/// It is gone from every zone and from `objects`, but the object is kept apart: a trigger or an ability of it that
/// is already on its way (a dies trigger, a sacrifice cost's ability) still needs the card it was printed on.
pub(crate) fn cease(state: &mut GameState, id: ObjectId) {
    for zone in crate::zones::Zones::all_zones(state.players.len()) {
        if state.zones.remove(zone, id) {
            break;
        }
    }
    if let Some(token) = state.objects.remove(&id) {
        state.ceased.insert(id, token);
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a token enters the battlefield as a new object of its controller, tapped or attacking as the event says, and ceases to exist the moment it is anywhere else, from every way it can leave; the state hash sees it come and go, and the events that cannot follow are refused.
    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::object::ObjectKind;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn created(game: &GameState, tapped: bool, attacking: Option<AttackTarget>) -> TokenCreated {
        TokenCreated {
            object: game.next_object(),
            copy_of: None,
            controller: ME,
            name: "Soldier".into(),
            tapped,
            attacking,
            definition: Arc::new(
                CardDefinition::new("Soldier")
                    .with_type(CardType::Creature)
                    .with_power_toughness(1, 1),
            ),
        }
    }

    fn enter(game: &mut GameState, tapped: bool) -> ObjectId {
        let event = created(game, tapped, None);
        let id = event.object;
        apply_event(game, &Event::TokenCreated(event)).unwrap();
        id
    }

    #[rstest]
    #[case::untapped(false)]
    #[case::tapped(true)]
    fn a_token_is_a_new_sick_object_of_its_controller_on_the_battlefield(#[case] tapped: bool) {
        let mut game = game();
        let before = (game.object_count(), game.state_hash());
        let id = enter(&mut game, tapped);
        let token = game.object(id).unwrap();
        assert_eq!(token.kind(), ObjectKind::Token);
        assert_eq!((token.owner(), token.controller()), (ME, ME));
        assert_eq!(token.zone(), Zone::Battlefield);
        assert_eq!(token.counter(), 0);
        let permanent = token.permanent().unwrap();
        assert_eq!(permanent.tapped(), tapped);
        assert!(permanent.summoning_sick());
        assert!(game.zone(Zone::Battlefield).contains(&id));
        assert_eq!(game.object_count(), before.0 + 1);
        assert_ne!(game.state_hash(), before.1);
        assert_eq!(game.next_object(), ObjectId(id.get() + 1), "ids go on");
    }

    #[test]
    fn a_token_that_enters_attacking_is_an_unblocked_attacker_that_was_never_declared() {
        let mut game = game();
        game.combat.attackers_declared = true;
        let event = created(&game, true, Some(AttackTarget::Player(FOE)));
        let id = event.object;
        apply_event(&mut game, &Event::TokenCreated(event)).unwrap();
        let [attacker] = game.combat().attackers() else {
            panic!("one attacker");
        };
        assert_eq!(attacker.object(), id);
        assert_eq!(attacker.target(), AttackTarget::Player(FOE));
        assert!(!attacker.blocked() && attacker.blockers().is_empty());
    }

    #[rstest]
    #[case::graveyard(Zone::Graveyard(ME), ZoneCause::Destroyed)]
    #[case::sacrificed(Zone::Graveyard(ME), ZoneCause::Sacrificed)]
    #[case::hand(Zone::Hand(ME), ZoneCause::Moved)]
    #[case::exile(Zone::Exile, ZoneCause::Exiled)]
    #[case::library(Zone::Library(ME), ZoneCause::Moved)]
    fn a_token_that_leaves_the_battlefield_ceases_to_exist(
        #[case] to: Zone,
        #[case] cause: ZoneCause,
    ) {
        let mut game = game();
        let id = enter(&mut game, false);
        let present = game.object_count();
        let hash = game.state_hash();
        let leave = Event::ZoneChanged {
            object: id,
            from: Zone::Battlefield,
            to,
            cause,
        };
        apply_event(&mut game, &leave).unwrap();
        assert!(game.object(id).is_none() && game.has_ceased(id));
        assert_eq!(game.object_count(), present - 1);
        assert!(!game.zone(Zone::Battlefield).contains(&id));
        assert!(!game.zone(to).contains(&id));
        assert_ne!(game.state_hash(), hash);
        // An ability of it that is already on its way still finds the card it came from.
        assert_eq!(
            game.source_object(id).map(|o| o.card().name.clone()),
            Some("Soldier".into())
        );
        assert!(
            apply_event(&mut game, &leave).is_err(),
            "it cannot move again"
        );
    }

    #[test]
    fn a_token_dying_to_state_based_actions_ceases_too() {
        let mut game = game();
        let id = enter(&mut game, false);
        let died = Event::CreatureDied {
            object: id,
            cause: crate::events::DeathCause::ZeroToughness,
        };
        apply_event(&mut game, &died).unwrap();
        assert!(game.has_ceased(id) && game.object(id).is_none());
    }

    #[test]
    fn a_card_that_leaves_stays_in_the_game() {
        let mut game = game();
        let source = game.zone(Zone::Battlefield)[0];
        let event = Event::ZoneChanged {
            object: source,
            from: Zone::Battlefield,
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Destroyed,
        };
        apply_event(&mut game, &event).unwrap();
        assert!(game.object(source).is_some() && !game.has_ceased(source));
    }

    #[test]
    fn a_ceased_token_cannot_come_back_under_its_old_id() {
        let mut game = game();
        let event = created(&game, false, None);
        let id = event.object;
        apply_event(&mut game, &Event::TokenCreated(event.clone())).unwrap();
        let leave = Event::ZoneChanged {
            object: id,
            from: Zone::Battlefield,
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        };
        apply_event(&mut game, &leave).unwrap();
        assert!(apply_event(&mut game, &Event::TokenCreated(event)).is_err());
    }
}
