//! The complete game state; only [`apply_event`](crate::fold::apply_event) mutates it.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::abilities::AbilitySource;
use crate::combat::CombatState;
use crate::effects::ContinuousEffect;
use crate::format::FormatRules;
use crate::hash::stable_hash;
use crate::layers::LayersCache;
use crate::object::{GameObject, ObjectRef};
use crate::player::Player;
use crate::priority::PriorityState;
use crate::resolve::Resolving;
use crate::rules::{RuleEffect, RulesCache};
use crate::triggers::{Delayed, PendingTrigger};
use crate::turn::TurnState;
use crate::zones::{Zone, Zones};

/// How the game ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum GameResult {
    /// One player is left standing.
    Winner(PlayerId),
    /// Everyone lost at once (or no one is left).
    Draw,
}

/// The coarse phase of the whole game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

/// What the cards of a game can do that the engine looks for only when they can, so a game of plain cards pays nothing
/// for it; derived from the setup and the tokens made.
// Independent facts the setup and the folds note by name, not the states of one machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Possible {
    /// Some card has an ability that works from a zone other than the battlefield.
    pub(crate) elsewhere: bool,
    /// Some card has a state trigger or gives one.
    pub(crate) states: bool,
    /// A +1/+1 or -1/-1 counter has been on a permanent, so the layers work out its power and toughness (CR 613.4c).
    pub(crate) counters: bool,
    /// Some card is an Aura, an Equipment or a Fortification, or a permanent starts attached.
    pub(crate) attachments: bool,
    /// Some card has a replacement effect, printed, given or added by a keyword, or a token made has one.
    pub(crate) replacements: bool,
    /// Some card is legendary.
    pub(crate) legends: bool,
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
    /// Tokens that left the battlefield and so cease to exist, kept for the abilities of theirs still to come.
    pub(crate) ceased: BTreeMap<ObjectId, GameObject>,
    pub(crate) zones: Zones,
    pub(crate) turn: TurnState,
    pub(crate) priority: Option<PriorityState>,
    pub(crate) combat: CombatState,
    pub(crate) effects: Vec<ContinuousEffect>,
    /// The rules of the game that resolved spells and abilities changed and that still last.
    pub(crate) rule_effects: Vec<RuleEffect>,
    pub(crate) next_effect: u64,
    pub(crate) next_timestamp: u64,
    pub(crate) next_object: u64,
    pub(crate) resolving: Option<Resolving>,
    pub(crate) pending_triggers: Vec<PendingTrigger>,
    pub(crate) trigger_round: u32,
    pub(crate) delayed: Vec<Delayed>,
    pub(crate) next_delayed: u64,
    pub(crate) limits_spent: BTreeSet<(ObjectRef, AbilitySource, usize)>,
    /// The planeswalkers that had a loyalty ability activated this turn: no more than one for each (CR 606.3).
    pub(crate) loyalty_used: BTreeSet<ObjectRef>,
    /// Whether any card of the game has a triggered ability or sets up a delayed one; derived from the setup.
    pub(crate) triggers_possible: bool,
    /// Whether any card of the game has a static ability; derived from the setup and the tokens made.
    pub(crate) statics_possible: bool,
    /// What the layers make of every permanent, worked out when first asked for and dropped at the next event.
    pub(crate) layers: LayersCache,
    /// Whether any card of the game has a rule of the game as an ability; derived from the setup and the tokens made.
    pub(crate) rules_possible: bool,
    /// The rules in force, gathered when first asked for and dropped at the next event that can change them.
    pub(crate) rules_cache: RulesCache,
    /// What the cards of the game can do that the engine looks for only when they can.
    pub(crate) possible: Possible,
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

    /// Whether `id` was a token that has left the battlefield and so ceased to exist (CR 111.7).
    pub fn has_ceased(&self, id: ObjectId) -> bool {
        self.ceased.contains_key(&id)
    }

    /// The object an ability comes from: `id` in the game, or the token it was before it ceased to exist. An ability
    /// outlives its source (CR 113.7a), and it needs the card the source was printed on.
    pub(crate) fn source_object(&self, id: ObjectId) -> Option<&GameObject> {
        self.objects.get(&id).or_else(|| self.ceased.get(&id))
    }

    /// The permanent `id` is attached to (it is an Aura or Equipment), if that host is still the very object it was
    /// attached to and on the battlefield: a host that left is a new object (CR 400.7).
    pub fn host_of(&self, id: ObjectId) -> Option<ObjectId> {
        let attached = self.object(id)?.permanent()?.attached_to()?;
        let host = self.object(attached.id)?;
        (host.zone() == Zone::Battlefield && host.counter() == attached.counter)
            .then_some(attached.id)
    }

    /// The permanents attached to `id`, in id order.
    pub fn attachments_of(&self, id: ObjectId) -> Vec<ObjectId> {
        self.objects()
            .map(GameObject::id)
            .filter(|attached| self.host_of(*attached) == Some(id))
            .collect()
    }

    /// Every host with the permanents attached to it, each list in id order: `attachments_of` for all hosts in one pass.
    pub fn attachment_index(&self) -> BTreeMap<ObjectId, Vec<ObjectId>> {
        let mut index: BTreeMap<ObjectId, Vec<ObjectId>> = BTreeMap::new();
        for attached in self.objects().map(GameObject::id) {
            if let Some(host) = self.host_of(attached) {
                index.entry(host).or_default().push(attached);
            }
        }
        index
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

    /// Registered changes to the rules of the game.
    pub fn rule_effects(&self) -> &[RuleEffect] {
        &self.rule_effects
    }

    /// The spell or ability that is resolving, with its journal.
    pub const fn resolving(&self) -> Option<&Resolving> {
        self.resolving.as_ref()
    }

    /// Triggered abilities that triggered and wait to go on the stack, in the order they triggered.
    pub fn pending_triggers(&self) -> &[PendingTrigger] {
        &self.pending_triggers
    }

    /// Delayed triggered abilities waiting for their event.
    pub fn delayed(&self) -> &[Delayed] {
        &self.delayed
    }

    /// Whether detecting triggers can find anything in this game: a card with a triggered ability or an effect that
    /// sets one up exists, so a game of vanilla cards pays nothing for them.
    pub(crate) fn watches(&self) -> bool {
        self.triggers_possible || !self.delayed.is_empty()
    }

    /// Whether the once-each-turn ability `ability` of `source` has been used this turn.
    pub(crate) fn limit_spent(
        &self,
        source: ObjectRef,
        origin: AbilitySource,
        ability: usize,
    ) -> bool {
        self.limits_spent.contains(&(source, origin, ability))
    }

    /// A new turn begins: the abilities used once each turn and the planeswalkers that activated a loyalty ability are free again.
    pub(crate) fn clear_turn_uses(&mut self) {
        self.limits_spent.clear();
        self.loyalty_used.clear();
    }

    /// Whether a loyalty ability of the planeswalker `permanent` has been activated this turn (CR 606.3).
    pub(crate) fn loyalty_spent(&self, permanent: ObjectRef) -> bool {
        self.loyalty_used.contains(&permanent)
    }

    /// The id the next object created during the game gets (abilities on the stack, and tokens later).
    pub const fn next_object(&self) -> ObjectId {
        ObjectId(self.next_object)
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
        self.ceased.hash(state);
        self.zones.hash(state);
        self.turn.hash(state);
        self.priority.hash(state);
        self.combat.hash(state);
        self.effects.hash(state);
        // A game with no change to a rule hashes as it did before they existed.
        if !self.rule_effects.is_empty() {
            self.rule_effects.hash(state);
        }
        self.next_effect.hash(state);
        self.next_timestamp.hash(state);
        self.next_object.hash(state);
        self.resolving.hash(state);
        self.pending_triggers.hash(state);
        self.trigger_round.hash(state);
        self.delayed.hash(state);
        self.next_delayed.hash(state);
        self.limits_spent.hash(state);
        // A game with no planeswalker hashes as it did before they existed.
        if !self.loyalty_used.is_empty() {
            self.loyalty_used.hash(state);
        }
        self.result.hash(state);
    }
}
