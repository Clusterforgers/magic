//! The manual actions that turn a double-faced permanent over, put a double-faced card onto the battlefield back face up,
//! and give a creature a combat permission until end of turn.

use mtg_cards::definition::Layout;
use mtg_cards::modify::CombatQuirk;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::characteristics::characteristics;
use crate::day_night::turned_over;
use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
use crate::events::Event;
use crate::manual::{MAX_STAT, ManualRefusal, ManualZone, moves, own};
use crate::object::GameObject;
use crate::rules::{Rule, RuleEffect};
use crate::state::GameState;
use crate::zones::Zone;

/// The events that turn the transforming double-faced permanent `id`, which the seat controls, to its other face.
pub(crate) fn transform(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
) -> Result<Vec<Event>, ManualRefusal> {
    let object = own(state, player, id)?;
    if object.controller() != player {
        return Err(ManualRefusal::NotYours);
    }
    if object.zone() != Zone::Battlefield || object.whole_card().layout != Layout::Transform {
        return Err(ManualRefusal::NotTransformable(id));
    }
    Ok(vec![Event::Transformed {
        object: id,
        face: turned_over(object.face()),
    }])
}

/// The events that put the double-faced card `id`, which is not on the battlefield, onto it with its back face up.
pub(crate) fn move_back(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
) -> Result<Vec<Event>, ManualRefusal> {
    let object = own(state, player, id)?;
    if !matches!(
        object.whole_card().layout,
        Layout::Transform | Layout::ModalDfc
    ) || object.zone() == Zone::Battlefield
    {
        return Err(ManualRefusal::NotDoubleFaced(id));
    }
    let mut events = moves(state, player, id, ManualZone::Battlefield)?;
    // The face is set right after the card enters, as a card put onto the battlefield transformed does.
    events.insert(
        1,
        Event::Transformed {
            object: id,
            face: Some(1),
        },
    );
    Ok(events)
}

/// The event that lets the creature `id`, which the seat controls, attack this turn as though it had no defender.
pub(crate) fn attack_despite_defender(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
) -> Result<Vec<Event>, ManualRefusal> {
    let creature = creature(state, player, id)?;
    let effect = RuleEffect {
        id: EffectId(state.next_effect),
        rule: Rule::Combat {
            quirk: CombatQuirk::AttackDespiteDefender,
            creatures: vec![creature.reference()],
        },
        duration: Duration::UntilEndOfTurn,
        source: None,
        controller: player,
        turn: state.turn().number(),
    };
    Ok(vec![Event::RuleCreated {
        effect: effect.into(),
    }])
}

/// The event that lets the creature `id`, which the seat controls, block `count` more creatures this turn.
pub(crate) fn block_additional(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    count: u32,
) -> Result<Vec<Event>, ManualRefusal> {
    if !(1..=MAX_STAT.unsigned_abs()).contains(&count) {
        return Err(ManualRefusal::OutOfRange("the number of extra blocks"));
    }
    let creature = creature(state, player, id)?;
    let effect = ContinuousEffect::new(
        EffectId(state.next_effect),
        Selector::Objects(vec![creature.reference()]),
        Modification::BlocksAdditional(count),
        Duration::UntilEndOfTurn,
        state.next_timestamp,
    )
    .begun(player, state.turn().number());
    Ok(vec![Event::EffectCreated {
        effect: effect.into(),
    }])
}

/// The creature `id` on the battlefield, which the seat controls.
fn creature(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
) -> Result<&GameObject, ManualRefusal> {
    let object = own(state, player, id)?;
    if object.controller() != player {
        return Err(ManualRefusal::NotYours);
    }
    if object.zone() != Zone::Battlefield
        || !characteristics(state, id).is_some_and(|c| c.is_creature())
    {
        return Err(ManualRefusal::NotACreature(id));
    }
    Ok(object)
}
