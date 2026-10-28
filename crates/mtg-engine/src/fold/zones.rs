//! Folding events that move objects between zones.

use mtg_cards::definition::Layout;
use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use super::opening::same_cards;
use super::{invalid, player_mut, priority_to_active, reset_passes};
use crate::announce::payments;
use crate::choose::Choices;
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::keywords::OptionalKind;
use crate::library::{Scried, Surveilled};
use crate::object::{FaceDown, GameObject, ObjectKind, PermanentState};
use crate::state::{GameState, Tally};
use crate::zones::{Placement, Zone};

/// Moves `id` to `to`, creating or dropping permanent state and leaving combat as needed.
pub(super) fn move_object(
    state: &mut GameState,
    id: ObjectId,
    to: Zone,
    placement: Placement,
) -> Result<(), EngineError> {
    if !state.zones.exists(to) {
        return Err(invalid(format!("zone {to:?} does not exist")));
    }
    // Whether it is a creature as it leaves the battlefield for a graveyard, before the move changes what it is; no other move
    // asks, so no other move works the layers out.
    let was_creature = matches!(to, Zone::Graveyard(_))
        && state
            .object(id)
            .is_some_and(|object| object.zone() == Zone::Battlefield)
        && crate::characteristics::characteristics(state, id)
            .is_some_and(|chars| chars.types.contains(CardType::Creature));
    // A permanent that enters takes the next timestamp, which its static abilities carry (CR 613.7).
    let timestamp = state.next_timestamp;
    if to == Zone::Battlefield {
        state.next_timestamp += 1;
    }
    let object = state
        .objects
        .get_mut(&id)
        .ok_or_else(|| invalid(format!("unknown object {id:?}")))?;
    // Who controlled it as it left, before a move out of play gives it back to its owner.
    let controlled_by = object.controller;
    let from = object.zone;
    if !state.zones.remove(from, id) {
        return Err(invalid(format!("{id:?} is not in {from:?}")));
    }
    state.zones.insert(to, id, placement);
    object.zone = to;
    state.commanders.moved(id);
    // A new zone makes it a new object (CR 400.7); what was announced for it on the stack goes with the stack.
    object.counter += 1;
    object.card_counters.clear();
    // A spell cast face down becomes a permanent that lies face down (CR 708.4); any other move shows the card (CR 708.9).
    if !(from == Zone::Stack
        && to == Zone::Battlefield
        && object
            .face_down
            .is_some_and(FaceDown::shows_characteristics))
    {
        object.face_down = None;
    }
    object.adventure = None;
    object.exiled_until = None;
    object.encoded_on = None;
    object.monarch_wait = None;
    object.cast_from_hand = false;
    object.cast_from_graveyard = false;
    // What the spell paid for stays with the permanent it becomes, for its abilities to read.
    let paid = if from == Zone::Stack && to == Zone::Battlefield {
        std::mem::take(&mut object.choices.optional_costs)
    } else {
        Vec::new()
    };
    let way = if from == Zone::Stack && to == Zone::Battlefield {
        object.choices.way
    } else {
        None
    };
    let x = if from == Zone::Stack && to == Zone::Battlefield {
        object.choices.x
    } else {
        None
    };
    // A modal double-faced card that resolves from the stack is the face it was cast as.
    let face = if from == Zone::Stack && to == Zone::Battlefield {
        object.choices.face
    } else {
        None
    };
    let spent = if from == Zone::Stack && to == Zone::Battlefield {
        object.choices.spent().to_vec()
    } else {
        Vec::new()
    };
    if from == Zone::Stack {
        object.choices = Choices::default();
    }
    let mut merged = Vec::new();
    // A permanent leaves from the phased-out zone only when the player who controls it leaves the game.
    if from.is_permanent_zone() {
        merged = crate::merged::taken(&mut object.permanent);
        object.permanent = None;
        state.combat.remove_object(id);
        // Creatures attacking a planeswalker that leaves go on attacking nothing (CR 506.4c).
        state.combat.planeswalker_left(id);
    }
    if to == Zone::Battlefield {
        object.permanent = Some(PermanentState::entering(
            timestamp, paid, way, x, face, spent,
        ));
    } else if to != Zone::Stack {
        object.controller = object.owner;
    }
    let (owner, token) = (object.owner, object.kind == ObjectKind::Token);
    let entered = object.reference();
    if to == Zone::Battlefield {
        state.tally_object(entered, None, Tally::Entered);
    }
    if from == Zone::Battlefield && matches!(to, Zone::Graveyard(_)) {
        state.tally(owner, Tally::Graved);
        if was_creature {
            tally_death(state, (owner, controlled_by), token);
        }
    }
    // A token is a token only on the battlefield (CR 111.7): anywhere else it ceases to exist.
    if token && !matches!(to, Zone::Battlefield | Zone::Stack) {
        crate::tokens::cease(state, id);
    }
    // Each card of a merged permanent that left is an object of its own in the new zone (CR 729.3).
    crate::merged::scatter(state, merged, owner, to, placement);
    Ok(())
}

/// Counts a creature put into a graveyard from the battlefield for its owner, and for the player who controlled it.
fn tally_death(state: &mut GameState, (owner, controller): (PlayerId, PlayerId), token: bool) {
    state.tally(owner, Tally::Died);
    state.tally(controller, Tally::DiedUnder);
    if !token {
        state.tally(owner, Tally::DiedNontoken);
        state.tally(controller, Tally::DiedNontokenUnder);
    }
}

fn expect_in(state: &GameState, id: ObjectId, zone: Zone) -> Result<(), EngineError> {
    if state.zones.get(zone).contains(&id) {
        Ok(())
    } else {
        Err(invalid(format!("{id:?} is not in {zone:?}")))
    }
}

fn set_controller(state: &mut GameState, id: ObjectId, player: PlayerId) {
    if let Some(object) = state.objects.get_mut(&id) {
        object.controller = player;
    }
}

/// A land played from a hand, or from exile, a graveyard or a library when a permission lets its player (CR 305.1), as
/// the face given.
fn land_played(
    state: &mut GameState,
    player: PlayerId,
    object: ObjectId,
    face: Option<u8>,
) -> Result<(), EngineError> {
    let from = [Zone::Exile, Zone::Graveyard(player), Zone::Library(player)]
        .into_iter()
        .find(|zone| state.zones.get(*zone).contains(&object))
        .unwrap_or(Zone::Hand(player));
    expect_in(state, object, from)?;
    move_object(state, object, Zone::Battlefield, Placement::Top)?;
    set_controller(state, object, player);
    if let Some(permanent) = state
        .objects
        .get_mut(&object)
        .and_then(|land| land.permanent.as_mut())
    {
        permanent.face = face;
    }
    state.turn.lands_played += 1;
    state.tally(player, Tally::LandsPlayed);
    reset_passes(state);
    Ok(())
}

/// A spell with rebound that is cast from a hand is exiled by a replacement effect as it resolves (CR 702.88a), so the
/// game has replacement effects to look for.
fn note_rebound(state: &mut GameState, spell: ObjectId, from: Zone) {
    if matches!(from, Zone::Hand(_))
        && crate::keywords::keywords_of(state, spell)
            .iter()
            .any(|keyword| matches!(keyword, mtg_cards::keyword::Keyword::Rebound))
    {
        state.possible.replacements = true;
    }
}

/// A spell with aftermath that is cast from a graveyard is exiled by a replacement effect when it leaves the stack
/// (CR 702.127a), so the game has replacement effects to look for.
fn note_aftermath(state: &mut GameState, spell: ObjectId, from: Zone) {
    if matches!(from, Zone::Graveyard(_)) && crate::keywords::aftermath::has(state, spell) {
        state.possible.replacements = true;
    }
}

/// A card is cast: it moves to the stack as the spell, with what was announced for it, and lies face down when it is cast that way.
fn spell_cast(
    state: &mut GameState,
    player: PlayerId,
    object: ObjectId,
    choices: &Choices,
) -> Result<(), EngineError> {
    state.tally(player, Tally::Cast);
    for target in choices.targeted_objects() {
        state.tally_object(target, Some(player), Tally::Targeted);
    }
    state.rng_index += u64::from(choices.random_modes);
    // A card is cast from its hand, or from the zone a permission names (CR 601.3): never from play.
    let from = state
        .objects
        .get(&object)
        .map(|spell| spell.zone)
        .ok_or_else(|| invalid(format!("unknown object {object:?}")))?;
    if matches!(from, Zone::Stack | Zone::Battlefield) {
        return Err(invalid(format!(
            "{object:?} is in {from:?}, not a card to cast"
        )));
    }
    expect_in(state, object, from)?;
    // The tax counts casts from the command zone (CR 903.8).
    state.commanders.cast(object, from);
    move_object(state, object, Zone::Stack, Placement::Top)?;
    set_controller(state, object, player);
    let instant_or_sorcery = state.objects.get(&object).is_some_and(|spell| {
        spell.card().is_type(CardType::Instant) || spell.card().is_type(CardType::Sorcery)
    });
    if instant_or_sorcery {
        state.tally(player, Tally::CastInstantOrSorcery);
    }
    // The face cast is the card a spell is from now on: its buyback and flashback below are those of the face.
    if let Some(spell) = state.objects.get_mut(&object) {
        spell.choices.clone_from(choices);
        spell.cast_from_hand = matches!(from, Zone::Hand(_));
        spell.cast_from_graveyard = matches!(from, Zone::Graveyard(_));
        if choices.way == Some(CastWay::FaceDown) {
            spell.face_down = Some(FaceDown::cast_as(&spell.def));
        }
    }
    note_rebound(state, object, from);
    note_aftermath(state, object, from);
    // A spell cast for its flashback cost, or with its buyback cost paid, is moved by a replacement effect.
    let bought_back = state.objects.get(&object).is_some_and(|spell| {
        payments(spell.card(), &choices.optional_costs, OptionalKind::Buyback) > 0
    });
    // An Adventure that resolves is exiled by a replacement as well (CR 715.4).
    let adventure = state.objects.get(&object).is_some_and(|spell| {
        spell.whole_card().layout == Layout::Adventure && spell.face() == Some(1)
    });
    if choices.way.is_some_and(CastWay::exiles_the_spell) || bought_back || adventure {
        state.possible.replacements = true;
    }
    reset_passes(state);
    Ok(())
}

pub(super) fn apply(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::LandPlayed {
            player,
            object,
            face,
        } => land_played(state, *player, *object, *face)?,
        Event::SpellCast {
            player,
            object,
            choices,
        } => spell_cast(state, *player, *object, choices)?,
        Event::SpellResolved { object } => resolve_spell(state, *object)?,
        Event::CommanderStayed { object } => state.commanders.stay(*object),
        Event::CardParted {
            permanent,
            card,
            to,
        } => crate::merged::part(state, *permanent, *card, *to)?,
        Event::Mutated {
            object,
            spell,
            over,
            ..
        } => crate::merged::fold(state, *object, *spell, *over)?,
        Event::ZoneChanged {
            object,
            from,
            to,
            cause,
        } => {
            expect_in(state, *object, *from)?;
            if state.object(*object).map(GameObject::kind) == Some(ObjectKind::Ability) {
                return Err(invalid(format!(
                    "{object:?} is an ability, which does not change zones"
                )));
            }
            let before = state.objects.get(object).map(|o| o.controller);
            move_object(state, *object, *to, Placement::Top)?;
            if *cause == ZoneCause::Discarded
                && let Zone::Hand(player) = *from
            {
                tally_discard(state, player, *object);
            }
            if *cause == ZoneCause::Adventure
                && let Some(card) = state.objects.get_mut(object)
            {
                card.adventure = before;
            }
            if let Some(how) = cause.face_down(state.turn.number())
                && let Some(object) = state.objects.get_mut(object)
            {
                object.face_down = Some(how);
            }
            if let ZoneCause::ExiledUntil { by } = cause {
                state.possible.linked_exile = true;
                if let Some(object) = state.objects.get_mut(object) {
                    object.exiled_until = Some(*by);
                }
            }
            if let ZoneCause::Encoded { by } = cause
                && let Some(object) = state.objects.get_mut(object)
            {
                object.encoded_on = Some(*by);
            }
            if let ZoneCause::ExiledUntilMonarch { by } = cause {
                state.possible.linked_exile = true;
                if let Some(object) = state.objects.get_mut(object) {
                    object.monarch_wait = Some((*by, false));
                }
            }
        }
        Event::Entered {
            object,
            tapped,
            controller,
            chosen,
        } => enter(state, *object, *tapped, *controller, chosen.as_ref())?,
        Event::CardDrawn { player, object } => card_drawn(state, *player, *object)?,
        Event::CardDiscarded { player, object } => {
            expect_in(state, *object, Zone::Hand(*player))?;
            let owner = owner_of(state, *object)?;
            move_object(state, *object, Zone::Graveyard(owner), Placement::Top)?;
            tally_discard(state, *player, *object);
        }
        Event::CreatureDied { object, .. } => {
            expect_in(state, *object, Zone::Battlefield)?;
            let owner = owner_of(state, *object)?;
            move_object(state, *object, Zone::Graveyard(owner), Placement::Top)?;
        }
        Event::ObjectLeftGame { object } => leave_game(state, *object)?,
        other => return Err(invalid(format!("not a zone event: {other:?}"))),
    }
    Ok(())
}

/// Moves the top card of the library to the hand and counts the draw, also the draw of the draw step.
fn card_drawn(
    state: &mut GameState,
    player: PlayerId,
    object: ObjectId,
) -> Result<(), EngineError> {
    if state.zones.get(Zone::Library(player)).last() != Some(&object) {
        return Err(invalid("drawn card is not on top of the library"));
    }
    move_object(state, object, Zone::Hand(player), Placement::Top)?;
    state.tally(player, Tally::Drawn);
    if state.turn.step == crate::turn::Step::Draw && state.active_player() == Some(player) {
        state.tally(player, Tally::C6BlkDrawStepDrawn);
    }
    Ok(())
}

/// Counts the card `id` among the discards of `player` this turn when it lies in a graveyard (mayhem, CR 702.190a), and
/// the player's discards, wherever the card is later.
fn tally_discard(state: &mut GameState, player: PlayerId, id: ObjectId) {
    state.tally(player, Tally::Discarded);
    let lies_there = state
        .object(id)
        .filter(|card| matches!(card.zone(), Zone::Graveyard(_)))
        .map(GameObject::reference);
    if let Some(reference) = lies_there {
        state.tally_object(reference, Some(player), Tally::Discarded);
    }
}

/// Sets how a permanent that an effect put onto the battlefield entered: tapped or not, under whose control.
fn enter(
    state: &mut GameState,
    id: ObjectId,
    tapped: bool,
    controller: PlayerId,
    chosen: Option<&crate::object::Chosen>,
) -> Result<(), EngineError> {
    player_mut(state, controller)?;
    let object = state
        .objects
        .get_mut(&id)
        .ok_or_else(|| invalid(format!("unknown object {id:?}")))?;
    let Some(permanent) = object.permanent.as_mut() else {
        return Err(invalid(format!(
            "{id:?} entered but is not on the battlefield"
        )));
    };
    permanent.tapped = tapped;
    permanent.chosen = chosen.cloned();
    object.controller = controller;
    Ok(())
}

/// Puts the cards a scry looked at back: they are the top cards of the library, `top` goes on top and `bottom`
/// under everything else, each list as it lies from the top down.
pub(super) fn scry(state: &mut GameState, scried: &Scried) -> Result<(), EngineError> {
    let zone = Zone::Library(scried.player);
    let library = state.zones.get(zone);
    let looked: Vec<ObjectId> = scried.top.iter().chain(&scried.bottom).copied().collect();
    let Some(split) = library.len().checked_sub(looked.len()) else {
        return Err(invalid(
            "a scry looked at more cards than the library holds",
        ));
    };
    let (rest, on_top) = library.split_at(split);
    if !same_cards(on_top, &looked) {
        return Err(invalid(
            "a scry did not put back the top cards of the library",
        ));
    }
    let arranged = scried
        .bottom
        .iter()
        .rev()
        .chain(rest)
        .chain(scried.top.iter().rev())
        .copied()
        .collect();
    state.zones.set(zone, arranged);
    Ok(())
}

/// Arranges the cards a surveil looked at: they are the top cards of the library, `top` goes on top as it lies from
/// the top down and `graveyard` just beneath it, where the moves that follow take it from.
pub(super) fn surveil(state: &mut GameState, surveilled: &Surveilled) -> Result<(), EngineError> {
    let zone = Zone::Library(surveilled.player);
    let library = state.zones.get(zone);
    let looked: Vec<ObjectId> = surveilled
        .top
        .iter()
        .chain(&surveilled.graveyard)
        .copied()
        .collect();
    let Some(split) = library.len().checked_sub(looked.len()) else {
        return Err(invalid(
            "a surveil looked at more cards than the library holds",
        ));
    };
    let (rest, on_top) = library.split_at(split);
    if !same_cards(on_top, &looked) {
        return Err(invalid(
            "a surveil did not look at the top cards of the library",
        ));
    }
    let arranged = rest
        .iter()
        .chain(surveilled.graveyard.iter().rev())
        .chain(surveilled.top.iter().rev())
        .copied()
        .collect();
    state.zones.set(zone, arranged);
    crate::sight::show(state, Some(surveilled.player), &looked);
    state.tally(surveilled.player, Tally::Surveilled);
    Ok(())
}

/// Moves cards that are in a library to its bottom, in the order given from the top down.
pub(super) fn bottom(
    state: &mut GameState,
    player: PlayerId,
    cards: &[ObjectId],
) -> Result<(), EngineError> {
    let zone = Zone::Library(player);
    let library = state.zones.get(zone);
    let distinct = cards
        .iter()
        .enumerate()
        .all(|(at, card)| !cards[..at].contains(card));
    if !distinct || !cards.iter().all(|card| library.contains(card)) {
        return Err(invalid(
            "bottomed cards are not all in the library, once each",
        ));
    }
    let arranged = cards
        .iter()
        .rev()
        .chain(library.iter().filter(|card| !cards.contains(card)))
        .copied()
        .collect();
    state.zones.set(zone, arranged);
    Ok(())
}

fn owner_of(state: &GameState, id: ObjectId) -> Result<PlayerId, EngineError> {
    state
        .object(id)
        .map(GameObject::owner)
        .ok_or_else(|| invalid(format!("unknown object {id:?}")))
}

fn resolve_spell(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    // What a resolution put on the stack (a copy, a spell cast during it) is above the spell, which leaves from beneath it.
    if !state.zones.get(Zone::Stack).contains(&id) {
        return Err(invalid(format!("{id:?} is not on the stack")));
    }
    let types = state
        .object(id)
        .map(GameObject::base_types)
        .ok_or_else(|| invalid(format!("unknown object {id:?}")))?;
    if types.contains(CardType::Instant) || types.contains(CardType::Sorcery) {
        let owner = owner_of(state, id)?;
        move_object(state, id, Zone::Graveyard(owner), Placement::Top)?;
    } else {
        move_object(state, id, Zone::Battlefield, Placement::Top)?;
    }
    priority_to_active(state);
    Ok(())
}

fn leave_game(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    let object = state
        .objects
        .remove(&id)
        .ok_or_else(|| invalid(format!("unknown object {id:?}")))?;
    if !state.zones.remove(object.zone, id) {
        return Err(invalid(format!("{id:?} is not in {:?}", object.zone)));
    }
    if state.resolving.as_ref().is_some_and(|r| r.object == id) {
        state.resolving = None;
    }
    state.combat.remove_object(id);
    state.combat.planeswalker_left(id);
    player_mut(state, object.owner).map(|_| ())
}
