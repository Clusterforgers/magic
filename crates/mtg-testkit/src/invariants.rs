//! Invariant checkers: run after every event (cheap, incremental) and after every answer (full).

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::combat::{AttackTarget, DamageTarget};
use mtg_engine::object::PermanentState;
use mtg_engine::player::Player;
use mtg_engine::state::Stage;
use mtg_engine::zones::{Zone, Zones};
use mtg_engine::{Event, GameSetup, GameState, apply_event, replay};
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
    /// A life total or damage counter disagrees with the damage dealt.
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
}

fn zone_error(message: impl Into<String>) -> Violation {
    Violation::Zones(message.into())
}

/// Stateless checks on a single state; callable at any decision point.
pub fn check_state(state: &GameState) -> Result<(), Violation> {
    check_zone_index(state)?;
    check_bookkeeping(state)
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

fn check_bookkeeping(state: &GameState) -> Result<(), Violation> {
    let book = |message: String| Violation::Bookkeeping(message);
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
    for attacker in state.combat().attackers() {
        let on_battlefield = |id: ObjectId| {
            state
                .object(id)
                .is_some_and(|o| o.zone() == Zone::Battlefield)
        };
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
            AttackTarget::Planeswalker(o) if !on_battlefield(o) => {
                return Err(book(format!("attacking missing planeswalker {o:?}")));
            }
            _ => {}
        }
    }
    if state.stage() != Stage::Finished
        && let Some(out) = state.objects().find(|o| !state.is_alive(o.owner()))
    {
        return Err(book(format!(
            "{:?} still exists after its owner left",
            out.id()
        )));
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
    expected_damage: BTreeMap<ObjectId, u32>,
    events_seen: usize,
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
        Ok(InvariantChecker {
            setup: setup.clone(),
            expected_objects: shadow.object_count(),
            expected_life: shadow.players().iter().map(Player::life).collect(),
            expected_pool: vec![[0; 6]; shadow.players().len()],
            expected_damage,
            shadow,
            events_seen: 0,
        })
    }

    /// Consumes every event of `log` not yet seen, then runs the full checks against `live`.
    pub fn observe(&mut self, live: &GameState, log: &[Event]) -> Result<(), Violation> {
        for event in log.get(self.events_seen..).unwrap_or_default() {
            self.observe_event(event)?;
            self.events_seen += 1;
        }
        check_state(live)?;
        let (live_hash, shadow_hash) = (live.state_hash(), self.shadow.state_hash());
        if live_hash != shadow_hash {
            return Err(Violation::Replay {
                live: live_hash,
                replayed: shadow_hash,
            });
        }
        self.check_damage_totals()
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
        apply_event(&mut self.shadow, event)
            .map_err(|e| Violation::Fold(format!("{event:?}: {e}")))?;
        self.account(event)?;
        let touched = event.touched_objects();
        if matches!(event, Event::ObjectLeftGame { .. }) {
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
            None if matches!(event, Event::ObjectLeftGame { .. }) => Ok(()),
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
            Event::ManaAdded { player, mana, .. } => {
                self.adjust_pool(*player, *mana, 1)?;
                self.check_pool(*player)
            }
            Event::ManaSpent { player, mana } => {
                for m in mana {
                    self.adjust_pool(*player, *m, -1)?;
                }
                self.check_pool(*player)
            }
            Event::ManaPoolEmptied { player } => {
                if let Some(cells) = self.expected_pool.get_mut(player.index()) {
                    *cells = [0; 6];
                }
                self.check_pool(*player)
            }
            Event::DamageDealt { target, amount, .. } => {
                match target {
                    DamageTarget::Player(p) => {
                        let life = self
                            .expected_life
                            .get_mut(p.index())
                            .ok_or_else(|| Violation::Damage(format!("unknown {p:?}")))?;
                        *life -= i32::try_from(*amount).unwrap_or(i32::MAX);
                        let actual = self.shadow.player(*p).map(Player::life);
                        if actual != Some(*life) {
                            return Err(Violation::Damage(format!(
                                "{p:?} life is {actual:?}, damage history says {life}"
                            )));
                        }
                    }
                    DamageTarget::Object(o) => {
                        *self.expected_damage.entry(*o).or_default() += amount;
                    }
                }
                Ok(())
            }
            Event::DamageCleared => {
                self.expected_damage.clear();
                Ok(())
            }
            _ => Ok(()),
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

    fn check_pool(&self, player: PlayerId) -> Result<(), Violation> {
        let expected = self.expected_pool.get(player.index());
        let actual = self.shadow.player(player).map(Player::mana);
        match (expected, actual) {
            (Some(expected), Some(pool)) => {
                for mana in ManaType::ALL {
                    let want = expected[mana.index()];
                    if want < 0 || i64::from(pool.get(mana)) != want {
                        return Err(Violation::Mana(format!(
                            "{player:?} {mana:?}: pool has {}, history says {want}",
                            pool.get(mana)
                        )));
                    }
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
