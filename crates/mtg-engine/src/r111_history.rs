//! Facts the r111-03 cards read across turns and within one: whether each player cast a spell or put a nontoken
//! permanent onto the battlefield during their latest turn (`Condition::R111CastOrPutLastTurn`), the players each creature
//! attacked in its controller's latest combat (`Condition::R111AttackedLastCombat`), and, from the turn's log, the damage
//! spells dealt to players and the damage each spell cast this turn dealt (`R111DamagedThisTurnBy`,
//! `Ref::R111LastDamagedYou`, `ValueKind::R111SpellDamage`).

use std::collections::{BTreeMap, BTreeSet};

use mtg_cards::refs::Selector;
use mtg_cards::value::TallyEvent;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::choose::Target;
use crate::object::ObjectRef;
use crate::resolve::{ResolveError, Scope};
use crate::snapshot::Snapshot;
use crate::state::{GameState, Tally};

/// What the game keeps across turns.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) struct R111Game {
    /// The players who cast a spell or put a nontoken permanent onto the battlefield during their latest finished turn.
    pub(crate) cast_or_put: BTreeSet<PlayerId>,
    /// Each player's latest combat phase on their own turn: every creature declared as an attacker in it with the player
    /// it attacked (planeswalkers left out).
    pub(crate) last_combat: BTreeMap<PlayerId, Vec<(ObjectRef, PlayerId)>>,
}

impl R111Game {
    /// Whether nothing is kept yet.
    pub(crate) fn is_empty(&self) -> bool {
        self.cast_or_put.is_empty() && self.last_combat.is_empty()
    }
}

/// What the game keeps for the turn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(crate) struct R111Turn {
    /// The players who put a nontoken permanent onto the battlefield this turn.
    pub(crate) put_nontoken: BTreeSet<PlayerId>,
}

impl R111Turn {
    /// Whether nothing is kept yet.
    pub(crate) fn is_empty(&self) -> bool {
        self.put_nontoken.is_empty()
    }
}

/// A nontoken permanent entered under `controller`: the player who put it there is the controller of the spell or ability
/// resolving, else the player who played it or whose spell it was.
pub(crate) fn r111_note_put(state: &mut GameState, controller: PlayerId) {
    let by = state.resolving_controller().unwrap_or(controller);
    state.tallies.r111.put_nontoken.insert(by);
}

/// The attacks of the combat now ending or ended: each attacker with the player it attacked, from the turn's log.
fn combat_attacks(state: &GameState) -> Vec<(ObjectRef, PlayerId)> {
    state
        .tallies
        .records
        .iter()
        .skip(state.tallies.combat_from)
        .filter(|record| record.event == TallyEvent::Attacked)
        .filter_map(|record| match (record.subject, record.other) {
            (Some(attacker), Some(Target::Player(player))) => Some((attacker, player)),
            _ => None,
        })
        .collect()
}

/// A new combat phase begins: the one before it this turn becomes the active player's latest combat.
pub(crate) fn r111_combat_began(state: &mut GameState) {
    if state.turn.combats > 1 {
        let attacks = combat_attacks(state);
        state.r111.last_combat.insert(state.turn.active, attacks);
    }
}

/// The turn of `ending` ends, before its counts are cleared: what they did during it, and their latest combat when it had one.
pub(crate) fn r111_turn_ended(state: &mut GameState, ending: PlayerId) {
    let did =
        state.tallied(ending, Tally::Cast) > 0 || state.tallies.r111.put_nontoken.contains(&ending);
    if did {
        state.r111.cast_or_put.insert(ending);
    } else {
        state.r111.cast_or_put.remove(&ending);
    }
    if state.turn.combats > 0 {
        let attacks = combat_attacks(state);
        state.r111.last_combat.insert(ending, attacks);
    }
}

impl Scope<'_> {
    /// Whether "you" cast a spell or put a nontoken permanent onto the battlefield during your latest turn.
    pub(crate) fn r111_cast_or_put_last_turn(&self) -> bool {
        self.state.r111.cast_or_put.contains(&self.controller)
    }

    /// Whether the object `by` names attacked "you" during its controller's latest combat.
    pub(crate) fn r111_attacked_last_combat(
        &self,
        by: &mtg_cards::refs::Ref,
    ) -> Result<bool, ResolveError> {
        let you = self.controller;
        for target in self.refs(by)? {
            let Some(object) = target.object() else {
                continue;
            };
            let Some(controller) = self
                .state
                .object(object.id)
                .map(crate::object::GameObject::controller)
            else {
                continue;
            };
            if self
                .state
                .r111
                .last_combat
                .get(&controller)
                .is_some_and(|attacks| attacks.contains(&(object, you)))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The sources, as they were, of the damage dealt to "you" this turn that the selector matches, in the order dealt.
    fn r111_damage_to_you(&self, by: &Selector) -> Result<Vec<Snapshot>, ResolveError> {
        let you = self.controller;
        let mut found = Vec::new();
        for record in &self.state.tallies.records {
            let (TallyEvent::Damaged, Some(Target::Player(player)), Some(shot)) =
                (record.event, record.other, record.shot.as_ref())
            else {
                continue;
            };
            if player != you {
                continue;
            }
            let mut scope = self.clone();
            scope.as_was = true;
            scope.snapshots.insert(0, shot);
            if scope.selects(by, shot.reference())? {
                found.push(shot.clone());
            }
        }
        Ok(found)
    }

    /// Whether "you" were dealt damage this turn by a source the selector matches as it was.
    pub(crate) fn r111_damaged_this_turn_by(&self, by: &Selector) -> Result<bool, ResolveError> {
        Ok(!self.r111_damage_to_you(by)?.is_empty())
    }

    /// The controller of the last source the selector matches that dealt damage to "you" this turn.
    pub(crate) fn r111_last_damaged_you(&self, by: &Selector) -> Result<Vec<Target>, ResolveError> {
        Ok(self
            .r111_damage_to_you(by)?
            .last()
            .map(|shot| shot.controller)
            .filter(|player| self.state.is_alive(*player))
            .map(Target::Player)
            .into_iter()
            .collect())
    }

    /// The player who controlled the creature `of` names the last time the creature `by` names blocked it this turn,
    /// matched by object id so that a creature since put into a graveyard is still found.
    pub(crate) fn r111_blocked_controller(
        &self,
        of: &mtg_cards::refs::Ref,
        by: &mtg_cards::refs::Ref,
    ) -> Result<Vec<Target>, ResolveError> {
        let blocked: Vec<ObjectId> = self
            .refs(of)?
            .iter()
            .filter_map(|target| target.object().map(|object| object.id))
            .collect();
        let blockers: Vec<ObjectRef> = self
            .refs(by)?
            .iter()
            .filter_map(|target| target.object())
            .collect();
        let controller = self
            .state
            .tallies
            .records
            .iter()
            .rev()
            .filter(|record| record.event == TallyEvent::Blocked)
            .filter(|record| {
                record
                    .subject
                    .is_some_and(|subject| blockers.contains(&subject))
            })
            .find_map(|record| match (record.other, record.other_shot.as_ref()) {
                (Some(Target::Object(attacker)), Some(shot)) if blocked.contains(&attacker.id) => {
                    Some(shot.controller)
                }
                _ => None,
            });
        Ok(controller
            .filter(|player| self.state.is_alive(*player))
            .map(Target::Player)
            .into_iter()
            .collect())
    }

    /// The spells the players `caster` names cast this turn that the selector matches as cast, each with the damage it
    /// dealt this turn, in the order they were cast.
    pub(crate) fn r111_spell_damages(
        &self,
        caster: &mtg_cards::refs::Ref,
        filter: &Selector,
    ) -> Result<Vec<(Snapshot, u32)>, ResolveError> {
        let casters = self.players(caster)?;
        let mut found = Vec::new();
        for (event, player, shot) in &self.state.tallies.logged {
            if *event != TallyEvent::SpellCast || !casters.contains(player) {
                continue;
            }
            let mut scope = self.clone();
            scope.as_was = true;
            scope.snapshots.insert(0, shot);
            if !scope.selects(filter, shot.reference())? {
                continue;
            }
            found.push((shot.clone(), damage_by(self.state, shot.id, shot.counter)));
        }
        Ok(found)
    }
}

/// The damage the object, as this one object, dealt this turn.
fn damage_by(state: &GameState, id: ObjectId, counter: u32) -> u32 {
    state
        .tallies
        .records
        .iter()
        .filter(|record| record.event == TallyEvent::Damaged)
        .filter(|record| {
            record
                .subject
                .is_some_and(|subject| subject.id == id && subject.counter == counter)
        })
        .map(|record| record.amount)
        .sum()
}
