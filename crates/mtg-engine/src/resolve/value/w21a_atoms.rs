//! The predicate atoms of the open selector language that read an object's own state or history: modified, face down,
//! a player target, and a card put into a graveyard from the battlefield this turn.

use mtg_cards::refs::{Pred, Selector, ZoneKind};
use mtg_cards::value::CastWay;
use mtg_core::card::Subtype;

use super::{ResolveError, Scope, Seen};
use crate::object::{GameObject, ObjectRef};
use crate::state::Tally;
use crate::zones::Zone;

/// Whether `seen` is a spell paid with mana from a snow source of one of its own colors (CR 106.3).
fn r113_snow_of_own_color(seen: &Seen<'_>) -> bool {
    seen.zone == Zone::Stack
        && seen.card.is_some_and(|card| {
            card.mana_spent_from()
                .iter()
                .filter(|source| {
                    source
                        .traits
                        .supertypes
                        .contains(&mtg_core::card::Supertype::Snow)
                })
                .flat_map(|source| &source.r113_mana)
                .filter_map(|mana| mana.color())
                .any(|color| seen.chars.colors.contains(color))
        })
}

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
            Pred::RingBearer => {
                seen.zone == Zone::Battlefield
                    && self
                        .state
                        .object(reference.id)
                        .is_some_and(|object| object.reference() == reference)
                    && crate::characteristics::controller_of(self.state, reference.id)
                        .is_some_and(|player| self.state.ring_bearer(player) == Some(reference.id))
            }
            Pred::FaceDown => {
                // A spell still being announced is face down when it is cast that way (morph, disguise).
                let announcing = self.casting.is_some_and(|(cast, _)| cast == reference.id)
                    && self
                        .cast_targets
                        .as_ref()
                        .is_some_and(|cast| cast.choices.way.is_some_and(CastWay::casts_face_down));
                announcing
                    || self.state.object(reference.id).is_some_and(|object| {
                        object.reference() == reference && object.face_down().is_some()
                    })
            }
            Pred::TargetsPlayer(who) => {
                let players = self.players(who)?;
                if let Some(found) = self.r305b_casting_targets_player(reference, &players) {
                    return Ok(found);
                }
                self.r114_stack_choices(reference, seen)
                    .is_some_and(|choices| {
                        choices
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
            Pred::SharesColorWithLastCast => self
                .state
                .tallies
                .logged
                .iter()
                .rev()
                .find(|(event, _, _)| *event == mtg_cards::value::TallyEvent::SpellCast)
                .is_some_and(|(_, _, last)| {
                    seen.chars
                        .colors
                        .iter()
                        .any(|color| last.characteristics.colors.contains(color))
                }),
            Pred::CouldAttackThisTurn => self.state.tallies.r5522.could_attack.contains(&reference),
            Pred::GravedThisCombat => {
                matches!(seen.zone, Zone::Graveyard(_))
                    && self.state.tallies.r5522.combat_graved.contains(&reference)
            }
            Pred::SpentFrom { from, least } => {
                seen.zone == Zone::Stack
                    && seen.card.is_some_and(|card| {
                        let spent: u32 = card
                            .mana_spent_from()
                            .iter()
                            .filter(|source| from.holds(&source.traits))
                            .map(|source| source.count)
                            .sum();
                        spent >= *least
                    })
            }
            Pred::R113SnowOfOwnColor => r113_snow_of_own_color(seen),
            Pred::ArrivedThisTurn => [
                ZoneKind::Battlefield,
                ZoneKind::Hand,
                ZoneKind::Library,
                ZoneKind::Graveyard,
                ZoneKind::Stack,
                ZoneKind::Exile,
                ZoneKind::Command,
                ZoneKind::Outside,
            ]
            .into_iter()
            .any(|origin| {
                self.state
                    .object_tallied(reference, None, Tally::ArrivedFrom(origin))
                    > 0
            }),
            Pred::ArrivedFrom(origin) => {
                self.state
                    .object_tallied(reference, None, Tally::ArrivedFrom(*origin))
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
