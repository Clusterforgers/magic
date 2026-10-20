//! The rules of the game that spells, abilities and permanents change: attack and block requirements, spells that can't
//! be countered, players who can't gain life, draw or search, library tops that are revealed, and triggers that trigger
//! an additional time (`docs/engine.md`, Rules of the game).
//!
//! The rules in force are gathered like the layers: from the `Effect::Rule`s that resolved and still last, and from the
//! `Ability::Rule`s that permanents have (read through `ability_reader`, so a permanent that lost its abilities lost
//! its rules) and that spells on the stack have for themselves. The result is kept with the state until an event that
//! can change it (`GameState::rules`), and each rule is read by one function that the one place deciding it asks.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::sync::OnceLock;

use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::ability_reader::{abilities_of, definition_has_rule};
use crate::characteristics::controller_of_object;
use crate::effects::{Duration, lasts};
use crate::object::{GameObject, ObjectKind, ObjectRef};
use crate::resolve::{ResolveError, Scope};
use crate::state::GameState;
use crate::zones::Zone;

/// A rule of the game as the engine plays it: the players or objects the data's references named, found when the rule
/// began (an effect) or whenever the state changes (a static ability).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Rule {
    /// The creatures attack each combat if able (CR 508.1d).
    MustAttack(Vec<ObjectRef>),
    /// The creatures block each combat if able (CR 509.1c).
    MustBlock(Vec<ObjectRef>),
    /// The spells can't be countered.
    CantBeCountered(Vec<ObjectRef>),
    /// The players can't gain life.
    CantGainLife(Vec<PlayerId>),
    /// The players can't draw cards.
    CantDraw(Vec<PlayerId>),
    /// The players can't search libraries.
    CantSearch(Vec<PlayerId>),
    /// The players play with the top card of their library revealed.
    RevealTop(Vec<PlayerId>),
    /// A triggered ability of a permanent the rule's controller controls triggers an additional time when the event
    /// the pattern describes causes it.
    TriggerAgain(#[cfg_attr(feature = "ts", ts(type = "unknown"))] Box<TriggerPattern>),
}

/// A rule of the game that a resolved spell or ability changed for a while (CR 611).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RuleEffect {
    /// Unique id, from the counter that continuous effects share.
    pub id: EffectId,
    /// The rule.
    pub rule: Rule,
    /// How long it lasts.
    #[cfg_attr(
        feature = "ts",
        ts(
            type = "\"UntilEndOfTurn\" | \"UntilEndOfCombat\" | \"UntilYourNextTurn\" | \"WhileSourceOnBattlefield\" | \"Forever\""
        )
    )]
    pub duration: Duration,
    /// The object whose spell or ability made it, as it was then; `WhileSourceOnBattlefield` follows this object.
    pub source: Option<ObjectRef>,
    /// Who controlled that spell or ability: whose next turn `UntilYourNextTurn` waits for, and whose permanents a rule
    /// that triggers again is about.
    pub controller: PlayerId,
    /// The number of the turn it began in.
    pub turn: u32,
}

impl RuleEffect {
    /// Whether the rule still applies in `state`: its duration has not ended and an object it names is still the object
    /// it named. A rule that does not stops applying at once; the `RuleExpired` event only removes it.
    pub fn is_active(&self, state: &GameState) -> bool {
        lasts(
            state,
            self.duration,
            self.turn,
            self.controller,
            self.source,
        ) && !self.spent(state)
    }

    /// Whether every object the rule names has become a new object or is gone: it can never apply to anything again.
    fn spent(&self, state: &GameState) -> bool {
        let named = match &self.rule {
            Rule::MustAttack(named) | Rule::MustBlock(named) | Rule::CantBeCountered(named) => {
                named
            }
            _ => return false,
        };
        named
            .iter()
            .all(|r| state.object(r.id).is_none_or(|o| o.counter() != r.counter))
    }
}

/// A trigger that triggers an additional time: whose permanents, and the event that causes it.
#[derive(Debug, Clone)]
pub(crate) struct Again {
    /// The player whose permanents' abilities trigger again.
    pub(crate) controller: PlayerId,
    /// The object that has the rule, for the pattern's `This`.
    pub(crate) source: ObjectRef,
    pub(crate) cause: TriggerPattern,
}

/// The rules in force in a state.
#[derive(Debug, Default)]
pub(crate) struct RulesInForce {
    must_attack: BTreeSet<ObjectId>,
    must_block: BTreeSet<ObjectId>,
    uncounterable: BTreeSet<ObjectId>,
    no_life_gain: BTreeSet<PlayerId>,
    no_draw: BTreeSet<PlayerId>,
    no_search: BTreeSet<PlayerId>,
    reveal_top: BTreeSet<PlayerId>,
    again: Vec<Again>,
}

impl RulesInForce {
    /// Puts a rule in force. `controller` and `source` are those of the effect or the permanent that has the rule.
    fn put(&mut self, rule: &Rule, controller: PlayerId, source: Option<ObjectRef>) {
        let ids = |named: &[ObjectRef]| named.iter().map(|r| r.id).collect::<Vec<_>>();
        match rule {
            Rule::MustAttack(named) => self.must_attack.extend(ids(named)),
            Rule::MustBlock(named) => self.must_block.extend(ids(named)),
            Rule::CantBeCountered(named) => self.uncounterable.extend(ids(named)),
            Rule::CantGainLife(players) => self.no_life_gain.extend(players),
            Rule::CantDraw(players) => self.no_draw.extend(players),
            Rule::CantSearch(players) => self.no_search.extend(players),
            Rule::RevealTop(players) => self.reveal_top.extend(players),
            Rule::TriggerAgain(cause) => {
                if let Some(source) = source {
                    self.again.push(Again {
                        controller,
                        source,
                        cause: (**cause).clone(),
                    });
                }
            }
        }
    }
}

/// What the data's references name in `scope`, as the engine plays the rule: the permanents, the spells or the players.
/// A rule the engine does not play refuses by name.
pub(crate) fn lower_rule(scope: &Scope<'_>, wanted: &GameRule) -> Result<Rule, ResolveError> {
    let objects = |who: &Ref, zone: Zone| -> Result<Vec<ObjectRef>, ResolveError> {
        let named = if matches!(who, Ref::Attached) {
            scope
                .state
                .host_of(scope.source.id)
                .and_then(|host| scope.state.object(host))
                .map(GameObject::reference)
                .into_iter()
                .collect()
        } else {
            scope.objects(who)?
        };
        // An object that has changed zones since it was named is a new object (CR 400.7).
        Ok(named
            .into_iter()
            .filter(|named| {
                scope
                    .state
                    .object(named.id)
                    .is_some_and(|o| o.counter() == named.counter && o.zone() == zone)
            })
            .collect())
    };
    Ok(match wanted {
        GameRule::DamageCantBePrevented => {
            return Err(ResolveError::Unsupported("Rule.DamageCantBePrevented"));
        }
        GameRule::MustAttack(who) => Rule::MustAttack(objects(who, Zone::Battlefield)?),
        GameRule::MustBlock(who) => Rule::MustBlock(objects(who, Zone::Battlefield)?),
        GameRule::CantBeCountered(who) => Rule::CantBeCountered(objects(who, Zone::Stack)?),
        GameRule::CantGainLife(who) => Rule::CantGainLife(scope.players(who)?),
        GameRule::CantDraw(who) => Rule::CantDraw(scope.players(who)?),
        GameRule::CantSearch(who) => Rule::CantSearch(scope.players(who)?),
        GameRule::RevealTop(who) => Rule::RevealTop(scope.players(who)?),
        GameRule::TriggerAgain { cause } => Rule::TriggerAgain(Box::new(cause.clone())),
    })
}

/// Whether a rule names no player and no object, so putting it in force changes nothing.
pub(crate) fn names_nothing(rule: &Rule) -> bool {
    match rule {
        Rule::MustAttack(named) | Rule::MustBlock(named) | Rule::CantBeCountered(named) => {
            named.is_empty()
        }
        Rule::CantGainLife(players)
        | Rule::CantDraw(players)
        | Rule::CantSearch(players)
        | Rule::RevealTop(players) => players.is_empty(),
        Rule::TriggerAgain(_) => false,
    }
}

thread_local! {
    /// Whether the rules of a state are being gathered on this thread.
    static GATHERING: Cell<bool> = const { Cell::new(false) };
}

/// Ends the gathering of this thread, whether it returned or unwound.
struct Gathering;

impl Gathering {
    fn enter() -> Gathering {
        GATHERING.with(|gathering| gathering.set(true));
        Gathering
    }
}

impl Drop for Gathering {
    fn drop(&mut self) {
        GATHERING.with(|gathering| gathering.set(false));
    }
}

/// Gathers the rules in force in `state`.
fn gather(state: &GameState) -> RulesInForce {
    let mut found = RulesInForce::default();
    for effect in state
        .rule_effects()
        .iter()
        .filter(|effect| effect.is_active(state))
    {
        found.put(&effect.rule, effect.controller, effect.source);
    }
    if !state.rules_possible {
        return found;
    }
    // A rule that a permanent has works from the battlefield, and one a spell has for itself from the stack.
    for (zone, from) in [
        (Zone::Battlefield, ZoneKind::Battlefield),
        (Zone::Stack, ZoneKind::Stack),
    ] {
        for &id in state.zone(zone) {
            let Some(object) = state.object(id).filter(|o| o.kind() != ObjectKind::Ability) else {
                continue;
            };
            // A rule can only be printed, so a card that prints none has none whatever the layers did.
            if !definition_has_rule(&object.def) {
                continue;
            }
            let controller = controller_of_object(state, object);
            if !state.is_alive(controller) {
                continue;
            }
            let scope = Scope::announcing(state, controller, object.reference());
            for (_, ability) in abilities_of(state, id) {
                let Ability::Rule {
                    rule,
                    condition,
                    from: works_from,
                } = &*ability
                else {
                    continue;
                };
                if *works_from != from
                    || condition
                        .as_ref()
                        .is_some_and(|condition| !scope.holds(condition).unwrap_or(false))
                {
                    continue;
                }
                if let Ok(lowered) = lower_rule(&scope, rule) {
                    found.put(&lowered, controller, Some(object.reference()));
                }
            }
        }
    }
    found
}

/// The rules of a game in which none can be in force, and of one whose rules are being gathered (the gathering reads
/// objects, never the rules).
static NONE: RulesInForce = RulesInForce {
    must_attack: BTreeSet::new(),
    must_block: BTreeSet::new(),
    uncounterable: BTreeSet::new(),
    no_life_gain: BTreeSet::new(),
    no_draw: BTreeSet::new(),
    no_search: BTreeSet::new(),
    reveal_top: BTreeSet::new(),
    again: Vec::new(),
};

/// The rules of a state, built when first asked for and dropped when the state changes.
#[derive(Default)]
pub(crate) struct RulesCache(OnceLock<RulesInForce>);

impl RulesCache {
    /// Forgets the result: the state it was built for has changed.
    pub(crate) fn reset(&mut self) {
        self.0 = OnceLock::new();
    }

    /// Forgets the result if there is one.
    pub(crate) fn reset_if_built(&mut self) {
        if self.0.get().is_some() {
            self.reset();
        }
    }
}

// A clone is a state of its own that gathers its own rules when it first needs them.
impl Clone for RulesCache {
    fn clone(&self) -> Self {
        RulesCache::default()
    }
}

impl std::fmt::Debug for RulesCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RulesCache")
    }
}

impl GameState {
    /// The rules in force now: none while no rule can be (a game of cards that change none costs nothing).
    pub(crate) fn rules(&self) -> &RulesInForce {
        if (self.rule_effects.is_empty() && !self.rules_possible) || GATHERING.with(Cell::get) {
            return &NONE;
        }
        self.rules_cache.0.get_or_init(|| {
            let _gathering = Gathering::enter();
            gather(self)
        })
    }

    /// The top card of `player`'s library while a rule has it revealed to every seat.
    pub fn revealed_top(&self, player: PlayerId) -> Option<ObjectId> {
        if !self.rules().reveal_top.contains(&player) {
            return None;
        }
        self.zone(Zone::Library(player)).last().copied()
    }
}

/// Whether the creature `id` attacks each combat if able: the one place the attack declaration reads this requirement.
pub(crate) fn must_attack(state: &GameState, id: ObjectId) -> bool {
    state.rules().must_attack.contains(&id)
}

/// Whether the creature `id` blocks each combat if able: the one place the block declaration reads this requirement.
pub(crate) fn must_block(state: &GameState, id: ObjectId) -> bool {
    state.rules().must_block.contains(&id)
}

/// Whether the spell `id` can't be countered: the one place a counter reads this rule.
pub(crate) fn cant_be_countered(state: &GameState, id: ObjectId) -> bool {
    state.rules().uncounterable.contains(&id)
}

/// Whether `player` can't gain life: the one place a gain of life reads this rule.
pub(crate) fn cant_gain_life(state: &GameState, player: PlayerId) -> bool {
    state.rules().no_life_gain.contains(&player)
}

/// Whether `player` can't draw cards: the one place a draw reads this rule.
pub(crate) fn cant_draw(state: &GameState, player: PlayerId) -> bool {
    state.rules().no_draw.contains(&player)
}

/// Whether `player` can't search libraries: the one place a search reads this rule.
pub(crate) fn cant_search(state: &GameState, player: PlayerId) -> bool {
    state.rules().no_search.contains(&player)
}

/// The triggers that trigger an additional time: the one place trigger detection reads this rule.
pub(crate) fn triggers_again(state: &GameState) -> &[Again] {
    &state.rules().again
}
