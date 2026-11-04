//! The predicates over how a spell was cast, a card's place in a library, combat pairings, being targeted, the greatest
//! value of a characteristic in a group, and suspend.

use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::value::StatKind;

use super::{ResolveError, Scope, Seen, zone_kind};
use crate::announce::payments;
use crate::combat::AttackTarget;
use crate::keywords::suspend::time_counter;
use crate::keywords::{OptionalKind, keywords_of};
use crate::object::{GameObject, ObjectKind, ObjectRef};
use crate::zones::Zone;

impl Scope<'_> {
    /// Whether a predicate of this module holds for the object; false for any other predicate.
    pub(super) fn fe1a_pred(
        &self,
        pred: &Pred,
        reference: ObjectRef,
        seen: &Seen<'_>,
    ) -> Result<bool, ResolveError> {
        Ok(match pred {
            Pred::WasCast { by, from } => self.fe1a_was_cast(by.as_ref(), *from, seen)?,
            Pred::CastWith(way) => {
                Self::fe1a_cast_object(seen).is_some_and(|card| card.cast_way() == Some(*way))
            }
            Pred::Attacking => self
                .state
                .combat()
                .attackers()
                .iter()
                .any(|a| a.object() == reference.id),
            Pred::AttackingPlayer(who) => {
                let attacked = self.players(who)?;
                self.state.combat().attackers().iter().any(|a| {
                    a.object() == reference.id
                        && matches!(a.target(), AttackTarget::Player(player) if attacked.contains(&player))
                })
            }
            Pred::Blocking => self
                .state
                .combat()
                .attackers()
                .iter()
                .any(|a| a.blockers().contains(&reference.id)),
            Pred::Unblocked => self.state.combat().is_unblocked(self.state, reference.id),
            Pred::TopOfLibrary => self.fe1a_library_end(reference, seen, true),
            Pred::BottomOfLibrary => self.fe1a_library_end(reference, seen, false),
            Pred::BlockedBy(named) => {
                let blockers = self.objects(named)?;
                self.state.combat().attackers().iter().any(|attacker| {
                    attacker.object() == reference.id
                        && blockers.iter().any(|b| attacker.blockers().contains(&b.id))
                })
            }
            Pred::Blocks(named) => {
                let attackers = self.objects(named)?;
                self.state.combat().attackers().iter().any(|attacker| {
                    attacker.blockers().contains(&reference.id)
                        && attackers.iter().any(|a| a.id == attacker.object())
                })
            }
            Pred::TargetedBy(named) => self.fe1a_targeted_by(named, reference)?,
            Pred::SingleTarget => {
                seen.zone == Zone::Stack
                    && seen
                        .card
                        .is_some_and(|card| card.choices().targets.iter().flatten().count() == 1)
            }
            Pred::Kicked => Self::fe1a_cast_object(seen).is_some_and(|card| {
                payments(card.card(), card.paid_costs(), OptionalKind::Kicker) > 0
            }),
            Pred::GreatestAmong { stat, group } => self.fe1a_greatest(*stat, group, seen, false)?,
            // Like `HasKeyword`, an object that changed zones has the keywords its snapshot holds.
            Pred::HasKeywordNamed(name) => match seen.keywords {
                Some(held) => held.iter().any(|keyword| keyword.name() == name),
                None => seen.card.is_some_and(|card| {
                    keywords_of(self.state, card.id())
                        .into_iter()
                        .any(|keyword| keyword.name() == name)
                }),
            },
            Pred::LeastAmong { stat, group } => self.fe1a_greatest(*stat, group, seen, true)?,
            Pred::Suspended => {
                seen.zone == Zone::Exile
                    && seen.card.is_some_and(|card| {
                        card.card_counters_of(&time_counter()) > 0
                            && keywords_of(self.state, card.id())
                                .iter()
                                .any(|keyword| matches!(keyword, Keyword::Suspend { .. }))
                    })
            }
            _ => false,
        })
    }

    /// The card of a spell on the stack or of a permanent, the only objects that were cast.
    fn fe1a_cast_object<'s>(seen: &Seen<'s>) -> Option<&'s GameObject> {
        seen.card.filter(|_| {
            seen.zone == Zone::Battlefield
                || (seen.zone == Zone::Stack && seen.kind != ObjectKind::Ability)
        })
    }

    /// Whether the object was cast by a player `by` names (anyone when absent) from a zone of the kind `from` (anywhere when absent).
    fn fe1a_was_cast(
        &self,
        by: Option<&Ref>,
        from: Option<ZoneKind>,
        seen: &Seen<'_>,
    ) -> Result<bool, ResolveError> {
        let Some(card) = Self::fe1a_cast_object(seen) else {
            return Ok(false);
        };
        // A spell still being announced has not left the zone it is cast from.
        let record = match self.casting {
            Some((cast, caster)) if cast == card.id() => Some((zone_kind(card.zone()), caster)),
            _ => card.cast_record(),
        };
        let Some((cast_from, caster)) = record else {
            return Ok(false);
        };
        if from.is_some_and(|zone| zone != cast_from) {
            return Ok(false);
        }
        match by {
            Some(named) => Ok(self.players(named)?.contains(&caster)),
            None => Ok(true),
        }
    }

    /// Whether the object is the top (or the bottom) card of the library it lies in.
    fn fe1a_library_end(&self, reference: ObjectRef, seen: &Seen<'_>, top: bool) -> bool {
        let Zone::Library(_) = seen.zone else {
            return false;
        };
        let cards = self.state.zone(seen.zone);
        let end = if top { cards.last() } else { cards.first() };
        end == Some(&reference.id)
    }

    /// Whether the object is a target of a spell or ability on the stack that the reference names.
    fn fe1a_targeted_by(&self, named: &Ref, reference: ObjectRef) -> Result<bool, ResolveError> {
        Ok(self.objects(named)?.into_iter().any(|source| {
            self.state
                .object(source.id)
                .filter(|object| object.counter() == source.counter && object.zone() == Zone::Stack)
                .is_some_and(|object| object.choices().targeted_objects().contains(&reference))
        }))
    }

    /// Whether no object the group selector matches has more (or, with `least`, less) of the characteristic than the object.
    fn fe1a_greatest(
        &self,
        stat: StatKind,
        group: &Selector,
        seen: &Seen<'_>,
        least: bool,
    ) -> Result<bool, ResolveError> {
        let Some(mine) = Self::stat(seen, stat) else {
            return Ok(false);
        };
        for other in self.matching(group)? {
            let theirs = self.seen(other).and_then(|held| Self::stat(&held, stat));
            if theirs.is_some_and(|value| if least { value < mine } else { value > mine }) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
