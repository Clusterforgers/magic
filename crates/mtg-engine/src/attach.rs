//! Attachments: Auras, Equipment and Fortifications attached to a permanent (CR 303.4, 301.5, 701.3), the target an Aura
//! spell has through its enchant ability, and the state-based actions that keep an attachment where it may be
//! (CR 704.5m to 704.5p).

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Selector, TargetSpec, Targetable};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::{characteristics, controller_of_object};
use crate::choose::{Target, legal_targets, options_for};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::fold::invalid;
use crate::keywords::keywords_of;
use crate::object::{GameObject, ObjectKind};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// What a permanent that attaches to another is, by its subtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// An Aura: it enchants what its enchant ability allows (CR 303.4).
    Aura,
    /// An Equipment: it equips a creature (CR 301.5).
    Equipment,
    /// A Fortification: it fortifies a land (CR 301.6).
    Fortification,
}

const KINDS: [(&str, Kind); 3] = [
    ("Aura", Kind::Aura),
    ("Equipment", Kind::Equipment),
    ("Fortification", Kind::Fortification),
];

/// Whether a card prints the subtype of an attachment, so a game that has none skips their state-based actions.
pub(crate) fn definition_attaches(definition: &CardDefinition) -> bool {
    definition
        .subtypes
        .iter()
        .any(|subtype| KINDS.iter().any(|(name, _)| subtype.0 == *name))
}

/// What `id` is as an attachment, by the subtypes it prints and those effects gave it.
pub(crate) fn kind_of(state: &GameState, id: ObjectId) -> Option<Kind> {
    let object = state.object(id)?;
    let printed = |name: &str| object.card().subtypes.iter().any(|s| s.0 == name);
    if let Some((_, kind)) = KINDS.iter().find(|(name, _)| printed(name)) {
        return Some(*kind);
    }
    let chars = characteristics(state, id)?;
    KINDS
        .iter()
        .find(|(name, _)| chars.subtypes.iter().any(|s| s.0 == *name))
        .map(|(_, kind)| *kind)
}

/// What the enchant abilities of `id` allow, as one selector since every one of them holds (CR 702.5d); `None` for an
/// object with none.
pub(crate) fn enchant_restriction(state: &GameState, id: ObjectId) -> Option<Selector> {
    let mut found = false;
    let mut preds = Vec::new();
    for keyword in keywords_of(state, id) {
        if let Keyword::Enchant(selector) = keyword {
            found = true;
            preds.extend(selector.0.iter().cloned());
        }
    }
    found.then_some(Selector(preds))
}

/// Whether `attachment` may be attached to `host` as they stand: the Aura enchants what its enchant ability allows and is
/// not a creature (CR 303.4d, 303.4e), an Equipment that is not a creature equips another creature (CR 301.5c), a
/// Fortification fortifies another land (CR 301.6). A permanent never attaches to itself.
pub(crate) fn legal_host(state: &GameState, attachment: ObjectId, host: ObjectId) -> bool {
    if attachment == host {
        return false;
    }
    let (Some(object), Some(target)) = (state.object(attachment), state.object(host)) else {
        return false;
    };
    if object.zone() != Zone::Battlefield
        || target.zone() != Zone::Battlefield
        || target.kind() == ObjectKind::Ability
    {
        return false;
    }
    let (Some(chars), Some(host_chars)) = (characteristics(state, attachment), characteristics(state, host))
    else {
        return false;
    };
    match kind_of(state, attachment) {
        Some(Kind::Aura) => !chars.is_creature() && may_enchant(state, object, target),
        Some(Kind::Equipment) => !chars.is_creature() && host_chars.is_creature(),
        Some(Kind::Fortification) => host_chars.types.contains(CardType::Land),
        None => false,
    }
}

/// Whether the enchant abilities of the Aura allow `host`, judged for the Aura's controller.
fn may_enchant(state: &GameState, aura: &GameObject, host: &GameObject) -> bool {
    let Some(selector) = enchant_restriction(state, aura.id()) else {
        return true;
    };
    Scope::announcing(state, controller_of_object(state, aura), aura.reference())
        .selects(&selector, host.reference())
        .unwrap_or(false)
}

/// The target specification of an Aura spell: what its enchant abilities allow, one target (CR 303.4a). `None` for
/// anything but an Aura card.
pub(crate) fn spell_target(state: &GameState, id: ObjectId) -> Option<TargetSpec> {
    let object = state.object(id)?;
    if object.kind() == ObjectKind::Ability
        || !object.card().subtypes.iter().any(|s| s.0 == "Aura")
        || object.base_types().contains(CardType::Instant)
        || object.base_types().contains(CardType::Sorcery)
    {
        return None;
    }
    let selector = enchant_restriction(state, id)?;
    Some(TargetSpec {
        what: Targetable::Object(selector),
        bounds: Bounds::default(),
        divide: None,
    })
}

/// Whether `player` can cast the spell `id` as far as its target goes: an Aura spell needs something to enchant that it
/// may target (CR 601.2c); any other spell asks nothing here.
pub(crate) fn announceable(state: &GameState, player: PlayerId, id: ObjectId) -> bool {
    spell_target(state, id)
        .is_none_or(|spec| options_for(state, id, player, 0, &spec, true, None).is_some())
}

/// Whether `id` is an Aura spell on the stack.
pub(crate) fn is_aura_spell(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|object| object.zone() == Zone::Stack)
        && spell_target(state, id).is_some()
}

/// The permanent the Aura spell `id` is going to enchant: the target it announced, if that is still legal as it
/// resolves (CR 608.2b).
pub(crate) fn spell_host(state: &GameState, id: ObjectId) -> Option<ObjectId> {
    let spec = spell_target(state, id)?;
    let spell = state.object(id)?;
    let target = spell.choices().targets.first()?.first()?.object()?;
    legal_targets(state, &spec, id, spell.controller())
        .contains(&Target::Object(target))
        .then_some(target.id)
}

/// The attachment of an Aura spell that resolves: it enters attached to what it targeted (CR 303.4f).
pub(crate) fn entering(state: &GameState, id: ObjectId) -> Option<Event> {
    if !is_aura_spell(state, id) {
        return None;
    }
    Some(Event::Attached {
        object: id,
        to: spell_host(state, id)?,
    })
}

/// The permanents an Aura card that enters the battlefield without being cast under `controller` may enchant, in id
/// order (CR 303.4g); `None` for a card that is not an Aura.
pub(crate) fn hosts_when_entering(
    state: &GameState,
    aura: &GameObject,
    controller: PlayerId,
) -> Option<Vec<ObjectId>> {
    let spec = spell_target(state, aura.id())?;
    let Targetable::Object(selector) = spec.what else {
        return None;
    };
    let scope = Scope::announcing(state, controller, aura.reference());
    let found = scope.matching(&selector).unwrap_or_default();
    Some(
        found
            .into_iter()
            .filter(|named| {
                state.object(named.id).is_some_and(|host| {
                    host.zone() == Zone::Battlefield && host.kind() != ObjectKind::Ability
                })
            })
            .map(|named| named.id)
            .collect(),
    )
}

/// Attaches `object` to `to` (CR 701.3): it takes the time it became attached as its timestamp (CR 613.7a).
pub(crate) fn fold_attached(
    state: &mut GameState,
    object: ObjectId,
    to: ObjectId,
) -> Result<(), EngineError> {
    if object == to {
        return Err(invalid("a permanent cannot be attached to itself"));
    }
    let host = state
        .object(to)
        .filter(|host| host.zone() == Zone::Battlefield)
        .map(GameObject::reference)
        .ok_or_else(|| invalid(format!("{to:?} is not on the battlefield to be attached to")))?;
    let stamp = state.next_timestamp;
    let permanent = state
        .objects
        .get_mut(&object)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{object:?} is not on the battlefield to be attached")))?;
    if permanent.attached_to == Some(host) {
        return Err(invalid(format!("{object:?} is attached to {to:?} already")));
    }
    permanent.attached_to = Some(host);
    permanent.timestamp = stamp;
    state.next_timestamp += 1;
    state.possible.attachments = true;
    Ok(())
}

/// Takes `object` off what it is attached to.
pub(crate) fn fold_unattached(state: &mut GameState, object: ObjectId) -> Result<(), EngineError> {
    let permanent = state
        .objects
        .get_mut(&object)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{object:?} is not on the battlefield to be unattached")))?;
    if permanent.attached_to.take().is_none() {
        return Err(invalid(format!("{object:?} is not attached to anything")));
    }
    Ok(())
}

/// The state-based actions of attachments, all at once (CR 704.5m to 704.5p): an Aura attached to nothing or to what it may
/// not enchant goes to its owner's graveyard, an Equipment or Fortification attached to what it may not be becomes
/// unattached, and any other permanent attached to something becomes unattached. A host that left is no host (CR 400.7).

pub(crate) fn state_based(state: &GameState, leaving: &[ObjectId]) -> Vec<Event> {
    let mut events = Vec::new();
    if !state.possible.attachments {
        return events;
    }
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state
            .object(id)
            .filter(|object| state.is_alive(object.controller()) && !leaving.contains(&id))
        else {
            continue;
        };
        let attached = object
            .permanent()
            .is_some_and(|permanent| permanent.attached_to().is_some());
        let legal = || {
            state
                .host_of(id)
                .is_some_and(|host| legal_host(state, id, host))
        };
        match kind_of(state, id) {
            Some(Kind::Aura) if !legal() => events.push(Event::ZoneChanged {
                object: id,
                from: Zone::Battlefield,
                to: Zone::Graveyard(object.owner()),
                cause: ZoneCause::StateBased,
            }),
            Some(Kind::Equipment | Kind::Fortification) if attached && !legal() => {
                events.push(Event::Unattached { object: id });
            }
            None if attached => events.push(Event::Unattached { object: id }),
            _ => {}
        }
    }
    events
}
