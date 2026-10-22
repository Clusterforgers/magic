//! Invariant checkers: run after every event (cheap, incremental) and after every answer (full).

use std::collections::{BTreeMap, BTreeSet};

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Supertype};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::abilities::AbilityRef;
use mtg_engine::ability_reader;
use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::Target;
use mtg_engine::combat::{AttackTarget, DamageForm, DamageTarget};
use mtg_engine::counters::CountersChanged;
use mtg_engine::effects::Duration;
use mtg_engine::keywords::{is_indestructible, keywords_of};
use mtg_engine::object::{GameObject, ObjectKind, ObjectRef, PermanentState};
use mtg_engine::player::Player;
use mtg_engine::resolve::Scope;
use mtg_engine::spend::{Restricted, Spend};
use mtg_engine::state::Stage;
use mtg_engine::zones::{Zone, Zones};
use mtg_engine::{Characteristics, characteristics};
use mtg_engine::{
    Decision, DecisionKind, Engine, Event, GameSetup, GameState, apply_event, replay,
};
use thiserror::Error;

/// A broken engine invariant.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Violation {
    /// Zone lists and object records disagree, or an object is in zero or several zones.
    #[error("zone index inconsistency: {0}")]
    Zones(String),
    /// Objects appeared or vanished without an explicit event.
    #[error("object count is {actual} but events account for {expected}")]
    Conservation {
        /// Count implied by setup plus creation and removal events.
        expected: usize,
        /// Objects actually in the state.
        actual: usize,
    },
    /// A mana pool disagrees with the mana added and spent.
    #[error("mana pool mismatch: {0}")]
    Mana(String),
    /// A life total or damage counter disagrees with the damage dealt and the life paid.
    #[error("life/damage mismatch: {0}")]
    Damage(String),
    /// An event could not be folded onto the shadow state.
    #[error("event does not fold: {0}")]
    Fold(String),
    /// Live state and the replayed state differ.
    #[error("replay hash {replayed:#x} differs from live hash {live:#x}")]
    Replay {
        /// Hash of the live state.
        live: u64,
        /// Hash of the state rebuilt from events.
        replayed: u64,
    },
    /// Turn, priority or combat bookkeeping is inconsistent.
    #[error("bookkeeping inconsistency: {0}")]
    Bookkeeping(String),
    /// The counters on a permanent or a player disagree with the counters put on and removed.
    #[error("counter mismatch: {0}")]
    Counters(String),
}

fn zone_error(message: impl Into<String>) -> Violation {
    Violation::Zones(message.into())
}

/// Stateless checks on a single state; callable at any decision point.
pub fn check_state(state: &GameState) -> Result<(), Violation> {
    check_zone_index(state)?;
    check_bookkeeping(state)
}

/// Checks what must hold where the engine has stopped to ask: the player asked is still in the game and only an
/// active player declares attackers (CR 800.4), and a player asked for priority has no triggered ability waiting for
/// the stack, since they go on it before a player would receive priority (CR 117.5, 603.3).
pub fn check_decision(engine: &Engine) -> Result<(), Violation> {
    if let Some(decision) = engine.decision() {
        let state = engine.state();
        if !state.is_alive(decision.player) {
            return Err(Violation::Bookkeeping(format!(
                "{:?} is asked but has left the game",
                decision.player
            )));
        }
        if matches!(decision.kind, DecisionKind::DeclareAttackers(_))
            && state.active_player() != Some(decision.player)
        {
            return Err(Violation::Bookkeeping(format!(
                "{:?} is asked for attackers but is not the active player",
                decision.player
            )));
        }
    }
    let asked_for_priority = engine
        .decision()
        .is_some_and(|d| matches!(d.kind, DecisionKind::Priority(_)));
    if asked_for_priority && !engine.state().pending_triggers().is_empty() {
        return Err(Violation::Bookkeeping(
            "a player has priority while triggered abilities wait for the stack".into(),
        ));
    }
    check_legends(engine)
}

/// At a decision point no creature that state-based actions would destroy is still on the battlefield:
/// zero toughness, lethal damage or a mark from deathtouch, the last two unless it is indestructible; and no
/// continuous effect is listed whose duration has ended, one that lasts until the end of the turn included (CR 514.2).
///
/// A resolution in progress defers state-based actions (CR 704.3) and a finished game stops them, so neither is judged.
pub fn check_settled(state: &GameState) -> Result<(), Violation> {
    if state.stage() != Stage::Turns || state.resolving().is_some() {
        return Ok(());
    }
    for effect in state.effects() {
        let from_an_earlier_turn =
            effect.duration == Duration::UntilEndOfTurn && effect.turn != state.turn().number();
        if from_an_earlier_turn || !effect.is_active(state) {
            return Err(Violation::Bookkeeping(format!(
                "{:?} is over but still listed",
                effect.id
            )));
        }
    }
    check_counters_settled(state)?;
    check_attachments_settled(state)?;
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id).filter(|o| state.is_alive(o.controller())) else {
            continue;
        };
        let Some(chars) = characteristics(state, id).filter(Characteristics::is_creature) else {
            continue;
        };
        let toughness = i64::from(chars.toughness.unwrap_or(0));
        let (damage, marked) = object
            .permanent()
            .map_or((0, false), |p| (p.damage(), p.deathtouched()));
        let destroyed = i64::from(damage) >= toughness || marked;
        if toughness <= 0 || (destroyed && !is_indestructible(state, id)) {
            return Err(Violation::Bookkeeping(format!(
                "{id:?} should have died: toughness {toughness}, damage {damage}, deathtouch mark {marked}"
            )));
        }
    }
    Ok(())
}

/// At a decision point every Aura on the battlefield is attached to a permanent it may enchant, an Equipment or Fortification
/// that is attached is attached to a creature or a land, and nothing else is attached (CR 704.5m to 704.5p).
fn check_attachments_settled(state: &GameState) -> Result<(), Violation> {
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id).filter(|o| state.is_alive(o.controller())) else {
            continue;
        };
        let printed = |name: &str| object.card().subtypes.iter().any(|s| s.0 == name);
        let attached = object.permanent().and_then(PermanentState::attached_to);
        let enchanted = state.player_host_of(id);
        let host = state.host_of(id).and_then(|host| state.object(host));
        let bad = |why: &str| Violation::Bookkeeping(format!("{id:?} {why} at a decision point"));
        if printed("Aura") {
            if let Some(player) = enchanted {
                let controller = controller_of(state, id).unwrap_or(object.controller());
                for keyword in keywords_of(state, id) {
                    if matches!(keyword, Keyword::EnchantPlayer { opponent: true })
                        && player == controller
                    {
                        return Err(bad(
                            "enchants its own controller though it enchants opponents",
                        ));
                    }
                }
                if !keywords_of(state, id)
                    .iter()
                    .any(|keyword| matches!(keyword, Keyword::EnchantPlayer { .. }))
                {
                    return Err(bad("enchants a player though it may not"));
                }
                continue;
            }
            let Some(host) = host else {
                // An Aura with a clause left to the table has its enchant ability in it and waits for its controller to
                // attach it by hand (`manual.rs`), so it stays unattached.
                if object.card().manual.is_empty() {
                    return Err(bad("is an Aura attached to nothing"));
                }
                continue;
            };
            let controller = controller_of(state, id).unwrap_or(object.controller());
            let scope = Scope::announcing(state, controller, object.reference());
            for keyword in keywords_of(state, id) {
                if let Keyword::Enchant(selector) = keyword
                    && !scope.selects(selector, host.reference()).unwrap_or(false)
                {
                    return Err(bad("is attached to what it may not enchant"));
                }
            }
        } else if printed("Equipment") || printed("Fortification") {
            if attached.is_none() {
                continue;
            }
            let Some(host) = host else {
                return Err(bad("is attached to a host that left"));
            };
            let is_creature =
                characteristics(state, host.id()).is_some_and(|chars| chars.is_creature());
            let is_land = host.base_types().contains(CardType::Land);
            let fits = if printed("Equipment") {
                is_creature
            } else {
                is_land
            };
            if !fits || host.id() == id {
                return Err(bad("is attached to what it may not be"));
            }
            // An Equipment that is a creature equips nothing unless it has reconfigure (CR 301.5c, 702.151).
            if printed("Equipment")
                && characteristics(state, id).is_some_and(|chars| chars.is_creature())
                && !keywords_of(state, id)
                    .iter()
                    .any(|keyword| matches!(keyword, Keyword::Reconfigure(_)))
            {
                return Err(bad(
                    "is an Equipment attached to a creature though it is one",
                ));
            }
        } else if attached.is_some() || enchanted.is_some() {
            return Err(bad(
                "is attached though it is no Aura, Equipment or Fortification",
            ));
        }
    }
    Ok(())
}

/// At a decision point no player controls two legendary permanents with one name (CR 704.5j), unless the engine is asking that
/// player which to keep: then the question names exactly those permanents and asks for one.
fn check_legends(engine: &Engine) -> Result<(), Violation> {
    let state = engine.state();
    if state.stage() != Stage::Turns || state.resolving().is_some() {
        return Ok(());
    }
    let mut groups: BTreeMap<(PlayerId, String), Vec<ObjectId>> = BTreeMap::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        let controller = controller_of(state, id).unwrap_or(object.controller());
        if object.card().supertypes.contains(&Supertype::Legendary) && state.is_alive(controller) {
            groups
                .entry((controller, object.card().name.clone()))
                .or_default()
                .push(id);
        }
    }
    groups.retain(|_, ids| ids.len() > 1);
    for ids in groups.values_mut() {
        ids.sort_unstable();
    }
    if groups.is_empty() {
        return Ok(());
    }
    let asked = match engine.decision() {
        Some(Decision {
            player,
            kind: DecisionKind::ChooseCards(options),
        }) if options.min == 1 && options.max == 1 => {
            let mut from = options.from.clone();
            from.sort_unstable();
            groups
                .iter()
                .any(|((owner, _), ids)| owner == player && *ids == from)
        }
        _ => false,
    };
    if asked {
        Ok(())
    } else {
        Err(Violation::Bookkeeping(format!(
            "a player controls two legendary permanents of one name at a decision that is not the legend rule: {groups:?}"
        )))
    }
}

/// At a decision point no permanent holds both a +1/+1 and a -1/-1 counter (CR 704.5q) and no player in the game holds ten
/// poison counters (CR 704.5c): state-based actions took them off, or the player out.
fn check_counters_settled(state: &GameState) -> Result<(), Violation> {
    for &id in state.zone(Zone::Battlefield) {
        let held = |kind: &CounterKind| {
            state
                .object(id)
                .and_then(|o| o.permanent())
                .map_or(0, |p| p.counters_of(kind))
        };
        if held(&CounterKind::PlusOnePlusOne) > 0 && held(&CounterKind::MinusOneMinusOne) > 0 {
            return Err(Violation::Counters(format!(
                "{id:?} holds +1/+1 and -1/-1 counters at a decision point"
            )));
        }
        // A planeswalker with no loyalty went to its owner's graveyard (CR 704.5i).
        let walker = state.object(id).filter(|object| {
            object.base_types().contains(CardType::Planeswalker)
                && state.is_alive(object.controller())
        });
        if walker.is_some() && held(&CounterKind::Loyalty) == 0 {
            return Err(Violation::Counters(format!(
                "{id:?} is a planeswalker with no loyalty at a decision point"
            )));
        }
    }
    for player in state.players().iter().filter(|p| p.is_in_game()) {
        if player.counters_of(&CounterKind::Poison) >= state.ruleset().poison_threshold() {
            return Err(Violation::Counters(format!(
                "{:?} holds {} poison counters and is still in the game",
                player.id(),
                player.counters_of(&CounterKind::Poison)
            )));
        }
        if let Some(lethal) = state.ruleset().commander_damage()
            && let Some(commander) = state
                .commanders()
                .find(|id| state.commander_damage(*id, player.id()) >= lethal)
        {
            return Err(Violation::Damage(format!(
                "{:?} has taken {lethal} or more from {commander:?} and is still in the game",
                player.id()
            )));
        }
    }
    Ok(())
}

fn check_zone_index(state: &GameState) -> Result<(), Violation> {
    let mut seen: BTreeSet<ObjectId> = BTreeSet::new();
    for zone in Zones::all_zones(state.players().len()) {
        for &id in state.zone(zone) {
            let object = state
                .object(id)
                .ok_or_else(|| zone_error(format!("{zone:?} lists unknown {id:?}")))?;
            if object.zone() != zone {
                return Err(zone_error(format!(
                    "{id:?} is listed in {zone:?} but records {:?}",
                    object.zone()
                )));
            }
            if !seen.insert(id) {
                return Err(zone_error(format!("{id:?} is in more than one zone")));
            }
            let owned_zone = match zone {
                Zone::Library(p) | Zone::Hand(p) | Zone::Graveyard(p) | Zone::Command(p) => Some(p),
                _ => None,
            };
            if owned_zone.is_some_and(|p| p != object.owner()) {
                return Err(zone_error(format!(
                    "{id:?} is in {zone:?} but owned by {:?}",
                    object.owner()
                )));
            }
            if object.permanent().is_some() != (zone == Zone::Battlefield) {
                return Err(zone_error(format!(
                    "{id:?} permanent state does not match {zone:?}"
                )));
            }
        }
    }
    if seen.len() != state.object_count() {
        return Err(zone_error(format!(
            "{} objects exist but {} are in a zone",
            state.object_count(),
            seen.len()
        )));
    }
    Ok(())
}

/// Every attacker and blocker is on the battlefield, no one attacks a player who is out, and what an attacker attacks is a
/// planeswalker in play, or nothing once the planeswalker has left (CR 506.4c), when no one may block it.
fn check_attackers(state: &GameState) -> Result<(), Violation> {
    let book = |message: String| Violation::Bookkeeping(message);
    let on_battlefield = |id: ObjectId| {
        state
            .object(id)
            .is_some_and(|o| o.zone() == Zone::Battlefield)
    };
    for attacker in state.combat().attackers() {
        if !on_battlefield(attacker.object()) {
            return Err(book(format!(
                "attacker {:?} left the battlefield",
                attacker.object()
            )));
        }
        if let Some(missing) = attacker.blockers().iter().find(|b| !on_battlefield(**b)) {
            return Err(book(format!("blocker {missing:?} left the battlefield")));
        }
        match attacker.target() {
            AttackTarget::Player(p) if !state.is_alive(p) => {
                return Err(book(format!("attacking eliminated {p:?}")));
            }
            AttackTarget::Planeswalker(o) if !on_battlefield(o) && !attacker.attacks_nothing() => {
                return Err(book(format!("attacking missing planeswalker {o:?}")));
            }
            AttackTarget::Planeswalker(_)
                if attacker.attacks_nothing() && attacker.defender(state).is_some() =>
            {
                return Err(book(format!(
                    "{:?} attacks nothing but has a defending player",
                    attacker.object()
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

fn check_bookkeeping(state: &GameState) -> Result<(), Violation> {
    let book = |message: String| Violation::Bookkeeping(message);
    if let Some(pair) = state
        .effects()
        .windows(2)
        .find(|pair| pair[0].id >= pair[1].id || pair[0].timestamp >= pair[1].timestamp)
    {
        return Err(book(format!(
            "effects {:?} and {:?} are not in the order of their ids and timestamps",
            pair[0].id, pair[1].id
        )));
    }
    let seats = state.players().len();
    if state.turn().active().index() >= seats {
        return Err(book("active player does not exist".into()));
    }
    if let Some(priority) = state.priority()
        && !state.is_alive(priority.holder())
        && state.stage() != Stage::Finished
    {
        return Err(book(format!(
            "{:?} holds priority but is out",
            priority.holder()
        )));
    }
    check_attackers(state)?;
    if state.stage() != Stage::Finished
        && let Some(out) = state.objects().find(|o| !state.is_alive(o.owner()))
    {
        return Err(book(format!(
            "{:?} still exists after its owner left",
            out.id()
        )));
    }
    if state.stage() != Stage::Finished {
        for waiting in state.pending_triggers() {
            let t = waiting.trigger();
            // A token that ceased to exist as a state-based action (a copy of a legendary permanent spell) leaves the
            // triggers it caused waiting, and they read its last known information (CR 603.10a).
            let gone = state.object(t.source.id).is_none() && t.snapshot.zone != Zone::Battlefield;
            if !state.is_alive(t.controller) || gone {
                return Err(book(format!(
                    "a triggered ability of {:?} waits for the stack but its controller or source has left the game",
                    t.source
                )));
            }
        }
    }
    if let Some(resolving) = state.resolving() {
        let id = resolving.object();
        let Some(at) = state.stack().iter().position(|object| *object == id) else {
            return Err(book(format!("{id:?} is resolving but is not on the stack")));
        };
        // What its resolution put on the stack (a copy, a spell cast during it) lies above it until it ends.
        for above in &state.stack()[at + 1..] {
            let put_by_it = state
                .object(*above)
                .is_some_and(|o| o.kind() == ObjectKind::Card || o.copied_from().is_some());
            if !put_by_it {
                return Err(book(format!(
                    "{id:?} is resolving but {above:?} above it was not put there by it"
                )));
            }
        }
    }
    for object in state.objects() {
        if object.kind() == ObjectKind::Ability && object.zone() != Zone::Stack {
            return Err(book(format!(
                "ability {:?} is outside the stack",
                object.id()
            )));
        }
        if object.zone() != Zone::Stack && !object.choices().is_empty() {
            return Err(book(format!(
                "{:?} keeps what was announced for it after leaving the stack",
                object.id()
            )));
        }
    }
    Ok(())
}

/// Follows a game event by event on a shadow copy and cross-checks it against the live engine.
#[derive(Debug)]
pub struct InvariantChecker {
    setup: GameSetup,
    shadow: GameState,
    expected_objects: usize,
    expected_life: Vec<i32>,
    expected_pool: Vec<[i64; 6]>,
    /// The restricted mana each pool holds, by the history of what was added and spent.
    expected_restricted: Vec<Vec<Restricted>>,
    expected_damage: BTreeMap<ObjectId, u32>,
    expected_object_counters: BTreeMap<ObjectId, BTreeMap<CounterKind, i64>>,
    expected_player_counters: BTreeMap<PlayerId, BTreeMap<CounterKind, i64>>,
    expected_attached: BTreeMap<ObjectId, ObjectRef>,
    /// The player each Aura that enchants one is attached to, as the events say.
    expected_enchanted: BTreeMap<ObjectId, PlayerId>,
    loyalty_turns: BTreeMap<ObjectRef, u32>,
    zones_seen: BTreeMap<ObjectId, (Zone, u32)>,
    events_seen: usize,
    full_every: usize,
    since_full: usize,
    /// The engine is asking about events it has not applied: what state-based actions would do is still to come.
    replacing: bool,
}

impl InvariantChecker {
    /// Starts from the initial state of `setup`.
    pub fn new(setup: &GameSetup) -> Result<Self, Violation> {
        let shadow = GameState::from_setup(setup).map_err(|e| Violation::Fold(e.to_string()))?;
        let expected_damage = shadow
            .objects()
            .filter_map(|o| Some((o.id(), o.permanent()?.damage())))
            .filter(|(_, damage)| *damage > 0)
            .collect();
        let expected_object_counters = shadow
            .objects()
            .filter_map(|o| {
                let held: BTreeMap<CounterKind, i64> = o
                    .permanent()?
                    .counters()
                    .map(|(kind, count)| (kind.clone(), i64::from(count)))
                    .collect();
                (!held.is_empty()).then_some((o.id(), held))
            })
            .collect();
        let expected_player_counters = shadow
            .players()
            .iter()
            .map(|p| {
                let held: BTreeMap<CounterKind, i64> = p
                    .counters()
                    .map(|(kind, count)| (kind.clone(), i64::from(count)))
                    .collect();
                (p.id(), held)
            })
            .filter(|(_, held)| !held.is_empty())
            .collect();
        let expected_attached = shadow
            .objects()
            .filter_map(|o| Some((o.id(), o.permanent()?.attached_to()?)))
            .collect();
        let expected_enchanted = shadow
            .objects()
            .filter_map(|o| Some((o.id(), o.permanent()?.attached_player()?)))
            .collect();
        Ok(InvariantChecker {
            setup: setup.clone(),
            expected_attached,
            expected_enchanted,
            expected_objects: shadow.object_count(),
            expected_life: shadow.players().iter().map(Player::life).collect(),
            expected_pool: vec![[0; 6]; shadow.players().len()],
            expected_restricted: vec![Vec::new(); shadow.players().len()],
            expected_damage,
            expected_object_counters,
            expected_player_counters,
            loyalty_turns: BTreeMap::new(),
            zones_seen: BTreeMap::new(),
            shadow,
            events_seen: 0,
            full_every: 1,
            replacing: false,
            since_full: 0,
        })
    }

    /// [`observe`](Self::observe) for a live engine, with the checks that depend on the decision it stands at.
    pub fn observe_engine(&mut self, engine: &Engine) -> Result<(), Violation> {
        self.replacing = engine.replacing();
        self.observe(engine.state(), engine.events())?;
        if self.replacing {
            return Ok(());
        }
        check_decision(engine)
    }

    /// Runs the whole-state checks only every `n` calls to [`observe`](Self::observe); per-event checks always run.
    #[must_use]
    pub fn with_full_check_every(mut self, n: usize) -> Self {
        self.full_every = n.max(1);
        self
    }

    /// Consumes every event of `log` not yet seen, then (on schedule) runs the whole-state checks against `live`.
    pub fn observe(&mut self, live: &GameState, log: &[Event]) -> Result<(), Violation> {
        for event in log.get(self.events_seen..).unwrap_or_default() {
            self.observe_event(event)?;
            self.events_seen += 1;
        }
        self.since_full += 1;
        if self.since_full < self.full_every {
            return Ok(());
        }
        self.full_check(live)
    }

    /// Runs the whole-state checks now: zone index, bookkeeping, damage totals and live-vs-shadow hash.
    pub fn full_check(&mut self, live: &GameState) -> Result<(), Violation> {
        self.since_full = 0;
        check_state(live)?;
        if !self.replacing {
            check_settled(live)?;
        }
        let (live_hash, shadow_hash) = (live.state_hash(), self.shadow.state_hash());
        if live_hash != shadow_hash {
            return Err(Violation::Replay {
                live: live_hash,
                replayed: shadow_hash,
            });
        }
        self.check_zone_changes()?;
        self.check_damage_totals()?;
        self.check_counter_totals()?;
        self.check_attached_totals()
    }

    /// What every permanent is attached to must equal what the attach and unattach events say, and an object that left the
    /// battlefield keeps nothing (CR 400.7).
    fn check_attached_totals(&self) -> Result<(), Violation> {
        for object in self.shadow.objects() {
            let held = object.permanent().and_then(PermanentState::attached_to);
            if held != self.expected_attached.get(&object.id()).copied() {
                return Err(Violation::Bookkeeping(format!(
                    "{:?} is attached to {held:?} but the events say {:?}",
                    object.id(),
                    self.expected_attached.get(&object.id())
                )));
            }
            let enchanting = object.permanent().and_then(PermanentState::attached_player);
            if enchanting != self.expected_enchanted.get(&object.id()).copied() {
                return Err(Violation::Bookkeeping(format!(
                    "{:?} enchants {enchanting:?} but the events say {:?}",
                    object.id(),
                    self.expected_enchanted.get(&object.id())
                )));
            }
        }
        Ok(())
    }

    /// An object that is in another zone than when last seen has a higher zone-change counter, and no counter ever falls (CR 400.7).
    fn check_zone_changes(&mut self) -> Result<(), Violation> {
        for object in self.shadow.objects() {
            let now = (object.zone(), object.counter());
            if let Some(&(zone, counter)) = self.zones_seen.get(&object.id()) {
                let moved_without_counting = now.1 == counter && now.0 != zone;
                if now.1 < counter || moved_without_counting {
                    return Err(zone_error(format!(
                        "{:?} was in {zone:?} at counter {counter} and is in {:?} at counter {}",
                        object.id(),
                        now.0,
                        now.1
                    )));
                }
            }
            self.zones_seen.insert(object.id(), now);
        }
        Ok(())
    }

    /// Replays the whole log from the setup and compares it with `live`.
    pub fn check_full_replay(&self, live: &GameState, log: &[Event]) -> Result<(), Violation> {
        let replayed = replay(&self.setup, log).map_err(|e| Violation::Fold(e.to_string()))?;
        if replayed.state_hash() == live.state_hash() {
            Ok(())
        } else {
            Err(Violation::Replay {
                live: live.state_hash(),
                replayed: replayed.state_hash(),
            })
        }
    }

    fn observe_event(&mut self, event: &Event) -> Result<(), Violation> {
        // An ability ceases to exist when its resolution ends, and the fold removes it, so look first.
        let ability_ends = matches!(event, Event::ResolutionEnded { object, .. }
            if self.shadow.object(*object).is_some_and(|o| o.kind() == ObjectKind::Ability));
        // A token ceases to exist the moment it is anywhere but the battlefield (CR 111.7), and the fold removes it.
        let is_token = |id: &ObjectId| {
            self.shadow
                .object(*id)
                .is_some_and(|o| o.kind() == ObjectKind::Token)
        };
        let token_ends = match event {
            Event::ZoneChanged { object, to, .. } => *to != Zone::Battlefield && is_token(object),
            Event::CreatureDied { object, .. } => is_token(object),
            _ => false,
        };
        // A copy of a spell ceases to exist as it resolves or ends, unless it becomes a permanent (CR 707.10).
        let copy_on_its_way = match event {
            Event::SpellResolved { object } | Event::ResolutionEnded { object, .. } => self
                .shadow
                .object(*object)
                .filter(|o| o.is_spell_copy())
                .map(mtg_engine::object::GameObject::id),
            _ => None,
        };
        // The next turn goes to the next player in turn order after the last active player, who may have left (CR 800.4).
        if let Event::TurnBegan { number, active } = event
            && self.shadow.turn().number() > 0
            && (self.shadow.next_alive_after(self.shadow.turn().active()) != Some(*active)
                || *number != self.shadow.turn().number() + 1)
        {
            return Err(Violation::Bookkeeping(format!(
                "turn {number} goes to {active:?}, not to the next player in turn order after {:?}",
                self.shadow.turn().active()
            )));
        }
        apply_event(&mut self.shadow, event)
            .map_err(|e| Violation::Fold(format!("{event:?}: {e}")))?;
        self.account(event)?;
        let touched = event.touched_objects();
        if matches!(event, Event::ObjectLeftGame { .. }) || ability_ends || token_ends {
            self.expected_objects -= 1;
        }
        if matches!(event, Event::TokenCreated(_)) {
            self.expected_objects += 1;
        }
        if matches!(
            event,
            Event::AbilityPutOnStack(_) | Event::SpellCopied(_) | Event::AbilityCopied(_)
        ) {
            self.expected_objects += 1;
        }
        if copy_on_its_way.is_some_and(|id| self.shadow.has_ceased(id)) {
            self.expected_objects -= 1;
        }
        if !touched.is_empty() {
            self.check_conservation()?;
            for id in touched {
                self.check_touched(id, event)?;
            }
        }
        Ok(())
    }

    fn check_conservation(&self) -> Result<(), Violation> {
        let actual = self.shadow.object_count();
        let in_zones: usize = Zones::all_zones(self.shadow.players().len())
            .map(|z| self.shadow.zone(z).len())
            .sum();
        if actual != self.expected_objects || in_zones != actual {
            return Err(Violation::Conservation {
                expected: self.expected_objects,
                actual: actual.min(in_zones),
            });
        }
        Ok(())
    }

    fn check_touched(&self, id: ObjectId, event: &Event) -> Result<(), Violation> {
        match self.shadow.object(id) {
            None if matches!(event, Event::ObjectLeftGame { .. }) || self.shadow.has_ceased(id) => {
                Ok(())
            }
            None => Err(zone_error(format!("{id:?} vanished after {event:?}"))),
            Some(object) => {
                let listed = self
                    .shadow
                    .zone(object.zone())
                    .iter()
                    .filter(|o| **o == id)
                    .count();
                if listed == 1 {
                    Ok(())
                } else {
                    Err(zone_error(format!(
                        "{id:?} is listed {listed} times in {:?} after {event:?}",
                        object.zone()
                    )))
                }
            }
        }
    }

    /// Updates the independent accumulators from the event and compares them to the shadow state.
    fn account(&mut self, event: &Event) -> Result<(), Violation> {
        match event {
            Event::ManaAdded {
                player, mana, only, ..
            } => {
                self.adjust_pool(*player, *mana, 1)?;
                if !only.is_empty() {
                    self.held_restricted(*player)?.push(Restricted {
                        mana: *mana,
                        only: only.clone(),
                    });
                }
                self.check_pool(*player)
            }
            Event::ManaSpent {
                player,
                mana,
                restricted,
            } => {
                for m in mana {
                    self.adjust_pool(*player, *m, -1)?;
                }
                self.check_restrictions_honoured(restricted)?;
                for unit in restricted {
                    let held = self.held_restricted(*player)?;
                    let Some(at) = held.iter().position(|h| h == unit) else {
                        return Err(Violation::Mana(format!(
                            "{player:?} spent {:?} mana restricted to something they did not hold",
                            unit.mana
                        )));
                    };
                    held.remove(at);
                }
                self.check_pool(*player)
            }
            Event::ManaPoolEmptied { player } => {
                if let Some(cells) = self.expected_pool.get_mut(player.index()) {
                    *cells = [0; 6];
                }
                self.held_restricted(*player)?.clear();
                self.check_pool(*player)
            }
            Event::DamageDealt {
                target,
                amount,
                form,
                toxic,
                ..
            } => match target {
                DamageTarget::Player(p) => self.damage_player(*p, *amount, *form, *toxic),
                DamageTarget::Object(o) => self.damage_object(*o, *amount, *form),
            },
            Event::AbilityActivated {
                player,
                source,
                ability,
                origin,
            } => self.check_loyalty_activation(*player, *source, *ability, *origin),
            Event::LifePaid { player, amount } => self.lose_life(*player, *amount, "life paid"),
            Event::LifeLost { player, amount } => self.lose_life(*player, *amount, "life lost"),
            Event::LifeGained { player, amount } => {
                self.change_life(*player, i64::from(*amount), "life gained")
            }
            Event::DamageCleared => {
                self.expected_damage.clear();
                Ok(())
            }
            // A permanent that regenerates loses its damage.
            Event::Regenerated { object } => {
                self.expected_damage.remove(object);
                Ok(())
            }
            // An object that leaves the battlefield is another object when it comes back (CR 400.7), with no damage.
            Event::ZoneChanged {
                object,
                from: Zone::Battlefield,
                ..
            }
            | Event::CreatureDied { object, .. } => {
                self.expected_damage.remove(object);
                // Counters stay with the object that held them, so a permanent that comes back holds none (CR 400.7);
                // nor does it come back attached to anything.
                self.expected_object_counters.remove(object);
                self.expected_attached.remove(object);
                self.expected_enchanted.remove(object);
                Ok(())
            }
            Event::CountersChanged(changed) => self.adjust_counters(changed),
            Event::Attached { .. } | Event::AttachedToPlayer { .. } | Event::Unattached { .. } => {
                self.account_attachment(event)
            }
            _ => Ok(()),
        }
    }

    /// What an attach, an attach to a player and an unattach leave every attachment attached to.
    fn account_attachment(&mut self, event: &Event) -> Result<(), Violation> {
        match event {
            Event::Attached { object, to } => {
                let host = self
                    .shadow
                    .object(*to)
                    .map(mtg_engine::object::GameObject::reference)
                    .ok_or_else(|| zone_error(format!("{object:?} attached to missing {to:?}")))?;
                self.expected_attached.insert(*object, host);
                self.expected_enchanted.remove(object);
            }
            Event::AttachedToPlayer { object, player } => {
                self.expected_attached.remove(object);
                self.expected_enchanted.insert(*object, *player);
            }
            Event::Unattached { object } => {
                self.expected_attached.remove(object);
                self.expected_enchanted.remove(object);
            }
            _ => {}
        }
        Ok(())
    }

    /// Damage to a player is life lost, or with infect poison counters instead; toxic gives poison counters on top.
    fn damage_player(
        &mut self,
        player: PlayerId,
        amount: u32,
        form: DamageForm,
        toxic: u32,
    ) -> Result<(), Violation> {
        let poison = if form == DamageForm::Infect {
            amount.saturating_add(toxic)
        } else {
            self.lose_life(player, amount, "damage")?;
            toxic
        };
        if poison == 0 {
            return Ok(());
        }
        self.adjust_counters(&CountersChanged {
            on: Target::Player(player),
            counter: CounterKind::Poison,
            delta: i32::try_from(poison).unwrap_or(i32::MAX),
        })
    }

    /// Damage to a permanent is marked on a creature (CR 120.3e) and removes that many loyalty counters from a
    /// planeswalker, no more than it holds (CR 120.3c); one that is both takes both.
    fn damage_object(
        &mut self,
        object: ObjectId,
        amount: u32,
        form: DamageForm,
    ) -> Result<(), Violation> {
        let printed = self
            .shadow
            .object(object)
            .map(mtg_engine::object::GameObject::base_types);
        let walker = printed.is_some_and(|types| types.contains(CardType::Planeswalker));
        let creature = printed.is_some_and(|types| types.contains(CardType::Creature));
        if (!walker || creature) && form.makes_counters() {
            // Wither and infect: -1/-1 counters instead of damage marked (CR 120.3d).
            if let Some(on) = self.shadow.object(object).map(GameObject::reference) {
                self.adjust_counters(&CountersChanged {
                    on: Target::Object(on),
                    counter: CounterKind::MinusOneMinusOne,
                    delta: i32::try_from(amount).unwrap_or(i32::MAX),
                })?;
            }
        } else if !walker || creature {
            *self.expected_damage.entry(object).or_default() += amount;
        }
        if walker && let Some(held) = self.expected_object_counters.get_mut(&object) {
            if let Some(loyalty) = held.get_mut(&CounterKind::Loyalty) {
                *loyalty = (*loyalty - i64::from(amount)).max(0);
                if *loyalty == 0 {
                    held.remove(&CounterKind::Loyalty);
                }
            }
            if held.is_empty() {
                self.expected_object_counters.remove(&object);
            }
        }
        Ok(())
    }

    /// A loyalty ability and an ability that may be activated only as a sorcery (Equip, Reconfigure) are activated by the
    /// active player in a main step with an empty stack (CR 606.3, 307.1), and a loyalty ability once for each planeswalker
    /// each turn.
    fn check_loyalty_activation(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        ability: usize,
        origin: mtg_engine::abilities::AbilitySource,
    ) -> Result<(), Violation> {
        let address = AbilityRef {
            object: source.id,
            ability,
            source: origin,
        };
        let rules = ability_reader::ability(&self.shadow, address);
        let loyalty = rules
            .as_ref()
            .is_some_and(|rules| matches!(**rules, Ability::Loyalty { .. }));
        let sorcery = rules.as_ref().is_some_and(|rules| {
            matches!(
                **rules,
                Ability::Activated {
                    timing: Timing::Sorcery,
                    ..
                }
            )
        });
        if !loyalty && !sorcery {
            return Ok(());
        }
        let turn = self.shadow.turn();
        if turn.active() != player || !turn.step().is_main() || !self.shadow.stack().is_empty() {
            return Err(Violation::Bookkeeping(format!(
                "{player:?} activated a sorcery-speed ability of {source:?} in {:?} of {:?}'s turn with {} on the stack",
                turn.step(),
                turn.active(),
                self.shadow.stack().len()
            )));
        }
        if !loyalty {
            return Ok(());
        }
        let number = turn.number();
        if self.loyalty_turns.insert(source, number) == Some(number) {
            return Err(Violation::Bookkeeping(format!(
                "{source:?} had a second loyalty ability activated in turn {number}"
            )));
        }
        Ok(())
    }

    /// Moves the independent history of the counters on `changed.on` by the change and fails when it would hold fewer
    /// than none.
    fn adjust_counters(&mut self, changed: &CountersChanged) -> Result<(), Violation> {
        let held = match changed.on {
            Target::Object(named) => self.expected_object_counters.entry(named.id).or_default(),
            Target::Player(player) => self.expected_player_counters.entry(player).or_default(),
        };
        let slot = held.entry(changed.counter.clone()).or_default();
        *slot += i64::from(changed.delta);
        if *slot < 0 {
            return Err(Violation::Counters(format!(
                "{:?} would hold {slot} {:?} counters",
                changed.on, changed.counter
            )));
        }
        if *slot == 0 {
            held.remove(&changed.counter);
        }
        self.expected_object_counters
            .retain(|_, held| !held.is_empty());
        self.expected_player_counters
            .retain(|_, held| !held.is_empty());
        Ok(())
    }

    /// The counters on every permanent and player must equal those put on less those taken off, and no kind is held in a
    /// quantity of none.
    fn check_counter_totals(&self) -> Result<(), Violation> {
        // Nothing was put on anything: nothing may hold any, which a game without counters sees without allocating.
        if self.expected_object_counters.is_empty() && self.expected_player_counters.is_empty() {
            let held = self
                .shadow
                .zone(Zone::Battlefield)
                .iter()
                .filter_map(|id| self.shadow.object(*id)?.permanent())
                .any(|permanent| permanent.counters().next().is_some())
                || self
                    .shadow
                    .players()
                    .iter()
                    .any(|player| player.counters().next().is_some());
            return if held {
                Err(Violation::Counters(
                    "counters are held that nothing put on".into(),
                ))
            } else {
                Ok(())
            };
        }
        let listed = |held: &BTreeMap<CounterKind, i64>| -> Vec<(CounterKind, i64)> {
            held.iter().map(|(kind, n)| (kind.clone(), *n)).collect()
        };
        let none = BTreeMap::new();
        for &id in self.shadow.zone(Zone::Battlefield) {
            let Some(permanent) = self.shadow.object(id).and_then(|o| o.permanent()) else {
                continue;
            };
            let state: Vec<(CounterKind, i64)> = permanent
                .counters()
                .map(|(kind, count)| (kind.clone(), i64::from(count)))
                .collect();
            let expected = listed(self.expected_object_counters.get(&id).unwrap_or(&none));
            if state.iter().any(|(_, count)| *count == 0) || state != expected {
                return Err(Violation::Counters(format!(
                    "{id:?} holds {state:?} but {expected:?} were put on"
                )));
            }
        }
        for player in self.shadow.players() {
            let state: Vec<(CounterKind, i64)> = player
                .counters()
                .map(|(kind, count)| (kind.clone(), i64::from(count)))
                .collect();
            let expected = listed(
                self.expected_player_counters
                    .get(&player.id())
                    .unwrap_or(&none),
            );
            if state.iter().any(|(_, count)| *count == 0) || state != expected {
                return Err(Violation::Counters(format!(
                    "{:?} holds {state:?} but {expected:?} were put on",
                    player.id()
                )));
            }
        }
        Ok(())
    }

    /// Takes `amount` off the independent life history of `player` and compares it with the shadow state.
    fn lose_life(&mut self, player: PlayerId, amount: u32, cause: &str) -> Result<(), Violation> {
        self.change_life(player, -i64::from(amount), cause)
    }

    /// Moves the independent life history of `player` by `delta` and compares it with the shadow state.
    fn change_life(&mut self, player: PlayerId, delta: i64, cause: &str) -> Result<(), Violation> {
        let life = self
            .expected_life
            .get_mut(player.index())
            .ok_or_else(|| Violation::Damage(format!("unknown {player:?}")))?;
        *life = i32::try_from(i64::from(*life) + delta).unwrap_or(i32::MAX);
        let actual = self.shadow.player(player).map(Player::life);
        if actual == Some(*life) {
            Ok(())
        } else {
            Err(Violation::Damage(format!(
                "{player:?} life is {actual:?}, the {cause} history says {life}"
            )))
        }
    }

    fn adjust_pool(
        &mut self,
        player: PlayerId,
        mana: ManaType,
        delta: i64,
    ) -> Result<(), Violation> {
        let cells = self
            .expected_pool
            .get_mut(player.index())
            .ok_or_else(|| Violation::Mana(format!("unknown {player:?}")))?;
        cells[mana.index()] += delta;
        Ok(())
    }

    /// The restricted mana the history says `player` holds.
    fn held_restricted(&mut self, player: PlayerId) -> Result<&mut Vec<Restricted>, Violation> {
        self.expected_restricted
            .get_mut(player.index())
            .ok_or_else(|| Violation::Mana(format!("unknown {player:?}")))
    }

    /// Restricted mana spent for a spell is mana its restriction allows: the spell is the top of the stack once its cast
    /// events have folded, which come before the mana it is paid with is spent.
    fn check_restrictions_honoured(&self, units: &[Restricted]) -> Result<(), Violation> {
        let top = self
            .shadow
            .zone(Zone::Stack)
            .last()
            .and_then(|id| self.shadow.object(*id));
        let Some(spell) = top.filter(|o| o.kind() == ObjectKind::Card) else {
            return Ok(());
        };
        let spend = Spend::for_spell(&self.shadow, spell.id(), spell.card().x_in_cost > 0);
        match units.iter().find(|unit| !spend.allows(&unit.only)) {
            Some(unit) => Err(Violation::Mana(format!(
                "{:?} mana restricted to {:?} was spent on {}",
                unit.mana,
                unit.only,
                spell.card().name
            ))),
            None => Ok(()),
        }
    }

    fn check_pool(&self, player: PlayerId) -> Result<(), Violation> {
        let expected = self.expected_pool.get(player.index());
        let actual = self.shadow.player(player).map(Player::mana);
        match (expected, actual) {
            (Some(expected), Some(pool)) => {
                for mana in ManaType::ALL {
                    let held = pool.get(mana)
                        + u32::try_from(
                            pool.restricted().iter().filter(|u| u.mana == mana).count(),
                        )
                        .unwrap_or(u32::MAX);
                    let want = expected[mana.index()];
                    if want < 0 || i64::from(held) != want {
                        return Err(Violation::Mana(format!(
                            "{player:?} {mana:?}: pool has {held}, history says {want}"
                        )));
                    }
                }
                let history = self.expected_restricted.get(player.index());
                if history.is_none_or(|h| h.as_slice() != pool.restricted()) {
                    return Err(Violation::Mana(format!(
                        "{player:?}: the pool holds {:?} restricted, history says {history:?}",
                        pool.restricted()
                    )));
                }
                Ok(())
            }
            _ => Err(Violation::Mana(format!("unknown {player:?}"))),
        }
    }

    /// Damage marked on every battlefield object must equal the damage events since it entered or was cleared.
    fn check_damage_totals(&mut self) -> Result<(), Violation> {
        self.expected_damage.retain(|id, _| {
            self.shadow
                .object(*id)
                .is_some_and(|o| o.zone() == Zone::Battlefield)
        });
        for &id in self.shadow.zone(Zone::Battlefield) {
            let marked = self
                .shadow
                .object(id)
                .and_then(|o| o.permanent())
                .map_or(0, PermanentState::damage);
            let expected = self.expected_damage.get(&id).copied().unwrap_or(0);
            if marked != expected {
                return Err(Violation::Damage(format!(
                    "{id:?} has {marked} damage marked but {expected} was dealt"
                )));
            }
        }
        Ok(())
    }
}
