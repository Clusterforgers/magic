//! The complete game state; only [`apply_event`](crate::fold::apply_event) mutates it.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::combat::CombatState;
use crate::effects::ContinuousEffect;
use crate::format::FormatRules;
use crate::hash::stable_hash;
use crate::object::GameObject;
use crate::player::Player;
use crate::priority::PriorityState;
use crate::turn::TurnState;
use crate::zones::{Zone, Zones};

/// How the game ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameResult {
    /// One player is left standing.
    Winner(PlayerId),
    /// Everyone lost at once (or no one is left).
    Draw,
}

/// The coarse phase of the whole game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stage {
    /// Nothing has happened yet; the first player and shuffles come next.
    PreGame,
    /// Players are choosing opening hands.
    Mulligan,
    /// Turns are being played.
    Turns,
    /// The game has ended.
    Finished,
}

/// The state of one game; a pure fold of events over the initial setup.
#[derive(Debug, Clone)]
pub struct GameState {
    pub(crate) format: Arc<dyn FormatRules>,
    pub(crate) seed: u64,
    pub(crate) rng_index: u64,
    pub(crate) stage: Stage,
    pub(crate) first_player: PlayerId,
    pub(crate) players: Vec<Player>,
    pub(crate) objects: BTreeMap<ObjectId, GameObject>,
    pub(crate) zones: Zones,
    pub(crate) turn: TurnState,
    pub(crate) priority: Option<PriorityState>,
    pub(crate) combat: CombatState,
    pub(crate) effects: Vec<ContinuousEffect>,
    pub(crate) next_effect: u64,
    pub(crate) next_timestamp: u64,
    pub(crate) result: Option<GameResult>,
}

impl GameState {
    /// The format being played.
    pub fn format(&self) -> &dyn FormatRules {
        &*self.format
    }

    /// The game seed.
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Number of random draws consumed so far (selects the next RNG substream).
    pub const fn rng_index(&self) -> u64 {
        self.rng_index
    }

    /// Coarse game stage.
    pub const fn stage(&self) -> Stage {
        self.stage
    }

    /// The player who took the first turn.
    pub const fn first_player(&self) -> PlayerId {
        self.first_player
    }

    /// All seats, indexed by player id.
    pub fn players(&self) -> &[Player] {
        &self.players
    }

    /// One seat.
    pub fn player(&self, id: PlayerId) -> Option<&Player> {
        self.players.get(id.index())
    }

    /// Looks up an object.
    pub fn object(&self, id: ObjectId) -> Option<&GameObject> {
        self.objects.get(&id)
    }

    /// Every object in the game, in id order.
    pub fn objects(&self) -> impl Iterator<Item = &GameObject> {
        self.objects.values()
    }

    /// Number of objects in the game.
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Contents of a zone.
    pub fn zone(&self, zone: Zone) -> &[ObjectId] {
        self.zones.get(zone)
    }

    /// The stack, top last.
    pub fn stack(&self) -> &[ObjectId] {
        self.zones.get(Zone::Stack)
    }

    /// Turn structure position.
    pub const fn turn(&self) -> &TurnState {
        &self.turn
    }

    /// Current priority state, if a priority round is under way.
    pub const fn priority(&self) -> Option<&PriorityState> {
        self.priority.as_ref()
    }

    /// Combat in progress.
    pub const fn combat(&self) -> &CombatState {
        &self.combat
    }

    /// Registered continuous effects.
    pub fn effects(&self) -> &[ContinuousEffect] {
        &self.effects
    }

    /// The result once the game is over.
    pub const fn result(&self) -> Option<GameResult> {
        self.result
    }

    /// Whether `id` is a seat still in the game.
    pub fn is_alive(&self, id: PlayerId) -> bool {
        self.player(id).is_some_and(Player::is_in_game)
    }

    /// Players still in the game, in seat order.
    pub fn alive_players(&self) -> impl Iterator<Item = PlayerId> + '_ {
        self.players
            .iter()
            .filter(|p| p.is_in_game())
            .map(Player::id)
    }

    /// Players still in the game in turn order, starting at `start` (or the next living seat).
    pub fn turn_order_from(&self, start: PlayerId) -> Vec<PlayerId> {
        let n = self.players.len();
        (0..n)
            .map(|i| (start.index() + i) % n.max(1))
            .filter(|i| self.players[*i].is_in_game())
            .map(PlayerId::from_index)
            .collect()
    }

    /// The next living player after `player` in turn order (possibly `player` itself).
    pub fn next_alive_after(&self, player: PlayerId) -> Option<PlayerId> {
        let n = self.players.len();
        (1..=n)
            .map(|i| (player.index() + i) % n.max(1))
            .find(|i| self.players[*i].is_in_game())
            .map(PlayerId::from_index)
    }

    /// Fingerprint of the whole state; equal for any two states reached by the same events.
    pub fn state_hash(&self) -> u64 {
        stable_hash(self)
    }
}

impl Hash for GameState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.format.name().hash(state);
        self.seed.hash(state);
        self.rng_index.hash(state);
        self.stage.hash(state);
        self.first_player.hash(state);
        self.players.hash(state);
        self.objects.hash(state);
        self.zones.hash(state);
        self.turn.hash(state);
        self.priority.hash(state);
        self.combat.hash(state);
        self.effects.hash(state);
        self.next_effect.hash(state);
        self.next_timestamp.hash(state);
        self.result.hash(state);
    }
}
