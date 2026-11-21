//! Copy effects of static abilities (`Modification::R707CopyOf`, CR 707.2, 613.2a): "Enchanted creature is a copy of the
//! chosen creature", "as long as the top card of your graveyard is a creature card, this creature has the full text of
//! that card". The copy is the first layer, which the layers read off the copiable values (`PermanentState::copy`),
//! so the ability is read from the card's own rules and housekeeping keeps the copies it makes in step with it: each
//! permanent it names is a copy of the object it names now, begun again when that object changes and ended once the
//! ability makes it no longer.

use mtg_cards::ability::Ability;
use mtg_cards::modify::Modification as Wanted;
use mtg_core::ids::ObjectId;

use super::{BecameCopy, CopyEffect, copiable_values};
use crate::characteristics::controller_of_object;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// One copy a static ability makes now: the permanent, the permanent with the ability, and the object copied.
struct Made {
    object: ObjectId,
    source: ObjectRef,
    of: ObjectRef,
    except: Vec<Wanted>,
}

impl CopyEffect {
    /// The permanents whose static abilities keep this effect or one it is over going, each with the object copied.
    pub(crate) fn r707_static_ends(&self) -> Vec<(ObjectRef, ObjectRef)> {
        let mut found: Vec<(ObjectRef, ObjectRef)> = self
            .under
            .as_ref()
            .map(|under| under.r707_static_ends())
            .unwrap_or_default();
        if let Some(super::C10XpCopyEnds::R707Static(by)) = self.ends {
            found.push((by, self.of));
        }
        found
    }
}

/// The copies the static copy abilities of the permanents on the battlefield make now, read from their cards' own rules
/// (the face shown), so a copy that took the ability away keeps it going.
fn made(state: &GameState) -> Vec<Made> {
    let mut found = Vec::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        // The card as printed, not the copy it may be under: the copy does not take its own ability away.
        let face = object.def.face(object.face().unwrap_or(0));
        for rules in crate::ability_reader::printed_rules(face) {
            let Ability::Static {
                affected,
                mods,
                condition,
                ..
            } = &**rules
            else {
                continue;
            };
            let scope = Scope::announcing(
                state,
                controller_of_object(state, object),
                object.reference(),
            );
            if condition
                .as_ref()
                .is_some_and(|condition| !scope.holds(condition).unwrap_or(false))
            {
                continue;
            }
            for wanted in mods {
                let Wanted::R707CopyOf { of, except } = wanted else {
                    continue;
                };
                let Some(copied) = scope
                    .objects(of)
                    .ok()
                    .and_then(|named| named.first().copied())
                else {
                    continue;
                };
                for target in scope.objects(affected).unwrap_or_default() {
                    if state.object(target.id).is_some_and(|it| {
                        it.zone() == Zone::Battlefield && it.counter() == target.counter
                    }) {
                        found.push(Made {
                            object: target.id,
                            source: object.reference(),
                            of: copied,
                            except: except.clone(),
                        });
                    }
                }
            }
        }
    }
    found
}

/// The `CopyEnded` events of the static copies no static ability makes any more, then the `BecameCopy` events of those
/// it makes that are not yet in place (CR 613.2a): the objects copied are read as they are now.
pub(crate) fn r707_static_copies(state: &GameState) -> Vec<Event> {
    if !state.statics_possible {
        return Vec::new();
    }
    let made = made(state);
    let mut events = Vec::new();
    let mut kept: Vec<(ObjectId, ObjectRef, ObjectRef)> = Vec::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(copy) = state.object(id).and_then(|object| object.copy_effect()) else {
            continue;
        };
        for (by, of) in copy.r707_static_ends() {
            if made
                .iter()
                .any(|it| it.object == id && it.source == by && it.of == of)
            {
                kept.push((id, by, of));
            } else {
                events.push(Event::CopyEnded {
                    object: id,
                    next_turn_of: None,
                    attached_by: None,
                    r6104_tapped: None,
                    r707_static: Some(by),
                });
            }
        }
    }
    for it in made {
        if kept.contains(&(it.object, it.source, it.of)) {
            continue;
        }
        let Some(mut values) = copiable_values(state, it.of, None) else {
            continue;
        };
        if !it
            .except
            .iter()
            .all(|modification| values.except(modification))
        {
            continue;
        }
        events.push(Event::BecameCopy(BecameCopy {
            object: it.object,
            of: it.of,
            definition: std::sync::Arc::new(values),
            until_end_of_turn: false,
            until_next_turn_of: None,
            attached_by: None,
            r6104_while_tapped: None,
            r707_static: Some(it.source),
            r115_until_face_down: false,
        }));
    }
    events
}
