//! Combat state and the legality of attacks and blocks.

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::characteristics::{cant_attack, cant_block, characteristics, controller_of};
use crate::decision::{AttackOptions, AttackerOption, BlockOptions, BlockerOption};
use crate::error::IllegalAnswer;
use crate::events::Event;
use crate::keywords;
use crate::state::GameState;
use crate::zones::Zone;

/// What an attacking creature is attacking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum AttackTarget {
    /// A player.
    Player(PlayerId),
    /// A planeswalker (damage to loyalty is not implemented until counters exist).
    Planeswalker(ObjectId),
}

/// Who or what receives damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum DamageTarget {
    /// A player's life total.
    Player(PlayerId),
    /// A permanent on the battlefield.
    Object(ObjectId),
}

/// An attacking creature and its blockers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Attacker {
    pub(crate) object: ObjectId,
    pub(crate) target: AttackTarget,
    pub(crate) blocked: bool,
    pub(crate) blockers: Vec<ObjectId>,
    pub(crate) assignment: Option<Vec<(ObjectId, u32)>>,
}

impl Attacker {
    /// The attacking creature.
    pub const fn object(&self) -> ObjectId {
        self.object
    }

    /// What it attacks.
    pub const fn target(&self) -> AttackTarget {
        self.target
    }

    /// Whether it was blocked (it stays blocked even if every blocker leaves).
    pub const fn blocked(&self) -> bool {
        self.blocked
    }

    /// Creatures currently blocking it.
    pub fn blockers(&self) -> &[ObjectId] {
        &self.blockers
    }
}

/// The combat in progress this turn.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CombatState {
    pub(crate) attackers: Vec<Attacker>,
    pub(crate) attackers_declared: bool,
    pub(crate) blockers_declared: Vec<PlayerId>,
}

impl CombatState {
    /// Declared attackers.
    pub fn attackers(&self) -> &[Attacker] {
        &self.attackers
    }

    /// Whether the declare-attackers choice has been made this turn.
    pub const fn attackers_declared(&self) -> bool {
        self.attackers_declared
    }

    /// Defending players who have already declared blockers.
    pub fn blockers_declared(&self) -> &[PlayerId] {
        &self.blockers_declared
    }

    /// Removes a creature that left the battlefield from combat (CR 506.4).
    pub(crate) fn remove_object(&mut self, id: ObjectId) {
        self.attackers.retain(|a| a.object != id);
        for attacker in &mut self.attackers {
            attacker.blockers.retain(|b| *b != id);
            if let Some(assignment) = &mut attacker.assignment {
                assignment.retain(|(b, _)| *b != id);
            }
        }
    }

    /// Removes attackers aimed at a player or planeswalker that left the game.
    pub(crate) fn remove_attackers_targeting(&mut self, target: AttackTarget) {
        self.attackers.retain(|a| a.target != target);
    }

    /// Forgets how attackers divided their damage: each damage step starts with every attacker undivided.
    pub(crate) fn clear_assignments(&mut self) {
        for attacker in &mut self.attackers {
            attacker.assignment = None;
        }
    }
}

/// The player an attack is aimed at, i.e. who may block it.
pub fn defending_player(state: &GameState, target: AttackTarget) -> Option<PlayerId> {
    match target {
        AttackTarget::Player(p) => Some(p),
        AttackTarget::Planeswalker(id) => controller_of(state, id),
    }
}

fn is_untapped_creature(state: &GameState, id: ObjectId) -> bool {
    let Some(object) = state.object(id) else {
        return false;
    };
    object.zone() == Zone::Battlefield
        && object.permanent().is_some_and(|p| !p.tapped())
        && characteristics(state, id).is_some_and(|c| c.is_creature())
}

/// Whether `id` may be declared as an attacker by `player` now.
pub(crate) fn can_attack(state: &GameState, id: ObjectId, player: PlayerId) -> bool {
    controller_of(state, id) == Some(player)
        && !cant_attack(state, id)
        && !keywords::summoning_sick(state, id)
        && !keywords::forbids_attacking(state, id)
        && is_untapped_creature(state, id)
}

pub(crate) fn defenders_for(state: &GameState, player: PlayerId) -> Vec<AttackTarget> {
    let mut targets: Vec<AttackTarget> = state
        .turn_order_from(player)
        .into_iter()
        .filter(|p| *p != player)
        .map(AttackTarget::Player)
        .collect();
    for &id in state.zone(Zone::Battlefield) {
        let is_walker =
            characteristics(state, id).is_some_and(|c| c.types.contains(CardType::Planeswalker));
        let hostile = controller_of(state, id).is_some_and(|c| c != player && state.is_alive(c));
        if is_walker && hostile {
            targets.push(AttackTarget::Planeswalker(id));
        }
    }
    targets
}

/// Every creature `player` may attack with and what each may attack.
pub(crate) fn attack_options(state: &GameState, player: PlayerId) -> AttackOptions {
    let defenders = defenders_for(state, player);
    let attackers = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| can_attack(state, *id, player))
        .map(|object| AttackerOption {
            object,
            defenders: defenders.clone(),
        })
        .collect();
    AttackOptions { attackers }
}

/// Players being attacked, in turn order after the active player.
pub(crate) fn defending_players(state: &GameState) -> Vec<PlayerId> {
    let active = state.turn().active();
    state
        .turn_order_from(active)
        .into_iter()
        .filter(|p| *p != active)
        .filter(|p| {
            state
                .combat()
                .attackers()
                .iter()
                .any(|a| defending_player(state, a.target) == Some(*p))
        })
        .collect()
}

/// Every creature `player` may block with and what each may block.
///
/// An attacker that needs several blockers (menace) is offered only while enough of `player`'s creatures could
/// block it, so a creature that could only join a block that cannot be made is not offered for it.
pub(crate) fn block_options(state: &GameState, player: PlayerId) -> BlockOptions {
    let blockable: Vec<ObjectId> = state
        .combat()
        .attackers()
        .iter()
        .filter(|a| defending_player(state, a.target) == Some(player))
        .map(Attacker::object)
        .collect();
    let candidates: Vec<(ObjectId, Vec<ObjectId>)> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            controller_of(state, *id) == Some(player)
                && is_untapped_creature(state, *id)
                && !cant_block(state, *id)
        })
        .map(|object| {
            let can_block: Vec<ObjectId> = blockable
                .iter()
                .copied()
                .filter(|attacker| !keywords::stops_block(state, *attacker, object))
                .collect();
            (object, can_block)
        })
        .collect();
    let blockable_together = |attacker: ObjectId| {
        let able = candidates
            .iter()
            .filter(|(_, can_block)| can_block.contains(&attacker))
            .count();
        able >= keywords::min_blockers(state, attacker)
    };
    let blockers = candidates
        .iter()
        .filter_map(|(object, can_block)| {
            let can_block: Vec<ObjectId> = can_block
                .iter()
                .copied()
                .filter(|attacker| blockable_together(*attacker))
                .collect();
            (!can_block.is_empty()).then_some(BlockerOption {
                object: *object,
                can_block,
            })
        })
        .collect();
    let min_blockers = blockable
        .iter()
        .copied()
        .filter(|attacker| blockable_together(*attacker))
        .map(|attacker| (attacker, keywords::min_blockers(state, attacker)))
        .filter(|(_, needed)| *needed > 1)
        .collect();
    BlockOptions {
        blockers,
        min_blockers,
    }
}

/// Checks a declared attack against the options.
pub(crate) fn check_attackers(
    options: &AttackOptions,
    answer: &[(ObjectId, AttackTarget)],
) -> Result<(), IllegalAnswer> {
    for (i, (object, target)) in answer.iter().enumerate() {
        if answer[..i].iter().any(|(o, _)| o == object) {
            return Err(IllegalAnswer::Duplicate("attacker"));
        }
        let allowed = options
            .attackers
            .iter()
            .find(|a| a.object == *object)
            .ok_or(IllegalAnswer::NotOffered("attacker"))?;
        if !allowed.defenders.contains(target) {
            return Err(IllegalAnswer::NotOffered("defender"));
        }
    }
    Ok(())
}

/// Checks declared blocks against the options, and that an attacker that needs several blockers gets them.
pub(crate) fn check_blockers(
    state: &GameState,
    options: &BlockOptions,
    answer: &[(ObjectId, ObjectId)],
) -> Result<(), IllegalAnswer> {
    for (i, (blocker, attacker)) in answer.iter().enumerate() {
        if answer[..i].iter().any(|(b, _)| b == blocker) {
            return Err(IllegalAnswer::Duplicate("blocker"));
        }
        let allowed = options
            .blockers
            .iter()
            .find(|b| b.object == *blocker)
            .ok_or(IllegalAnswer::NotOffered("blocker"))?;
        if !allowed.can_block.contains(attacker) {
            return Err(IllegalAnswer::NotOffered("attacker to block"));
        }
    }
    for (_, attacker) in answer {
        let got = answer.iter().filter(|(_, a)| a == attacker).count();
        let needed = keywords::min_blockers(state, *attacker);
        if got < needed {
            return Err(IllegalAnswer::TooFewBlockers { needed, got });
        }
    }
    Ok(())
}

/// Events for a validated attack declaration: the declaration and tapping the attackers that tap to attack.
pub(crate) fn attack_events(
    state: &GameState,
    player: PlayerId,
    attackers: &[(ObjectId, AttackTarget)],
) -> Vec<Event> {
    let mut events = vec![Event::AttackersDeclared {
        player,
        attackers: attackers.to_vec(),
    }];
    events.extend(
        attackers
            .iter()
            .filter(|(object, _)| !keywords::attacks_untapped(state, *object))
            .map(|(object, _)| Event::PermanentTapped { object: *object }),
    );
    events
}
