//! The event log vocabulary: every change to game state is exactly one of these.

use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use serde::{Deserialize, Serialize};

use crate::combat::{AttackTarget, DamageTarget};
use crate::effects::ContinuousEffect;
use crate::player::LossReason;
use crate::state::GameResult;
use crate::turn::Step;

/// Why a player was not asked to decide something optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NothingToDo {
    /// The player has no creature able to attack.
    Attack,
    /// The player has no creature able to block an attacker.
    Block,
}

/// Why a creature was put into its owner's graveyard by state-based actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeathCause {
    /// Damage marked on it was at least its toughness (CR 704.5g).
    LethalDamage,
    /// Its toughness was 0 or less (CR 704.5f).
    ZeroToughness,
}

/// A fact about the game; folding the full list over the initial setup yields the game state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// The starting player was picked at random.
    FirstPlayerChosen {
        /// Who goes first.
        player: PlayerId,
    },
    /// A library was shuffled into this order (bottom to top).
    LibraryShuffled {
        /// Owner of the library.
        player: PlayerId,
        /// The new order.
        order: Vec<ObjectId>,
    },
    /// The opening-hand procedure began.
    MulliganPhaseBegan,
    /// An opening hand was dealt from the top of the library.
    HandDealt {
        /// Who received it.
        player: PlayerId,
        /// Cards in draw order.
        cards: Vec<ObjectId>,
    },
    /// The player chose to mulligan, pending the rest of the table.
    MulliganDeclared {
        /// Who chose.
        player: PlayerId,
    },
    /// The hand was shuffled into the library.
    MulliganTaken {
        /// Who mulliganed.
        player: PlayerId,
        /// The shuffled library, bottom to top.
        library_order: Vec<ObjectId>,
    },
    /// The player kept their hand.
    MulliganKept {
        /// Who kept.
        player: PlayerId,
    },
    /// Cards were put on the bottom of the library after keeping.
    CardsBottomed {
        /// Whose cards.
        player: PlayerId,
        /// Cards in the order placed (the last is the bottom).
        cards: Vec<ObjectId>,
    },
    /// All players kept; the game proper begins.
    MulliganPhaseEnded,
    /// A new turn began.
    TurnBegan {
        /// Turn number.
        number: u32,
        /// The active player.
        active: PlayerId,
    },
    /// A step began.
    StepBegan {
        /// The step.
        step: Step,
    },
    /// A player had no legal option for an optional decision, so it was skipped.
    NothingToDo {
        /// The player.
        player: PlayerId,
        /// What they could not do.
        kind: NothingToDo,
    },
    /// A priority round started with this player.
    PriorityGranted {
        /// Who has priority.
        player: PlayerId,
    },
    /// A player passed priority.
    PriorityPassed {
        /// Who passed.
        player: PlayerId,
    },
    /// A land was played from hand.
    LandPlayed {
        /// Who played it.
        player: PlayerId,
        /// The land.
        object: ObjectId,
    },
    /// A spell was cast and put on the stack.
    SpellCast {
        /// The caster.
        player: PlayerId,
        /// The spell.
        object: ObjectId,
    },
    /// The top spell of the stack resolved.
    SpellResolved {
        /// The spell.
        object: ObjectId,
    },
    /// A permanent became tapped.
    PermanentTapped {
        /// The permanent.
        object: ObjectId,
    },
    /// A permanent became untapped.
    PermanentUntapped {
        /// The permanent.
        object: ObjectId,
    },
    /// Mana was added to a pool.
    ManaAdded {
        /// Whose pool.
        player: PlayerId,
        /// The mana.
        mana: ManaType,
        /// The producing permanent.
        source: Option<ObjectId>,
    },
    /// Mana was spent from a pool.
    ManaSpent {
        /// Whose pool.
        player: PlayerId,
        /// The mana spent.
        mana: Vec<ManaType>,
    },
    /// Unspent mana disappeared at the end of a step or phase.
    ManaPoolEmptied {
        /// Whose pool.
        player: PlayerId,
    },
    /// A card was drawn.
    CardDrawn {
        /// Who drew.
        player: PlayerId,
        /// The card.
        object: ObjectId,
    },
    /// A draw was attempted from an empty library.
    DrewFromEmptyLibrary {
        /// Who tried.
        player: PlayerId,
    },
    /// A card was discarded from hand.
    CardDiscarded {
        /// Who discarded.
        player: PlayerId,
        /// The card.
        object: ObjectId,
    },
    /// Attackers were declared (possibly none).
    AttackersDeclared {
        /// The attacking player.
        player: PlayerId,
        /// Each attacker and its target.
        attackers: Vec<(ObjectId, AttackTarget)>,
    },
    /// A defending player declared blockers (possibly none).
    BlockersDeclared {
        /// The defending player.
        player: PlayerId,
        /// Pairs of (blocker, attacker blocked).
        blocks: Vec<(ObjectId, ObjectId)>,
    },
    /// An attacker's damage was divided among its blockers.
    DamageAssigned {
        /// The attacker.
        attacker: ObjectId,
        /// Damage per blocker.
        assignment: Vec<(ObjectId, u32)>,
    },
    /// Damage was dealt.
    DamageDealt {
        /// The source.
        source: ObjectId,
        /// The recipient.
        target: DamageTarget,
        /// Amount (never zero).
        amount: u32,
    },
    /// Combat ended and its state was cleared.
    CombatEnded,
    /// All damage marked on permanents was removed.
    DamageCleared,
    /// A continuous effect was registered.
    EffectCreated {
        /// The effect.
        effect: ContinuousEffect,
    },
    /// A continuous effect ended and was removed.
    EffectExpired {
        /// The effect.
        id: EffectId,
    },
    /// A creature was put into its owner's graveyard by state-based actions.
    CreatureDied {
        /// The creature.
        object: ObjectId,
        /// Why.
        cause: DeathCause,
    },
    /// A player lost the game.
    PlayerLost {
        /// Who lost.
        player: PlayerId,
        /// Why.
        reason: LossReason,
    },
    /// An object left the game because its owner did.
    ObjectLeftGame {
        /// The object.
        object: ObjectId,
    },
    /// The game ended.
    GameEnded {
        /// The result.
        result: GameResult,
    },
}

impl Event {
    /// Objects this event moves or changes, for incremental consistency checks.
    pub fn touched_objects(&self) -> Vec<ObjectId> {
        match self {
            Event::LibraryShuffled { .. }
            | Event::MulliganTaken { .. }
            | Event::DamageCleared
            | Event::EffectCreated { .. }
            | Event::EffectExpired { .. }
            | Event::FirstPlayerChosen { .. }
            | Event::MulliganPhaseBegan
            | Event::MulliganDeclared { .. }
            | Event::MulliganKept { .. }
            | Event::MulliganPhaseEnded
            | Event::TurnBegan { .. }
            | Event::StepBegan { .. }
            | Event::NothingToDo { .. }
            | Event::PriorityGranted { .. }
            | Event::PriorityPassed { .. }
            | Event::ManaAdded { .. }
            | Event::ManaSpent { .. }
            | Event::ManaPoolEmptied { .. }
            | Event::DrewFromEmptyLibrary { .. }
            | Event::BlockersDeclared { .. }
            | Event::DamageAssigned { .. }
            | Event::CombatEnded
            | Event::PlayerLost { .. }
            | Event::GameEnded { .. } => Vec::new(),
            Event::HandDealt { cards, .. } | Event::CardsBottomed { cards, .. } => cards.clone(),
            Event::LandPlayed { object, .. }
            | Event::SpellCast { object, .. }
            | Event::SpellResolved { object }
            | Event::PermanentTapped { object }
            | Event::PermanentUntapped { object }
            | Event::CardDrawn { object, .. }
            | Event::CardDiscarded { object, .. }
            | Event::CreatureDied { object, .. }
            | Event::ObjectLeftGame { object } => vec![*object],
            Event::AttackersDeclared { attackers, .. } => {
                attackers.iter().map(|(o, _)| *o).collect()
            }
            Event::DamageDealt { source, target, .. } => match target {
                DamageTarget::Object(o) => vec![*source, *o],
                DamageTarget::Player(_) => vec![*source],
            },
        }
    }
}
