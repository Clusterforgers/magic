//! The rules of the game that spells, abilities and permanents change: attack and block requirements, spells that can't
//! be countered, players who can't gain life, draw or search, library tops that are revealed, and triggers that trigger
//! an additional time (`docs/engine.md`, Rules of the game).
//!
//! The rules in force are gathered like the layers: from the `Effect::Rule`s that resolved and still last, and from the
//! `Ability::Rule`s that permanents have (read through `ability_reader`, so a permanent that lost its abilities lost
//! its rules) and that spells on the stack have for themselves. The result is kept with the state until an event that
//! can change it (`GameState::rules`), and each rule is read by one function that the one place deciding it asks.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Attacked, GameRule};
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use serde::{Deserialize, Serialize};

use crate::ability_reader::{abilities_of, definition_has_rule};
use crate::characteristics::controller_of_object;
use crate::choose::Target;
use crate::effects::{Duration, lasts};
use crate::error::EngineError;
use crate::fold::invalid;
use crate::object::{GameObject, ObjectKind, ObjectRef};
use crate::resolve::{ResolveError, Scope};
use crate::state::{GameState, Stage, Tally};
use crate::zones::Zone;

/// A rule of the game as the engine plays it: the players or objects the data's references named, found when the rule
/// began (an effect) or whenever the state changes (a static ability).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Rule {
    /// The creatures attack each combat if able (CR 508.1d).
    MustAttack(Vec<ObjectRef>),
    /// Each creature attacks the player paired with it each combat of its duration if able (Encore, CR 702.141).
    MustAttackPlayers(Vec<(ObjectRef, PlayerId)>),
    /// The creatures block each combat if able (CR 509.1c).
    MustBlock(Vec<ObjectRef>),
    /// Creatures can't attack the players unless their controller pays for each of them as the attack is declared
    /// (CR 508.1h).
    AttackTax(AttackTax),
    /// The spells can't be countered.
    CantBeCountered(Vec<ObjectRef>),
    /// The players can't gain life.
    CantGainLife(Vec<PlayerId>),
    /// Damage dealt to the players causes no loss of life.
    NoLifeLossFromDamage(Vec<PlayerId>),
    /// The players gain life rather than lose life from radiation.
    RadiationGainsLife(Vec<PlayerId>),
    /// The players' life totals can't change (CR 119.7, 119.8).
    LifeTotalCantChange(Vec<PlayerId>),
    /// The players have the keyword, such as protection from everything.
    PlayersHave {
        /// The players.
        who: Vec<PlayerId>,
        /// The keyword they have.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        keyword: Keyword,
    },
    /// The players can't draw cards.
    CantDraw(Vec<PlayerId>),
    /// The players can't search libraries.
    CantSearch(Vec<PlayerId>),
    /// The players skip each extra turn they would begin (CR 500.7).
    SkipExtraTurns(Vec<PlayerId>),
    /// The players play with the top card of their library revealed.
    RevealTop(Vec<PlayerId>),
    /// The players have no maximum hand size (CR 402.2).
    NoMaxHandSize(Vec<PlayerId>),
    /// The players skip their draw steps (CR 614.10).
    SkipDrawStep(Vec<PlayerId>),
    /// The players may look at the top card of their library any time.
    LookAtTop(Vec<PlayerId>),
    /// Each player may play this many lands more on each of their turns (CR 305.2).
    AdditionalLandPlays(Vec<(PlayerId, u32)>),
    /// The players may cast the cards the selector picks from a zone, without paying their mana costs or as though they
    /// had flash (CR 601.3).
    MayCast {
        /// The players who may cast.
        who: Vec<PlayerId>,
        /// The cards they may cast, judged when they are cast; every card when empty.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        what: Selector,
        /// The zone they are cast from; wherever they are when absent.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        from: Option<ZoneKind>,
        /// They are cast without paying their mana costs.
        free: bool,
        /// They are cast as though they had flash.
        flash: bool,
        /// Only these objects, as they were named when the rule began; every card `what` selects when absent.
        only: Option<Vec<ObjectRef>>,
        /// A land among the cards may be played too.
        play: bool,
        /// Only once during each of the rule controller's turns, and only on their turns.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(optional, as = "Option<bool>"))]
        once: bool,
    },
    /// A triggered ability of a permanent the rule's controller controls triggers an additional time when the event
    /// the pattern describes causes it.
    TriggerAgain(#[cfg_attr(feature = "ts", ts(type = "unknown"))] Box<TriggerPattern>),
    /// Damage from the sources can't be prevented (CR 614.17); every source when the selector is empty.
    DamageCantBePrevented {
        /// The sources, judged when the damage would be dealt.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        source: Selector,
        /// Only combat damage (`true`) or only other damage (`false`); both when absent.
        combat: Option<bool>,
    },
    /// A prevention shield (CR 615): it replaces damage with nothing, up to an amount, until it is used up or ends.
    PreventDamage(DamageShield),
    /// A regeneration shield (CR 701.19): the next time the permanent would be destroyed, it regenerates instead.
    Regenerate(ObjectRef),
    /// A shield over deaths (CR 614): each of the permanents or spells, fixed when it was made, that would go from the
    /// battlefield or the stack to a graveyard goes to the zone instead ("if that creature would die this turn, exile it
    /// instead", "if that spell would be put into your graveyard, exile it instead").
    DiesInstead {
        /// The permanents and spells it covers.
        objects: Vec<ObjectRef>,
        /// Where they go instead.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        to: ZoneKind,
    },
}

/// A tax on attacking: whom it protects, which attackers it taxes and what each costs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AttackTax {
    /// The players it protects.
    pub players: Vec<PlayerId>,
    /// Whether it protects the players, the planeswalkers they control or both.
    #[cfg_attr(feature = "ts", ts(type = "\"Player\" | \"Planeswalkers\" | \"Both\""))]
    pub attacked: Attacked,
    /// The attackers it taxes, judged when the attack is declared; every creature when empty.
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub creatures: Selector,
    /// What each taxed creature costs, counted when the rule began or the state last changed.
    pub mana: ManaCost,
    /// The life each taxed creature costs, counted the same way.
    #[serde(default)]
    pub life: u32,
}

/// What a prevention shield protects and how much it prevents.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DamageShield {
    /// The players and objects it protects, fixed when it was made; anything when absent.
    pub to: Option<Vec<Target>>,
    /// The sources of the damage it prevents, judged when the damage would be dealt; any source when empty.
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub source: Selector,
    /// The sources it prevents damage from, fixed when it was made (a target spell or object); any when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub from: Option<Vec<Target>>,
    /// Only combat damage (`true`) or only other damage (`false`); both when absent.
    pub combat: Option<bool>,
    /// What it still prevents; all damage when absent.
    pub left: Option<u32>,
}

impl DamageShield {
    /// Whether it can never prevent anything again: it is used up, or every object it protects, or every source it
    /// names, is a new object or gone.
    fn spent(&self, state: &GameState) -> bool {
        let gone = |named: &Target| match named {
            Target::Player(_) => false,
            Target::Object(named) => state
                .object(named.id)
                .is_none_or(|o| o.counter() != named.counter),
        };
        self.from.as_ref().is_some_and(|from| from.iter().all(gone))
            || self.left == Some(0)
            || self.to.as_ref().is_some_and(|to| {
                !to.is_empty()
                    && to.iter().all(|target| match target {
                        Target::Player(_) => false,
                        Target::Object(named) => state
                            .object(named.id)
                            .is_none_or(|o| o.counter() != named.counter),
                    })
            })
    }
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
            type = "\"UntilEndOfTurn\" | \"UntilEndOfCombat\" | \"UntilYourNextTurn\" | \"UntilEndOfYourNextTurn\" | \"WhileSourceOnBattlefield\" | \"Forever\""
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
        let gone = |r: &ObjectRef| state.object(r.id).is_none_or(|o| o.counter() != r.counter);
        match &self.rule {
            Rule::MustAttack(named)
            | Rule::MustBlock(named)
            | Rule::CantBeCountered(named)
            | Rule::DiesInstead { objects: named, .. }
            | Rule::MayCast {
                only: Some(named), ..
            } => named.iter().all(gone),
            Rule::MustAttackPlayers(pairs) => pairs.iter().all(|(named, _)| gone(named)),
            Rule::Regenerate(named) => gone(named),
            Rule::PreventDamage(shield) => shield.spent(state),
            _ => false,
        }
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
    attack_players: Vec<(ObjectId, PlayerId)>,
    must_block: BTreeSet<ObjectId>,
    uncounterable: BTreeSet<ObjectId>,
    no_life_gain: BTreeSet<PlayerId>,
    no_damage_loss: BTreeSet<PlayerId>,
    radiation_gains: BTreeSet<PlayerId>,
    no_life_change: BTreeSet<PlayerId>,
    player_keywords: Vec<(PlayerId, Keyword)>,
    no_draw: BTreeSet<PlayerId>,
    no_search: BTreeSet<PlayerId>,
    skip_extra_turns: BTreeSet<PlayerId>,
    reveal_top: BTreeSet<PlayerId>,
    no_hand_limit: BTreeSet<PlayerId>,
    skip_draw: BTreeSet<PlayerId>,
    look_top: BTreeSet<PlayerId>,
    extra_lands: BTreeMap<PlayerId, u32>,
    again: Vec<Again>,
    unpreventable: Vec<Unpreventable>,
    casts: Vec<Cast>,
    taxes: Vec<Taxing>,
}

/// A tax on attacking in force, with whose it is.
#[derive(Debug, Clone)]
pub(crate) struct Taxing {
    /// The controller of the effect or permanent that taxes, for the selector's "you".
    pub(crate) controller: PlayerId,
    /// The object that taxes, for the selector's "this"; none for a rule that no object began.
    pub(crate) source: Option<ObjectRef>,
    /// The tax.
    pub(crate) tax: AttackTax,
}

/// A permission to cast cards, with whose it is.
#[derive(Debug, Clone)]
// The flags mirror those of `CastPermission`.
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Cast {
    /// The controller of the effect or permanent that gives it, for the selector's "you".
    pub(crate) controller: PlayerId,
    /// The object that gives it.
    pub(crate) source: ObjectRef,
    /// The players who may cast.
    pub(crate) who: Vec<PlayerId>,
    /// The cards they may cast; every card when empty.
    pub(crate) what: Selector,
    /// The zone they are cast from; wherever they are when absent.
    pub(crate) from: Option<ZoneKind>,
    /// Cast without paying their mana costs.
    pub(crate) free: bool,
    /// Cast as though they had flash.
    pub(crate) flash: bool,
    /// Only these objects; every card `what` selects when absent.
    pub(crate) only: Option<Vec<ObjectRef>>,
    /// A land among the cards may be played too.
    pub(crate) play: bool,
    /// Only once during each of the controller's turns, and only on their turns.
    pub(crate) once: bool,
}

impl Cast {
    /// Whether the permission lets `player` cast the card `card`, which is in its zone now.
    pub(crate) fn covers(&self, state: &GameState, player: PlayerId, card: &GameObject) -> bool {
        self.c11b_open(state)
            && self.who.contains(&player)
            && self
                .only
                .as_ref()
                .is_none_or(|only| only.contains(&card.reference()))
            && self
                .from
                .is_none_or(|zone| crate::resolve::zone_kind(card.zone()) == zone)
            && (self.what.is_empty()
                || Scope::announcing(state, self.controller, self.source)
                    .selects(&self.what, card.reference())
                    .unwrap_or(false))
    }

    /// Whether a permission that works once each turn is open: it is its controller's turn and no spell was cast through
    /// it this turn (`Tally::C11bOnceUsed` of its source). Any other permission is always open.
    fn c11b_open(&self, state: &GameState) -> bool {
        !self.once
            || (state.turn().active() == self.controller
                && state.object_tallied(self.source, None, Tally::C11bOnceUsed) == 0)
    }

    /// Whether the permission lets `player` play the land `card` from the zone it is in.
    pub(crate) fn plays(&self, state: &GameState, player: PlayerId, card: &GameObject) -> bool {
        self.play && self.covers(state, player, card)
    }
}

/// Damage that can't be prevented: whose rule it is, and which damage it covers.
#[derive(Debug, Clone)]
struct Unpreventable {
    controller: PlayerId,
    source: ObjectRef,
    from: Selector,
    combat: Option<bool>,
}

impl RulesInForce {
    /// Puts a rule in force. `controller` and `source` are those of the effect or the permanent that has the rule.
    fn put(&mut self, rule: &Rule, controller: PlayerId, source: Option<ObjectRef>) {
        let ids = |named: &[ObjectRef]| named.iter().map(|r| r.id).collect::<Vec<_>>();
        match rule {
            Rule::MustAttack(named) => self.must_attack.extend(ids(named)),
            Rule::MustAttackPlayers(pairs) => self
                .attack_players
                .extend(pairs.iter().map(|(named, player)| (named.id, *player))),
            Rule::MustBlock(named) => self.must_block.extend(ids(named)),
            Rule::AttackTax(tax) => self.taxes.push(Taxing {
                controller,
                source,
                tax: tax.clone(),
            }),
            Rule::CantBeCountered(named) => self.uncounterable.extend(ids(named)),
            Rule::CantGainLife(players) => self.no_life_gain.extend(players),
            Rule::NoLifeLossFromDamage(players) => self.no_damage_loss.extend(players),
            Rule::RadiationGainsLife(players) => self.radiation_gains.extend(players),
            Rule::LifeTotalCantChange(players) => self.no_life_change.extend(players),
            Rule::PlayersHave { who, keyword } => self
                .player_keywords
                .extend(who.iter().map(|player| (*player, keyword.clone()))),
            Rule::CantDraw(players) => self.no_draw.extend(players),
            Rule::CantSearch(players) => self.no_search.extend(players),
            Rule::SkipExtraTurns(players) => self.skip_extra_turns.extend(players),
            Rule::RevealTop(players) => self.reveal_top.extend(players),
            Rule::NoMaxHandSize(players) => self.no_hand_limit.extend(players),
            Rule::SkipDrawStep(players) => self.skip_draw.extend(players),
            Rule::LookAtTop(players) => self.look_top.extend(players),
            Rule::AdditionalLandPlays(plays) => {
                for (player, count) in plays {
                    let total = self.extra_lands.entry(*player).or_default();
                    *total = total.saturating_add(*count);
                }
            }
            Rule::TriggerAgain(cause) => {
                if let Some(source) = source {
                    self.again.push(Again {
                        controller,
                        source,
                        cause: (**cause).clone(),
                    });
                }
            }
            Rule::DamageCantBePrevented {
                source: from,
                combat,
            } => {
                if let Some(source) = source {
                    self.unpreventable.push(Unpreventable {
                        controller,
                        source,
                        from: from.clone(),
                        combat: *combat,
                    });
                }
            }
            Rule::MayCast {
                who,
                what,
                from,
                free,
                flash,
                only,
                play,
                once,
            } => {
                if let Some(source) = source {
                    self.casts.push(Cast {
                        controller,
                        source,
                        who: who.clone(),
                        what: what.clone(),
                        from: *from,
                        free: *free,
                        flash: *flash,
                        only: only.clone(),
                        play: *play,
                        once: *once,
                    });
                }
            }
            // Shields are read by the replacement step from the rule changes themselves.
            Rule::PreventDamage(_) | Rule::Regenerate(_) | Rule::DiesInstead { .. } => {}
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
        GameRule::DamageCantBePrevented { source, combat } => Rule::DamageCantBePrevented {
            source: source.clone(),
            combat: *combat,
        },
        GameRule::MustAttack(who) => Rule::MustAttack(objects(who, Zone::Battlefield)?),
        GameRule::MustAttackPlayers { who, players } => Rule::MustAttackPlayers(
            objects(who, Zone::Battlefield)?
                .into_iter()
                .zip(scope.players(players)?)
                .collect(),
        ),
        GameRule::MustBlock(who) => Rule::MustBlock(objects(who, Zone::Battlefield)?),
        GameRule::AttackTax {
            who,
            attacked,
            attackers,
            mana,
            life,
            per,
        } => {
            let times = usize::try_from(scope.amount(per)?.max(0)).unwrap_or(usize::MAX);
            Rule::AttackTax(AttackTax {
                players: scope.players(who)?,
                attacked: *attacked,
                creatures: attackers.clone(),
                mana: repeated(mana.rest(), times),
                life: life.saturating_mul(u32::try_from(times).unwrap_or(u32::MAX)),
            })
        }
        GameRule::CantBeCountered(who) => Rule::CantBeCountered(objects(who, Zone::Stack)?),
        GameRule::CantGainLife(who) => Rule::CantGainLife(scope.players(who)?),
        GameRule::NoLifeLossFromDamage(who) => Rule::NoLifeLossFromDamage(scope.players(who)?),
        GameRule::RadiationGainsLife(who) => Rule::RadiationGainsLife(scope.players(who)?),
        GameRule::LifeTotalCantChange(who) => Rule::LifeTotalCantChange(scope.players(who)?),
        GameRule::PlayersHave { who, keyword } => Rule::PlayersHave {
            who: scope.players(who)?,
            keyword: keyword.clone(),
        },
        GameRule::CantDraw(who) => Rule::CantDraw(scope.players(who)?),
        GameRule::CantSearch(who) => Rule::CantSearch(scope.players(who)?),
        GameRule::SkipExtraTurns(who) => Rule::SkipExtraTurns(scope.players(who)?),
        GameRule::RevealTop(who) => Rule::RevealTop(scope.players(who)?),
        GameRule::MayCast(permission) => Rule::MayCast {
            who: scope.players(&permission.who)?,
            what: permission.what.clone(),
            from: permission.from,
            free: permission.free,
            flash: permission.flash,
            only: permission
                .only
                .as_ref()
                .map(|named| scope.objects(named))
                .transpose()?,
            play: permission.play,
            once: permission.once,
        },
        GameRule::TriggerAgain { cause } => Rule::TriggerAgain(Box::new(cause.clone())),
        GameRule::LookAtTopAnyTime(who) => Rule::LookAtTop(scope.players(who)?),
        GameRule::NoMaxHandSize(who) => Rule::NoMaxHandSize(scope.players(who)?),
        GameRule::SkipDrawStep(who) => Rule::SkipDrawStep(scope.players(who)?),
        GameRule::AdditionalLandPlays { who, count } => {
            let count = u32::try_from(scope.amount(count)?.max(0)).unwrap_or(u32::MAX);
            Rule::AdditionalLandPlays(
                scope
                    .players(who)?
                    .into_iter()
                    .map(|player| (player, count))
                    .collect(),
            )
        }
        // Eligibility is read from the definition when a seat is checked (`CardDefinition::can_be_commander`); no
        // rule is ever in force and an effect cannot start one.
        GameRule::CanBeCommander => return Err(ResolveError::Unsupported("Rule.CanBeCommander")),
    })
}

/// The cost paid `times` over: its symbols repeated, the generic ones added up.
pub(crate) fn repeated(cost: &ManaCost, times: usize) -> ManaCost {
    ManaCost::from_symbols(
        cost.symbols()
            .iter()
            .copied()
            .cycle()
            .take(cost.symbols().len().saturating_mul(times)),
    )
}

/// Whether a rule names no player and no object, so putting it in force changes nothing.
pub(crate) fn names_nothing(rule: &Rule) -> bool {
    match rule {
        Rule::AttackTax(tax) => {
            tax.players.is_empty() || (tax.mana.symbols().is_empty() && tax.life == 0)
        }
        Rule::MustAttack(named) | Rule::MustBlock(named) | Rule::CantBeCountered(named) => {
            named.is_empty()
        }
        Rule::MustAttackPlayers(pairs) => pairs.is_empty(),
        Rule::CantGainLife(players)
        | Rule::NoLifeLossFromDamage(players)
        | Rule::RadiationGainsLife(players)
        | Rule::LifeTotalCantChange(players)
        | Rule::CantDraw(players)
        | Rule::CantSearch(players)
        | Rule::SkipExtraTurns(players)
        | Rule::RevealTop(players)
        | Rule::LookAtTop(players)
        | Rule::SkipDrawStep(players)
        | Rule::NoMaxHandSize(players) => players.is_empty(),
        Rule::AdditionalLandPlays(plays) => plays.iter().all(|(_, count)| *count == 0),
        Rule::MayCast { who, only, .. } => {
            who.is_empty() || only.as_ref().is_some_and(Vec::is_empty)
        }
        Rule::PlayersHave { who, .. } => who.is_empty(),
        Rule::TriggerAgain(_) | Rule::DamageCantBePrevented { .. } | Rule::Regenerate(_) => false,
        Rule::PreventDamage(shield) => {
            shield.to.as_ref().is_some_and(Vec::is_empty)
                || shield.from.as_ref().is_some_and(Vec::is_empty)
        }
        Rule::DiesInstead { objects, .. } => objects.is_empty(),
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
    // A rule that a permanent has works from the battlefield, one a spell has for itself from the stack, and a card's
    // permission to cast itself from its owner's graveyard (CR 601.2a, 113.6) from there.
    let graveyards = state
        .players()
        .iter()
        .map(|player| (Zone::Graveyard(player.id()), ZoneKind::Graveyard));
    for (zone, from) in [
        (Zone::Battlefield, ZoneKind::Battlefield),
        (Zone::Stack, ZoneKind::Stack),
    ]
    .into_iter()
    .chain(graveyards)
    {
        for &id in state.zone(zone) {
            let Some(object) = state.object(id).filter(|o| o.kind() != ObjectKind::Ability) else {
                continue;
            };
            // A rule can only be printed, so a card that prints none has none whatever the layers did.
            if !definition_has_rule(object.card()) {
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
    attack_players: Vec::new(),
    must_block: BTreeSet::new(),
    uncounterable: BTreeSet::new(),
    no_life_gain: BTreeSet::new(),
    no_damage_loss: BTreeSet::new(),
    radiation_gains: BTreeSet::new(),
    no_life_change: BTreeSet::new(),
    player_keywords: Vec::new(),
    no_draw: BTreeSet::new(),
    no_search: BTreeSet::new(),
    skip_extra_turns: BTreeSet::new(),
    reveal_top: BTreeSet::new(),
    no_hand_limit: BTreeSet::new(),
    skip_draw: BTreeSet::new(),
    look_top: BTreeSet::new(),
    extra_lands: BTreeMap::new(),
    again: Vec::new(),
    unpreventable: Vec::new(),
    casts: Vec::new(),
    taxes: Vec::new(),
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

    /// The top card of `player`'s library while a rule has it revealed to every seat: the house rule for every
    /// library once the opening shuffle has made its order unknown, or a card rule for the libraries it names.
    pub fn revealed_top(&self, player: PlayerId) -> Option<ObjectId> {
        let by_house = self.ruleset().revealed_library_top() && self.stage() != Stage::PreGame;
        if !by_house && !self.rules().reveal_top.contains(&player) {
            return None;
        }
        self.zone(Zone::Library(player)).last().copied()
    }
}

/// Folds `RuleCreated`: the change joins those in state; a duration that follows the source needs one, and an id is
/// used once.
pub(crate) fn fold_created(state: &mut GameState, effect: &RuleEffect) -> Result<(), EngineError> {
    if effect.duration == Duration::WhileSourceOnBattlefield && effect.source.is_none() {
        return Err(invalid(
            "a rule that lasts while its source stays has a source",
        ));
    }
    if state.rule_effects.iter().any(|e| e.id == effect.id) {
        return Err(invalid(format!("duplicate rule {:?}", effect.id)));
    }
    state.next_effect = state.next_effect.max(effect.id.0 + 1);
    if matches!(
        effect.rule,
        Rule::PreventDamage(_) | Rule::Regenerate(_) | Rule::DiesInstead { .. }
    ) {
        state.possible.replacements = true;
    }
    if matches!(
        &effect.rule,
        Rule::PlayersHave {
            keyword: Keyword::Protection(_),
            ..
        }
    ) {
        state.possible.protection = true;
    }
    state.rule_effects.push(effect.clone());
    Ok(())
}

/// Folds `ShieldUsed`: the shield prevents no more than `left`.
pub(crate) fn fold_used(state: &mut GameState, id: EffectId, left: u32) -> Result<(), EngineError> {
    let shield = state
        .rule_effects
        .iter_mut()
        .find_map(|effect| match &mut effect.rule {
            Rule::PreventDamage(shield) if effect.id == id => Some(shield),
            _ => None,
        });
    match shield {
        Some(shield) => {
            shield.left = Some(left);
            Ok(())
        }
        None => Err(invalid(format!("unknown prevention shield {id:?}"))),
    }
}

/// Folds `RuleExpired`: the change leaves the state.
pub(crate) fn fold_expired(state: &mut GameState, id: EffectId) -> Result<(), EngineError> {
    let before = state.rule_effects.len();
    state.rule_effects.retain(|e| e.id != id);
    if state.rule_effects.len() == before {
        return Err(invalid(format!("unknown rule {id:?}")));
    }
    Ok(())
}

/// Whether the creature `id` attacks each combat if able: the one place the attack declaration reads this requirement.
pub fn must_attack(state: &GameState, id: ObjectId) -> bool {
    state.rules().must_attack.contains(&id)
}

/// The players the creature `id` attacks this combat if able, in the order the rules named them: the one place the attack
/// declaration reads this requirement.
pub fn must_attack_players(state: &GameState, id: ObjectId) -> Vec<PlayerId> {
    state
        .rules()
        .attack_players
        .iter()
        .filter(|(object, _)| *object == id)
        .map(|(_, player)| *player)
        .collect()
}

/// Whether the creature `id` blocks each combat if able: the one place the block declaration reads this requirement.
pub fn must_block(state: &GameState, id: ObjectId) -> bool {
    state.rules().must_block.contains(&id)
}

/// Whether the spell `id` can't be countered: the one place a counter reads this rule.
pub fn cant_be_countered(state: &GameState, id: ObjectId) -> bool {
    state.rules().uncounterable.contains(&id)
}

/// Whether `player` can't gain life: the one place a gain of life reads this rule. A life total that can't change
/// gains none either.
pub fn cant_gain_life(state: &GameState, player: PlayerId) -> bool {
    let rules = state.rules();
    rules.no_life_gain.contains(&player) || rules.no_life_change.contains(&player)
}

/// Whether the life total of `player` can't change (CR 119.7, 119.8): the one place a loss or payment of life reads
/// this rule.
pub fn life_total_cant_change(state: &GameState, player: PlayerId) -> bool {
    state.rules().no_life_change.contains(&player)
}

/// Whether damage dealt to `player` causes no loss of life: the one place the damage events read this rule.
pub fn damage_causes_no_life_loss(state: &GameState, player: PlayerId) -> bool {
    let rules = state.rules();
    rules.no_damage_loss.contains(&player) || rules.no_life_change.contains(&player)
}

/// Whether `player` gains life rather than loses life from radiation: the one place the rad rule reads this rule.
pub(crate) fn w8b_radiation_gains_life(state: &GameState, player: PlayerId) -> bool {
    state.rules().radiation_gains.contains(&player)
}

/// The keywords the rules in force give `player` for a time, such as protection from everything.
pub(crate) fn granted_player_keywords(state: &GameState, player: PlayerId) -> Vec<&Keyword> {
    state
        .rules()
        .player_keywords
        .iter()
        .filter(|(who, _)| *who == player)
        .map(|(_, keyword)| keyword)
        .collect()
}

/// Whether `player` skips each extra turn they would begin: the one place the turn order reads this rule.
pub fn skips_extra_turns(state: &GameState, player: PlayerId) -> bool {
    state.rules().skip_extra_turns.contains(&player)
}

/// Whether `player` can't draw cards: the one place a draw reads this rule.
pub fn cant_draw(state: &GameState, player: PlayerId) -> bool {
    state.rules().no_draw.contains(&player)
}

/// Whether `player` skips their draw step: the one place the turn's steps read this rule.
pub(crate) fn skips_draw_step(state: &GameState, player: PlayerId) -> bool {
    state.rules().skip_draw.contains(&player)
}

/// Whether `player` has no maximum hand size: the one place cleanup reads this rule, beside the house rule's limit.
pub(crate) fn no_max_hand_size(state: &GameState, player: PlayerId) -> bool {
    state.rules().no_hand_limit.contains(&player)
}

/// Whether `player` may look at the top card of their library any time: the one place the sight of a seat reads this
/// rule.
pub(crate) fn looks_at_top(state: &GameState, player: PlayerId) -> bool {
    state.rules().look_top.contains(&player)
}

/// How many lands more than the format's limit `player` may play in a turn: the one place a land play reads this rule.
pub(crate) fn extra_land_plays(state: &GameState, player: PlayerId) -> u32 {
    state
        .rules()
        .extra_lands
        .get(&player)
        .copied()
        .unwrap_or_default()
}

/// Whether `player` can't search libraries: the one place a search reads this rule.
pub fn cant_search(state: &GameState, player: PlayerId) -> bool {
    state.rules().no_search.contains(&player)
}

/// Whether the damage `dealer` would deal can't be prevented: the one place a prevention effect reads this rule. A source
/// that is gone is read as it is in the state (the damage of a creature that dies in the same step is dealt first).
pub(crate) fn damage_cant_be_prevented(state: &GameState, dealer: ObjectId, combat: bool) -> bool {
    state.rules().unpreventable.iter().any(|rule| {
        rule.combat.is_none_or(|only| only == combat)
            && (rule.from.is_empty()
                || state.object(dealer).is_some_and(|object| {
                    Scope::announcing(state, rule.controller, rule.source)
                        .selects(&rule.from, object.reference())
                        .unwrap_or(false)
                }))
    })
}

/// The permissions to cast cards in force: the one place a cast reads them (`ways`).
pub(crate) fn cast_permissions(state: &GameState) -> &[Cast] {
    &state.rules().casts
}

/// The source of the permission that works once each turn which a cast of `card` by `player` from `from` for `way` uses
/// up: a cast for the mana cost from a zone other than a hand or the command zone that no other permission for that zone
/// covers, through the first such permission that covers it. None for any other cast.
pub(crate) fn c11b_once_source(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    from: Zone,
    way: Option<mtg_cards::value::CastWay>,
) -> Option<ObjectRef> {
    if way.is_some() || matches!(from, Zone::Hand(_) | Zone::Command(_)) {
        return None;
    }
    let card = state.object(card)?;
    let zone = crate::resolve::zone_kind(from);
    let covering: Vec<&Cast> = cast_permissions(state)
        .iter()
        .filter(|cast| !cast.free && cast.from == Some(zone) && cast.covers(state, player, card))
        .collect();
    if covering.iter().any(|cast| !cast.once) {
        return None;
    }
    covering.first().map(|cast| cast.source)
}

/// The taxes on attacking in force: the one place the attack declaration reads this rule (`attack_tax`).
pub(crate) fn attack_taxes(state: &GameState) -> &[Taxing] {
    &state.rules().taxes
}

/// The triggers that trigger an additional time: the one place trigger detection reads this rule.
pub(crate) fn triggers_again(state: &GameState) -> &[Again] {
    &state.rules().again
}

#[cfg(test)]
mod tests {
    //! Goal: a rule of the game is in force for the players and objects its references name and for as long as its source has it or its duration lasts: a static rule stops with its source, with its condition and with the abilities its source loses, an effect ends by the rule of its duration or when the objects it named are gone, two rules of one kind together name whoever either names and each ends alone, the order the rules began in changes nothing, and a rule the engine does not play refuses by name.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;
    use crate::effects::{ContinuousEffect, Modification, Selector};
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::turn::Step;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// What a table asks of the rules in force.
    #[derive(Debug, Clone, Copy)]
    enum Probe {
        Life(PlayerId),
        Draw(PlayerId),
        Search(PlayerId),
        Top(PlayerId),
        Looks(PlayerId),
        Plays(PlayerId, u32),
        Attack(&'static str),
        Block(&'static str),
    }

    use Probe::{Attack, Block, Draw, Life, Looks, Plays, Search, Top};

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    fn rule_ability(rule: &str) -> Ability {
        ability(&format!(r#"{{"Rule":{{"rule":{rule}}}}}"#))
    }

    /// A creature of seat 0 that has the abilities.
    fn holder(rules: Vec<Ability>) -> Arc<CardDefinition> {
        let card = CardDefinition::new("Holder")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1);
        Arc::new(rules.into_iter().fold(card, CardDefinition::with_rules))
    }

    /// Seat 0 controls the holder and a Wolf, seat 1 a Bear.
    fn game(rules: Vec<Ability>) -> GameState {
        state(
            vec![vec![], vec![]],
            vec![
                vec![holder(rules), creature("Wolf", 2, 2)],
                vec![creature("Bear", 2, 2)],
            ],
        )
    }

    fn id(game: &GameState, name: &str) -> ObjectId {
        game.objects()
            .find(|o| o.card().name == name)
            .map(GameObject::id)
            .unwrap()
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.object(id(game, name)).unwrap().reference()
    }

    fn holds(game: &GameState, probe: Probe) -> bool {
        match probe {
            Life(player) => cant_gain_life(game, player),
            Draw(player) => cant_draw(game, player),
            Search(player) => cant_search(game, player),
            Top(player) => game.revealed_top(player).is_some(),
            Looks(player) => looks_at_top(game, player),
            Plays(player, extra) => extra_land_plays(game, player) == extra,
            Attack(name) => must_attack(game, id(game, name)),
            Block(name) => must_block(game, id(game, name)),
        }
    }

    fn leaves(game: &mut GameState, name: &str) {
        let object = id(game, name);
        let owner = game.object(object).unwrap().owner();
        let event = Event::ZoneChanged {
            object,
            from: Zone::Battlefield,
            to: Zone::Graveyard(owner),
            cause: ZoneCause::Moved,
        };
        apply_event(game, &event).unwrap();
    }

    /// A rule of an effect of seat 0 that began in turn 3, with the holder as its source.
    fn effect(game: &GameState, rule: Rule, duration: Duration) -> RuleEffect {
        RuleEffect {
            id: EffectId(game.next_effect),
            rule,
            duration,
            source: Some(named(game, "Holder")),
            controller: ME,
            turn: 3,
        }
    }

    fn put_in_force(game: &mut GameState, rule: Rule) {
        let effect = effect(game, rule, Duration::Forever);
        apply_event(game, &Event::RuleCreated { effect }).unwrap();
    }

    #[rstest]
    #[case::my_life(r#"{"CantGainLife":"You"}"#, &[(Life(ME), true), (Life(FOE), false)])]
    #[case::their_life(r#"{"CantGainLife":"Opponents"}"#, &[(Life(ME), false), (Life(FOE), true)])]
    #[case::everyones_life(r#"{"CantGainLife":"EachPlayer"}"#, &[(Life(ME), true), (Life(FOE), true)])]
    #[case::my_draws(r#"{"CantDraw":"You"}"#, &[(Draw(ME), true), (Draw(FOE), false)])]
    #[case::their_draws(r#"{"CantDraw":"Opponents"}"#, &[(Draw(ME), false), (Draw(FOE), true)])]
    #[case::everyones_draws(r#"{"CantDraw":"EachPlayer"}"#, &[(Draw(ME), true), (Draw(FOE), true)])]
    #[case::my_searches(r#"{"CantSearch":"You"}"#, &[(Search(ME), true), (Search(FOE), false)])]
    #[case::their_searches(r#"{"CantSearch":"Opponents"}"#, &[(Search(ME), false), (Search(FOE), true)])]
    #[case::everyones_searches(r#"{"CantSearch":"EachPlayer"}"#, &[(Search(ME), true), (Search(FOE), true)])]
    #[case::my_top(r#"{"RevealTop":"You"}"#, &[(Top(ME), true), (Top(FOE), false)])]
    #[case::their_top(r#"{"RevealTop":"Opponents"}"#, &[(Top(ME), false), (Top(FOE), true)])]
    #[case::every_top(r#"{"RevealTop":"EachPlayer"}"#, &[(Top(ME), true), (Top(FOE), true)])]
    #[case::my_look(r#"{"LookAtTopAnyTime":"You"}"#, &[(Looks(ME), true), (Looks(FOE), false)])]
    #[case::their_look(r#"{"LookAtTopAnyTime":"Opponents"}"#, &[(Looks(ME), false), (Looks(FOE), true)])]
    #[case::every_look(r#"{"LookAtTopAnyTime":"EachPlayer"}"#, &[(Looks(ME), true), (Looks(FOE), true)])]
    #[case::looking_is_not_revealing(r#"{"LookAtTopAnyTime":"You"}"#, &[(Top(ME), false), (Top(FOE), false)])]
    #[case::an_extra_land(r#"{"AdditionalLandPlays":{}}"#, &[(Plays(ME, 1), true), (Plays(FOE, 0), true), (Plays(FOE, 1), false)])]
    #[case::two_extra_lands(r#"{"AdditionalLandPlays":{"count":2}}"#, &[(Plays(ME, 2), true), (Plays(ME, 1), false), (Plays(FOE, 0), true)])]
    #[case::an_extra_land_each(r#"{"AdditionalLandPlays":{"who":"EachPlayer"}}"#, &[(Plays(ME, 1), true), (Plays(FOE, 1), true)])]
    #[case::an_extra_land_for_them(r#"{"AdditionalLandPlays":{"who":"Opponents"}}"#, &[(Plays(ME, 0), true), (Plays(FOE, 1), true)])]
    #[case::lands_by_a_count(r#"{"AdditionalLandPlays":{"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#, &[(Plays(ME, 2), true), (Plays(FOE, 0), true)])]
    #[case::this_attacks(r#"{"MustAttack":"This"}"#, &[(Attack("Holder"), true), (Attack("Wolf"), false), (Attack("Bear"), false)])]
    #[case::my_creatures_attack(r#"{"MustAttack":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#, &[(Attack("Holder"), true), (Attack("Wolf"), true), (Attack("Bear"), false)])]
    #[case::their_creatures_attack(r#"{"MustAttack":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}"#, &[(Attack("Holder"), false), (Attack("Wolf"), false), (Attack("Bear"), true)])]
    #[case::all_creatures_attack(r#"{"MustAttack":{"Each":[{"Type":"Creature"}]}}"#, &[(Attack("Holder"), true), (Attack("Wolf"), true), (Attack("Bear"), true)])]
    #[case::this_blocks(r#"{"MustBlock":"This"}"#, &[(Block("Holder"), true), (Block("Wolf"), false)])]
    #[case::all_creatures_block(r#"{"MustBlock":{"Each":[{"Type":"Creature"}]}}"#, &[(Block("Holder"), true), (Block("Wolf"), true), (Block("Bear"), true)])]
    #[case::attacking_is_not_blocking(r#"{"MustAttack":"This"}"#, &[(Block("Holder"), false)])]
    fn a_static_rule_names_the_players_and_objects_its_references_name(
        #[case] rule: &str,
        #[case] probes: &[(Probe, bool)],
    ) {
        let game = game(vec![rule_ability(rule)]);
        for (probe, expected) in probes {
            assert_eq!(holds(&game, *probe), *expected, "{probe:?}");
        }
    }

    #[rstest]
    #[case::life(r#"{"CantGainLife":"EachPlayer"}"#, Life(ME))]
    #[case::draws(r#"{"CantDraw":"EachPlayer"}"#, Draw(FOE))]
    #[case::searches(r#"{"CantSearch":"EachPlayer"}"#, Search(ME))]
    #[case::top(r#"{"RevealTop":"EachPlayer"}"#, Top(FOE))]
    #[case::look(r#"{"LookAtTopAnyTime":"EachPlayer"}"#, Looks(ME))]
    #[case::lands(r#"{"AdditionalLandPlays":{"who":"EachPlayer"}}"#, Plays(FOE, 1))]
    #[case::attack(r#"{"MustAttack":"This"}"#, Attack("Holder"))]
    #[case::block(r#"{"MustBlock":"This"}"#, Block("Holder"))]
    fn a_static_rule_stops_with_its_source(#[case] rule: &str, #[case] probe: Probe) {
        let mut game = game(vec![rule_ability(rule)]);
        assert!(holds(&game, probe));
        let holder = id(&game, "Holder");
        leaves(&mut game, "Holder");
        assert_ne!(game.object(holder).unwrap().zone(), Zone::Battlefield);
        assert!(!holds(&game, probe));
    }

    #[test]
    fn a_static_rule_with_a_condition_applies_while_it_holds() {
        let rule = ability(r#"{"Rule":{"rule":{"CantGainLife":"You"},"condition":"YourTurn"}}"#);
        let mut game = game(vec![rule]);
        assert!(holds(&game, Life(ME)), "it is the holder's turn");
        game.turn.active = FOE;
        game.rules_cache.reset();
        assert!(!holds(&game, Life(ME)), "it is another seat's turn");
        game.turn.active = ME;
        game.rules_cache.reset();
        assert!(holds(&game, Life(ME)));
    }

    #[test]
    fn a_permanent_that_lost_its_abilities_has_no_rule() {
        let mut game = game(vec![rule_ability(r#"{"CantDraw":"EachPlayer"}"#)]);
        assert!(holds(&game, Draw(ME)));
        let holder = named(&game, "Holder");
        let lost = ContinuousEffect::new(
            EffectId(game.next_effect),
            Selector::Objects(vec![holder]),
            Modification::LoseAbilities,
            Duration::Forever,
            game.next_timestamp,
        );
        apply_event(&mut game, &Event::EffectCreated { effect: lost }).unwrap();
        assert!(!holds(&game, Draw(ME)));
    }

    #[test]
    fn a_card_in_a_hand_has_no_rule() {
        let game = state(
            vec![
                vec![holder(vec![rule_ability(r#"{"CantDraw":"EachPlayer"}"#)])],
                vec![],
            ],
            vec![vec![], vec![]],
        );
        assert!(game.rules_possible);
        assert!(!holds(&game, Draw(ME)) && !holds(&game, Draw(FOE)));
    }

    #[rstest]
    #[case::end_of_turn_stays_through_the_turn(Duration::UntilEndOfTurn, (3, ME, Step::End), true)]
    #[case::end_of_turn_waits_for_the_cleanup_event(Duration::UntilEndOfTurn, (4, FOE, Step::Main1), true)]
    #[case::forever(Duration::Forever, (9, FOE, Step::Main2), true)]
    #[case::combat_before_it(Duration::UntilEndOfCombat, (3, ME, Step::Main1), true)]
    #[case::combat_to_its_last_step(Duration::UntilEndOfCombat, (3, ME, Step::EndCombat), true)]
    #[case::combat_after_it(Duration::UntilEndOfCombat, (3, ME, Step::Main2), false)]
    #[case::combat_of_another_turn(Duration::UntilEndOfCombat, (4, FOE, Step::Main1), false)]
    #[case::your_turn_still_going(Duration::UntilYourNextTurn, (3, ME, Step::End), true)]
    #[case::your_turn_when_another_seat_plays(Duration::UntilYourNextTurn, (4, FOE, Step::Main1), true)]
    #[case::your_turn_when_it_comes_again(Duration::UntilYourNextTurn, (5, ME, Step::Upkeep), false)]
    #[case::while_the_source_stays(Duration::WhileSourceOnBattlefield, (3, ME, Step::Main1), true)]
    fn a_rule_of_an_effect_lasts_as_long_as_its_duration(
        #[case] duration: Duration,
        #[case] now: (u32, PlayerId, Step),
        #[case] active: bool,
    ) {
        let mut game = game(Vec::new());
        let rule = effect(&game, Rule::CantGainLife(vec![ME]), duration);
        (game.turn.number, game.turn.active, game.turn.step) = now;
        assert_eq!(rule.is_active(&game), active);
        apply_event(&mut game, &Event::RuleCreated { effect: rule }).unwrap();
        assert_eq!(holds(&game, Life(ME)), active);
    }

    #[test]
    fn a_rule_that_lasts_while_its_source_stays_ends_when_it_leaves() {
        let mut game = game(Vec::new());
        let rule = effect(
            &game,
            Rule::CantDraw(vec![ME, FOE]),
            Duration::WhileSourceOnBattlefield,
        );
        apply_event(&mut game, &Event::RuleCreated { effect: rule }).unwrap();
        assert!(holds(&game, Draw(FOE)));
        leaves(&mut game, "Holder");
        assert!(!holds(&game, Draw(FOE)));
        let ended = crate::sba::housekeeping(&game);
        assert!(matches!(ended.as_slice(), [Event::RuleExpired { .. }]));
        for event in &ended {
            apply_event(&mut game, event).unwrap();
        }
        assert_eq!(game.rule_effects(), []);
    }

    #[test]
    fn a_rule_that_names_objects_is_spent_when_they_are_gone() {
        let mut game = game(Vec::new());
        let wolf = named(&game, "Wolf");
        put_in_force(&mut game, Rule::MustAttack(vec![wolf]));
        assert!(holds(&game, Attack("Wolf")));
        leaves(&mut game, "Wolf");
        assert!(!holds(&game, Attack("Wolf")));
        assert!(
            game.rule_effects()
                .iter()
                .all(|rule| !rule.is_active(&game))
        );
    }

    /// Moves `object` through each zone in turn, from the zone it is in; the object it is then is a new one.
    fn moved_through(game: &mut GameState, object: ObjectId, zones: &[Zone]) -> ObjectRef {
        for to in zones {
            let from = game.object(object).unwrap().zone();
            let event = Event::ZoneChanged {
                object,
                from,
                to: *to,
                cause: ZoneCause::Exiled,
            };
            apply_event(game, &event).unwrap();
        }
        game.object(object).unwrap().reference()
    }

    /// Exiles the permanent `name`.
    fn exiled(game: &mut GameState, name: &str) -> ObjectRef {
        let object = id(game, name);
        moved_through(game, object, &[Zone::Exile])
    }

    fn permission(only: Option<Vec<ObjectRef>>, play: bool) -> Rule {
        Rule::MayCast {
            who: vec![ME],
            what: mtg_cards::refs::Selector::default(),
            from: Some(ZoneKind::Exile),
            free: false,
            flash: false,
            only,
            play,
            once: false,
        }
    }

    #[rstest]
    #[case::the_card_named(0, ME, true)]
    #[case::another_player(0, FOE, false)]
    #[case::the_other_card(1, ME, false)]
    fn a_permission_to_play_covers_the_exiled_cards_it_names_and_no_other(
        #[case] card: usize,
        #[case] player: PlayerId,
        #[case] covered: bool,
    ) {
        let mut game = game(Vec::new());
        let named = [exiled(&mut game, "Wolf"), exiled(&mut game, "Bear")];
        put_in_force(&mut game, permission(Some(vec![named[0]]), true));
        let [permission] = cast_permissions(&game) else {
            panic!("one permission");
        };
        let object = game.object(named[card].id).unwrap();
        assert_eq!(permission.covers(&game, player, object), covered);
        assert_eq!(permission.plays(&game, player, object), covered);
    }

    #[test]
    fn a_permission_without_the_play_covers_a_cast_and_no_land_play() {
        let mut game = game(Vec::new());
        let wolf = exiled(&mut game, "Wolf");
        put_in_force(&mut game, permission(Some(vec![wolf]), false));
        let [permission] = cast_permissions(&game) else {
            panic!("one permission");
        };
        let object = game.object(wolf.id).unwrap();
        assert!(permission.covers(&game, ME, object));
        assert!(!permission.plays(&game, ME, object));
    }

    #[test]
    fn a_card_that_left_exile_and_came_back_is_a_new_object_the_permission_does_not_cover() {
        let mut game = game(Vec::new());
        let wolf = exiled(&mut game, "Wolf");
        let bear = exiled(&mut game, "Bear");
        put_in_force(&mut game, permission(Some(vec![wolf, bear]), true));
        let back = moved_through(&mut game, wolf.id, &[Zone::Graveyard(ME), Zone::Exile]);
        assert_ne!(back.counter, wolf.counter);
        let [permission] = cast_permissions(&game) else {
            panic!("one permission while a card it names is where it was");
        };
        assert!(!permission.covers(&game, ME, game.object(back.id).unwrap()));
        assert!(permission.covers(&game, ME, game.object(bear.id).unwrap()));
        moved_through(&mut game, bear.id, &[Zone::Graveyard(FOE)]);
        assert!(
            cast_permissions(&game).is_empty(),
            "every card it names is gone"
        );
        assert!(
            game.rule_effects()
                .iter()
                .all(|rule| !rule.is_active(&game))
        );
    }

    #[rstest]
    #[case::no_card(Some(Vec::new()), true)]
    #[case::a_card(Some(vec![ObjectRef { id: ObjectId(0), counter: 0 }]), false)]
    #[case::any_card(None, false)]
    fn a_permission_that_names_no_card_begins_nothing(
        #[case] only: Option<Vec<ObjectRef>>,
        #[case] nothing: bool,
    ) {
        assert_eq!(names_nothing(&permission(only, true)), nothing);
    }

    #[test]
    fn two_rules_of_one_kind_name_whoever_either_names_and_each_ends_alone() {
        let mut game = game(vec![rule_ability(r#"{"CantGainLife":"You"}"#)]);
        let mut fresh = effect(
            &game,
            Rule::CantGainLife(vec![FOE]),
            Duration::UntilEndOfTurn,
        );
        fresh.turn = game.turn().number();
        apply_event(
            &mut game,
            &Event::RuleCreated {
                effect: fresh.clone(),
            },
        )
        .unwrap();
        assert!(holds(&game, Life(ME)) && holds(&game, Life(FOE)));
        apply_event(&mut game, &Event::RuleExpired { id: fresh.id }).unwrap();
        assert!(holds(&game, Life(ME)) && !holds(&game, Life(FOE)));
        leaves(&mut game, "Holder");
        assert!(!holds(&game, Life(ME)));
    }

    #[test]
    fn the_same_rule_twice_is_one_rule() {
        let mut game = game(vec![rule_ability(r#"{"CantDraw":"You"}"#)]);
        put_in_force(&mut game, Rule::CantDraw(vec![ME]));
        put_in_force(&mut game, Rule::CantDraw(vec![ME]));
        assert!(holds(&game, Draw(ME)) && !holds(&game, Draw(FOE)));
        let ids: Vec<EffectId> = game.rule_effects().iter().map(|rule| rule.id).collect();
        for id in ids {
            apply_event(&mut game, &Event::RuleExpired { id }).unwrap();
        }
        assert!(holds(&game, Draw(ME)), "the static rule is still in force");
    }

    #[rstest]
    #[case::in_the_order_listed(false)]
    #[case::in_the_reverse_order(true)]
    fn the_order_rules_began_in_changes_nothing(#[case] reversed: bool) {
        let mut game = game(vec![rule_ability(r#"{"CantSearch":"You"}"#)]);
        let wolf = named(&game, "Wolf");
        let mut rules = vec![
            Rule::CantDraw(vec![FOE]),
            Rule::MustBlock(vec![wolf]),
            Rule::RevealTop(vec![ME, FOE]),
        ];
        if reversed {
            rules.reverse();
        }
        for rule in rules {
            put_in_force(&mut game, rule);
        }
        let probes = [
            Search(ME),
            Search(FOE),
            Draw(ME),
            Draw(FOE),
            Block("Wolf"),
            Block("Bear"),
            Top(ME),
            Top(FOE),
        ];
        let seen: Vec<bool> = probes.iter().map(|probe| holds(&game, *probe)).collect();
        assert_eq!(
            seen,
            [true, false, false, true, true, false, true, true],
            "the same rules in force whichever began first"
        );
    }

    #[rstest]
    #[case::creatures_that_exist(r#"{"MustAttack":{"Each":[{"Type":"Creature"}]}}"#, false)]
    #[case::no_such_creature(r#"{"MustAttack":{"Each":[{"Type":"Planeswalker"}]}}"#, true)]
    #[case::no_spell_on_the_stack(
        r#"{"CantBeCountered":{"Each":[{"Type":"Creature"},"Spell"]}}"#,
        true
    )]
    #[case::players(r#"{"RevealTop":"Opponents"}"#, false)]
    fn a_rule_that_names_nothing_is_not_begun(#[case] rule: &str, #[case] nothing: bool) {
        let game = game(Vec::new());
        let scope = Scope::announcing(&game, ME, named(&game, "Holder"));
        let wanted: GameRule = serde_json::from_str(rule).unwrap();
        assert_eq!(
            names_nothing(&lower_rule(&scope, &wanted).unwrap()),
            nothing
        );
    }

    #[rstest]
    #[case::a_commander(r#""CanBeCommander""#, "Rule.CanBeCommander")]
    fn a_rule_the_engine_does_not_play_refuses_by_its_name(
        #[case] rule: &str,
        #[case] name: &'static str,
    ) {
        let game = game(Vec::new());
        let scope = Scope::announcing(&game, ME, named(&game, "Holder"));
        let wanted: GameRule = serde_json::from_str(rule).unwrap();
        assert_eq!(
            lower_rule(&scope, &wanted).err(),
            Some(ResolveError::Unsupported(name))
        );
    }

    #[test]
    fn a_rule_that_triggers_again_holds_a_pattern_and_the_player_whose_permanents_it_is_about() {
        let cause = r#"{"ZoneChange":{"what":[{"Type":"Creature"}],"to":"Battlefield"}}"#;
        let rule = format!(r#"{{"TriggerAgain":{{"cause":{cause}}}}}"#);
        let game = game(vec![rule_ability(&rule)]);
        let again = triggers_again(&game);
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].controller, ME);
        assert_eq!(again[0].source, named(&game, "Holder"));
        assert_eq!(
            again[0].cause,
            serde_json::from_str::<TriggerPattern>(cause).unwrap()
        );
    }

    #[test]
    fn the_top_of_a_revealed_library_is_shown_to_every_seat_and_nothing_below_it() {
        let mut game = game(vec![rule_ability(r#"{"RevealTop":"You"}"#)]);
        let library = game.zone(Zone::Library(ME)).to_vec();
        let (top, second) = (library[library.len() - 1], library[library.len() - 2]);
        assert_eq!(game.revealed_top(ME), Some(top));
        assert_eq!(game.revealed_top(FOE), None);
        for viewer in [ME, FOE] {
            assert!(game.is_shown_to(viewer, top));
            assert!(!game.is_shown_to(viewer, second));
        }
        // Drawing the card shows the next one; the drawn card is in a hand, which only its owner sees.
        let drawn = Event::CardDrawn {
            player: ME,
            object: top,
        };
        apply_event(&mut game, &drawn).unwrap();
        assert_eq!(game.revealed_top(ME), Some(second));
        assert!(game.is_shown_to(FOE, second) && !game.is_shown_to(FOE, top));
    }

    /// A table of `seats` players: seat 0 controls the holder with the rules, a Forest sits in its hand and in its
    /// graveyard, and every seat has a library of five creatures.
    fn table(seats: usize, rules: Vec<Ability>) -> GameState {
        let forest = || {
            Arc::new(CardDefinition::basic_land(Some(
                mtg_core::color::Color::Green,
            )))
        };
        let mut battlefields = vec![vec![holder(rules)]];
        battlefields.extend((1..seats).map(|_| Vec::new()));
        let mut hands = vec![vec![forest(), forest()]];
        hands.extend((1..seats).map(|_| Vec::new()));
        let mut setup = crate::fixtures::setup(hands, battlefields);
        setup.players[0].graveyard = vec![forest()];
        match GameState::from_setup(&setup) {
            Ok(state) => state,
            Err(error) => panic!("the setup is valid: {error}"),
        }
    }

    #[rstest]
    #[case::you("You", 1)]
    #[case::each_player("EachPlayer", 0)]
    #[case::opponents("Opponents", 0)]
    fn rules_about_players_reach_each_seat_of_a_table_of_two_to_four(
        #[values(2, 3, 4)] seats: usize,
        #[case] who: &str,
        #[case] fixed: usize,
    ) {
        let reached = match who {
            "You" => fixed,
            "Opponents" => seats - 1,
            _ => seats,
        };
        let look = rule_ability(&format!(r#"{{"LookAtTopAnyTime":"{who}"}}"#));
        let lands = rule_ability(&format!(
            r#"{{"AdditionalLandPlays":{{"who":"{who}","count":2}}}}"#
        ));
        let game = table(seats, vec![look, lands]);
        let seats = (0..seats).map(|n| PlayerId(u64::try_from(n).unwrap()));
        assert_eq!(
            seats
                .clone()
                .filter(|seat| looks_at_top(&game, *seat))
                .count(),
            reached
        );
        assert_eq!(
            seats.map(|seat| extra_land_plays(&game, seat)).sum::<u32>(),
            2 * u32::try_from(reached).unwrap()
        );
    }

    #[test]
    fn two_sources_of_extra_land_plays_add_up_and_each_ends_alone() {
        let one = || rule_ability(r#"{"AdditionalLandPlays":{}}"#);
        let mut game = game(vec![one(), one()]);
        assert_eq!(extra_land_plays(&game, ME), 2);
        // A second permanent that has the rule joins the first: the count follows the sources in play.
        let wolf = id(&game, "Wolf");
        let effect = effect(
            &game,
            Rule::AdditionalLandPlays(vec![(ME, 3)]),
            Duration::Forever,
        );
        apply_event(&mut game, &Event::RuleCreated { effect }).unwrap();
        assert_eq!(extra_land_plays(&game, ME), 5);
        assert_eq!(extra_land_plays(&game, FOE), 0);
        leaves(&mut game, "Holder");
        assert_eq!(
            extra_land_plays(&game, ME),
            3,
            "the effect outlasts the holder"
        );
        assert!(game.object(wolf).is_some());
    }

    #[test]
    fn extra_land_plays_let_the_active_player_play_more_lands_in_a_turn() {
        let rules = vec![rule_ability(r#"{"AdditionalLandPlays":{}}"#)];
        let plain = table(2, Vec::new());
        let extra = table(2, rules);
        let playable =
            |game: &GameState| crate::priority::priority_options(game, ME).playable_lands;
        let play = |game: &mut GameState| {
            let land = playable(game)[0];
            let event = Event::LandPlayed {
                player: ME,
                object: land,
                face: None,
            };
            apply_event(game, &event).unwrap();
        };
        let (mut plain, mut extra) = (plain, extra);
        assert_eq!(playable(&plain).len(), 2);
        play(&mut plain);
        play(&mut extra);
        assert_eq!(playable(&plain), Vec::<ObjectId>::new(), "one land a turn");
        assert_eq!(playable(&extra).len(), 1, "one more with the rule");
        play(&mut extra);
        assert_eq!(playable(&extra), Vec::<ObjectId>::new());
        extra.turn.lands_played = 0;
        assert_eq!(playable(&extra).len(), 0, "the hand is empty");
    }

    #[test]
    fn the_top_card_is_shown_to_its_owner_alone_while_a_rule_lets_them_look() {
        let mut game = game(vec![rule_ability(r#"{"LookAtTopAnyTime":"You"}"#)]);
        let library = game.zone(Zone::Library(ME)).to_vec();
        let (top, second) = (library[library.len() - 1], library[library.len() - 2]);
        assert!(game.is_shown_to(ME, top));
        assert!(!game.is_shown_to(FOE, top), "an opponent does not see it");
        assert!(!game.is_shown_to(ME, second), "nothing below the top");
        assert_eq!(
            game.revealed_top(ME),
            None,
            "it is not revealed to the table"
        );
        let drawn = Event::CardDrawn {
            player: ME,
            object: top,
        };
        apply_event(&mut game, &drawn).unwrap();
        assert!(game.is_shown_to(ME, second), "the next card is the top");
        leaves(&mut game, "Holder");
        assert!(
            !game.is_shown_to(ME, second),
            "the rule ended with its source"
        );
    }

    #[test]
    fn a_land_in_the_graveyard_is_played_under_a_permission_to_play_lands_from_there() {
        let permission = r#"{"MayCast":{"what":[{"Type":"Land"}],"from":"Graveyard","play":true}}"#;
        let playable =
            |game: &GameState| crate::priority::priority_options(game, ME).playable_lands;
        let graveyard = |game: &GameState| game.zone(Zone::Graveyard(ME)).to_vec();
        let mut game = table(2, vec![rule_ability(permission)]);
        let card = graveyard(&game)[0];
        assert!(playable(&game).contains(&card));
        let event = Event::LandPlayed {
            player: ME,
            object: card,
            face: None,
        };
        apply_event(&mut game, &event).unwrap();
        assert_eq!(game.object(card).unwrap().zone(), Zone::Battlefield);
        assert_eq!(graveyard(&game), Vec::<ObjectId>::new());
        let without = table(2, Vec::new());
        assert!(!playable(&without).contains(&graveyard(&without)[0]));
        // The permission does not reach a graveyard of another seat.
        assert_eq!(
            crate::priority::priority_options(&game, FOE).playable_lands,
            Vec::<ObjectId>::new()
        );
    }

    #[test]
    fn a_game_whose_cards_change_no_rule_gathers_nothing() {
        let game = game(Vec::new());
        assert!(!game.rules_possible && game.rule_effects().is_empty());
        assert!(std::ptr::eq(game.rules(), &raw const NONE));
    }

    #[test]
    fn a_clone_gathers_its_own_rules() {
        let mut game = game(vec![rule_ability(r#"{"CantDraw":"You"}"#)]);
        assert!(holds(&game, Draw(ME)));
        let copy = game.clone();
        leaves(&mut game, "Holder");
        assert!(!holds(&game, Draw(ME)));
        assert!(holds(&copy, Draw(ME)), "the copy still has the holder");
    }
}
