//! The manual actions on who controls a permanent and how it may act: ending its summoning sickness, sacrificing it, and
//! giving it to another seat, each as the events a spell that does the same produces.

use mtg_core::card::CardType;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::battle;
use crate::characteristics::controller_of_object;
use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
use crate::events::{Event, ZoneCause};
use crate::keywords::summoning_sick;
use crate::loyalty;
use crate::manual::{ManualAction, ManualRefusal, ManualZone};
use crate::object::{GameObject, ObjectKind};
use crate::saga;
use crate::state::GameState;
use crate::zones::Zone;

/// The permanent `id` when the seat controls it.
fn controlled(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
) -> Result<&GameObject, ManualRefusal> {
    let object = state
        .object(id)
        .filter(|object| object.kind() != ObjectKind::Ability)
        .ok_or(ManualRefusal::Unknown(id))?;
    if object.zone() != Zone::Battlefield {
        return Err(ManualRefusal::WrongZone {
            object: id,
            zone: object.zone(),
        });
    }
    if controller_of_object(state, object) == player {
        Ok(object)
    } else {
        Err(ManualRefusal::NotYours)
    }
}

/// A permanent the seat controls that summoning sickness holds back stops being sick; the change is the action itself,
/// folded by [`apply`].
pub(crate) fn remove_sickness(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
) -> Result<Vec<Event>, ManualRefusal> {
    controlled(state, player, id)?;
    if !summoning_sick(state, id) {
        return Err(ManualRefusal::NoChange);
    }
    Ok(Vec::new())
}

/// The seat sacrifices a permanent it controls: it goes to its owner's graveyard as a sacrifice (CR 701.21), so what
/// triggers on dying or on a sacrifice sees it.
pub(crate) fn sacrifice(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
) -> Result<Vec<Event>, ManualRefusal> {
    let permanent = controlled(state, player, id)?;
    Ok(vec![Event::ZoneChanged {
        object: id,
        from: Zone::Battlefield,
        to: Zone::Graveyard(permanent.owner()),
        cause: ZoneCause::Sacrificed,
    }])
}

/// Seat `to` gains control of a permanent the seat controls, by an effect without end in the control layer, or a card
/// the seat owns in its graveyard or in exile enters the battlefield under the control of `to`.
pub(crate) fn give_control(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    to: PlayerId,
) -> Result<Vec<Event>, ManualRefusal> {
    if to == player || !state.is_alive(to) {
        return Err(ManualRefusal::NotAnotherSeat);
    }
    let object = state
        .object(id)
        .filter(|object| object.kind() != ObjectKind::Ability)
        .ok_or(ManualRefusal::Unknown(id))?;
    if object.zone() == Zone::Battlefield {
        let permanent = controlled(state, player, id)?;
        let effect = ContinuousEffect::new(
            EffectId(state.next_effect),
            Selector::Objects(vec![permanent.reference()]),
            Modification::SetController(to),
            Duration::Forever,
            state.next_timestamp,
        )
        .begun(player, state.turn().number());
        return Ok(vec![Event::EffectCreated {
            effect: effect.into(),
        }]);
    }
    if !matches!(object.zone(), Zone::Exile | Zone::Graveyard(_)) {
        return Err(ManualRefusal::WrongZone {
            object: id,
            zone: object.zone(),
        });
    }
    if object.owner() != player {
        return Err(ManualRefusal::NotYours);
    }
    let types = object.base_types();
    if types.contains(CardType::Instant) || types.contains(CardType::Sorcery) {
        return Err(ManualRefusal::NoDestination(ManualZone::Battlefield));
    }
    let mut events = vec![
        Event::ZoneChanged {
            object: id,
            from: object.zone(),
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        },
        Event::Entered {
            object: id,
            tapped: false,
            controller: to,
            chosen: None,
        },
    ];
    events.extend(loyalty::entering(state, id));
    events.extend(battle::entering(state, id));
    events.extend(saga::entering(state, id));
    Ok(events)
}

/// What folding a manual action records beyond its events: a permanent whose summoning sickness ends counts as under
/// its controller's control since the turn began.
pub(crate) fn apply(state: &mut GameState, action: &ManualAction) {
    let ManualAction::RemoveSickness { object } = action else {
        return;
    };
    let Some(controller) = state
        .object(*object)
        .map(|held| controller_of_object(state, held))
    else {
        return;
    };
    if let Some(permanent) = state
        .objects
        .get_mut(object)
        .and_then(|held| held.permanent.as_mut())
    {
        permanent.summoning_sick = false;
        permanent.observed = Some(controller);
    }
}
