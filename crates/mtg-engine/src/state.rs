//! The complete game state; only [`apply_event`](crate::fold::apply_event) mutates it.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_core::card::Subtype;
use mtg_core::house::HouseRules;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::abilities::AbilitySource;
use crate::combat::CombatState;
use crate::effects::ContinuousEffect;
use crate::format::{FormatRules, RuleSet};
use crate::hash::stable_hash;
use crate::layers::LayersCache;
use crate::object::{GameObject, ObjectRef};
use crate::player::Player;
use crate::priority::PriorityState;
use crate::resolve::Resolving;
use crate::rules::{RuleEffect, RulesCache};
use crate::triggers::{Delayed, PendingTrigger};
use crate::turn::{ExtraTurn, TurnState};
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
    /// A +1/+1, -1/-1 or keyword counter has been on a permanent, so the layers work out its power, toughness and keywords
    /// (CR 613.4c, 122.1b).
    pub(crate) counters: bool,
    /// Some card is an Aura, an Equipment or a Fortification, or a permanent starts attached.
    pub(crate) attachments: bool,
    /// Some card has a replacement effect, printed, given or added by a keyword, or a token made has one.
    pub(crate) replacements: bool,
    /// Some card is legendary.
    pub(crate) legends: bool,
    /// Some card or token made has a cost modifier, so the cost of a spell is worked out against the permanents.
    pub(crate) costs: bool,
    /// Some card has daybound or nightbound, so the day and night are watched.
    pub(crate) day_night: bool,
    /// Some card has ascend, so the city's blessing is watched.
    pub(crate) ascend: bool,
    /// Some card, token or player has protection or an effect that gives it, so damage is checked against it.
    pub(crate) protection: bool,
    /// A card has been exiled until a permanent leaves the battlefield, so the cards waiting in exile are watched.
    pub(crate) linked_exile: bool,
}

/// A kind of event the game counts in a turn: for each player ("your second card each turn") or for each object
/// ("becomes the target for the first time each turn").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Tally {
    /// A card drawn.
    Drawn,
    /// A card drawn by the active player in the draw step under way (cleared as each draw step begins).
    C6BlkDrawStepDrawn,
    /// A spell cast.
    Cast,
    /// An instant or sorcery spell cast (counted for the player).
    CastInstantOrSorcery,
    /// A noncreature spell cast (counted for the player).
    C10AnyNoncreatureCast,
    /// A creature spell cast (counted for the player).
    C11aCreatureCast,
    /// An artifact spell cast (counted for the player).
    C11aArtifactCast,
    /// A spell cast through a permission that works once each turn (counted for the permission's source).
    C11bOnceUsed,
    /// A creature exerted (counted for the object, and for the player who exerted it).
    W11aExerted,
    /// Life gained, once for each event.
    Gained,
    /// An object declared as an attacker (counted for the object).
    Attacked,
    /// An object chosen as a target of a spell or ability (counted for the object, and for the controller of what targets it).
    Targeted,
    /// An object dealt damage, once for each event (counted for the object).
    Damaged,
    /// A card put into a graveyard by a discard (counted for the card as it lies there, and for the player who discarded
    /// it, once for each discard).
    Discarded,
    /// A permanent put into a graveyard from the battlefield (counted for its owner, gravestorm).
    Graved,
    /// A card put into a graveyard by a mill (counted for the card as it lies there: "a card that was milled this turn").
    W13bMilled,
    /// A creature put into a graveyard from the battlefield, a token too (counted for its owner).
    Died,
    /// A nontoken creature put into a graveyard from the battlefield (counted for its owner).
    DiedNontoken,
    /// A creature put into a graveyard from the battlefield, a token too (counted for the player who controlled it as it died).
    DiedUnder,
    /// A nontoken creature put into a graveyard from the battlefield (counted for the player who controlled it as it died).
    DiedNontokenUnder,
    /// A Phyrexian put into a graveyard from the battlefield (counted for the player who controlled it as it died).
    W14bPhyrexianDiedUnder,
    /// A card that left a graveyard for any zone (counted for its owner).
    W21aLeftGraveyard,
    /// A card put into a graveyard from the battlefield (counted for the card as it lies there).
    W21aGraved,
    /// A creature declared as an attacker (counted for its controller, once for each creature).
    Attackers,
    /// A Vehicle that became crewed (counted for the object).
    Crewed,
    /// A Mount that became saddled (counted for the object).
    Saddled,
    /// Damage dealt to a player after prevention (counted for the player, by the amount).
    Hurt,
    /// A permanent put onto the battlefield, from any zone or made as a token (counted for the permanent).
    Entered,
    /// Life lost, by damage that causes it, by a loss and by a payment (counted for the player, by the amount).
    Lost,
    /// A time life was lost, once for each loss, payment or damage that causes one (counted for the player).
    Losses,
    /// A card cycled (counted for the player).
    Cycled,
    /// A surveil (counted for the player).
    Surveilled,
    /// A token created (counted for the player who controls it).
    Tokens,
    /// A kind of counter put on a permanent or a card in exile, once for each event (counted for the object).
    Countered,
    /// Life gained, by the amount (counted for the player).
    GainedTotal,
    /// A land played from a hand or another zone (counted for the player).
    LandsPlayed,
    /// Mana spent to cast spells, by the amount (counted for the player; expend, CR 701.59).
    W9aSpellMana,
}

/// The events counted in the turn so far.
#[derive(Debug, Clone, Default)]
pub(crate) struct Tallies {
    /// How many events of each kind each player has had.
    pub(crate) players: BTreeMap<(PlayerId, Tally), u32>,
    /// How many events of each kind each object has had, by the player who caused them where that matters (the
    /// controller of what targets it; `None` counts them all). An object that changed zones is a new one.
    pub(crate) objects: BTreeMap<(ObjectRef, Option<PlayerId>, Tally), u32>,
    /// The creatures that dealt combat damage to a player, in the order they did.
    pub(crate) hits: Vec<CombatHit>,
    /// The monarch as the turn began (CR 724): who held the designation before anything in the turn changed it.
    pub(crate) monarch_began: Option<PlayerId>,
    /// The creatures tapped to crew or saddle a permanent, as (permanent, saddled, creature).
    pub(crate) crews: BTreeSet<(ObjectRef, bool, ObjectRef)>,
    /// The objects that dealt damage to an object this turn, as (the object damaged, the source of the damage).
    pub(crate) damaged_by: BTreeSet<(ObjectRef, ObjectRef)>,
    /// The modes the abilities chose, as (source, where the ability comes from, its index, the mode).
    pub(crate) modes: BTreeSet<(ObjectRef, AbilitySource, usize, usize)>,
}

/// A creature that dealt combat damage to a player this turn, as it was then (prowl, CR 702.76).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CombatHit {
    /// Who controlled it.
    pub(crate) controller: PlayerId,
    /// Its creature types.
    pub(crate) types: Vec<Subtype>,
    /// It had every creature type (changeling).
    pub(crate) every_type: bool,
}

/// The state of one game; a pure fold of events over the initial setup.
#[derive(Debug, Clone)]
pub struct GameState {
    pub(crate) format: Arc<dyn FormatRules>,
    /// What the table's creator set in place of the format's values.
    pub(crate) house_rules: HouseRules,
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
    /// The extra turns still to take, the latest added last: it is taken first (CR 500.7).
    pub(crate) extra_turns: Vec<ExtraTurn>,
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
    /// The modes the abilities that choose a mode not yet chosen chose, in any turn, as (source, where the ability comes
    /// from, its index, the mode); a source that changed zones is a new object with none.
    pub(crate) modes_ever: BTreeSet<(ObjectRef, AbilitySource, usize, usize)>,
    /// What the game has counted this turn, from zero at the turn's start.
    pub(crate) tallies: Tallies,
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
    /// Day or night (CR 726); neither until a permanent with daybound or nightbound appears.
    pub(crate) day_night: Option<crate::day_night::DayNight>,
    /// The monarch (CR 724): at most one player, from the first card that makes one.
    pub(crate) monarch: Option<PlayerId>,
    /// The players who have the city's blessing (CR 702.131), for the rest of the game.
    pub(crate) citys_blessing: std::collections::BTreeSet<PlayerId>,
    /// The dungeons (CR 309): each player's venture marker and the dungeons they completed.
    pub(crate) w11d: crate::w11d_dungeon::W11dDungeons,
    /// Who has the initiative (CR 725) and the object that stands for it.
    pub(crate) w15a_initiative: Option<crate::w15a_initiative::W15aHeld>,
    /// The commanders of the game and what is counted for them (CR 903); empty outside Commander.
    pub(crate) commanders: crate::commanders::Commanders,
    /// The companions the seats named, outside the game until their owners bring them in (CR 702.139).
    pub(crate) companions: crate::companions::Companions,
}

impl GameState {
    /// The format being played.
    pub fn format(&self) -> &dyn FormatRules {
        &*self.format
    }

    /// What the table's creator set in place of the format's values.
    pub fn house_rules(&self) -> &HouseRules {
        &self.house_rules
    }

    /// The rules the game plays by: the format's values with the house rules over them.
    pub fn ruleset(&self) -> RuleSet<'_> {
        RuleSet::new(&*self.format, &self.house_rules)
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
        let object = self.object(id).filter(|o| o.zone() == Zone::Battlefield)?;
        let attached = object.permanent()?.attached_to()?;
        let host = self.object(attached.id)?;
        (host.zone() == Zone::Battlefield && host.counter() == attached.counter)
            .then_some(attached.id)
    }

    /// The player `id` is attached to (an Aura that enchants a player), while that player is still in the game: a
    /// player who left takes the Auras on them away (CR 800.4a, 704.5m).
    pub fn player_host_of(&self, id: ObjectId) -> Option<PlayerId> {
        let object = self.object(id).filter(|o| o.zone() == Zone::Battlefield)?;
        let player = object.permanent()?.attached_player()?;
        self.is_alive(player).then_some(player)
    }

    /// The objects that may be attached, in id order: none while no card of the game attaches (`Possible::attachments`),
    /// so a snapshot of each permanent of a crowded table does not scan the table.
    fn attachable(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.possible
            .attachments
            .then(|| self.objects().map(GameObject::id))
            .into_iter()
            .flatten()
    }

    /// The permanents attached to the player `player`, in id order.
    pub fn attachments_of_player(&self, player: PlayerId) -> Vec<ObjectId> {
        self.attachable()
            .filter(|attached| self.player_host_of(*attached) == Some(player))
            .collect()
    }

    /// The permanents attached to `id`, in id order.
    pub fn attachments_of(&self, id: ObjectId) -> Vec<ObjectId> {
        self.attachable()
            .filter(|attached| self.host_of(*attached) == Some(id))
            .collect()
    }

    /// Every host with the permanents attached to it, each list in id order: `attachments_of` for all hosts in one pass.
    pub fn attachment_index(&self) -> BTreeMap<ObjectId, Vec<ObjectId>> {
        let mut index: BTreeMap<ObjectId, Vec<ObjectId>> = BTreeMap::new();
        for attached in self.attachable() {
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

    /// The extra turns still to take, the latest added last: it is taken first (CR 500.7).
    pub fn extra_turns(&self) -> &[ExtraTurn] {
        &self.extra_turns
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
        self.tallies = Tallies::default();
        self.tallies.monarch_began = self.monarch;
    }

    /// Counts one more event of the kind for the player this turn.
    pub(crate) fn tally(&mut self, player: PlayerId, kind: Tally) {
        let count = self.tallies.players.entry((player, kind)).or_insert(0);
        *count = count.saturating_add(1);
    }

    /// Counts `by` more of the kind for the player this turn.
    pub(crate) fn tally_by(&mut self, player: PlayerId, kind: Tally, by: u32) {
        let count = self.tallies.players.entry((player, kind)).or_insert(0);
        *count = count.saturating_add(by);
    }

    /// Counts a loss of `amount` life for the player this turn: the life lost, and one more time life was lost.
    pub(crate) fn tally_loss(&mut self, player: PlayerId, amount: u32) {
        self.tally_by(player, Tally::Lost, amount);
        self.tally(player, Tally::Losses);
    }

    /// How many permanents were put into a graveyard from the battlefield this turn, whoever owned them.
    pub(crate) fn graved_this_turn(&self) -> u32 {
        self.players()
            .iter()
            .map(|player| self.tallied(player.id(), Tally::Graved))
            .sum()
    }

    /// How many events of the kind the player has had this turn.
    pub(crate) fn tallied(&self, player: PlayerId, kind: Tally) -> u32 {
        let counted = self.tallies.players.get(&(player, kind));
        counted.copied().unwrap_or(0)
    }

    /// Counts one more event of the kind for the object this turn, and for `by` when a player caused it.
    pub(crate) fn tally_object(&mut self, object: ObjectRef, by: Option<PlayerId>, kind: Tally) {
        let keys = std::iter::once(None).chain(by.map(Some));
        for key in keys {
            let count = self.tallies.objects.entry((object, key, kind)).or_insert(0);
            *count = count.saturating_add(1);
        }
    }

    /// How many events of the kind the object has had this turn, caused by `by` (any player when `None`).
    pub(crate) fn object_tallied(
        &self,
        object: ObjectRef,
        by: Option<PlayerId>,
        kind: Tally,
    ) -> u32 {
        let key = (object, by, kind);
        self.tallies.objects.get(&key).copied().unwrap_or(0)
    }

    /// Records that `source` dealt damage to the permanent `victim` this turn.
    pub(crate) fn note_damaged_by(&mut self, victim: ObjectId, source: ObjectId) {
        let reference = |id: ObjectId| self.object(id).map(GameObject::reference);
        if let (Some(victim), Some(source)) = (reference(victim), reference(source)) {
            self.tallies.damaged_by.insert((victim, source));
        }
    }

    /// Whether `source` dealt damage to `victim` this turn.
    pub(crate) fn damaged_by(&self, victim: ObjectRef, source: ObjectRef) -> bool {
        self.tallies.damaged_by.contains(&(victim, source))
    }

    /// Whether `creature` was tapped this turn to crew (`saddle` false) or saddle (true) `permanent`.
    pub(crate) fn crewed_by(
        &self,
        permanent: ObjectRef,
        saddle: bool,
        creature: ObjectRef,
    ) -> bool {
        self.tallies.crews.contains(&(permanent, saddle, creature))
    }

    /// Whether `player` discarded the card `id` this turn and it still lies in the graveyard it went to: the condition of
    /// mayhem (CR 702.190a). A card that left the graveyard and came back is a new object and was not discarded.
    pub(crate) fn discarded_this_turn(&self, player: PlayerId, id: ObjectId) -> bool {
        self.object(id).is_some_and(|card| {
            self.object_tallied(card.reference(), Some(player), Tally::Discarded) > 0
        })
    }

    /// Whether an opponent of `player` lost life this turn: the condition of spectacle (CR 702.137a).
    pub(crate) fn opponent_lost_life(&self, player: PlayerId) -> bool {
        self.players()
            .iter()
            .filter(|other| other.id() != player)
            .any(|other| self.tallied(other.id(), Tally::Lost) > 0)
    }

    /// Whether a creature `player` controlled dealt combat damage to a player this turn and shared a creature type with
    /// a card that has the types `types` (every one when `every_type`): the condition of prowl (CR 702.76a).
    pub(crate) fn dealt_combat_damage_with(
        &self,
        player: PlayerId,
        types: &[Subtype],
        every_type: bool,
    ) -> bool {
        self.tallies.hits.iter().any(|hit| {
            hit.controller == player
                && ((every_type && (hit.every_type || !hit.types.is_empty()))
                    || (hit.every_type && !types.is_empty())
                    || hit.types.iter().any(|t| types.contains(t)))
        })
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

    /// The active player, or `None` once they have left the game: the turn goes on without one (CR 800.4).
    pub fn active_player(&self) -> Option<PlayerId> {
        Some(self.turn.active).filter(|player| self.is_alive(*player))
    }

    /// Who comes first in the turn's order of play: the active player, or the next player in turn order after
    /// the one who left (CR 800.4); the seat itself if nobody is left.
    pub fn active_or_next(&self) -> PlayerId {
        let active = self.turn.active;
        self.turn_order_from(active)
            .first()
            .copied()
            .unwrap_or(active)
    }

    /// Whether a turn of `next` that follows one of `previous` opens a round: turns go through the seats in turn order
    /// from the first player, so a round opens when the order wraps round to or past the seat that just played.
    pub(crate) fn opens_round(&self, previous: PlayerId, next: PlayerId) -> bool {
        let (seats, first) = (self.players.len().max(1), self.first_player.index());
        let place = |seat: PlayerId| (seat.index() + seats - first % seats) % seats;
        place(next) <= place(previous)
    }

    /// Whether the turn now being played is the last of its round: the next turn is the first of another.
    pub(crate) fn ends_round(&self) -> bool {
        let regular = self.turn.regular;
        self.extra_turns.is_empty()
            && self.opens_round(regular, self.next_alive_after(regular).unwrap_or(regular))
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
        self.house_rules.hash(state);
        self.seed.hash(state);
        self.rng_index.hash(state);
        self.stage.hash(state);
        self.first_player.hash(state);
        self.players.hash(state);
        self.objects.hash(state);
        self.ceased.hash(state);
        self.zones.hash(state);
        self.turn.hash(state);
        // A game that added no extra turn hashes as it did before they existed.
        if !self.extra_turns.is_empty() {
            self.extra_turns.hash(state);
        }
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
        if !self.modes_ever.is_empty() {
            self.modes_ever.hash(state);
        }
        // Likewise a game that counted nothing this turn.
        if !self.tallies.players.is_empty() {
            self.tallies.players.hash(state);
        }
        if !self.tallies.objects.is_empty() {
            self.tallies.objects.hash(state);
        }
        if !self.tallies.hits.is_empty() {
            self.tallies.hits.hash(state);
        }
        if let Some(began) = self.tallies.monarch_began {
            began.hash(state);
        }
        if !self.tallies.crews.is_empty() {
            self.tallies.crews.hash(state);
        }
        if !self.tallies.damaged_by.is_empty() {
            self.tallies.damaged_by.hash(state);
        }
        if !self.tallies.modes.is_empty() {
            self.tallies.modes.hash(state);
        }
        self.result.hash(state);
        // A game that has seen neither day nor night hashes as it did before they existed.
        if let Some(day_night) = self.day_night {
            day_night.hash(state);
        }
        // Likewise a game without a monarch.
        if let Some(monarch) = self.monarch {
            monarch.hash(state);
        }
        // Likewise a game where nobody has the city's blessing.
        if !self.citys_blessing.is_empty() {
            self.citys_blessing.hash(state);
        }
        // Likewise a game where nobody ventured.
        if !self.w11d.is_empty() {
            self.w11d.hash(state);
        }
        // Likewise a game where nobody took the initiative.
        if let Some(held) = self.w15a_initiative {
            held.hash(state);
        }
        // Likewise a game without commanders.
        self.commanders.hash(state);
        // Likewise a game without companions.
        self.companions.hash(state);
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the active player is none once they have left the game, the first player in turn order of that turn is the next seat in the game, wrapping round the table and skipping the seats that left before, a round ends with the last seat in play before the first player, and the creatures that dealt combat damage this turn open prowl for the cards that share a creature type with one and are forgotten at the next turn, and a card counts for mayhem only while it lies in the graveyard a discard of its owner this turn put it in.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::setup;
    use crate::fold::apply_event;
    use crate::player::LossReason;

    #[rstest]
    #[case::in_the_game(3, 0, &[], Some(0), 0)]
    #[case::middle_seat_left(3, 1, &[1], None, 2)]
    #[case::last_seat_left_wraps(3, 2, &[2], None, 0)]
    #[case::skips_a_seat_that_left_before(4, 1, &[1, 2], None, 3)]
    #[case::wraps_over_a_seat_that_left_before(4, 3, &[3, 0], None, 1)]
    #[case::others_left_not_the_active_one(4, 1, &[2, 3], Some(1), 1)]
    fn the_active_player_and_who_comes_first_when_they_have_left(
        #[case] seats: usize,
        #[case] active: usize,
        #[case] left: &[usize],
        #[case] player: Option<usize>,
        #[case] first: usize,
    ) {
        let mut game =
            GameState::from_setup(&setup(vec![Vec::new(); seats], vec![Vec::new(); seats]))
                .unwrap();
        game.turn.active = PlayerId::from_index(active);
        for seat in left {
            let lost = Event::PlayerLost {
                player: PlayerId::from_index(*seat),
                reason: LossReason::Conceded,
            };
            apply_event(&mut game, &lost).unwrap();
        }
        assert_eq!(game.active_player(), player.map(PlayerId::from_index));
        assert_eq!(game.active_or_next(), PlayerId::from_index(first));
    }

    #[rstest]
    #[case::the_first_seat_of_the_table_mid_round(3, 0, &[], 0, false)]
    #[case::the_second_seat_mid_round(3, 0, &[], 1, false)]
    #[case::the_last_seat_ends_it(3, 0, &[], 2, true)]
    #[case::a_first_player_in_the_middle_ends_at_the_seat_before(4, 2, &[], 1, true)]
    #[case::and_not_before(4, 2, &[], 0, false)]
    #[case::the_seat_before_the_end_has_left(4, 0, &[3], 2, true)]
    #[case::the_first_seat_has_left(4, 0, &[0], 3, true)]
    #[case::the_only_one_left(3, 0, &[1, 2], 0, true)]
    fn a_round_ends_with_the_last_seat_in_play_before_the_first_players(
        #[case] seats: usize,
        #[case] first: usize,
        #[case] left: &[usize],
        #[case] active: usize,
        #[case] ends: bool,
    ) {
        let mut game =
            GameState::from_setup(&setup(vec![Vec::new(); seats], vec![Vec::new(); seats]))
                .unwrap();
        game.first_player = PlayerId::from_index(first);
        game.turn.active = PlayerId::from_index(active);
        game.turn.regular = game.turn.active;
        for seat in left {
            let lost = Event::PlayerLost {
                player: PlayerId::from_index(*seat),
                reason: LossReason::Conceded,
            };
            apply_event(&mut game, &lost).unwrap();
        }
        assert_eq!(game.ends_round(), ends);
    }

    #[rstest]
    #[case::no_extra_turn_waiting(false, true)]
    #[case::an_extra_turn_waiting(true, false)]
    fn a_waiting_extra_turn_keeps_the_round_open(#[case] waiting: bool, #[case] ends: bool) {
        let mut game =
            GameState::from_setup(&setup(vec![Vec::new(); 3], vec![Vec::new(); 3])).unwrap();
        game.turn.active = PlayerId::from_index(2);
        game.turn.regular = PlayerId::from_index(2);
        if waiting {
            let added = Event::ExtraTurnAdded {
                player: PlayerId::from_index(0),
                skip_untap: false,
            };
            apply_event(&mut game, &added).unwrap();
        }
        assert_eq!(game.ends_round(), ends);
    }

    fn subtypes(names: &[&str]) -> Vec<Subtype> {
        names
            .iter()
            .map(|name| Subtype((*name).to_owned()))
            .collect()
    }

    #[rstest]
    #[case::a_shared_type(0, &["Goblin", "Rogue"], false, 0, &["Rogue"], false, true)]
    #[case::no_shared_type(0, &["Goblin"], false, 0, &["Rogue"], false, false)]
    #[case::a_creature_of_another_player(1, &["Rogue"], false, 0, &["Rogue"], false, false)]
    #[case::a_changeling_spell_and_a_creature_with_a_type(0, &["Elf"], false, 0, &[], true, true)]
    #[case::a_changeling_creature_and_a_spell_with_a_type(0, &[], true, 0, &["Rogue"], false, true)]
    #[case::a_creature_with_no_type_and_a_changeling_spell(0, &[], false, 0, &[], true, false)]
    #[case::a_changeling_creature_and_a_changeling_spell(0, &[], true, 0, &[], true, true)]
    fn a_creature_that_dealt_combat_damage_opens_prowl_for_a_card_that_shares_a_type(
        #[case] hit_by: usize,
        #[case] creature_types: &[&str],
        #[case] creature_has_every_type: bool,
        #[case] caster: usize,
        #[case] card_types: &[&str],
        #[case] card_has_every_type: bool,
        #[case] open: bool,
    ) {
        let mut game =
            GameState::from_setup(&setup(vec![Vec::new(); 3], vec![Vec::new(); 3])).unwrap();
        game.tallies.hits.push(CombatHit {
            controller: PlayerId::from_index(hit_by),
            types: subtypes(creature_types),
            every_type: creature_has_every_type,
        });
        let found = game.dealt_combat_damage_with(
            PlayerId::from_index(caster),
            &subtypes(card_types),
            card_has_every_type,
        );
        assert_eq!(found, open);
        game.clear_turn_uses();
        assert!(!game.dealt_combat_damage_with(
            PlayerId::from_index(caster),
            &subtypes(card_types),
            card_has_every_type
        ));
    }

    /// What happens to the one card of seat 0 that sits in its hand.
    #[derive(Debug, Clone, Copy)]
    enum Fate {
        /// A discard (`CardDiscarded`).
        Discard,
        /// A discard whose move a replacement effect logged as a zone change.
        DiscardMoved,
        /// Put into the graveyard by some other effect.
        Moved,
        /// Discarded, then exiled, then put into the graveyard again.
        DiscardThenAnotherZone,
    }

    #[rstest]
    #[case::a_discard(Fate::Discard, true)]
    #[case::a_discard_logged_as_a_move(Fate::DiscardMoved, true)]
    #[case::any_other_move(Fate::Moved, false)]
    #[case::a_card_that_left_and_came_back(Fate::DiscardThenAnotherZone, false)]
    fn only_a_card_discarded_into_a_graveyard_this_turn_counts_for_mayhem(
        #[case] fate: Fate,
        #[case] counted: bool,
        #[values(2, 3, 4)] seats: usize,
    ) {
        let hands = (0..seats)
            .map(|seat| vec![crate::fixtures::creature(&format!("Card {seat}"), 1, 1)])
            .collect();
        let mut game = GameState::from_setup(&setup(hands, vec![Vec::new(); seats])).unwrap();
        let me = PlayerId::from_index(0);
        let card = game.zone(Zone::Hand(me))[0];
        let moved = |from, to, cause| Event::ZoneChanged {
            object: card,
            from,
            to,
            cause,
        };
        let events = match fate {
            Fate::Discard => vec![Event::CardDiscarded {
                player: me,
                object: card,
            }],
            Fate::DiscardMoved => vec![moved(
                Zone::Hand(me),
                Zone::Graveyard(me),
                ZoneCause::Discarded,
            )],
            Fate::Moved => vec![moved(Zone::Hand(me), Zone::Graveyard(me), ZoneCause::Moved)],
            Fate::DiscardThenAnotherZone => vec![
                Event::CardDiscarded {
                    player: me,
                    object: card,
                },
                moved(Zone::Graveyard(me), Zone::Exile, ZoneCause::Exiled),
                moved(Zone::Exile, Zone::Graveyard(me), ZoneCause::Moved),
            ],
        };
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        assert_eq!(game.zone(Zone::Graveyard(me)), [card]);
        assert_eq!(game.discarded_this_turn(me, card), counted);
        // Nobody else discarded it.
        for seat in 1..seats {
            assert!(!game.discarded_this_turn(PlayerId::from_index(seat), card));
        }
        game.clear_turn_uses();
        assert!(!game.discarded_this_turn(me, card), "the turn is over");
    }
}
