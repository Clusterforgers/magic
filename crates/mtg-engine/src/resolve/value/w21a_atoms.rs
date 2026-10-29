//! The predicate atoms of the open selector language that read an object's own state or history: modified, face down,
//! a player target, and a card put into a graveyard from the battlefield this turn.

use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::Subtype;

use super::{ResolveError, Scope, Seen};
use crate::object::{GameObject, ObjectRef};
use crate::state::Tally;
use crate::zones::Zone;

impl Scope<'_> {
    /// Whether a predicate of this module holds for the object; false for any other predicate.
    pub(super) fn w21a_pred(
        &self,
        pred: &Pred,
        reference: ObjectRef,
        seen: &Seen<'_>,
    ) -> Result<bool, ResolveError> {
        Ok(match pred {
            Pred::Modified => self.w21a_modified(seen)?,
            Pred::FaceDown => self.state.object(reference.id).is_some_and(|object| {
                object.reference() == reference && object.face_down().is_some()
            }),
            Pred::TargetsPlayer(who) => {
                let players = self.players(who)?;
                seen.zone == Zone::Stack
                    && seen.card.is_some_and(|card| {
                        card.choices()
                            .targets
                            .iter()
                            .flatten()
                            .filter_map(|target| target.player())
                            .any(|player| players.contains(&player))
                    })
            }
            Pred::GravedThisTurn => {
                matches!(seen.zone, Zone::Graveyard(_))
                    && self
                        .state
                        .object_tallied(reference, None, Tally::W21aGraved)
                        > 0
            }
            _ => false,
        })
    }

    /// Whether the permanent is modified (CR 700.9): it holds a counter, or an Equipment or an Aura its controller
    /// controls is attached to it.
    fn w21a_modified(&self, seen: &Seen<'_>) -> Result<bool, ResolveError> {
        let Some(host) = seen.card.filter(|_| seen.zone == Zone::Battlefield) else {
            return Ok(false);
        };
        if seen.counters.c10_any_total() > 0 {
            return Ok(true);
        }
        let equipment = Selector(vec![Pred::Subtype(Subtype("Equipment".to_owned()))]);
        let aura = Selector(vec![Pred::Subtype(Subtype("Aura".to_owned()))]);
        for id in self.state.attachments_of(host.id()) {
            let Some(object) = self.state.object(id) else {
                continue;
            };
            let attached = GameObject::reference(object);
            if self.selects(&equipment, attached)?
                || (object.controller() == seen.controller && self.selects(&aura, attached)?)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
