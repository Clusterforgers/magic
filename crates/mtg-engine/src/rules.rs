//! The rules of the game that spells, abilities and permanents change: attack and block requirements, spells that can't
//! be countered, players who can't gain life, draw or search, library tops that are revealed, and triggers that trigger
//! an additional time (`docs/engine.md`, Rules of the game).
//!
//! The rules in force are gathered like the layers: from the `Effect::Rule`s that resolved and still last, and from the
//! `Ability::Rule`s that permanents have (read through `ability_reader`, so a permanent that lost its abilities lost
//! its rules) and that spells on the stack have for themselves. The result is kept with the state until an event that
//! can change it (`GameState::rules`), and each rule is read by one function that the one place deciding it asks.

mod player_effects;

pub use player_effects::{PlayerEffect, player_effects};

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::effect::Place;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Attacked, CastRider, GameRule, ManaChange, Prohibited, SpendAs};
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use mtg_cards::value::{Condition, CounterKind};
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaType};
use serde::{Deserialize, Serialize};

use crate::ability_reader::{abilities_of, definition_has_rule, is_rule, may_hold};
use crate::characteristics::controller_of_object;
use crate::choose::Target;
use crate::combat::AttackTarget;
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
    /// Creatures attack the planeswalker each combat if able (CR 508.1d): the creatures named and every creature the
    /// players control as attackers are declared.
    MustAttackAt {
        /// The creatures named.
        creatures: Vec<ObjectRef>,
        /// The players whose creatures attack it.
        players: Vec<PlayerId>,
        /// The planeswalker they attack.
        at: ObjectRef,
        /// The players whose next turn the rule lasts through: the named players, or the controllers of the named
        /// creatures as it began.
        turn_of: Vec<PlayerId>,
    },
    /// Each of the players attacks with at least one creature each combat if able (CR 508.1d); with defenders named,
    /// one attacking one of them.
    MustAttackWithOne {
        /// The players who must attack.
        players: Vec<PlayerId>,
        /// The players and planeswalkers the creature attacks; any defender when empty.
        at: Vec<AttackTarget>,
    },
    /// The creatures block each combat if able (CR 509.1c).
    MustBlock(Vec<ObjectRef>),
    /// The creatures block the attacker if able (Provoke, CR 509.1c).
    MustBlockAttacker {
        /// The creatures that must block it.
        blockers: Vec<ObjectRef>,
        /// The attacking creature.
        attacker: ObjectRef,
    },
    /// A permission or a limit on how the creatures take part in combat.
    Combat {
        /// What the rule says of them.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        quirk: mtg_cards::modify::CombatQuirk,
        /// The creatures.
        creatures: Vec<ObjectRef>,
    },
    /// The players may target the permanents and the players as though they did not have hexproof, and shroud when `shroud`.
    IgnoreHexproof {
        /// The players whose spells and abilities may.
        by: Vec<PlayerId>,
        /// The permanents covered.
        what: Vec<ObjectRef>,
        /// The players covered.
        players: Vec<PlayerId>,
        /// Shroud is ignored too.
        shroud: bool,
    },
    /// Creatures can't attack the players unless their controller pays for each of them as the attack is declared
    /// (CR 508.1h).
    AttackTax(AttackTax),
    /// Creatures can't block unless their controller pays for each of them as blockers are declared (CR 509.1d).
    BlockTax(crate::block_tax::BlockTax),
    /// No more than this many creatures can attack each combat (CR 508.1c).
    AttackLimit(u32),
    /// No more than `most` creatures can attack each of the players or planeswalkers each combat (CR 508.1c).
    AttackLimitAt {
        /// The most creatures that can attack each of them.
        most: u32,
        /// The players and planeswalkers it protects.
        at: Vec<AttackTarget>,
    },
    /// No more than this many creatures can block each combat (CR 509.1b).
    BlockLimit(u32),
    /// No more than `most` creatures each of the players controls can block each combat (CR 509.1b).
    BlockLimitEach {
        /// The most creatures each of them can block with.
        most: u32,
        /// The players whose blocks it limits.
        who: Vec<PlayerId>,
    },
    /// The spells can't be countered.
    CantBeCountered(Vec<ObjectRef>),
    /// The spell enters with additional +1/+1 counters and gains a keyword: mana with such a rider was spent to cast it.
    EntersWith {
        /// The spell, on the stack.
        spell: ObjectRef,
        /// The additional +1/+1 counters it enters with.
        counters: u32,
        /// The keyword the permanent gains.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        keyword: Option<Keyword>,
        /// The keyword lasts until end of turn; otherwise while the permanent stays.
        this_turn: bool,
    },
    /// The permanents can't be regenerated.
    CantBeRegenerated(Vec<ObjectRef>),
    /// The players can't gain life.
    CantGainLife(Vec<PlayerId>),
    /// Damage dealt to the players causes no loss of life.
    NoLifeLossFromDamage(Vec<PlayerId>),
    /// Damage can't take the life total of the players below the floor while it is at least that much (CR 614.1, 120.3a).
    LifeFloor {
        /// The players.
        players: Vec<PlayerId>,
        /// The life total damage leaves them at least at.
        floor: i32,
    },
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
    /// The mana the permanents the selector picks add as the players tap them for mana changes (CR 106.6, 614.1).
    ManaProduced {
        /// The permanents, judged as they are tapped.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        what: Selector,
        /// The players whose taps are changed.
        who: Vec<PlayerId>,
        /// The change.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        change: ManaChange,
    },
    /// The players can't draw cards.
    CantDraw(Vec<PlayerId>),
    /// The players draw from the bottom of their libraries.
    DrawFromBottom(Vec<PlayerId>),
    /// The players can't search libraries.
    CantSearch(Vec<PlayerId>),
    /// Spells and abilities the players control can't cause their controller to search their own library.
    OwnSpellsCantSearch(Vec<PlayerId>),
    /// A permanent with a counter of this kind that would untap during its controller's untap step loses all of them
    /// instead (CR 614.1a).
    UntapRemovesCounters(CounterKind),
    /// The players search only the top `count` cards of a library they would search (CR 614.1a).
    SearchTop {
        /// Who searches that way.
        players: Vec<PlayerId>,
        /// How many cards from the top.
        count: u32,
    },
    /// The players may vote an additional time (CR 701.38d).
    VoteAgain(Vec<PlayerId>),
    /// The players get an additional vote (CR 701.38d).
    ExtraVote(Vec<PlayerId>),
    /// The turn order runs the other way round the table.
    ReverseTurnOrder,
    /// Each player may attack only the nearest opponent on the left (`Some(true)`) or the right of them and the
    /// planeswalkers that opponent controls; nothing when no side was chosen.
    AttackOnlyNearest(Option<bool>),
    /// The players skip each extra turn they would begin (CR 500.7).
    SkipExtraTurns(Vec<PlayerId>),
    /// The players flip two coins for each coin they would flip and ignore one (CR 705.3).
    FlipTwoCoins(Vec<PlayerId>),
    /// The players roll one die more for each roll of one or more dice and ignore the lowest roll.
    ExtraDie(Vec<PlayerId>),
    /// A rule about planar dice or planeswalking (Ichor Elixir, Susan Foreman): no game the engine plays has a planar
    /// deck, so it changes nothing.
    Planar,
    /// The players keep unspent mana of the type (every type when absent) as steps and phases end (CR 500.4).
    KeepUnspentMana(Vec<PlayerId>, Option<ManaType>),
    /// Unspent mana the players would lose becomes mana of the type instead.
    UnspentManaBecomes(Vec<PlayerId>, ManaType),
    /// The players draw no more than this many cards each turn.
    DrawLimit(Vec<PlayerId>, u32),
    /// The players may pay 2 life rather than each mana symbol of the color in a cost.
    PayLifeForMana(
        Vec<PlayerId>,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))] mtg_core::color::Color,
    ),
    /// The players may pay 2 life as they cast a spell the selector picks to have it cost one symbol of the color less.
    PayLifeForOneSymbol {
        /// The players.
        players: Vec<PlayerId>,
        /// The color of the symbol.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        color: mtg_core::color::Color,
        /// The spells it is for.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        spells: Selector,
    },
    /// Spells and abilities of the players' opponents can't make them sacrifice permanents.
    OpponentsCantMakeSacrifice(Vec<PlayerId>),
    /// Spells and abilities of the players' opponents can't make them discard cards or sacrifice permanents.
    OpponentsCantMakeDiscardOrSacrifice(Vec<PlayerId>),
    /// The players may spend mana as though it were mana of any color or type (CR 609.4b), to cast the spells the selector
    /// picks or for everything when there is none.
    SpendManaAs {
        /// The players.
        players: Vec<PlayerId>,
        /// Any color or any type.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        spend_as: mtg_cards::modify::SpendAs,
        /// The spells it is for; everything when absent.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        spells: Option<Selector>,
        /// With `spells`, the permanents whose abilities it is also for.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        abilities: Option<Selector>,
    },
    /// The players play with the top card of their library revealed.
    RevealTop(Vec<PlayerId>),
    /// The players play with their hands revealed to every seat.
    RevealHand(Vec<PlayerId>),
    /// The rule's controller controls the players while they search their libraries (CR 722).
    ControlSearches(Vec<PlayerId>),
    /// The players exile each card they find while they search their libraries, and the rule's controller may play
    /// those cards while they remain exiled (Opposition Agent).
    ExileFound(Vec<PlayerId>),
    /// The players have no maximum hand size (CR 402.2).
    NoMaxHandSize(Vec<PlayerId>),
    /// The players may activate each of their planeswalkers' loyalty abilities twice each turn (CR 606.3).
    LoyaltyTwice(Vec<PlayerId>),
    /// The players may pay the cost rather than the echo cost of each permanent they control (CR 702.30).
    EchoRather {
        /// The players it covers.
        players: Vec<PlayerId>,
        /// The cost they may pay instead.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        cost: mtg_cards::cost::Cost,
    },
    /// The controller reveals the first card they draw each turn (only on their turns with `your_turns`, only when
    /// they choose to with `may`).
    RevealFirstDraw {
        /// Whether the reveal is optional.
        may: bool,
        /// Whether it works only on the controller's turns.
        your_turns: bool,
    },
    /// Creatures the players control can boast twice during each of their turns (CR 702.142c).
    BoastTwice(Vec<PlayerId>),
    /// The players may activate loyalty abilities of planeswalkers they control whenever they could cast an instant.
    LoyaltyAnyTime(Vec<PlayerId>),
    /// Each player's loyalty abilities cost this many more loyalty counters to activate.
    LoyaltyCostsMore(Vec<(PlayerId, u32)>),
    /// The players' maximum hand size is this many cards (CR 402.2).
    MaxHandSize(Vec<(PlayerId, u32)>),
    /// The players' maximum hand size changes by this many cards, after any size that is set (CR 402.2).
    HandSizeChange(Vec<(PlayerId, i64)>),
    /// The legend rule does not apply to the players' permanents (CR 704.5j).
    NoLegendRule(Vec<PlayerId>),
    /// The legend rule does not apply to the permanents the selector picks, judged for the rule's controller (CR
    /// 704.5j).
    NoLegendRuleFor(#[cfg_attr(feature = "ts", ts(type = "unknown"))] Selector),
    /// The controller may activate abilities of the creatures the selector picks as though they had haste (CR 302.6).
    AbilitiesAsThoughHaste(#[cfg_attr(feature = "ts", ts(type = "unknown"))] Selector),
    /// The permanents the selector picks that the rule's controller controls untap during the untap steps of the players
    /// (CR 502.3).
    UntapDuring {
        /// The permanents, judged when the step untaps.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        what: Selector,
        /// The players whose untap steps untap them.
        whose: Vec<PlayerId>,
    },
    /// The players can't untap more than `count` of their permanents the selector picks during their untap steps (CR
    /// 502.3).
    UntapLimit {
        /// The players whose untap steps are limited.
        who: Vec<PlayerId>,
        /// The permanents counted, judged when the step untaps.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        what: Selector,
        /// The most of them that untap.
        count: u32,
    },
    /// The players skip their draw steps (CR 614.10).
    SkipDrawStep(Vec<PlayerId>),
    /// The players may activate exhaust abilities again during their turns until they activate one.
    R6007ExhaustAgain(Vec<PlayerId>),
    /// The players may look at the top card of their library any time.
    LookAtTop(Vec<PlayerId>),
    /// The players may look at the face-down cards in exile the selector picks (CR 406.3).
    LookAtExiled {
        /// The players who may look.
        who: Vec<PlayerId>,
        /// The cards, judged when a seat looks; every face-down card in exile when empty.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        what: Selector,
        /// Only these objects, as they were named when the rule began; every card `what` selects when absent.
        only: Option<Vec<ObjectRef>>,
    },
    /// The players may look at the face-down permanents the selector picks any time (CR 708.5).
    LookAtFaceDown {
        /// The players who may look.
        who: Vec<PlayerId>,
        /// The permanents, judged when a seat looks; every face-down permanent when empty.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        what: Selector,
    },
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
        /// With `once`, once each turn on every player's turn.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(optional, as = "Option<bool>"))]
        any_turn: bool,
        /// What the caster pays in addition to the other costs of a cast through it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional, type = "unknown"))]
        additional: Option<Box<Cost>>,
        /// What a cast through it pays rather than the mana cost; an amount in it counts the card cast.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional, type = "unknown"))]
        rather: Option<Box<Cost>>,
        /// How the mana of a cast through it may be spent.
        #[serde(default, skip_serializing_if = "SpendAs::is_normal")]
        #[cfg_attr(feature = "ts", ts(optional, type = "unknown"))]
        spend_as: SpendAs,
        /// The permission is open only while this holds for the controller of the rule; always when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional, type = "unknown"))]
        when: Option<Box<Condition>>,
        /// It opens no zone: its free cast or cost is offered for the casts the players may make anyway.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(optional, as = "Option<bool>"))]
        any_cast: bool,
        /// What a spell cast through it gets ("If you cast a spell this way, that artifact enters tapped").
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(optional, type = "unknown"))]
        riders: Vec<CastRider>,
        /// At most this many of the `only` objects may be cast or played through it within the turn.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        limit: Option<u32>,
    },
    /// Each owner may cast the exiled card paired with them for `cost` rather than its mana cost (CR 118.9, 601.2f), while it
    /// stays exiled.
    MayCastRather {
        /// The owner of each card and the card, as it was named when the rule began.
        casts: Vec<(PlayerId, ObjectRef)>,
        /// The mana paid in place of the mana cost.
        cost: ManaCost,
    },
    /// A triggered ability of a permanent the rule's controller controls triggers an additional time when the event
    /// the pattern describes causes it.
    TriggerAgain {
        /// The event that causes the trigger; every event when absent.
        #[cfg_attr(feature = "ts", ts(optional, type = "unknown"))]
        cause: Option<Box<TriggerPattern>>,
        /// Only abilities of the permanents it selects trigger again; every permanent of the controller when absent.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        source: Option<Selector>,
    },
    /// The next spell its controller casts that `what` selects gets the benefit; the cast ends the rule.
    NextSpell {
        /// The cards the cast must match, judged when it is cast; every card when empty.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        what: Selector,
        /// What the spell gets; a cost change counts its times (`per`) as a fixed number.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        benefit: mtg_cards::modify::SpellBenefit,
    },
    /// The spells `applies_to` selects cost `mana` more or less to cast (CR 601.2f); "you" is the rule's controller.
    SpellCost {
        /// The spells, judged as they are cast.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        applies_to: Selector,
        /// More or less.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        change: mtg_cards::ability::CostChange,
        /// The mana added or removed.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        mana: mtg_cards::mana::ManaCostX,
        /// How many times, counted as each spell is cast.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        #[serde(
            default = "mtg_cards::value::Amount::one",
            skip_serializing_if = "mtg_cards::value::Amount::is_one"
        )]
        per: mtg_cards::value::Amount,
    },
    /// Damage from the sources can't be prevented (CR 614.17); every source when the selector is empty.
    DamageCantBePrevented {
        /// The sources, judged when the damage would be dealt.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        source: Selector,
        /// Only combat damage (`true`) or only other damage (`false`); both when absent.
        combat: Option<bool>,
    },
    /// A prevention shield (CR 615): it replaces damage with nothing, up to an amount, until it is used up or ends.
    PreventDamage(Box<DamageShield>),
    /// A regeneration shield (CR 701.19): the next time the permanent would be destroyed, it regenerates instead.
    Regenerate(ObjectRef),
    /// A shield over deaths (CR 614): each of the permanents or spells, fixed when it was made, that would go from the
    /// battlefield or the stack to a graveyard, or with `leaves` from the battlefield to any other zone, goes to the zone
    /// instead ("if that creature would die this turn, exile it instead", "if it would leave the battlefield, exile it
    /// instead").
    DiesInstead {
        /// The permanents and spells it covers.
        objects: Vec<ObjectRef>,
        /// Where they go instead.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        to: ZoneKind,
        /// The place in its owner's library a countered spell goes to, when `to` is the library.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(type = "unknown", optional))]
        place: Option<Place>,
        /// It covers a spell that is countered only, not one that resolves or fizzles.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
        countered: bool,
        /// A card named in another zone is covered as the spell it becomes when cast, one zone change on (CR 400.7).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
        cast: bool,
        /// It covers any move from the battlefield ("if it would leave the battlefield"), not only one to a graveyard.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
        leaves: bool,
    },
    /// A shield over any event of a kind, judged when the event would happen: "if a creature would die this turn, exile it
    /// instead", "permanents enter tapped this turn". The controller and source of the effect that made it read its references.
    Replaces {
        /// The event it replaces.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        when: ReplaceWhen,
        /// What happens instead.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        by: ReplaceBy,
        /// The players a draw shield covers, fixed as it began ("if target player would draw a card"); empty when its
        /// event names them.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "ts", ts(optional, as = "Option<Vec<PlayerId>>"))]
        players: Vec<PlayerId>,
    },
    /// Cards in graveyards can't be the targets of spells or abilities (CR 115.4).
    GraveyardCardsUntargetable,
    /// All damage is dealt as though its source had wither (CR 702.80).
    DamageAsWither,
    /// A player losing unspent mana loses that much life (mana burn).
    ManaBurn,
    /// Creatures can attack the players and their planeswalkers as though they had haste (CR 302.6).
    AttackAsThoughHasteAt(Vec<PlayerId>),
    /// The players can't lose the game, and the other players can't win it (CR 104.3).
    CantLoseGame {
        /// The players who can't lose.
        who: Vec<PlayerId>,
        /// The players who can't win.
        winless: Vec<PlayerId>,
        /// Only the loss for having 0 or less life does not happen (CR 704.5a).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        life_only: bool,
    },
    /// The players can't cast spells, only during this player's turns when one is named.
    CantCastSpells {
        /// The players who can't cast spells.
        who: Vec<PlayerId>,
        /// The player during whose turns they can't; every turn when absent.
        turn: Option<PlayerId>,
    },
    /// Each creature the players control assigns combat damage equal to its toughness (CR 510.1a).
    ToughnessAssignsDamage(Vec<PlayerId>),
    /// Lethal damage to each creature the players control is determined by its power (CR 704.5g): `GameRule::LethalByPower`.
    LethalByPower(Vec<PlayerId>),
    /// All damage dealt to the players is dealt as though its source had infect: `GameRule::DamageAsInfect`.
    DamageAsInfect(Vec<PlayerId>),
    /// The players can't venture more than `per_turn` times each turn: `GameRule::VentureLimit`.
    VentureLimit {
        /// Who is limited.
        who: Vec<PlayerId>,
        /// The ventures each turn.
        per_turn: u32,
    },
    /// The players' devotion is greater by `by`: `GameRule::DevotionIncreased`.
    DevotionIncreased {
        /// Whose devotion.
        who: Vec<PlayerId>,
        /// By how much.
        by: u32,
    },
    /// The players and objects can't do the action (CR 101.2): `GameRule::Cant`.
    Cant {
        /// The players and objects it is about.
        who: Vec<Target>,
        /// What they can't do.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        action: Box<Prohibited>,
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
    /// The kind of object it protects ("creatures you control"), judged when the damage would be dealt (CR 611.2c);
    /// no limit when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "unknown", optional))]
    pub each: Option<Selector>,
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
    /// What happens as it prevents damage, reading the damage as `ReplaceBy::PreventAnd` does; nothing when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(type = "unknown", optional))]
    pub then: Vec<mtg_cards::effect::Effect>,
    /// It prevents the next damage event it covers and ends ("the next time a source of your choice would deal damage").
    #[serde(default)]
    pub once: bool,
    /// What it does to the damage it covers in place of preventing it: scales it or sends it elsewhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "unknown", optional))]
    pub instead: Option<mtg_cards::trigger::ReplaceBy>,
    /// The player or object the damage it redirects is dealt to, fixed when it was made (`ReplaceBy::RedirectUpTo`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub redirect: Option<Target>,
    /// The targets of the ability that made it, by the names its effects read them under, fixed when it was made
    /// ("if damage is prevented this way, ~ deals that much damage to any target").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub bound: Vec<(String, Vec<Target>)>,
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
            || self.each.is_none()
                && self.to.as_ref().is_some_and(|to| {
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
            type = "\"UntilEndOfTurn\" | \"UntilEndOfCombat\" | \"UntilYourNextTurn\" | \"UntilYourNextEndStep\" | \"UntilEndOfYourNextTurn\" | \"UntilEndOfCombatOnYourNextTurn\" | \"UntilItsControllersNextCombatEnds\" | \"WhileSourceOnBattlefield\" | \"Forever\""
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
            &self.duration,
            self.turn,
            self.controller,
            self.source,
        ) && !self.spent(state)
            && (self.duration != Duration::WhileTapped || self.r5509_stays_tapped(state))
    }

    /// Whether every object a prohibition names is still tapped on the battlefield (`Duration::WhileTapped`); a rule
    /// of any other kind names no object to follow.
    fn r5509_stays_tapped(&self, state: &GameState) -> bool {
        let Rule::Cant { who, .. } = &self.rule else {
            return false;
        };
        let named: Vec<ObjectRef> = who
            .iter()
            .filter_map(|subject| match subject {
                Target::Object(named) => Some(*named),
                Target::Player(_) => None,
            })
            .collect();
        crate::effects::r5509_stays_tapped(state, &named)
    }

    /// Whether every object the rule names has become a new object or is gone: it can never apply to anything again.
    fn spent(&self, state: &GameState) -> bool {
        let gone = |r: &ObjectRef| state.object(r.id).is_none_or(|o| o.counter() != r.counter);
        match &self.rule {
            Rule::MustAttack(named)
            | Rule::MustBlock(named)
            | Rule::CantBeCountered(named)
            | Rule::CantBeRegenerated(named)
            | Rule::MayCast {
                only: Some(named), ..
            } => named.iter().all(gone),
            Rule::EntersWith { spell, .. } => gone(spell),
            Rule::DiesInstead { objects, cast, .. } => objects.iter().all(|named| {
                // A card named in another zone is the spell on the stack one zone change on (CR 400.7).
                let cast_now = *cast
                    && state
                        .object(named.id)
                        .is_some_and(|o| o.counter() == named.counter + 1);
                gone(named) && !cast_now
            }),
            Rule::MustAttackPlayers(pairs) => pairs.iter().all(|(named, _)| gone(named)),
            Rule::MustAttackAt {
                creatures,
                players,
                at,
                ..
            } => gone(at) || (players.is_empty() && creatures.iter().all(gone)),
            Rule::MustBlockAttacker { blockers, attacker } => {
                gone(attacker) || blockers.iter().all(gone)
            }
            Rule::Combat { creatures, .. } => creatures.iter().all(gone),
            Rule::Regenerate(named) => gone(named),
            // A prohibition about objects alone ends once each has become a new object (CR 400.7).
            Rule::Cant { who, .. } => who.iter().all(|subject| match subject {
                Target::Object(named) => gone(named),
                Target::Player(_) => false,
            }),
            Rule::PreventDamage(shield) => shield.spent(state),
            _ => false,
        }
    }
}

/// What a requirement to attack a planeswalker binds: one creature, or every creature of a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AttackerOf {
    /// The creature named.
    Creature(ObjectRef),
    /// Each creature the player controls.
    Player(PlayerId),
}

/// A trigger that triggers an additional time: whose permanents, and the event that causes it.
#[derive(Debug, Clone)]
pub(crate) struct Again {
    /// The player whose permanents' abilities trigger again.
    pub(crate) controller: PlayerId,
    /// The object that has the rule, for the pattern's `This`.
    pub(crate) source: ObjectRef,
    /// The permanents whose abilities trigger again; every one of the controller when absent.
    pub(crate) from: Option<Selector>,
    /// The event that causes the trigger; every event when absent.
    pub(crate) cause: Option<TriggerPattern>,
}

/// A change to the mana of a tap that a permanent in play or an effect has in force.
#[derive(Debug, Clone)]
pub(crate) struct ManaRule {
    /// The player who controls the rule.
    pub(crate) controller: PlayerId,
    /// The permanent or effect that has the rule.
    pub(crate) source: ObjectRef,
    /// The players whose taps are changed.
    pub(crate) who: Vec<PlayerId>,
    /// The permanents tapped that it covers.
    pub(crate) what: Selector,
    /// The change.
    pub(crate) change: ManaChange,
}

/// The rules in force in a state.
#[derive(Debug, Default)]
pub(crate) struct RulesInForce {
    /// The combat permissions and requirements of `combat_quirks`.
    pub(crate) quirks: crate::combat_quirks::QuirkRules,
    must_attack: BTreeSet<ObjectId>,
    /// The players who must attack with at least one creature, each with the defenders it attacks (any when empty).
    pub(crate) attack_with_one: Vec<(Vec<PlayerId>, Vec<AttackTarget>)>,
    attack_players: Vec<(ObjectId, PlayerId)>,
    /// The planeswalkers creatures attack if able: by creature, and by the player whose creatures do.
    attack_at: Vec<(AttackerOf, ObjectRef)>,
    must_block: BTreeSet<ObjectId>,
    uncounterable: BTreeSet<ObjectId>,
    unregenerable: BTreeSet<ObjectId>,
    no_life_gain: BTreeSet<PlayerId>,
    no_damage_loss: BTreeSet<PlayerId>,
    life_floors: BTreeMap<PlayerId, Vec<i32>>,
    radiation_gains: BTreeSet<PlayerId>,
    no_life_change: BTreeSet<PlayerId>,
    player_keywords: Vec<(PlayerId, Keyword)>,
    no_draw: BTreeSet<PlayerId>,
    draw_bottom: BTreeSet<PlayerId>,
    no_search: BTreeSet<PlayerId>,
    no_own_search: BTreeSet<PlayerId>,
    /// The fewest top cards each player may search of a library (Aven Mindcensor); every card when absent.
    search_top: BTreeMap<PlayerId, u32>,
    /// The kinds of counter that hold a permanent down in its controller's untap step (Freyalise's Winds).
    pub(crate) untap_removes: BTreeSet<CounterKind>,
    /// The changes to the mana of a tap.
    pub(crate) mana: Vec<ManaRule>,
    vote_again: BTreeSet<PlayerId>,
    /// The players who get an additional vote (CR 701.38d), once for each rule that gives one.
    extra_vote: Vec<PlayerId>,
    /// How many rules reverse the turn order; an odd count runs it the other way.
    r376_reversals: usize,
    /// The sides each player may attack only the nearest opponent on, left `true`, one entry for each rule.
    r376_nearest: Vec<bool>,
    skip_extra_turns: BTreeSet<PlayerId>,
    /// One entry for each rule that doubles a player's coins: a player listed twice flips four.
    flip_two: Vec<PlayerId>,
    /// One entry for each rule that adds a die to a player's rolls: a player listed twice rolls two more.
    extra_die: Vec<PlayerId>,
    /// The players who keep unspent mana as steps end, with the type kept (every type when `None`).
    keep_mana: Vec<(PlayerId, Option<ManaType>)>,
    /// The players whose lost mana becomes another type, in timestamp order.
    mana_becomes: Vec<(PlayerId, ManaType)>,
    /// The most cards each listed player may draw in a turn.
    draw_limit: Vec<(PlayerId, u32)>,
    /// The players who may pay 2 life rather than mana of a color.
    life_for_mana: Vec<(PlayerId, mtg_core::color::Color)>,
    /// The players who may pay 2 life for one symbol of a color of the spells a selector picks, once for each rule.
    life_for_one: Vec<(PlayerId, mtg_core::color::Color, Selector)>,
    /// The players whose opponents' spells and abilities can't make them sacrifice.
    spared_sacrifice: BTreeSet<PlayerId>,
    /// The players whose opponents' spells and abilities can't make them discard.
    spared_discard: BTreeSet<PlayerId>,
    /// The players who may spend mana as though it were of any color or type, with the spells it is for.
    spend_mana_as: Vec<(
        PlayerId,
        mtg_cards::modify::SpendAs,
        Option<Selector>,
        Option<Selector>,
    )>,
    reveal_top: BTreeSet<PlayerId>,
    reveal_hand: BTreeSet<PlayerId>,
    /// Who controls each player while they search their library, the latest rule last.
    control_searches: Vec<(PlayerId, PlayerId)>,
    /// Whose searches exile what they find, with the controller and source of each rule that says so.
    exile_found: Vec<(PlayerId, PlayerId, Option<ObjectRef>)>,
    no_hand_limit: BTreeSet<PlayerId>,
    /// The players who may activate each planeswalker's loyalty abilities twice each turn.
    loyalty_twice: BTreeSet<PlayerId>,
    /// The costs each player may pay rather than the echo cost of a permanent they control, in the order the rules began.
    echo_rather: Vec<(PlayerId, mtg_cards::cost::Cost)>,
    /// The reveals of first draws in force: the controller, the source, whether optional, whether on their turns only.
    reveal_first_draw: Vec<(PlayerId, Option<ObjectRef>, bool, bool)>,
    /// The players whose creatures can boast twice during each of their turns.
    pub(crate) boast_twice: BTreeSet<PlayerId>,
    /// The players who may activate loyalty abilities whenever they could cast an instant.
    loyalty_any_time: BTreeSet<PlayerId>,
    /// The loyalty counters each player's loyalty abilities cost in addition.
    loyalty_more: BTreeMap<PlayerId, u32>,
    skip_draw: BTreeSet<PlayerId>,
    /// The players who may activate exhaust abilities again (`R6007ExhaustAgain`).
    exhaust_again: BTreeSet<PlayerId>,
    look_top: BTreeSet<PlayerId>,
    extra_lands: BTreeMap<PlayerId, u32>,
    again: Vec<Again>,
    max_hand: BTreeMap<PlayerId, u32>,
    hand_change: BTreeMap<PlayerId, i64>,
    no_legend: BTreeSet<PlayerId>,
    no_legend_for: Vec<NoLegendFor>,
    /// The creatures whose abilities may be activated as though they had haste, by rule.
    haste_for_abilities: Vec<NoLegendFor>,
    untap_during: Vec<UntapDuring>,
    untap_limits: Vec<crate::rmt4_untap_limit::UntapLimit>,
    unpreventable: Vec<Unpreventable>,
    casts: Vec<Cast>,
    taxes: Vec<Taxing>,
    block_taxes: Vec<crate::block_tax::BlockTaxing>,
    looks: Vec<Look>,
    face_down_looks: Vec<Look>,
    attack_limit: Option<u32>,
    attack_limit_at: BTreeMap<AttackTarget, u32>,
    block_limit: Option<u32>,
    block_limit_each: BTreeMap<PlayerId, u32>,
    /// The prohibitions (`prohibit.rs`).
    pub(crate) cants: Vec<crate::prohibit::Banned>,
    /// The spell cost rules of static abilities, with their controllers and sources; those of resolved effects are read
    /// from the effects (`announce/modifiers.rs`).
    pub(crate) static_spell_costs: Vec<(Rule, PlayerId, ObjectRef)>,
    /// The rules about targets in graveyards, losing, casting and combat damage.
    pub(crate) w17b: crate::w17b_statics::W17bRules,
    /// All damage is dealt as though its source had wither.
    pub(crate) damage_as_wither: bool,
    /// Players losing unspent mana lose that much life.
    pub(crate) mana_burn: bool,
    /// The players creatures can attack as though they had haste.
    pub(crate) haste_at: BTreeSet<PlayerId>,
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

/// A permission to look at face-down cards in exile, with whose it is.
#[derive(Debug, Clone)]
pub(crate) struct Look {
    /// The controller of the effect or permanent that gives it, for the selector's "you".
    controller: PlayerId,
    /// The object that gives it.
    source: ObjectRef,
    /// The players who may look.
    who: Vec<PlayerId>,
    /// The cards they may look at; every face-down card in exile when empty.
    what: Selector,
    /// Only these objects; every card `what` selects when absent.
    only: Option<Vec<ObjectRef>>,
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
    /// With `once`, once each turn on every player's turn.
    pub(crate) any_turn: bool,
    /// What the caster pays in addition to the other costs of a cast through it.
    pub(crate) additional: Option<Cost>,
    /// How the mana of a cast through it may be spent.
    pub(crate) spend_as: SpendAs,
    /// The permission is open only while this holds for the controller.
    pub(crate) when: Option<Condition>,
    /// The mana paid in place of the mana cost of the card.
    pub(crate) rather: Option<ManaCost>,
    /// What is paid in place of the mana cost of the card, its amounts counted for the card cast.
    pub(crate) instead: Option<Cost>,
    /// It opens no zone: it covers only a card its player may cast anyway.
    pub(crate) opens_no_zone: bool,
    /// What a spell cast through it gets.
    pub(crate) riders: Vec<CastRider>,
    /// At most this many of the `only` objects may be cast or played through it within the turn.
    pub(crate) limit: Option<u32>,
}

impl Cast {
    /// Whether a cast through the permission from `zone` is for the printed mana cost, with what the permission adds: not a free
    /// cast and not one for another mana cost.
    pub(crate) fn adds_to_printed(&self, zone: ZoneKind) -> bool {
        !self.free && self.rather.is_none() && self.instead.is_none() && self.from == Some(zone)
    }

    /// Whether the permission lets `player` cast the card `card`, which is in its zone now.
    pub(crate) fn covers(&self, state: &GameState, player: PlayerId, card: &GameObject) -> bool {
        (!self.opens_no_zone || r305c_cast_anyway(state, player, card))
            && self.c11b_open(state)
            && self.r5503_open(state)
            && self.when_holds(state)
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

    /// Whether the condition of the permission holds for its controller now; a condition that cannot be read does not.
    fn when_holds(&self, state: &GameState) -> bool {
        self.when.as_ref().is_none_or(|condition| {
            Scope::announcing(state, self.controller, self.source)
                .holds(condition)
                .unwrap_or(false)
        })
    }

    /// Whether a permission that works once each turn is open: it is its controller's turn (any turn with `any_turn`) and
    /// no spell was cast through it this turn (`Tally::C11bOnceUsed` of its source). Any other permission is always open.
    fn c11b_open(&self, state: &GameState) -> bool {
        !self.once
            || ((self.any_turn || state.is_active(self.controller))
                && state.object_tallied(self.source, None, Tally::C11bOnceUsed) == 0)
    }

    /// Whether a permission with a limit has room for another card this turn (`Tally::R5503LimitUsed` of its source,
    /// keyed by the first object it names). Any other permission always has.
    fn r5503_open(&self, state: &GameState) -> bool {
        self.limit.is_none_or(|limit| {
            state.object_tallied(self.source, None, Tally::R5503LimitUsed(self.r5503_key())) < limit
        })
    }

    /// The object that keys the uses of a limited permission: the first object it names, else its source.
    fn r5503_key(&self) -> ObjectRef {
        self.only
            .as_ref()
            .and_then(|only| only.first().copied())
            .unwrap_or(self.source)
    }

    /// Whether the permission lets `player` play the land `card` from the zone it is in.
    pub(crate) fn plays(&self, state: &GameState, player: PlayerId, card: &GameObject) -> bool {
        self.play && self.covers(state, player, card)
    }
}

/// Permanents the legend rule does not apply to: whose rule it is and which permanents.
#[derive(Debug, Clone)]
pub(crate) struct NoLegendFor {
    /// The player the selector is judged for.
    pub(crate) controller: PlayerId,
    /// The object that has the rule.
    pub(crate) source: ObjectRef,
    /// The permanents.
    pub(crate) what: Selector,
}

/// Permanents that untap in other untap steps: whose rule it is, which permanents, and whose steps.
#[derive(Debug, Clone)]
pub(crate) struct UntapDuring {
    /// The player whose permanents untap.
    pub(crate) controller: PlayerId,
    /// The object that has the rule.
    pub(crate) source: ObjectRef,
    /// The permanents, among those the controller controls.
    pub(crate) what: Selector,
    /// The players whose untap steps untap them.
    pub(crate) whose: Vec<PlayerId>,
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
    /// Keeps the least limit on the attacks at each of the defenders.
    fn w14a_limit_attacks_at(&mut self, most: u32, at: &[AttackTarget]) {
        for target in at {
            let least = self
                .attack_limit_at
                .get(target)
                .map_or(most, |was| (*was).min(most));
            self.attack_limit_at.insert(*target, least);
        }
    }

    /// Keeps the least limit on the blocks of each of the players.
    fn w14a_limit_blocks(&mut self, most: u32, who: &[PlayerId]) {
        for player in who {
            let least = self
                .block_limit_each
                .get(player)
                .map_or(most, |was| (*was).min(most));
            self.block_limit_each.insert(*player, least);
        }
    }

    /// Puts a rule about hands, the legend rule or the untap step in force.
    fn put_option(&mut self, rule: &Rule, controller: PlayerId, source: Option<ObjectRef>) {
        match rule {
            Rule::MaxHandSize(sizes) => {
                for (player, size) in sizes {
                    let least = self
                        .max_hand
                        .get(player)
                        .map_or(*size, |was| (*was).min(*size));
                    self.max_hand.insert(*player, least);
                }
            }
            Rule::HandSizeChange(changes) => {
                for (player, by) in changes {
                    *self.hand_change.entry(*player).or_insert(0) += by;
                }
            }
            Rule::NoLegendRule(players) => self.no_legend.extend(players),
            Rule::TriggerAgain {
                cause,
                source: from,
            } => {
                if let Some(source) = source {
                    self.again.push(Again {
                        controller,
                        source,
                        from: from.clone(),
                        cause: cause.as_deref().cloned(),
                    });
                }
            }
            Rule::AbilitiesAsThoughHaste(what) => {
                if let Some(source) = source {
                    self.haste_for_abilities.push(NoLegendFor {
                        controller,
                        source,
                        what: what.clone(),
                    });
                }
            }
            Rule::NoLegendRuleFor(what) => {
                if let Some(source) = source {
                    self.no_legend_for.push(NoLegendFor {
                        controller,
                        source,
                        what: what.clone(),
                    });
                }
            }
            Rule::UntapDuring { what, whose } => {
                if let Some(source) = source {
                    self.untap_during.push(UntapDuring {
                        controller,
                        source,
                        what: what.clone(),
                        whose: whose.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    /// Opens the flash or free cast of a `NextSpell` rule; costs and keywords are read from the rule when a spell is
    /// announced.
    fn put_next_spell(
        &mut self,
        what: &Selector,
        benefit: &mtg_cards::modify::SpellBenefit,
        controller: PlayerId,
        source: Option<ObjectRef>,
    ) {
        use mtg_cards::modify::SpellBenefit;
        let (free, flash) = match benefit {
            SpellBenefit::Free => (true, false),
            SpellBenefit::Flash => (false, true),
            // "Rather than pay the mana cost of the next spell you cast this turn, you may pay ...": a cast for that cost.
            SpellBenefit::Rather(cost) => {
                if let Some(source) = source {
                    self.casts.push(Cast {
                        controller,
                        source,
                        who: vec![controller],
                        what: what.clone(),
                        from: None,
                        free: false,
                        flash: false,
                        only: None,
                        play: false,
                        once: false,
                        any_turn: false,
                        additional: None,
                        spend_as: SpendAs::Normal,
                        when: None,
                        rather: None,
                        instead: Some(cost.clone()),
                        opens_no_zone: false,
                        riders: Vec::new(),
                        limit: None,
                    });
                }
                return;
            }
            SpellBenefit::Cost { .. } | SpellBenefit::Keyword(_) | SpellBenefit::Uncounterable => {
                return;
            }
        };
        if let Some(source) = source {
            self.casts.push(Cast {
                controller,
                source,
                who: vec![controller],
                what: what.clone(),
                from: None,
                free,
                flash,
                only: None,
                play: false,
                once: false,
                any_turn: false,
                additional: None,
                spend_as: SpendAs::Normal,
                when: None,
                rather: None,
                instead: None,
                opens_no_zone: false,
                riders: Vec::new(),
                limit: None,
            });
        }
    }

    /// Puts a rule in force. `controller` and `source` are those of the effect or the permanent that has the rule.
    #[allow(clippy::too_many_lines)]
    fn put(&mut self, rule: &Rule, controller: PlayerId, source: Option<ObjectRef>) {
        let ids = |named: &[ObjectRef]| named.iter().map(|r| r.id).collect::<Vec<_>>();
        match rule {
            Rule::MustAttack(named) => self.must_attack.extend(ids(named)),
            Rule::MustAttackWithOne { players, at } => {
                self.attack_with_one.push((players.clone(), at.clone()));
            }
            Rule::MustAttackPlayers(pairs) => self
                .attack_players
                .extend(pairs.iter().map(|(named, player)| (named.id, *player))),
            Rule::MustAttackAt {
                creatures,
                players,
                at,
                ..
            } => {
                self.attack_at.extend(
                    creatures
                        .iter()
                        .map(|named| (AttackerOf::Creature(*named), *at))
                        .chain(
                            players
                                .iter()
                                .map(|player| (AttackerOf::Player(*player), *at)),
                        ),
                );
            }
            Rule::MustBlock(named) => self.must_block.extend(ids(named)),
            Rule::MustBlockAttacker { .. } | Rule::Combat { .. } | Rule::IgnoreHexproof { .. } => {
                self.quirks.put(rule);
            }
            Rule::AttackTax(tax) => self.taxes.push(Taxing {
                controller,
                source,
                tax: tax.clone(),
            }),
            Rule::BlockTax(tax) => self.block_taxes.push(crate::block_tax::BlockTaxing {
                controller,
                source,
                tax: tax.clone(),
            }),
            Rule::AttackLimit(most) => {
                self.attack_limit = Some(self.attack_limit.map_or(*most, |was| was.min(*most)));
            }
            Rule::AttackLimitAt { most, at } => self.w14a_limit_attacks_at(*most, at),
            Rule::BlockLimit(most) => {
                self.block_limit = Some(self.block_limit.map_or(*most, |was| was.min(*most)));
            }
            Rule::BlockLimitEach { most, who } => self.w14a_limit_blocks(*most, who),
            Rule::CantBeCountered(named) => self.uncounterable.extend(ids(named)),
            Rule::CantBeRegenerated(named) => self.unregenerable.extend(ids(named)),
            Rule::CantGainLife(players) => self.no_life_gain.extend(players),
            Rule::NoLifeLossFromDamage(players) => self.no_damage_loss.extend(players),
            Rule::LifeFloor { players, floor } => {
                for player in players {
                    self.life_floors.entry(*player).or_default().push(*floor);
                }
            }
            Rule::RadiationGainsLife(players) => self.radiation_gains.extend(players),
            Rule::LifeTotalCantChange(players) => self.no_life_change.extend(players),
            Rule::PlayersHave { who, keyword } => self
                .player_keywords
                .extend(who.iter().map(|player| (*player, keyword.clone()))),
            Rule::CantDraw(players) => self.no_draw.extend(players),
            Rule::DrawFromBottom(players) => self.draw_bottom.extend(players),
            Rule::CantSearch(players) => self.no_search.extend(players),
            Rule::OwnSpellsCantSearch(players) => self.no_own_search.extend(players),
            Rule::UntapRemovesCounters(kind) => {
                self.untap_removes.insert(kind.clone());
            }
            Rule::SearchTop { players, count } => {
                for player in players {
                    let top = self.search_top.entry(*player).or_insert(*count);
                    *top = (*top).min(*count);
                }
            }
            Rule::ManaProduced { what, who, change } => {
                if let Some(source) = source {
                    self.mana.push(ManaRule {
                        controller,
                        source,
                        who: who.clone(),
                        what: what.clone(),
                        change: *change,
                    });
                }
            }
            Rule::VoteAgain(players) => self.vote_again.extend(players),
            Rule::ExtraVote(players) => self.extra_vote.extend(players),
            Rule::ReverseTurnOrder => self.r376_reversals += 1,
            Rule::AttackOnlyNearest(left) => self.r376_nearest.extend(left),
            Rule::SkipExtraTurns(players) => self.skip_extra_turns.extend(players),
            Rule::FlipTwoCoins(players) => self.flip_two.extend(players),
            Rule::ExtraDie(players) => self.extra_die.extend(players),
            Rule::KeepUnspentMana(players, mana) => {
                self.keep_mana.extend(players.iter().map(|p| (*p, *mana)));
            }
            Rule::SpendManaAs {
                players,
                spend_as,
                spells,
                abilities,
            } => {
                self.spend_mana_as.extend(
                    players
                        .iter()
                        .map(|p| (*p, *spend_as, spells.clone(), abilities.clone())),
                );
            }
            Rule::DrawLimit(players, most) => {
                self.draw_limit.extend(players.iter().map(|p| (*p, *most)));
            }
            Rule::OpponentsCantMakeSacrifice(players) => self.spared_sacrifice.extend(players),
            Rule::OpponentsCantMakeDiscardOrSacrifice(players) => {
                self.spared_sacrifice.extend(players);
                self.spared_discard.extend(players);
            }
            Rule::PayLifeForOneSymbol {
                players,
                color,
                spells,
            } => {
                self.life_for_one
                    .extend(players.iter().map(|p| (*p, *color, spells.clone())));
            }
            Rule::PayLifeForMana(players, color) => {
                self.life_for_mana
                    .extend(players.iter().map(|p| (*p, *color)));
            }
            Rule::UnspentManaBecomes(players, into) => {
                self.mana_becomes
                    .extend(players.iter().map(|p| (*p, *into)));
            }
            Rule::RevealTop(players) => self.reveal_top.extend(players),
            Rule::RevealHand(players) => self.reveal_hand.extend(players),
            Rule::ExileFound(players) => self
                .exile_found
                .extend(players.iter().map(|player| (*player, controller, source))),
            Rule::ControlSearches(players) => self
                .control_searches
                .extend(players.iter().map(|player| (*player, controller))),
            Rule::NoMaxHandSize(players) => self.no_hand_limit.extend(players),
            Rule::LoyaltyTwice(players) => self.loyalty_twice.extend(players),
            Rule::EchoRather { players, cost } => self
                .echo_rather
                .extend(players.iter().map(|player| (*player, cost.clone()))),
            Rule::RevealFirstDraw { may, your_turns } => {
                self.reveal_first_draw
                    .push((controller, source, *may, *your_turns));
            }
            Rule::BoastTwice(players) => self.boast_twice.extend(players),
            Rule::LoyaltyAnyTime(players) => self.loyalty_any_time.extend(players),
            Rule::LoyaltyCostsMore(more) => {
                for (player, plus) in more {
                    let held = self.loyalty_more.entry(*player).or_insert(0);
                    *held = held.saturating_add(*plus);
                }
            }
            Rule::MaxHandSize(_)
            | Rule::HandSizeChange(_)
            | Rule::NoLegendRule(_)
            | Rule::NoLegendRuleFor(_)
            | Rule::AbilitiesAsThoughHaste(_)
            | Rule::UntapDuring { .. } => {
                self.put_option(rule, controller, source);
            }
            Rule::SkipDrawStep(players) => self.skip_draw.extend(players),
            Rule::R6007ExhaustAgain(players) => self.exhaust_again.extend(players),
            Rule::UntapLimit { who, what, count } => {
                if let Some(source) = source {
                    self.untap_limits.push(crate::rmt4_untap_limit::UntapLimit {
                        controller,
                        source,
                        who: who.clone(),
                        what: what.clone(),
                        count: *count,
                    });
                }
            }
            Rule::LookAtTop(players) => self.look_top.extend(players),
            Rule::LookAtExiled { who, what, only } => {
                if let Some(source) = source {
                    self.looks.push(Look {
                        controller,
                        source,
                        who: who.clone(),
                        what: what.clone(),
                        only: only.clone(),
                    });
                }
            }
            Rule::LookAtFaceDown { who, what } => {
                if let Some(source) = source {
                    self.face_down_looks.push(Look {
                        controller,
                        source,
                        who: who.clone(),
                        what: what.clone(),
                        only: None,
                    });
                }
            }
            Rule::AdditionalLandPlays(plays) => {
                for (player, count) in plays {
                    let total = self.extra_lands.entry(*player).or_default();
                    *total = total.saturating_add(*count);
                }
            }
            Rule::TriggerAgain { .. } => self.put_option(rule, controller, source),
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
                any_turn,
                additional,
                rather,
                spend_as,
                when,
                any_cast,
                riders,
                limit,
            } => {
                if let Some(source) = source {
                    self.casts.push(Cast {
                        limit: *limit,
                        riders: riders.clone(),
                        opens_no_zone: *any_cast,
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
                        any_turn: *any_turn,
                        additional: additional.as_deref().cloned(),
                        spend_as: *spend_as,
                        when: when.as_deref().cloned(),
                        rather: None,
                        instead: rather.as_deref().cloned(),
                    });
                }
            }
            Rule::MayCastRather { casts, cost } => {
                if let Some(source) = source {
                    for (owner, card) in casts {
                        self.casts.push(Cast {
                            controller,
                            source,
                            who: vec![*owner],
                            what: Selector::default(),
                            from: Some(ZoneKind::Exile),
                            free: false,
                            flash: false,
                            only: Some(vec![*card]),
                            play: false,
                            once: false,
                            any_turn: false,
                            additional: None,
                            spend_as: SpendAs::Normal,
                            when: None,
                            rather: Some(cost.clone()),
                            instead: None,
                            opens_no_zone: false,
                            riders: Vec::new(),
                            limit: None,
                        });
                    }
                }
            }
            Rule::NextSpell { what, benefit } => {
                self.put_next_spell(what, benefit, controller, source);
            }
            Rule::Cant { who, action } => self.cants.push(crate::prohibit::Banned::new(
                who, action, controller, source,
            )),
            // Shields are read by the replacement step, spell costs by the cost step and the bonuses of a spell's mana
            // where it resolves (`spend_rider::entering`), from the rule changes themselves.
            Rule::EntersWith { .. }
            | Rule::Planar
            | Rule::SpellCost { .. }
            | Rule::PreventDamage(_)
            | Rule::Regenerate(_)
            | Rule::DiesInstead { .. }
            | Rule::Replaces { .. } => {}
            Rule::DamageAsWither => self.damage_as_wither = true,
            Rule::ManaBurn => self.mana_burn = true,
            Rule::AttackAsThoughHasteAt(players) => {
                self.haste_at
                    .extend(crate::r303b_haste_at::r303b_named(players));
            }
            Rule::GraveyardCardsUntargetable
            | Rule::CantLoseGame { .. }
            | Rule::CantCastSpells { .. }
            | Rule::ToughnessAssignsDamage(_)
            | Rule::LethalByPower(_)
            | Rule::DevotionIncreased { .. }
            | Rule::DamageAsInfect(_)
            | Rule::VentureLimit { .. } => self.w17b.w17b_put(rule, source),
        }
    }
}

/// The exiled cards `only` names, each with its owner, to be cast for `cost` rather than their mana costs.
fn may_cast_rather(
    scope: &Scope<'_>,
    only: &Ref,
    cost: &mtg_cards::mana::ManaCostX,
) -> Result<Rule, ResolveError> {
    let casts = w17b_rule_objects(scope, only, Zone::Exile)?
        .into_iter()
        .filter_map(|named| Some((scope.state.object(named.id)?.owner(), named)))
        .collect();
    Ok(Rule::MayCastRather {
        casts,
        cost: cost.rest().clone(),
    })
}

/// The benefit of a `NextSpell` rule with the times of a cost change counted now.
fn next_spell(
    scope: &Scope<'_>,
    what: &Selector,
    benefit: &mtg_cards::modify::SpellBenefit,
) -> Result<Rule, ResolveError> {
    use mtg_cards::modify::SpellBenefit;
    let benefit = match benefit {
        SpellBenefit::Cost { change, mana, per } => SpellBenefit::Cost {
            change: *change,
            mana: mana.clone(),
            per: mtg_cards::value::Amount::Fixed(
                i32::try_from(scope.amount(per)?.clamp(0, 100)).unwrap_or(0),
            ),
        },
        other => other.clone(),
    };
    Ok(Rule::NextSpell {
        what: what.clone(),
        benefit,
    })
}

/// The players and planeswalkers `at` names, as what creatures attack.
fn w10a_defenders(scope: &Scope<'_>, at: &Ref) -> Result<Vec<AttackTarget>, ResolveError> {
    Ok(scope
        .refs(at)?
        .into_iter()
        .map(|named| match named {
            Target::Player(player) => AttackTarget::Player(player),
            Target::Object(object) => AttackTarget::Planeswalker(object.id),
        })
        .collect())
}

/// "Each opponent can't block with more than one creature this combat": the limit and the players it names.
fn w14a_block_limit_each(scope: &Scope<'_>, most: u32, who: &Ref) -> Result<Rule, ResolveError> {
    Ok(Rule::BlockLimitEach {
        most,
        who: scope.players(who)?,
    })
}

/// "No more than N creatures can attack you each combat": the limit and the defenders it names.
fn attack_limit_at(scope: &Scope<'_>, most: u32, at: &Ref) -> Result<Rule, ResolveError> {
    Ok(Rule::AttackLimitAt {
        most,
        at: w10a_defenders(scope, at)?,
    })
}

/// The objects `who` names in `scope` that are in `zone` now: what the source is attached to for `Attached`.
fn w17b_rule_objects(
    scope: &Scope<'_>,
    who: &Ref,
    zone: Zone,
) -> Result<Vec<ObjectRef>, ResolveError> {
    let named = if matches!(who, Ref::Attached) {
        scope.host().into_iter().collect()
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
}

/// The battlefield objects `who` names in `scope`.
pub(crate) fn lower_objects(scope: &Scope<'_>, who: &Ref) -> Result<Vec<ObjectRef>, ResolveError> {
    w17b_rule_objects(scope, who, Zone::Battlefield)
}

/// What the data's references name in `scope`, as the engine plays the rule: the permanents, the spells or the players.
/// A rule the engine does not play refuses by name.
#[allow(clippy::too_many_lines)] // one arm for every rule
pub(crate) fn lower_rule(scope: &Scope<'_>, wanted: &GameRule) -> Result<Rule, ResolveError> {
    let objects = |who: &Ref, zone: Zone| w17b_rule_objects(scope, who, zone);
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
        GameRule::MustAttackAt { who, of, at } => {
            let creatures = match who {
                Some(who) => objects(who, Zone::Battlefield)?,
                None => Vec::new(),
            };
            let players = match of {
                Some(of) => scope.players(of)?,
                None => Vec::new(),
            };
            let mut turn_of = players.clone();
            for creature in &creatures {
                if let Some(controller) =
                    crate::characteristics::controller_of(scope.state, creature.id)
                    && !turn_of.contains(&controller)
                {
                    turn_of.push(controller);
                }
            }
            match objects(at, Zone::Battlefield)?.first() {
                Some(at) => Rule::MustAttackAt {
                    creatures,
                    players,
                    at: *at,
                    turn_of,
                },
                None => Rule::MustAttackAt {
                    creatures: Vec::new(),
                    players: Vec::new(),
                    at: scope.source,
                    turn_of,
                },
            }
        }
        GameRule::MustAttackWithOne { who, at } => Rule::MustAttackWithOne {
            players: scope.players(who)?,
            at: at
                .as_ref()
                .map(|at| w10a_defenders(scope, at))
                .transpose()?
                .unwrap_or_default(),
        },
        GameRule::MustBlock(who) => Rule::MustBlock(objects(who, Zone::Battlefield)?),
        GameRule::MustBlockAttacker { .. }
        | GameRule::Combat { .. }
        | GameRule::IgnoreHexproof { .. } => crate::combat_quirks::lower(scope, wanted)?,
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
        GameRule::BlockTax { .. } => crate::block_tax::lower(scope, wanted)?,
        GameRule::AttackLimit(most) => Rule::AttackLimit(*most),
        GameRule::AttackLimitAt { most, at } => attack_limit_at(scope, *most, at)?,
        GameRule::BlockLimit(most) => Rule::BlockLimit(*most),
        GameRule::BlockLimitEach { most, who } => w14a_block_limit_each(scope, *most, who)?,
        GameRule::CantBeCountered(who) => Rule::CantBeCountered(objects(who, Zone::Stack)?),
        GameRule::CantBeRegenerated(who) => {
            Rule::CantBeRegenerated(objects(who, Zone::Battlefield)?)
        }
        GameRule::CantGainLife(who) => Rule::CantGainLife(scope.players(who)?),
        GameRule::NoLifeLossFromDamage(who) => Rule::NoLifeLossFromDamage(scope.players(who)?),
        GameRule::LifeFloor { who, floor } => Rule::LifeFloor {
            players: scope.players(who)?,
            floor: *floor,
        },
        GameRule::RadiationGainsLife(who) => Rule::RadiationGainsLife(scope.players(who)?),
        GameRule::LifeTotalCantChange(who) => Rule::LifeTotalCantChange(scope.players(who)?),
        GameRule::PlayersHave { who, keyword } => Rule::PlayersHave {
            who: scope.players(who)?,
            keyword: keyword.clone(),
        },
        GameRule::CantDraw(who) => Rule::CantDraw(scope.players(who)?),
        GameRule::DrawFromBottom(who) => Rule::DrawFromBottom(scope.players(who)?),
        GameRule::CantSearch(who) => Rule::CantSearch(scope.players(who)?),
        GameRule::OwnSpellsCantSearch(who) => Rule::OwnSpellsCantSearch(scope.players(who)?),
        GameRule::UntapRemovesCounters(kind) => Rule::UntapRemovesCounters(kind.clone()),
        GameRule::SearchTop { who, count } => Rule::SearchTop {
            players: scope.players(who)?,
            count: *count,
        },
        GameRule::ManaProduced { what, who, change } => Rule::ManaProduced {
            what: what.clone(),
            who: scope.players(who)?,
            change: *change,
        },
        GameRule::VoteAgain(who) => Rule::VoteAgain(scope.players(who)?),
        GameRule::ExtraVote(who) => Rule::ExtraVote(scope.players(who)?),
        GameRule::ReverseTurnOrder => Rule::ReverseTurnOrder,
        GameRule::AttackOnlyNearest(side) => {
            let word = match side {
                mtg_cards::modify::SideOf::Entered => scope.entered_choice(),
                mtg_cards::modify::SideOf::ChosenAs(name) => scope.chosen_value(name),
            };
            Rule::AttackOnlyNearest(match word {
                Some(crate::object::Chosen::Word(word)) if word == "left" => Some(true),
                Some(crate::object::Chosen::Word(word)) if word == "right" => Some(false),
                _ => None,
            })
        }
        GameRule::SkipExtraTurns(who) => Rule::SkipExtraTurns(scope.players(who)?),
        GameRule::FlipTwoCoins(who) => Rule::FlipTwoCoins(scope.players(who)?),
        GameRule::ExtraDie(who) => Rule::ExtraDie(scope.players(who)?),
        GameRule::ExtraPlanarDie(_) | GameRule::PlaneswalkLookTwo(_) => Rule::Planar,
        GameRule::KeepUnspentMana { who, mana } => {
            Rule::KeepUnspentMana(scope.players(who)?, *mana)
        }
        GameRule::SpendManaAs {
            who,
            spend_as,
            spells,
            abilities,
        } => Rule::SpendManaAs {
            players: scope.players(who)?,
            spend_as: *spend_as,
            spells: spells.clone(),
            abilities: abilities.clone(),
        },
        GameRule::DrawLimit { who, most } => Rule::DrawLimit(scope.players(who)?, *most),
        GameRule::PayLifeForMana { who, color } => {
            Rule::PayLifeForMana(scope.players(who)?, *color)
        }
        GameRule::PayLifeForOneSymbol { who, color, spells } => Rule::PayLifeForOneSymbol {
            players: scope.players(who)?,
            color: *color,
            spells: spells.clone(),
        },
        GameRule::OpponentsCantMakeSacrifice(who) => {
            Rule::OpponentsCantMakeSacrifice(scope.players(who)?)
        }
        GameRule::OpponentsCantMakeDiscardOrSacrifice(who) => {
            Rule::OpponentsCantMakeDiscardOrSacrifice(scope.players(who)?)
        }
        GameRule::UnspentManaBecomes { who, into } => {
            Rule::UnspentManaBecomes(scope.players(who)?, *into)
        }
        GameRule::RevealTop(who) => Rule::RevealTop(scope.players(who)?),
        GameRule::RevealHand(who) => Rule::RevealHand(scope.players(who)?),
        GameRule::ControlSearches(who) => Rule::ControlSearches(scope.players(who)?),
        GameRule::ExileFound(who) => Rule::ExileFound(scope.players(who)?),
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
            any_turn: permission.any_turn,
            additional: permission.additional.clone().map(Box::new),
            rather: permission.rather.clone().map(Box::new),
            spend_as: permission.spend_as,
            when: permission.when.clone().map(Box::new),
            any_cast: permission.any_cast,
            riders: permission.riders.clone(),
            limit: permission.limit,
        },
        GameRule::NextSpell { what, benefit } => next_spell(scope, what, benefit)?,
        GameRule::SpellCost {
            applies_to,
            change,
            mana,
            per,
        } => Rule::SpellCost {
            applies_to: applies_to.clone(),
            change: *change,
            mana: mana.clone(),
            per: per.clone(),
        },
        GameRule::MayCastRather { only, cost } => may_cast_rather(scope, only, cost)?,
        GameRule::TriggerAgain { cause, source } => Rule::TriggerAgain {
            cause: cause.clone().map(Box::new),
            source: source.clone(),
        },
        GameRule::MaxHandSize { .. }
        | GameRule::HandSizeChange { .. }
        | GameRule::NoLegendRule(_)
        | GameRule::NoLegendRuleFor(_)
        | GameRule::AbilitiesAsThoughHaste(_)
        | GameRule::UntapDuring { .. } => lower_option(scope, wanted)?,
        GameRule::LookAtTopAnyTime(who) => Rule::LookAtTop(scope.players(who)?),
        GameRule::UntapLimit { who, what, count } => Rule::UntapLimit {
            who: scope.players(who)?,
            what: what.clone(),
            count: *count,
        },
        GameRule::LookAtExiled { who, what, only } => Rule::LookAtExiled {
            who: scope.players(who)?,
            what: what.clone(),
            only: only
                .as_ref()
                .map(|named| scope.objects(named))
                .transpose()?,
        },
        GameRule::LookAtFaceDown { who, what } => Rule::LookAtFaceDown {
            who: scope.players(who)?,
            what: what.clone(),
        },
        GameRule::NoMaxHandSize(who) => Rule::NoMaxHandSize(scope.players(who)?),
        GameRule::LoyaltyTwice(who) => Rule::LoyaltyTwice(scope.players(who)?),
        GameRule::EchoRather(cost) => Rule::EchoRather {
            players: scope.players(&Ref::You)?,
            cost: cost.clone(),
        },
        GameRule::RevealFirstDraw { may, your_turns } => Rule::RevealFirstDraw {
            may: *may,
            your_turns: *your_turns,
        },
        GameRule::BoastTwice(who) => Rule::BoastTwice(scope.players(who)?),
        GameRule::LoyaltyAnyTime(who) => Rule::LoyaltyAnyTime(scope.players(who)?),
        GameRule::LoyaltyCostsMore { who, plus } => Rule::LoyaltyCostsMore(
            scope
                .players(who)?
                .into_iter()
                .map(|player| (player, *plus))
                .collect(),
        ),
        GameRule::SkipDrawStep(who) => Rule::SkipDrawStep(scope.players(who)?),
        GameRule::R6007ExhaustAgain(who) => Rule::R6007ExhaustAgain(scope.players(who)?),
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
        GameRule::Cant { who, action } => {
            let (who, action) = crate::cant_named::lower(scope, scope.refs(who)?, action)?;
            Rule::Cant {
                who,
                action: Box::new(action),
            }
        }
        GameRule::CanBeCommander => return Err(ResolveError::Unsupported("Rule.CanBeCommander")),
        // The deck check reads it from the card; nothing changes in a game.
        GameRule::AnyNumberInDeck => return Err(ResolveError::Unsupported("Rule.AnyNumberInDeck")),
        GameRule::UpToInDeck(_) => return Err(ResolveError::Unsupported("Rule.UpToInDeck")),
        // A draft rule works only during a booster draft; nothing changes in a game.
        GameRule::Draft(_) => return Err(ResolveError::Unsupported("Rule.Draft")),
        GameRule::DamageAsWither => Rule::DamageAsWither,
        GameRule::ManaBurn => Rule::ManaBurn,
        GameRule::AttackAsThoughHasteAt(who) => Rule::AttackAsThoughHasteAt(scope.players(who)?),
        // A copy effect reads it from the spell's card (`copy_spell`); it is no rule of the game.
        GameRule::CantBeCopied => return Err(ResolveError::Unsupported("Rule.CantBeCopied")),
        // The cleanup step reads it from the permanent (`r5516_damage`); it is no rule of the game.
        GameRule::KeepsDamage => return Err(ResolveError::Unsupported("Rule.KeepsDamage")),
        // The announcement reads it from the permanents as targets are chosen (`announce::r5509_flagbearer`).
        GameRule::Flagbearers => return Err(ResolveError::Unsupported("Rule.Flagbearers")),
        // A resolving spell reads it from the card in the graveyard (`Pred::Named`); it is no rule of the game.
        GameRule::CountsAsNamed(_) => return Err(ResolveError::Unsupported("Rule.CountsAsNamed")),
        GameRule::CantSpendMana => return Err(ResolveError::Unsupported("Rule.CantSpendMana")),
        // The cost of the cast reads it from the card (`r5500_tax_life`); it is no rule of the game.
        GameRule::TaxAsLife(_) => return Err(ResolveError::Unsupported("Rule.TaxAsLife")),
        GameRule::SpendOnlyFrom(_) => return Err(ResolveError::Unsupported("Rule.SpendOnlyFrom")),
        GameRule::GraveyardCardsUntargetable
        | GameRule::CantLoseGame { .. }
        | GameRule::CantCastSpells { .. }
        | GameRule::ToughnessAssignsDamage(_)
        | GameRule::LethalByPower(_)
        | GameRule::DevotionIncreased { .. }
        | GameRule::DamageAsInfect(_)
        | GameRule::VentureLimit { .. }
        | GameRule::CountersRemain => crate::w17b_statics::w17b_lower(scope, wanted)?,
    })
}

/// The rules about hands, the legend rule and the untap step, with their players and amounts resolved.
fn lower_option(scope: &Scope<'_>, wanted: &GameRule) -> Result<Rule, ResolveError> {
    Ok(match wanted {
        GameRule::MaxHandSize { who, size } => {
            let size = u32::try_from(scope.amount(size)?.max(0)).unwrap_or(u32::MAX);
            Rule::MaxHandSize(
                scope
                    .players(who)?
                    .into_iter()
                    .map(|player| (player, size))
                    .collect(),
            )
        }
        GameRule::HandSizeChange { who, by } => {
            let by = scope.amount(by)?;
            Rule::HandSizeChange(
                scope
                    .players(who)?
                    .into_iter()
                    .map(|player| (player, by))
                    .collect(),
            )
        }
        GameRule::NoLegendRule(who) => Rule::NoLegendRule(scope.players(who)?),
        GameRule::NoLegendRuleFor(what) => Rule::NoLegendRuleFor(what.clone()),
        GameRule::AbilitiesAsThoughHaste(what) => Rule::AbilitiesAsThoughHaste(what.clone()),
        GameRule::UntapDuring { what, whose } => Rule::UntapDuring {
            what: what.clone(),
            whose: scope.players(whose)?,
        },
        _ => return Err(ResolveError::Unsupported("Rule")),
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
#[allow(clippy::too_many_lines)] // one arm for every rule
pub(crate) fn names_nothing(rule: &Rule) -> bool {
    match rule {
        Rule::AttackTax(tax) => {
            tax.players.is_empty() || (tax.mana.symbols().is_empty() && tax.life == 0)
        }
        Rule::BlockTax(tax) => tax.mana.symbols().is_empty() && tax.life == 0,
        Rule::MustAttack(named)
        | Rule::MustBlock(named)
        | Rule::CantBeCountered(named)
        | Rule::CantBeRegenerated(named) => named.is_empty(),
        Rule::EntersWith {
            counters, keyword, ..
        } => *counters == 0 && keyword.is_none(),
        Rule::MustAttackPlayers(pairs) => pairs.is_empty(),
        Rule::MustAttackAt {
            creatures, players, ..
        } => creatures.is_empty() && players.is_empty(),
        Rule::MustBlockAttacker { blockers, .. } => blockers.is_empty(),
        Rule::Combat { creatures, .. } => creatures.is_empty(),
        Rule::IgnoreHexproof {
            by, what, players, ..
        } => by.is_empty() || (what.is_empty() && players.is_empty()),
        Rule::LifeFloor { players, .. }
        | Rule::MustAttackWithOne { players, .. }
        | Rule::CantGainLife(players)
        | Rule::NoLifeLossFromDamage(players)
        | Rule::RadiationGainsLife(players)
        | Rule::LifeTotalCantChange(players)
        | Rule::CantDraw(players)
        | Rule::DrawFromBottom(players)
        | Rule::CantSearch(players)
        | Rule::OwnSpellsCantSearch(players)
        | Rule::SearchTop { players, .. }
        | Rule::VoteAgain(players)
        | Rule::ExtraVote(players)
        | Rule::SkipExtraTurns(players)
        | Rule::FlipTwoCoins(players)
        | Rule::ExtraDie(players)
        | Rule::KeepUnspentMana(players, _)
        | Rule::UnspentManaBecomes(players, _)
        | Rule::DrawLimit(players, _)
        | Rule::PayLifeForMana(players, _)
        | Rule::PayLifeForOneSymbol { players, .. }
        | Rule::OpponentsCantMakeSacrifice(players)
        | Rule::OpponentsCantMakeDiscardOrSacrifice(players)
        | Rule::SpendManaAs { players, .. }
        | Rule::RevealTop(players)
        | Rule::RevealHand(players)
        | Rule::ControlSearches(players)
        | Rule::ExileFound(players)
        | Rule::LookAtTop(players)
        | Rule::SkipDrawStep(players)
        | Rule::R6007ExhaustAgain(players)
        | Rule::NoLegendRule(players)
        | Rule::NoMaxHandSize(players)
        | Rule::LoyaltyTwice(players)
        | Rule::EchoRather { players, .. }
        | Rule::BoastTwice(players)
        | Rule::LoyaltyAnyTime(players)
        | Rule::AttackAsThoughHasteAt(players) => players.is_empty(),
        Rule::MayCastRather { casts, .. } => casts.is_empty(),
        Rule::MaxHandSize(sizes) => sizes.is_empty(),
        Rule::LoyaltyCostsMore(more) => more.iter().all(|(_, plus)| *plus == 0),
        Rule::HandSizeChange(changes) => changes.is_empty(),
        Rule::UntapDuring { whose, .. } => whose.is_empty(),
        Rule::AttackLimitAt { at, .. } => at.is_empty(),
        Rule::AdditionalLandPlays(plays) => plays.iter().all(|(_, count)| *count == 0),
        Rule::LookAtExiled { who, only, .. } | Rule::MayCast { who, only, .. } => {
            who.is_empty() || only.as_ref().is_some_and(Vec::is_empty)
        }
        Rule::BlockLimitEach { who, .. }
        | Rule::LookAtFaceDown { who, .. }
        | Rule::UntapLimit { who, .. }
        | Rule::PlayersHave { who, .. }
        | Rule::ManaProduced { who, .. } => who.is_empty(),
        Rule::Cant { who, .. } => who.is_empty(),
        Rule::TriggerAgain { .. }
        | Rule::NextSpell { .. }
        | Rule::SpellCost { .. }
        | Rule::DamageCantBePrevented { .. }
        | Rule::Regenerate(_)
        | Rule::Replaces { .. }
        | Rule::AttackLimit(_)
        | Rule::BlockLimit(_)
        | Rule::NoLegendRuleFor(_)
        | Rule::AbilitiesAsThoughHaste(_)
        | Rule::ReverseTurnOrder
        | Rule::DamageAsWither
        | Rule::ManaBurn
        | Rule::RevealFirstDraw { .. }
        | Rule::UntapRemovesCounters(_) => false,
        Rule::Planar => true,
        Rule::AttackOnlyNearest(left) => left.is_none(),
        Rule::PreventDamage(shield) => {
            shield.to.as_ref().is_some_and(Vec::is_empty)
                || shield.from.as_ref().is_some_and(Vec::is_empty)
        }
        Rule::DiesInstead { objects, .. } => objects.is_empty(),
        Rule::GraveyardCardsUntargetable
        | Rule::CantLoseGame { .. }
        | Rule::CantCastSpells { .. }
        | Rule::ToughnessAssignsDamage(_)
        | Rule::LethalByPower(_)
        | Rule::DevotionIncreased { .. }
        | Rule::DamageAsInfect(_)
        | Rule::VentureLimit { .. } => crate::w17b_statics::w17b_names_nothing(rule),
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
        .filter(|effect| effect.is_active(state) && !crate::r377_skips::r377_waits(state, effect))
    {
        found.put(&effect.rule, effect.controller, effect.source);
    }
    if !state.rules_possible {
        return found;
    }
    // A rule that a permanent has works from the battlefield, one a spell has for itself from the stack, and a card's
    // permission to cast itself from its owner's graveyard, from exile or from a hand ("you may cast this spell as though
    // it had flash if ...", CR 601.2a, 113.6) from there.
    // A rule that works from the command zone ("as long as ~ is in the command zone or on the battlefield") works
    // from its owner's command zone.
    let graveyards = state.players().iter().flat_map(|player| {
        [
            (Zone::Graveyard(player.id()), ZoneKind::Graveyard),
            (Zone::Hand(player.id()), ZoneKind::Hand),
            (Zone::Command(player.id()), ZoneKind::Command),
        ]
    });
    for (zone, from) in [
        (Zone::Battlefield, ZoneKind::Battlefield),
        (Zone::Stack, ZoneKind::Stack),
        (Zone::Exile, ZoneKind::Exile),
    ]
    .into_iter()
    .chain(graveyards)
    {
        for &id in state.zone(zone) {
            let Some(object) = state.object(id).filter(|o| o.kind() != ObjectKind::Ability) else {
                continue;
            };
            if !may_hold(state, object, definition_has_rule, is_rule) {
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
                    if matches!(lowered, Rule::SpellCost { .. }) {
                        found
                            .static_spell_costs
                            .push((lowered, controller, object.reference()));
                    }
                }
            }
        }
    }
    found
}

/// The rules of a game in which none can be in force, and of one whose rules are being gathered (the gathering reads
/// objects, never the rules).
static NONE: RulesInForce = RulesInForce {
    quirks: crate::combat_quirks::QuirkRules::NONE,
    must_attack: BTreeSet::new(),
    attack_with_one: Vec::new(),
    attack_players: Vec::new(),
    attack_at: Vec::new(),
    must_block: BTreeSet::new(),
    uncounterable: BTreeSet::new(),
    unregenerable: BTreeSet::new(),
    no_life_gain: BTreeSet::new(),
    no_damage_loss: BTreeSet::new(),
    life_floors: BTreeMap::new(),
    radiation_gains: BTreeSet::new(),
    no_life_change: BTreeSet::new(),
    player_keywords: Vec::new(),
    no_draw: BTreeSet::new(),
    draw_bottom: BTreeSet::new(),
    no_search: BTreeSet::new(),
    no_own_search: BTreeSet::new(),
    search_top: BTreeMap::new(),
    untap_removes: BTreeSet::new(),
    mana: Vec::new(),
    vote_again: BTreeSet::new(),
    extra_vote: Vec::new(),
    r376_reversals: 0,
    r376_nearest: Vec::new(),
    skip_extra_turns: BTreeSet::new(),
    flip_two: Vec::new(),
    extra_die: Vec::new(),
    keep_mana: Vec::new(),
    mana_becomes: Vec::new(),
    draw_limit: Vec::new(),
    life_for_mana: Vec::new(),
    life_for_one: Vec::new(),
    spared_sacrifice: BTreeSet::new(),
    spared_discard: BTreeSet::new(),
    spend_mana_as: Vec::new(),
    reveal_top: BTreeSet::new(),
    reveal_hand: BTreeSet::new(),
    control_searches: Vec::new(),
    exile_found: Vec::new(),
    no_hand_limit: BTreeSet::new(),
    loyalty_twice: BTreeSet::new(),
    echo_rather: Vec::new(),
    reveal_first_draw: Vec::new(),
    boast_twice: BTreeSet::new(),
    loyalty_any_time: BTreeSet::new(),
    loyalty_more: BTreeMap::new(),
    skip_draw: BTreeSet::new(),
    exhaust_again: BTreeSet::new(),
    look_top: BTreeSet::new(),
    extra_lands: BTreeMap::new(),
    again: Vec::new(),
    max_hand: BTreeMap::new(),
    hand_change: BTreeMap::new(),
    no_legend: BTreeSet::new(),
    no_legend_for: Vec::new(),
    haste_for_abilities: Vec::new(),
    untap_during: Vec::new(),
    untap_limits: Vec::new(),
    unpreventable: Vec::new(),
    casts: Vec::new(),
    taxes: Vec::new(),
    block_taxes: Vec::new(),
    looks: Vec::new(),
    face_down_looks: Vec::new(),
    attack_limit: None,
    attack_limit_at: BTreeMap::new(),
    block_limit: None,
    block_limit_each: BTreeMap::new(),
    cants: Vec::new(),
    static_spell_costs: Vec::new(),
    w17b: crate::w17b_statics::W17B_NONE,
    damage_as_wither: false,
    mana_burn: false,
    haste_at: BTreeSet::new(),
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

    /// Who controls `player` while they search their library, by the latest rule in force that says so: the one place the
    /// answerer of a search reads this rule.
    pub fn search_controller(&self, player: PlayerId) -> Option<PlayerId> {
        self.rules()
            .control_searches
            .iter()
            .rev()
            .find(|(searched, by)| *searched == player && *by != player && self.is_alive(*by))
            .map(|(_, by)| *by)
    }

    /// The rules under which `player` exiles what they find in their library, as the player who may play those cards and
    /// the source of the rule; an opponent's rule whose controller has left does nothing.
    pub fn exile_found_by(&self, player: PlayerId) -> Vec<(PlayerId, Option<ObjectRef>)> {
        self.rules()
            .exile_found
            .iter()
            .filter(|(searched, by, _)| *searched == player && *by != player && self.is_alive(*by))
            .map(|(_, by, source)| (*by, *source))
            .collect()
    }

    /// Whether any rule reveals first draws.
    pub(crate) fn r4708_any_first_draw_reveals(&self) -> bool {
        !self.rules().reveal_first_draw.is_empty()
    }

    /// The sources whose rules reveal the first card `player` draws this turn, each with whether its reveal is optional,
    /// in the order the rules were gathered.
    pub(crate) fn r4708_first_draw_reveals(&self, player: PlayerId) -> Vec<(ObjectId, bool)> {
        let active = self.active_player() == Some(player);
        self.rules()
            .reveal_first_draw
            .iter()
            .filter(|(by, _, _, your_turns)| *by == player && (active || !*your_turns))
            .filter_map(|(_, source, may, _)| source.map(|source| (source.id, *may)))
            .collect()
    }

    /// Whether a rule has `player` play with their hand revealed to every seat.
    pub fn hand_revealed(&self, player: PlayerId) -> bool {
        self.rules().reveal_hand.contains(&player)
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
        Rule::PreventDamage(_)
            | Rule::Regenerate(_)
            | Rule::DiesInstead { .. }
            | Rule::Replaces { .. }
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
    if changes_costs(effect) {
        state.possible.costs = true;
    }
    state.rule_effects.push(effect.clone());
    Ok(())
}

/// Whether a rule in force changes the costs of spells (`GameState::possible`): a cost change of the next spell or of
/// every matching spell for a time.
pub(crate) const fn changes_costs(effect: &RuleEffect) -> bool {
    matches!(
        effect.rule,
        Rule::NextSpell {
            benefit: mtg_cards::modify::SpellBenefit::Cost { .. },
            ..
        } | Rule::SpellCost { .. }
    )
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

/// Ends the `NextSpell` rules the cast of `spell` by `player` spends (the card is judged where it is cast from, as the
/// announcement judged it), and says whether one of them keeps the spell from being countered.
pub(crate) fn spend_next_spells(state: &mut GameState, player: PlayerId, spell: ObjectId) -> bool {
    let Some(target) = state.object(spell).map(GameObject::reference) else {
        return false;
    };
    let spent: Vec<EffectId> = state
        .rule_effects
        .iter()
        .filter_map(|effect| {
            let Rule::NextSpell { what, .. } = &effect.rule else {
                return None;
            };
            let scope = Scope {
                casting: Some((spell, player)),
                ..Scope::announcing(state, effect.controller, effect.source.unwrap_or(target))
            };
            (effect.controller == player
                && (what.is_empty() || scope.selects(what, target).unwrap_or(false)))
            .then_some(effect.id)
        })
        .collect();
    let uncounterable = state.rule_effects.iter().any(|effect| {
        spent.contains(&effect.id)
            && matches!(
                effect.rule,
                Rule::NextSpell {
                    benefit: mtg_cards::modify::SpellBenefit::Uncounterable,
                    ..
                }
            )
    });
    state
        .rule_effects
        .retain(|effect| !spent.contains(&effect.id));
    uncounterable
}

/// The spell `spell` on the stack can't be countered from now on, as a `NextSpell` rule its cast spent said: a rule of its
/// own that names the spell, and so no other object once it leaves the stack.
pub(crate) fn uncounterable_from_now(state: &mut GameState, player: PlayerId, spell: ObjectId) {
    let Some(named) = state.object(spell).map(GameObject::reference) else {
        return;
    };
    let effect = RuleEffect {
        id: EffectId(state.next_effect),
        rule: Rule::CantBeCountered(vec![named]),
        duration: Duration::Forever,
        source: Some(named),
        controller: player,
        turn: state.turn().number(),
    };
    state.next_effect += 1;
    state.rule_effects.push(effect);
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

/// The most creatures that may attack together this combat, the least of the limits in force (CR 508.1c); none while
/// no rule limits the declaration.
pub fn attack_limit(state: &GameState) -> Option<usize> {
    state
        .rules()
        .attack_limit
        .map(|most| usize::try_from(most).unwrap_or(usize::MAX))
}

/// The most creatures that may attack each limited player or planeswalker together this combat, the least of the
/// limits on it in force (CR 508.1c); empty while no rule limits the attacks at one.
pub fn w10a_attack_limits_at(state: &GameState) -> Vec<(AttackTarget, usize)> {
    state
        .rules()
        .attack_limit_at
        .iter()
        .map(|(target, most)| (*target, usize::try_from(*most).unwrap_or(usize::MAX)))
        .collect()
}

/// The most creatures `player` may block with this combat, the least of the limits on every declaration and on theirs
/// in force (CR 509.1b); none while no rule limits the declaration.
pub fn block_limit(state: &GameState, player: PlayerId) -> Option<usize> {
    let rules = state.rules();
    let theirs = rules.block_limit_each.get(&player).copied();
    let most = match (rules.block_limit, theirs) {
        (Some(all), Some(each)) => Some(all.min(each)),
        (all, each) => all.or(each),
    };
    most.map(|most| usize::try_from(most).unwrap_or(usize::MAX))
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

/// The planeswalkers the creature `id` attacks this combat if able, in the order the rules named them: those a rule names
/// it for, and those a rule names its controller for.
pub fn must_attack_at(state: &GameState, id: ObjectId) -> Vec<ObjectId> {
    let controller = crate::characteristics::controller_of(state, id);
    let counter = state.object(id).map(crate::object::GameObject::counter);
    state
        .rules()
        .attack_at
        .iter()
        .filter(|(by, _)| match by {
            AttackerOf::Creature(named) => named.id == id && Some(named.counter) == counter,
            AttackerOf::Player(player) => Some(*player) == controller,
        })
        .filter(|(_, at)| {
            state
                .object(at.id)
                .is_some_and(|o| o.counter() == at.counter)
        })
        .map(|(_, at)| at.id)
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

/// Whether the permanent `id` can't be regenerated: the one place a regeneration shield reads this rule.
pub fn cant_be_regenerated(state: &GameState, id: ObjectId) -> bool {
    state.rules().unregenerable.contains(&id)
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

/// The life total damage can't take `player` below at `life`, if a rule sets one: the highest floor the total is at or above,
/// since a floor that is not yet reached has nothing to hold (CR 614.1); the one place the damage events read this rule.
pub fn damage_life_floor(state: &GameState, player: PlayerId, life: i32) -> Option<i32> {
    state
        .rules()
        .life_floors
        .get(&player)
        .and_then(|floors| floors.iter().copied().filter(|floor| life >= *floor).max())
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

/// How many times each turn `player` may activate the loyalty abilities of one planeswalker they control: twice under a
/// rule that allows it (Oath of Teferi), else once (CR 606.3).
pub fn loyalty_activations(state: &GameState, player: PlayerId) -> u32 {
    if state.rules().loyalty_twice.contains(&player) {
        2
    } else {
        1
    }
}

/// The costs `player` may pay rather than the echo cost of a permanent they control (Thick-Skinned Goblin), in the
/// order the rules began, each once.
pub(crate) fn echo_rather(state: &GameState, player: PlayerId) -> Vec<mtg_cards::cost::Cost> {
    let mut costs: Vec<mtg_cards::cost::Cost> = Vec::new();
    for (whose, cost) in &state.rules().echo_rather {
        if *whose == player && !costs.contains(cost) {
            costs.push(cost.clone());
        }
    }
    costs
}

/// Whether `player` may activate loyalty abilities whenever they could cast an instant (Teferi's Talent's emblem).
pub fn loyalty_any_time(state: &GameState, player: PlayerId) -> bool {
    state.rules().loyalty_any_time.contains(&player)
}

/// The loyalty counters the loyalty abilities `player` activates cost in addition (Carth the Lion), 0 under no such rule.
pub fn loyalty_costs_more(state: &GameState, player: PlayerId) -> u32 {
    state
        .rules()
        .loyalty_more
        .get(&player)
        .copied()
        .unwrap_or(0)
}

/// Whether `player` can't draw cards: the one place a draw reads this rule.
pub fn cant_draw(state: &GameState, player: PlayerId) -> bool {
    state.rules().no_draw.contains(&player)
}

/// Whether `player` draws from the bottom of their library: the one place a draw reads this rule.
pub(crate) fn draws_from_bottom(state: &GameState, player: PlayerId) -> bool {
    state.rules().draw_bottom.contains(&player)
}

/// Whether `player` may activate an exhaust ability of theirs that was activated before: a rule in force says so, it is
/// their turn and they haven't activated an exhaust ability this turn (Elvish Refueler).
pub(crate) fn r6007_exhaust_again(state: &GameState, player: PlayerId) -> bool {
    state.rules().exhaust_again.contains(&player)
        && state.active_player() == Some(player)
        && !state.tallies.records.iter().any(|record| {
            record.player == Some(player)
                && record.event
                    == mtg_cards::value::TallyEvent::KindActivated(
                        mtg_cards::ability::ActivationKind::Exhaust,
                    )
        })
}

/// Whether the limit `limit` of an ability `player` activates holds: an exhaust ability (once each game) does not hold
/// `player` back while they may activate exhaust abilities again.
pub(crate) fn r6007_limit_holds(
    state: &GameState,
    player: PlayerId,
    limit: mtg_cards::ability::Limit,
) -> bool {
    !(limit == mtg_cards::ability::Limit::OncePerGame && r6007_exhaust_again(state, player))
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

/// Whether a rule lets the controller of the creature `id` activate its abilities as though it had haste, judged for the
/// rule's controller.
pub(crate) fn ep24_abilities_as_though_haste(state: &GameState, id: ObjectId) -> bool {
    let Some(object) = state.object(id) else {
        return false;
    };
    state.rules().haste_for_abilities.iter().any(|rule| {
        Scope::announcing(state, rule.controller, rule.source)
            .selects(&rule.what, object.reference())
            .unwrap_or(false)
    })
}

/// The cost `player` pays for `mana` under the rules that let them pay 2 life rather than mana of a color: each symbol of
/// such a color is Phyrexian.
pub(crate) fn ep24_life_for_mana(state: &GameState, player: PlayerId, mana: ManaCost) -> ManaCost {
    state
        .rules()
        .life_for_mana
        .iter()
        .filter(|(who, _)| *who == player)
        .fold(mana, |mana, (_, color)| mana.with_phyrexian(*color))
}

/// `mana` as `player` may pay it for the spell `spell`: one symbol of the color made Phyrexian for each rule that lets
/// them pay 2 life to cast a spell like it for that much less (an ability's cost is left as it is).
pub(crate) fn r284_life_for_one(
    state: &GameState,
    player: PlayerId,
    spell: Option<ObjectId>,
    mana: ManaCost,
) -> ManaCost {
    let Some(object) = spell.and_then(|id| state.object(id)) else {
        return mana;
    };
    let reference = object.reference();
    state
        .rules()
        .life_for_one
        .iter()
        .filter(|(who, _, spells)| {
            *who == player
                && Scope::announcing(state, player, reference)
                    .selects(spells, reference)
                    .unwrap_or(false)
        })
        .fold(mana, |mana, (_, color, _)| mana.with_one_phyrexian(*color))
}

/// How many more cards `player` may draw this turn under the draw limits in force; `None` when no limit holds.
pub(crate) fn ep24_draws_left(state: &GameState, player: PlayerId) -> Option<usize> {
    let most = state
        .rules()
        .draw_limit
        .iter()
        .filter(|(who, _)| *who == player)
        .map(|(_, most)| *most)
        .min()?;
    let drawn = state.tallied(player, crate::state::Tally::Drawn);
    usize::try_from(most.saturating_sub(drawn)).ok()
}

/// Whether a spell or ability `by` controls can't make `player` sacrifice permanents: a rule spares `player` from their
/// opponents' effects.
pub(crate) fn ep24_spared_sacrifice(state: &GameState, player: PlayerId, by: PlayerId) -> bool {
    state.is_opponent(by, player) && state.rules().spared_sacrifice.contains(&player)
}

/// Whether a spell or ability `by` controls can't make `player` discard cards: a rule spares `player` from their
/// opponents' effects.
pub(crate) fn r4702_spared_discard(state: &GameState, player: PlayerId, by: PlayerId) -> bool {
    state.is_opponent(by, player) && state.rules().spared_discard.contains(&player)
}

/// How `player` may spend mana on the spell `spell`, or on an ability of `source` when `spell` is `None`, under the rules
/// in force: any type over any color over the normal rule.
pub(crate) fn ep24_spend_as(
    state: &GameState,
    player: PlayerId,
    spell: Option<ObjectId>,
    source: Option<ObjectId>,
) -> SpendAs {
    let selects = |selector: &Selector, id: ObjectId| {
        state.object(id).is_some_and(|object| {
            let reference = object.reference();
            Scope::announcing(state, player, reference)
                .selects(selector, reference)
                .unwrap_or(false)
        })
    };
    let mut found = SpendAs::Normal;
    for (who, spend_as, spells, abilities) in &state.rules().spend_mana_as {
        if *who != player {
            continue;
        }
        let applies = match (spells, spell) {
            // Only to activate the abilities of the permanents `abilities` selects ("to activate abilities of
            // creatures you control"): no spell.
            (None, None) if abilities.is_some() => abilities
                .as_ref()
                .zip(source)
                .is_some_and(|(abilities, source)| selects(abilities, source)),
            (None, Some(_)) if abilities.is_some() => false,
            (None, _) => true,
            (Some(spells), Some(spell)) => selects(spells, spell),
            (Some(_), None) => abilities
                .as_ref()
                .zip(source)
                .is_some_and(|(abilities, source)| selects(abilities, source)),
        };
        if applies {
            match spend_as {
                SpendAs::AnyType => return SpendAs::AnyType,
                SpendAs::AnyColor => found = SpendAs::AnyColor,
                SpendAs::Normal => {}
            }
        }
    }
    found
}

/// What happens to the unspent mana `player` would lose as a step or phase ends: the types kept (every type when the
/// list holds `None`), and the type the rest becomes instead of being lost (the earliest such rule).
pub(crate) fn ep24_unspent_mana(
    state: &GameState,
    player: PlayerId,
) -> (Vec<Option<ManaType>>, Option<ManaType>) {
    let rules = state.rules();
    let kept = rules
        .keep_mana
        .iter()
        .filter(|(who, _)| *who == player)
        .map(|(_, mana)| *mana)
        .collect();
    let becomes = rules
        .mana_becomes
        .iter()
        .find(|(who, _)| *who == player)
        .map(|(_, into)| *into);
    (kept, becomes)
}

/// How many dice `player` rolls more than asked, ignoring as many of the lowest rolls: one for each rule that adds one, at
/// most 8.
pub fn ep29_extra_dice(state: &GameState, player: PlayerId) -> u32 {
    let extra = state
        .rules()
        .extra_die
        .iter()
        .filter(|who| **who == player)
        .count()
        .min(8);
    u32::try_from(extra).unwrap_or(0)
}

/// How many coins `player` flips for one coin they would flip: two for each rule that doubles them, at most 32.
pub fn ep22_coins_per_flip(state: &GameState, player: PlayerId) -> u32 {
    let doublings = state
        .rules()
        .flip_two
        .iter()
        .filter(|who| **who == player)
        .count()
        .min(5);
    1 << doublings
}

/// How many additional votes `player` gets while voting (CR 701.38d): one for each rule that gives one.
pub fn ep33_extra_votes(state: &GameState, player: PlayerId) -> usize {
    state
        .rules()
        .extra_vote
        .iter()
        .filter(|seat| **seat == player)
        .count()
}

/// The sides on which each player may attack only the nearest opponent, one for each rule in force; none when no rule
/// limits attacks so.
pub(crate) fn r376_nearest_sides(state: &GameState) -> Vec<mtg_cards::refs::Side> {
    state
        .rules()
        .r376_nearest
        .iter()
        .map(|left| {
            if *left {
                mtg_cards::refs::Side::Left
            } else {
                mtg_cards::refs::Side::Right
            }
        })
        .collect()
}

/// Whether the game's turn order runs the other way round the table: an odd number of rules reverse it.
pub fn r376_turn_order_reversed(state: &GameState) -> bool {
    state.rules().r376_reversals % 2 == 1
}

/// Whether `player` may vote an additional time while voting (CR 701.38d).
pub fn w17c_votes_again(state: &GameState, player: PlayerId) -> bool {
    state.rules().vote_again.contains(&player)
}

/// Whether `player` can't search libraries: the one place a search reads this rule.
pub fn cant_search(state: &GameState, player: PlayerId) -> bool {
    state.rules().no_search.contains(&player)
}

/// How many cards from the top of a library `player` searches, when a rule limits it (Aven Mindcensor).
pub fn search_top(state: &GameState, player: PlayerId) -> Option<u32> {
    state.rules().search_top.get(&player).copied()
}

/// Whether `player`'s own spells and abilities can't cause them to search their own library: the one place a search
/// reads this rule.
pub fn own_spells_cant_search(state: &GameState, player: PlayerId) -> bool {
    state.rules().no_own_search.contains(&player)
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

/// Whether a rule in force lets `viewer` look at `card`, a face-down card in exile (CR 406.3) or a face-down permanent
/// (CR 708.5): the one place the sight of a seat reads these rules.
pub(crate) fn grants_look(state: &GameState, viewer: PlayerId, card: &GameObject) -> bool {
    let looks = match card.zone() {
        Zone::Exile => &state.rules().looks,
        Zone::Battlefield => &state.rules().face_down_looks,
        _ => return false,
    };
    looks.iter().any(|look| {
        look.who.contains(&viewer)
            && look
                .only
                .as_ref()
                .is_none_or(|only| only.contains(&card.reference()))
            && (look.what.is_empty()
                || Scope::announcing(state, look.controller, look.source)
                    .selects(&look.what, card.reference())
                    .unwrap_or(false))
    })
}

/// Whether `player` may cast `card` without a permission that opens no zone: from their hand, as their commander from
/// their command zone, or from a zone another permission opens for its mana cost.
fn r305c_cast_anyway(state: &GameState, player: PlayerId, card: &GameObject) -> bool {
    match card.zone() {
        Zone::Hand(owner) => owner == player,
        Zone::Command(owner) => owner == player && state.is_commander(card.id()),
        zone => cast_permissions(state).iter().any(|other| {
            !other.opens_no_zone
                && other.adds_to_printed(crate::resolve::zone_kind(zone))
                && other.covers(state, player, card)
        }),
    }
}

/// The permissions to cast cards in force: the one place a cast reads them (`ways`).
pub(crate) fn cast_permissions(state: &GameState) -> &[Cast] {
    &state.rules().casts
}

/// The source and the tally a cast or land play of `card` by `player` from `from` uses up: that of the first permission with
/// a limit that covers it, when no permission without one for that zone does. None for a card from a hand or the command
/// zone.
pub(crate) fn r5503_limit_use(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    from: Zone,
) -> Option<(ObjectRef, Tally)> {
    if matches!(from, Zone::Hand(_) | Zone::Command(_)) {
        return None;
    }
    let card = state.object(card)?;
    let zone = crate::resolve::zone_kind(from);
    let covering: Vec<&Cast> = cast_permissions(state)
        .iter()
        .filter(|cast| cast.from == Some(zone) && cast.covers(state, player, card))
        .collect();
    if covering.iter().any(|cast| cast.limit.is_none()) {
        return None;
    }
    covering
        .first()
        .map(|cast| (cast.source, Tally::R5503LimitUsed(cast.r5503_key())))
}

/// The source of the permission that works once each turn which a cast of `card` by `player` from `from` for `way` uses
/// up: a cast for the mana cost from a zone other than a hand or the command zone that no other permission for that zone
/// covers, through the first such permission that covers it; a free cast as `r5503_free_once_source` finds it. None for any
/// other cast.
pub(crate) fn c11b_once_source(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    from: Zone,
    way: Option<mtg_cards::value::CastWay>,
) -> Option<ObjectRef> {
    if way == Some(mtg_cards::value::CastWay::Alternative) {
        return r400_rather_once_source(state, player, card);
    }
    if way == Some(mtg_cards::value::CastWay::Free) {
        return r5503_free_once_source(state, player, card);
    }
    if way.is_some() || matches!(from, Zone::Hand(_) | Zone::Command(_)) {
        return None;
    }
    let card = state.object(card)?;
    let zone = crate::resolve::zone_kind(from);
    let covering: Vec<&Cast> = cast_permissions(state)
        .iter()
        .filter(|cast| cast.adds_to_printed(zone) && cast.covers(state, player, card))
        .collect();
    if covering.iter().any(|cast| !cast.once) {
        return None;
    }
    covering.first().map(|cast| cast.source)
}

/// The source of the permission that works once each turn which a free cast of `card` by `player` uses up: the first free
/// permission that covers the card, when every free one that does works once each turn; none otherwise.
fn r5503_free_once_source(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
) -> Option<ObjectRef> {
    let card = state.object(card)?;
    let covering: Vec<&Cast> = cast_permissions(state)
        .iter()
        .filter(|cast| cast.free && cast.covers(state, player, card))
        .collect();
    if covering.iter().any(|cast| !cast.once) {
        return None;
    }
    covering.first().map(|cast| cast.source)
}

/// The riders a cast of `card` by `player` from `from` for `way` takes from the permission it goes through: a cast for the
/// mana cost from a zone other than a hand or the command zone, a free cast, or a cast for the cost a permission puts in
/// place of the mana cost goes through the first permission that covers it, one that works once each turn after the others (as [`c11b_once_source`] counts it). None for any other
/// cast.
pub(crate) fn r401_cast_riders(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    from: Zone,
    way: Option<mtg_cards::value::CastWay>,
) -> Vec<CastRider> {
    let free = way == Some(mtg_cards::value::CastWay::Free);
    // A cast for the cost a permission puts in place of the mana cost (Glimpse the Cosmos) takes that permission's riders.
    let rather = way == Some(mtg_cards::value::CastWay::Alternative);
    if (way.is_some() && !free && !rather) || matches!(from, Zone::Hand(_) | Zone::Command(_)) {
        return Vec::new();
    }
    let Some(card) = state.object(card) else {
        return Vec::new();
    };
    let zone = crate::resolve::zone_kind(from);
    let mut covering: Vec<&Cast> = cast_permissions(state)
        .iter()
        .filter(|cast| {
            !cast.opens_no_zone
                && cast.from == Some(zone)
                && (if free {
                    cast.free
                } else if rather {
                    cast.rather.is_some() || cast.instead.is_some()
                } else {
                    cast.adds_to_printed(zone)
                })
                && cast.covers(state, player, card)
        })
        .collect();
    covering.sort_by_key(|cast| cast.once);
    covering
        .first()
        .map(|cast| cast.riders.clone())
        .unwrap_or_default()
}

/// The source of the permission that works once each turn whose cost a cast of `card` by `player` for an alternative cost
/// pays: the first permission that covers the card with a cost in place of the mana cost, which is the way that cast
/// takes (`ways::rather_way`); none when that permission works every time.
fn r400_rather_once_source(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
) -> Option<ObjectRef> {
    let card = state.object(card)?;
    let rather = cast_permissions(state)
        .iter()
        .find(|cast| cast.instead.is_some() && cast.covers(state, player, card))?;
    rather.once.then_some(rather.source)
}

/// The taxes on blocking in force: the one place the block declaration reads this rule (`block_tax`).
pub(crate) fn block_taxes(state: &GameState) -> &[crate::block_tax::BlockTaxing] {
    &state.rules().block_taxes
}

/// The taxes on attacking in force: the one place the attack declaration reads this rule (`attack_tax`).
pub(crate) fn attack_taxes(state: &GameState) -> &[Taxing] {
    &state.rules().taxes
}

/// The maximum hand size of `player` under the rules in force, `base` when no rule sets one: the one place cleanup reads
/// it, beside `no_max_hand_size`.
pub(crate) fn max_hand_size(state: &GameState, player: PlayerId, base: usize) -> usize {
    let set = state
        .rules()
        .max_hand
        .get(&player)
        .map_or(base, |size| usize::try_from(*size).unwrap_or(usize::MAX));
    let change = state.rules().hand_change.get(&player).copied().unwrap_or(0);
    let size = i64::try_from(set)
        .unwrap_or(i64::MAX)
        .saturating_add(change)
        .max(0);
    usize::try_from(size).unwrap_or(usize::MAX)
}

/// Whether the legend rule does not apply to the permanents `player` controls: the one place the state-based actions
/// read this rule.
pub fn no_legend_rule(state: &GameState, player: PlayerId) -> bool {
    state.rules().no_legend.contains(&player)
}

/// Whether a rule says the legend rule does not apply to the permanent `id`, judged for the rule's controller: the
/// one place the state-based actions read these rules.
pub fn no_legend_rule_for(state: &GameState, id: ObjectId) -> bool {
    let Some(object) = state.object(id) else {
        return false;
    };
    state.rules().no_legend_for.iter().any(|rule| {
        Scope::announcing(state, rule.controller, rule.source)
            .selects(&rule.what, object.reference())
            .unwrap_or(false)
    })
}

/// The untap limits in force (CR 502.3).
pub(crate) fn untap_limits(state: &GameState) -> &[crate::rmt4_untap_limit::UntapLimit] {
    &state.rules().untap_limits
}

/// The permanents that untap during the untap step of `active` besides its own: those each rule in force selects among
/// the permanents its controller controls, in battlefield order.
pub(crate) fn untap_during(state: &GameState, active: PlayerId) -> Vec<ObjectId> {
    let mut found = Vec::new();
    for rule in state
        .rules()
        .untap_during
        .iter()
        .filter(|rule| rule.whose.contains(&active))
    {
        let scope = Scope::announcing(state, rule.controller, rule.source);
        for &id in state.zone(Zone::Battlefield) {
            let Some(object) = state.object(id) else {
                continue;
            };
            if crate::characteristics::controller_of(state, id) == Some(rule.controller)
                && !found.contains(&id)
                && scope
                    .selects(&rule.what, object.reference())
                    .unwrap_or(false)
            {
                found.push(id);
            }
        }
    }
    found
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
        apply_event(
            game,
            &Event::RuleCreated {
                effect: effect.into(),
            },
        )
        .unwrap();
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
        apply_event(
            &mut game,
            &Event::EffectCreated {
                effect: lost.into(),
            },
        )
        .unwrap();
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
        apply_event(
            &mut game,
            &Event::RuleCreated {
                effect: rule.into(),
            },
        )
        .unwrap();
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
        apply_event(
            &mut game,
            &Event::RuleCreated {
                effect: rule.into(),
            },
        )
        .unwrap();
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
            rather: None,
            who: vec![ME],
            what: mtg_cards::refs::Selector::default(),
            from: Some(ZoneKind::Exile),
            free: false,
            flash: false,
            only,
            play,
            once: false,
            any_turn: false,
            additional: None,
            spend_as: SpendAs::Normal,
            when: None,
            any_cast: false,
            riders: Vec::new(),
            limit: None,
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

    #[rstest]
    #[case::while_it_holds(Condition::YourTurn, true)]
    #[case::while_it_fails(Condition::Not(Box::new(Condition::YourTurn)), false)]
    fn a_permission_with_a_condition_covers_only_while_it_holds(
        #[case] when: Condition,
        #[case] covered: bool,
    ) {
        let mut game = game(Vec::new());
        let wolf = exiled(&mut game, "Wolf");
        let mut rule = permission(Some(vec![wolf]), false);
        if let Rule::MayCast { when: slot, .. } = &mut rule {
            *slot = Some(Box::new(when));
        }
        put_in_force(&mut game, rule);
        let [permission] = cast_permissions(&game) else {
            panic!("one permission");
        };
        let mine = game.turn().active() == permission.controller;
        let object = game.object(wolf.id).unwrap();
        assert_eq!(permission.covers(&game, ME, object), covered == mine);
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
                effect: fresh.clone().into(),
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
            Some(serde_json::from_str::<TriggerPattern>(cause).unwrap())
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
        apply_event(
            &mut game,
            &Event::RuleCreated {
                effect: effect.into(),
            },
        )
        .unwrap();
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

    /// Seat 0 controls the holder with the grant; the top card of seat 0's library is exiled face down sealed by `by`
    /// (the holder, or another permanent when `by_holder` is false); returns the card.
    fn sealed(game: &mut GameState, by_holder: bool) -> ObjectId {
        let by = if by_holder {
            game.zone(Zone::Battlefield)[0]
        } else {
            game.zone(Zone::Hand(ME))[0]
        };
        let by = game.object(by).unwrap().reference();
        let card = *game.zone(Zone::Library(ME)).last().unwrap();
        let seal = Event::ZoneChanged {
            object: card,
            from: Zone::Library(ME),
            to: Zone::Exile,
            cause: ZoneCause::Sealed { by },
        };
        apply_event(game, &seal).unwrap();
        card
    }

    #[rstest]
    #[case::no_grant(None, true, &[])]
    #[case::you_look_at_what_it_exiled(Some("{}"), true, &[0])]
    #[case::not_at_what_another_exiled(Some("{}"), false, &[])]
    #[case::each_player(Some(r#"{"who":"EachPlayer"}"#), true, &[0, 1, 2, 3])]
    #[case::opponents(Some(r#"{"who":"Opponents"}"#), true, &[1, 2, 3])]
    fn a_sealed_card_is_looked_at_only_through_a_grant_while_the_grant_is_in_force(
        #[values(2, 3, 4)] seats: usize,
        #[case] grant: Option<&str>,
        #[case] by_holder: bool,
        #[case] lookers: &[usize],
    ) {
        let rules = grant
            .map(|who| {
                let mut rule: serde_json::Value = serde_json::from_str(who).unwrap();
                rule["what"] = serde_json::json!([{"ExiledWith": "This"}]);
                rule_ability(&format!(r#"{{"LookAtExiled":{rule}}}"#))
            })
            .into_iter()
            .collect();
        let mut game = table(seats, rules);
        let card = sealed(&mut game, by_holder);
        let sees = |game: &GameState| -> Vec<usize> {
            (0..seats)
                .filter(|seat| game.may_look_at(PlayerId::from_index(*seat), card))
                .collect()
        };
        let expected: Vec<usize> = lookers
            .iter()
            .copied()
            .filter(|seat| *seat < seats)
            .collect();
        assert_eq!(sees(&game), expected);
        // The grant ends with the holder: the card is sealed for every seat again.
        leaves(&mut game, "Holder");
        assert_eq!(sees(&game), Vec::<usize>::new());
    }
}
