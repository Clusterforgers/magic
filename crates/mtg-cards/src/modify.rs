//! Modifications of objects and rules, and how long an effect lasts.

use std::sync::Arc;

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use mtg_core::mana::ManaType;
use serde::{Deserialize, Serialize};

use crate::ability::{Ability, ActivationKind};
use crate::cost::Cost;
use crate::keyword::Keyword;
use crate::mana::ManaCostX;
use crate::refs::{Ref, Selector, ZoneKind};
use crate::trigger::{Step, TriggerPattern};
use crate::value::{Amount, Condition, CounterKind};

/// A change to an object. Static abilities apply them continuously; `Effect::Continuous` applies
/// them for a duration. The layer (CR 613) follows from the kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Modification {
    /// Adds to power and toughness (layer 7c).
    ModifyPT {
        /// Power change.
        power: Amount,
        /// Toughness change.
        toughness: Amount,
    },
    /// Sets power and toughness (layer 7b).
    SetPT {
        /// New power.
        power: Amount,
        /// New toughness.
        toughness: Amount,
    },
    /// Sets the power alone (layer 7b): "has base power 0".
    SetPower(Amount),
    /// Sets the toughness alone (layer 7b).
    SetToughness(Amount),
    /// Switches power and toughness (layer 7d).
    SwitchPT,
    /// Adds a card type (layer 4).
    AddType(CardType),
    /// Removes a card type (layer 4).
    RemoveType(CardType),
    /// Makes these the card types, the others gone (layer 4): "loses all other card types", "is a land". The creature
    /// types go with the type when it is no longer a creature.
    SetTypes(Vec<CardType>),
    /// Takes a supertype away from the copiable values of a copy: "except it isn't legendary". Only the exceptions of a
    /// copy have it, never a static ability or the effect of a spell.
    RemoveSupertype(Supertype),
    /// Adds a supertype to the copiable values of a copy: "except it is legendary in addition to its other types". Only the
    /// exceptions of a copy have it.
    AddSupertype(Supertype),
    /// Names the copiable values of a copy: "except its name is Lazav, Dimir Mastermind". Only the exceptions of a copy
    /// have it.
    SetName(String),
    /// Gives the copiable values of a copy the ability that is resolving: "except it has this ability". Only the
    /// exceptions of a `BecomeCopy` have it.
    ThisAbility,
    /// Adds a subtype (layer 4).
    AddSubtype(Subtype),
    /// Replaces the creature types with these (layer 4): "becomes a red Weird" (CR 205.1b).
    SetCreatureTypes(Vec<Subtype>),
    /// Makes these the land types, the old ones and the abilities of the land gone, and gives it the mana ability of each
    /// basic land type among them (layer 4): "Nonbasic lands are Mountains" (CR 305.7).
    SetLandTypes(Vec<Subtype>),
    /// Makes it every creature type (layer 4): changeling (CR 702.73).
    AddAllCreatureTypes,
    /// Replaces the colors (layer 5).
    SetColors(Vec<Color>),
    /// Adds these colors to the ones it has (layer 5): "is a black Zombie in addition to its other colors".
    AddColors(Vec<Color>),
    /// Makes the color the resolution chose under this name the only color (layer 5): "becomes the color of your choice".
    /// Only an effect of a spell or ability has it, never a static ability.
    SetColorsChosen(String),
    /// Makes the land type the resolution chose under this name the only land type (layer 4): "becomes the basic land type
    /// of your choice". Only an effect of a spell or ability has it.
    SetLandTypesChosen(String),
    /// Makes the creature type the resolution chose under this name the only creature type (layer 4): "becomes the
    /// creature type of your choice". Only an effect of a spell or ability has it.
    SetCreatureTypesChosen(String),
    /// Adds a keyword ability (layer 6).
    AddKeyword(Keyword),
    /// Adds protection from a colour the controller chooses as the effect begins (layer 6): "protection from the
    /// color of your choice". Only an effect of a spell or ability has it, never a static ability.
    AddProtectionFromChosenColor,
    /// Adds protection from a card type the controller chooses as the effect begins (layer 6): "protection from the
    /// card type of your choice". Only an effect of a spell or ability has it, never a static ability.
    AddProtectionFromChosenCardType,
    /// Removes a keyword ability (layer 6).
    RemoveKeyword(Keyword),
    /// Removes a keyword ability and the object can't have or gain it afterwards, whichever effect is newer (layer 6):
    /// "lose flying and can't have or gain flying".
    LockKeyword(Keyword),
    /// Gives the object an activated or a triggered ability (layer 6); a keyword is `AddKeyword`.
    GrantAbility(Arc<Ability>),
    /// The object has the activated abilities printed on the cards the reference names, wherever they are (layer 6): "has all
    /// activated abilities of all creature cards in graveyards". A card that leaves its zone is a new object and gives none.
    GainAbilitiesOf(Ref),
    /// The object loses all its abilities (layer 6); abilities granted after it still count.
    LoseAbilities,
    /// Changes the controller to the player (layer 2).
    SetController(Ref),
    /// The object can't attack (a restriction, no layer).
    CantAttack,
    /// The object can't attack the controller of the effect or ability ("you"), the planeswalkers they control or both
    /// (a restriction, no layer).
    CantAttackYou(Attacked),
    /// The object can't block (a restriction, no layer).
    CantBlock,
    /// The object does not untap during its controller's untap step (a restriction, no layer).
    DoesntUntap,
    /// The object can't be turned face up (a restriction, no layer).
    CantBeTurnedFaceUp,
    /// The object can't be blocked (a restriction, no layer).
    CantBeBlocked,
    /// The object can't be blocked by creatures the selector matches (a restriction, no layer).
    CantBeBlockedBy(Selector),
    /// The object can block only the attackers the selector matches (a restriction, no layer): "can block only
    /// creatures with flying".
    CanBlockOnly(Selector),
    /// The object can't be blocked except by this many creatures or more (a restriction, no layer): menace without
    /// the keyword.
    MinBlockers(u32),
    /// The object can't be blocked by more than this many creatures (a restriction, no layer).
    MaxBlockers(u32),
    /// The object can block this many creatures beyond the first each combat (a restriction, no layer): "can block an
    /// additional creature each combat" (CR 509.1a).
    BlocksAdditional(u32),
    /// The object is goaded by the controller of the effect or ability (CR 701.15; a restriction, no layer): it attacks
    /// each combat if able and attacks a player other than the goader if able.
    Goaded,
}

impl Modification {
    /// Whether this adds or takes away a keyword a player can have (hexproof, shroud, protection): a static ability gives these to players.
    pub fn is_player_keyword(&self) -> bool {
        matches!(
            self,
            Modification::AddKeyword(keyword)
            | Modification::RemoveKeyword(keyword)
            | Modification::LockKeyword(keyword)
                if matches!(keyword, Keyword::Hexproof | Keyword::Shroud | Keyword::Protection(_))
        )
    }

    /// The layer of CR 613 this applies in, or `None` for a restriction that changes no characteristic.
    pub const fn layer(&self) -> Option<&'static str> {
        match self {
            Modification::AddSupertype(_)
            | Modification::SetName(_)
            | Modification::ThisAbility => Some("1"),
            Modification::SetController(_) => Some("2"),
            Modification::AddType(_)
            | Modification::RemoveType(_)
            | Modification::SetTypes(_)
            | Modification::RemoveSupertype(_)
            | Modification::AddSubtype(_)
            | Modification::SetCreatureTypes(_)
            | Modification::SetLandTypes(_)
            | Modification::SetLandTypesChosen(_)
            | Modification::SetCreatureTypesChosen(_)
            | Modification::AddAllCreatureTypes => Some("4"),
            Modification::SetColors(_)
            | Modification::AddColors(_)
            | Modification::SetColorsChosen(_) => Some("5"),
            Modification::AddKeyword(_)
            | Modification::AddProtectionFromChosenColor
            | Modification::AddProtectionFromChosenCardType
            | Modification::LockKeyword(_)
            | Modification::RemoveKeyword(_)
            | Modification::GrantAbility(_)
            | Modification::GainAbilitiesOf(_)
            | Modification::LoseAbilities => Some("6"),
            Modification::SetPT { .. }
            | Modification::SetPower(_)
            | Modification::SetToughness(_) => Some("7b"),
            Modification::ModifyPT { .. } => Some("7c"),
            Modification::SwitchPT => Some("7d"),
            Modification::CantAttack
            | Modification::CantAttackYou(_)
            | Modification::CantBlock
            | Modification::DoesntUntap
            | Modification::CantBeTurnedFaceUp
            | Modification::CantBeBlocked
            | Modification::CantBeBlockedBy(_)
            | Modification::CanBlockOnly(_)
            | Modification::MinBlockers(_)
            | Modification::MaxBlockers(_)
            | Modification::BlocksAdditional(_)
            | Modification::Goaded => None,
        }
    }
}

/// How long a continuous effect, rule or delayed trigger lasts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Duration {
    /// Until the cleanup step of this turn.
    UntilEndOfTurn,
    /// Until the end of combat.
    UntilEndOfCombat,
    /// Until the controller's next turn begins.
    UntilYourNextTurn,
    /// Until the end of the controller's next turn.
    UntilEndOfYourNextTurn,
    /// For as long as the source stays on the battlefield.
    WhileSourceOnBattlefield,
    /// For as long as the controller of the effect controls the source ("gain control of target creature for as long as you control ~"): it ends when the source leaves the battlefield or another player controls it (CR 611.2b).
    WhileYouControlSource,
    /// For as long as the source stays tapped ("for as long as ~ remains tapped", CR 611.2b): it ends when the source untaps, leaves
    /// the battlefield or is another object, and does not come back when it is tapped again. Only a continuous effect lasts this long.
    WhileSourceTapped,
    /// Until a player who is an opponent of the controller becomes the monarch (an exile that lasts that long).
    UntilOpponentBecomesMonarch,
    /// Through the next untap step of the player who controls a permanent it applies to ("doesn't untap during its
    /// controller's next untap step", CR 502.3): only the rule that a permanent doesn't untap lasts this long.
    NextUntapStep,
    /// For as long as every object the effect names is still on the battlefield with a counter of the kind ('Each of those
    /// creatures can't attack you for as long as it has a vow counter on it'): it ends when one of them loses its last counter
    /// of the kind or leaves, and a counter put on another permanent later changes nothing. Only a continuous effect that
    /// names its objects (a reference, not a group) lasts this way.
    WhileHasCounter(Box<CounterKind>),
    /// Until the turn it makes a player skip has been skipped ("skip your next turn", CR 614.10): only the rule that a
    /// player skips a turn lasts this long.
    UntilTurnSkipped,
    /// Without end (until the object or the game is gone).
    Forever,
}

/// A permission to cast cards (CR 601.3): which cards, from where, and what is waived.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
// Each flag is one independent printed waiver or limit, written as a plain JSON field.
#[allow(clippy::struct_excessive_bools)]
pub struct CastPermission {
    /// The players who may cast; the controller of the rule when absent.
    #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
    pub who: Ref,
    /// The cards they may cast; every card when empty.
    #[serde(default, skip_serializing_if = "Selector::is_empty")]
    pub what: Selector,
    /// The zone the cards are cast from: a hand when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<ZoneKind>,
    /// They are cast without paying their mana costs.
    #[serde(default, skip_serializing_if = "crate::effect::is_false")]
    pub free: bool,
    /// They are cast as though they had flash.
    #[serde(default, skip_serializing_if = "crate::effect::is_false")]
    pub flash: bool,
    /// Only the objects the reference names when the rule begins, as the objects they are then (CR 400.7: a card that
    /// leaves its zone is a new object and is no longer covered); every card `what` selects when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Ref>,
    /// A land among the cards may be played too (CR 305.1), with a land play and at the timing of any land.
    #[serde(default, skip_serializing_if = "crate::effect::is_false")]
    pub play: bool,
    /// Only once during each of the controller's turns, and only on their turns: a spell cast through it uses it up for the
    /// turn ("Once during each of your turns, you may cast a creature spell from your graveyard").
    #[serde(default, skip_serializing_if = "crate::effect::is_false")]
    pub once: bool,
    /// What the caster pays in addition to the other costs of a cast through it (CR 601.2b): "by paying 3 life and
    /// discarding a card in addition to paying its other costs".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional: Option<Cost>,
    /// How the mana of a cast through it may be spent: as mana of any color or of any type ("spend mana as though it were mana of any color").
    #[serde(default, skip_serializing_if = "SpendAs::is_normal")]
    pub spend_as: SpendAs,
}

/// Which mana may pay for a cast through a permission.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpendAs {
    /// Mana pays what its type says.
    #[default]
    Normal,
    /// Mana of any color pays a colored symbol (CR 609.4b); the engine lets any mana pay it.
    AnyColor,
    /// Mana of any type pays the cost (CR 609.4b).
    AnyType,
}

impl SpendAs {
    /// Whether mana pays what its type says.
    pub fn is_normal(&self) -> bool {
        *self == SpendAs::Normal
    }
}

/// Whether an attack tax names no mana.
fn no_mana(mana: &ManaCostX) -> bool {
    *mana == ManaCostX::default()
}

/// What a rule that taxes or restricts attackers protects: the players, the planeswalkers they control, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Attacked {
    /// Creatures attacking the players.
    Player,
    /// Creatures attacking planeswalkers the players control.
    Planeswalkers,
    /// Creatures attacking the players or planeswalkers they control.
    Both,
}

/// How many creatures must block an attacker that "must be blocked".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Blocking {
    /// At least one creature blocks it if any creature is able to (CR 509.1c).
    AtLeastOne,
    /// Every creature able to block it does.
    Every,
}

/// What a `GameRule::Combat` says of creatures: a permission, or a limit on a declaration that is not a plain
/// requirement.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CombatQuirk {
    /// They can't attack unless another creature attacks too (CR 506.5).
    CantAttackAlone,
    /// They can't block unless another creature blocks too.
    CantBlockAlone,
    /// They can attack as though they did not have defender.
    AttackDespiteDefender,
    /// They can block as though they were untapped.
    BlockWhileTapped,
    /// Their controller may choose not to untap them during their untap step (CR 502.3): asked for each one that is tapped.
    MayStayTapped,
    /// They assign no combat damage (CR 510.1): the damage they would deal in the combat damage steps is not dealt.
    AssignsNoCombatDamage,
    /// Their controller may have each of them assign its combat damage as though it were not blocked (CR 510.1c): asked when
    /// it is blocked and the damage is divided.
    AssignsAsUnblocked,
    /// They assign combat damage equal to their toughness rather than their power (CR 510.1a).
    AssignsByToughness,
    /// They must be blocked if able: a requirement on the block declaration (CR 509.1c).
    MustBeBlocked(Blocking),
    /// They can be blocked as though they did not have the landwalk ability the selector names (CR 702.14).
    IgnoreLandwalk(Selector),
    /// They can be blocked as though they did not have any landwalk ability.
    IgnoreAllLandwalk,
    /// They can block creatures with landwalk as though those did not have it.
    BlockIgnoringLandwalk,
    /// Tapped to crew or saddle, they count for another amount than their power.
    TapPower(TapPowerAs),
}

/// What a creature counts for when it is tapped to crew or saddle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TapPowerAs {
    /// Its toughness rather than its power.
    Toughness,
    /// Its power, this much greater.
    Plus(u32),
}

/// What a `GameRule::Cant` forbids its subjects. A subject is covered when it is the object an action is about or the
/// player who does it (or owns the object). Adding a prohibition is one entry here and one check where the engine does
/// the action.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Prohibited {
    /// Activate abilities of the sources the selector picks (every source when empty), except mana abilities (CR 605).
    Activate(Selector),
    /// Be the target of spells and abilities from the sources the selector picks (CR 115); every source when empty.
    BeTargeted(Selector),
    /// Play lands as the special action (CR 305.1).
    PlayLand,
    /// Sacrifice permanents, or be sacrificed (CR 701.21).
    Sacrifice,
    /// Have counters put on them (CR 122).
    PutCounter,
    /// Have counters of one kind put on them (CR 122).
    PutCounterOf(crate::value::CounterKind),
    /// Have triggered abilities of the subjects' permanents trigger from the event the pattern describes (CR 603).
    Trigger(TriggerPattern),
    /// Become attached, or have anything attached to them (CR 701.3).
    Attach,
    /// Discard cards, or be discarded (CR 701.9).
    Discard,
    /// Be exiled (CR 406).
    Exile,
    /// Transform (CR 701.28).
    Transform,
    /// Skip the step on their turns (CR 614.10); the cleanup step is never skipped.
    SkipStep(Step),
    /// Skip their turns (CR 614.10).
    SkipTurn,
    /// Cast spells (CR 601): those whose card the filter selects (any when empty), and with `more_than` only once they have
    /// cast that many such spells this turn; with `unless` only while the condition does not hold for the caster.
    Cast {
        /// The cards that can't be cast, described by their characteristics, not their zone.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        filter: Selector,
        /// The spells of the filter a player may cast each turn; none are allowed when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        more_than: Option<u32>,
        /// The cast is open while this holds for the caster.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unless: Option<Condition>,
    },
    /// Attack a player or a planeswalker they control (CR 508.1b), unless the condition holds for the defending player
    /// ("you" in it is that player).
    Attack {
        /// The attack is open while this holds for the defending player.
        unless: Condition,
    },
    /// Activate abilities of one kind of the sources the selector picks (every source when empty): the kind is a loyalty
    /// ability, the equip ability or the cycling ability, or any activated ability but a mana ability.
    ActivateKind {
        /// The kind of ability.
        kind: ActivationKind,
        /// The sources; every source when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        sources: Selector,
    },
}

/// What the next spell of a `GameRule::NextSpell` gets.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpellBenefit {
    /// Its cost changes by `mana` once for each of `per` (CR 601.2f).
    Cost {
        /// More or less.
        change: crate::ability::CostChange,
        /// The mana added or removed each time.
        mana: ManaCostX,
        /// How many times; fixed when the rule begins.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        per: Amount,
    },
    /// It can be cast as though it had flash (CR 601.3).
    Flash,
    /// It can be cast without paying its mana cost (CR 118.9).
    Free,
    /// It has the keyword while it is cast: convoke, improvise or affinity, which help pay for it.
    Keyword(Keyword),
}

/// A change to the rules of the game rather than to an object. `Effect::Rule` makes it for a duration and
/// `Ability::Rule` for as long as its source has the ability; the references name the players or objects it is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameRule {
    /// Damage from the sources can't be prevented, and prevention effects do nothing to it (CR 614.17).
    DamageCantBePrevented {
        /// The sources; any source when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        source: Selector,
        /// Only combat damage (`true`) or only other damage (`false`); both when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        combat: Option<bool>,
    },
    /// The creatures attack each combat if able: a requirement on the declaration (CR 508.1d), not a restriction.
    MustAttack(Ref),
    /// Each creature attacks its own player this turn if able (Encore, CR 702.141): the creatures and the players are
    /// paired in order, a creature with no player to pair with is not bound, and a pair whose player cannot be attacked
    /// binds nothing.
    MustAttackPlayers {
        /// The creatures, in the order they are paired.
        who: Ref,
        /// The players, in the order they are paired.
        players: Ref,
    },
    /// The creatures block each combat if able: a requirement on the declaration (CR 509.1c).
    MustBlock(Ref),
    /// Each creature `who` names blocks the attacker `attacker` names if able (Provoke): a requirement on the block
    /// declaration (CR 509.1c). The creatures and the attacker are named when the rule begins.
    MustBlockAttacker {
        /// The creatures that must block it.
        who: Ref,
        /// The attacking creature.
        attacker: Ref,
    },
    /// A permission or a limit on how the creatures take part in combat (`CombatQuirk`).
    Combat {
        /// The creatures.
        who: Ref,
        /// What the rule says of them.
        quirk: CombatQuirk,
    },
    /// Spells and abilities the players `by` name may target what the rule names as though it did not have hexproof (and
    /// shroud when `shroud`): the permanents `what` names and the players `players` names.
    IgnoreHexproof {
        /// The players whose spells and abilities may target them.
        by: Ref,
        /// The permanents it covers; none when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        what: Option<Ref>,
        /// The players it covers; none when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        players: Option<Ref>,
        /// Shroud is ignored too.
        #[serde(default, skip_serializing_if = "crate::effect::is_false")]
        shroud: bool,
    },
    /// Creatures can't attack the players (or their planeswalkers) unless their controller pays the mana for each of them
    /// as the attack is declared (CR 508.1h): the declaration is made only with the payment.
    AttackTax {
        /// The players it protects.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// Whether it protects the players, the planeswalkers they control or both.
        attacked: Attacked,
        /// The attacking creatures it taxes, judged when the attack is declared; every creature when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        attackers: Selector,
        /// The mana each taxed creature costs, once for each of `per`; no `{X}`.
        #[serde(default, skip_serializing_if = "no_mana")]
        mana: ManaCostX,
        /// The life each taxed creature costs, once for each of `per` ("pays 2 life for each of those creatures").
        #[serde(default, skip_serializing_if = "crate::spend::is_zero")]
        life: u32,
        /// How many times the mana is paid for each creature, counted when the attack is declared.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        per: Amount,
    },
    /// No more than this many creatures can attack each combat: a restriction on the whole declaration (CR 508.1c).
    AttackLimit(u32),
    /// No more than `most` creatures can attack each player or planeswalker `at` names each combat ("no more than two
    /// creatures can attack you each combat"): a restriction on the attacks at each of them (CR 508.1c).
    AttackLimitAt {
        /// The most creatures that can attack each of them together, at least one.
        most: u32,
        /// The players ("you") or planeswalkers ("~") it protects.
        at: Ref,
    },
    /// No more than this many creatures can block each combat: a restriction on the whole declaration (CR 509.1b).
    BlockLimit(u32),
    /// No more than `most` creatures each player `who` names controls can block each combat ("each opponent can't
    /// block with more than one creature this combat"): a restriction on each of their declarations (CR 509.1b).
    BlockLimitEach {
        /// The most creatures each of them can block with, at least one.
        most: u32,
        /// The players whose blocks it limits.
        who: Ref,
    },
    /// The spells can't be countered.
    CantBeCountered(Ref),
    /// The permanents can't be regenerated (CR 701.19c): a regeneration shield does nothing to their destruction.
    CantBeRegenerated(Ref),
    /// The players can't gain life.
    CantGainLife(Ref),
    /// Damage dealt to the players causes no loss of life: it is still dealt, and every other effect of it happens.
    NoLifeLossFromDamage(Ref),
    /// Damage that would reduce the life total of the players to less than `floor` reduces it to `floor` instead, while it is
    /// `floor` or more: the damage is dealt in full and only the life lost changes.
    LifeFloor {
        /// The players.
        who: Ref,
        /// The life total damage can't take them below.
        floor: i32,
    },
    /// The players gain life rather than lose life from radiation (CR 725.2): the life the rad rule would make them lose.
    RadiationGainsLife(Ref),
    /// The players' life totals can't change (CR 119.7, 119.8): they gain and lose no life, pay no life and have no life
    /// total set; damage is still dealt.
    LifeTotalCantChange(Ref),
    /// The players have a keyword, such as protection from everything (CR 702.16j), while the rule lasts.
    PlayersHave {
        /// The players.
        who: Ref,
        /// The keyword they have.
        keyword: Keyword,
    },
    /// The players can't draw cards.
    CantDraw(Ref),
    /// The players can't search libraries.
    CantSearch(Ref),
    /// The players may vote an additional time each time they vote: "While voting, you may vote an additional time."
    /// (CR 701.38d).
    VoteAgain(Ref),
    /// A player in the group who would begin an extra turn skips that turn instead (CR 500.7).
    SkipExtraTurns(Ref),
    /// The players play with the top card of their library revealed to every seat.
    RevealTop(Ref),
    /// Players may cast cards the ordinary rules do not let them cast: from another zone, without paying their mana
    /// costs, or as though they had flash (CR 601.3).
    MayCast(CastPermission),
    /// The players may look at the top card of their own library any time; no one else sees it.
    LookAtTopAnyTime(Ref),
    /// The players have no maximum hand size (CR 402.2).
    NoMaxHandSize(Ref),
    /// The players' maximum hand size is `size` (CR 402.2): the least of the sizes in force.
    MaxHandSize {
        /// The players.
        who: Ref,
        /// The cards they may keep at cleanup.
        size: Amount,
    },
    /// The legend rule (CR 704.5j) does not apply to the legendary permanents of the players.
    NoLegendRule(Ref),
    /// The permanents the selector picks that the rule's controller controls untap during the untap step of each of the
    /// players (CR 502.3): "untap all creatures you control during each other player's untap step".
    UntapDuring {
        /// The permanents, among those the rule's controller controls.
        what: Selector,
        /// The players whose untap steps untap them.
        whose: Ref,
    },
    /// The players skip their draw steps (CR 614.10): the step does not begin on their turns.
    SkipDrawStep(Ref),
    /// The players may play additional lands each of their turns (CR 305.2).
    AdditionalLandPlays {
        /// Who may play them.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// How many more lands each turn.
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        count: Amount,
    },
    /// The card can be a player's commander (CR 903.3), in every zone; the rule is written without `from`.
    CanBeCommander,
    /// The next spell the rule's controller casts that `what` selects gets the benefit, and the rule ends with that cast
    /// ("the next creature spell you cast this turn costs {1} less"); with the duration `UntilEndOfTurn` it also ends with
    /// the turn. `what` names the cards, not the stack object (no `Spell`).
    NextSpell {
        /// The cards the cast must match.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// What the spell gets.
        benefit: SpellBenefit,
    },
    /// A triggered ability of a permanent the rule's controller controls triggers an additional time when the event
    /// `cause` describes makes it trigger ("if a creature dying causes a triggered ability ... to trigger"), or any event
    /// when it names none.
    TriggerAgain {
        /// The event that causes the trigger, as a triggered ability describes it; every event when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cause: Option<TriggerPattern>,
        /// Only abilities of the permanents it selects trigger again; every permanent of the controller when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<Selector>,
    },
    /// Cards in graveyards can't be the targets of spells or abilities (CR 115.4).
    GraveyardCardsUntargetable,
    /// The players can't lose the game (CR 104.3), and with `opponents_cant_win` their opponents can't win it.
    CantLoseGame {
        /// The players who can't lose.
        who: Ref,
        /// Their opponents can't win the game either.
        #[serde(default, skip_serializing_if = "crate::effect::is_false")]
        opponents_cant_win: bool,
    },
    /// The players can't cast spells, only during the turns of the rule's controller with `your_turn`.
    CantCastSpells {
        /// The players who can't cast spells.
        who: Ref,
        /// Only during the turns of the rule's controller.
        #[serde(default, skip_serializing_if = "crate::effect::is_false")]
        your_turn: bool,
    },
    /// Each creature the players control assigns combat damage equal to its toughness rather than its power (CR 510.1a).
    ToughnessAssignsDamage(Ref),
    /// Counters remain on the card as it moves to any zone other than a hand or a library (CR 122.2); the card reads it
    /// in every zone, so the rule is written without `from`.
    CountersRemain,
    /// The owner of each exiled card `only` names may cast it for `cost` rather than its mana cost, while it stays exiled
    /// (airbend: CR 118.9).
    MayCastRather {
        /// The exiled cards, named when the rule begins.
        only: Ref,
        /// The mana paid in place of the mana cost.
        cost: ManaCostX,
    },
    /// The subjects `who` names (players or objects) can't do what `action` names: it is not offered, and an effect that
    /// would have it done does nothing.
    Cant {
        /// The players or objects the prohibition is about.
        who: Ref,
        /// What they can't do.
        action: Prohibited,
    },
    /// The mana the permanents `what` selects add as they are tapped for mana, when a player in `who` taps them, changes as
    /// `change` says (CR 614.1, 106.6): "If a land is tapped for mana, it produces {B} instead of any other type and amount."
    ManaProduced {
        /// The permanents, judged as they are tapped.
        what: Selector,
        /// The players whose taps are changed.
        who: Ref,
        /// The change.
        change: ManaChange,
    },
}

/// How a `GameRule::ManaProduced` changes the mana of a tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ManaChange {
    /// Every mana is of this type instead of any other type, in the same amount.
    OnlyType(ManaType),
    /// One mana of this type instead of any other type and amount.
    One(ManaType),
    /// As many times as much mana.
    Times(u32),
    /// Mana of the first type is of the second instead.
    Convert {
        /// The type that changes.
        from: ManaType,
        /// The type it becomes.
        to: ManaType,
    },
}

#[cfg(test)]
mod tests {
    //! Goal: every modification belongs to the layer CR 613 gives it, and the JSON forms are stable.
    use super::*;
    use crate::ability::Timing;
    use crate::cost::Cost;
    use crate::refs::ZoneKind;
    use rstest::rstest;

    fn granted() -> Arc<Ability> {
        Arc::new(Ability::Activated {
            cost: Cost(Vec::new()),
            timing: Timing::Instant,
            condition: None,
            limit: None,
            by: crate::ability::Activators::Controller,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: Vec::new(),
        })
    }

    #[rstest]
    #[case(Modification::SetController(Ref::You), Some("2"))]
    #[case(Modification::AddType(CardType::Artifact), Some("4"))]
    #[case(Modification::RemoveType(CardType::Creature), Some("4"))]
    #[case(Modification::SetTypes(vec![CardType::Land]), Some("4"))]
    #[case(Modification::RemoveSupertype(Supertype::Legendary), Some("4"))]
    #[case(Modification::GrantAbility(granted()), Some("6"))]
    #[case(Modification::LoseAbilities, Some("6"))]
    #[case(Modification::AddSubtype(Subtype("Elf".into())), Some("4"))]
    #[case(Modification::SetColors(vec![Color::Blue]), Some("5"))]
    #[case(Modification::AddKeyword(Keyword::Flying), Some("6"))]
    #[case(Modification::RemoveKeyword(Keyword::Flying), Some("6"))]
    #[case(Modification::LockKeyword(Keyword::Flying), Some("6"))]
    #[case(Modification::SetPT { power: Amount::Fixed(1), toughness: Amount::Fixed(1) }, Some("7b"))]
    #[case(Modification::ModifyPT { power: Amount::Fixed(1), toughness: Amount::Fixed(1) }, Some("7c"))]
    #[case(Modification::SwitchPT, Some("7d"))]
    #[case(Modification::CantAttack, None)]
    #[case(Modification::CantAttackYou(Attacked::Both), None)]
    #[case(Modification::CantBlock, None)]
    #[case(Modification::CantBeTurnedFaceUp, None)]
    #[case(Modification::CantBeBlocked, None)]
    #[case(Modification::Goaded, None)]
    #[case(Modification::MinBlockers(2), None)]
    #[case(Modification::MaxBlockers(1), None)]
    #[case(Modification::BlocksAdditional(1), None)]
    #[case(Modification::CanBlockOnly(Selector(vec![crate::refs::Pred::HasKeyword(Keyword::Flying)])), None)]
    #[case(Modification::CantBeBlockedBy(Selector(vec![crate::refs::Pred::Type(CardType::Creature)])), None)]
    fn layers_follow_the_kind(#[case] modification: Modification, #[case] layer: Option<&str>) {
        assert_eq!(modification.layer(), layer);
        let json = serde_json::to_string(&modification).unwrap();
        assert_eq!(
            serde_json::from_str::<Modification>(&json).unwrap(),
            modification
        );
    }

    #[rstest]
    #[case(
        GameRule::AttackTax {
            who: Ref::You,
            attacked: Attacked::Player,
            attackers: Selector::default(),
            mana: "{2}".parse().unwrap(),
            life: 0,
            per: Amount::Fixed(1),
        },
        r#"{"AttackTax":{"attacked":"Player","mana":"{2}"}}"#
    )]
    #[case(
        GameRule::AttackTax {
            who: Ref::You,
            attacked: Attacked::Both,
            attackers: Selector::default(),
            mana: ManaCostX::default(),
            life: 2,
            per: Amount::Fixed(1),
        },
        r#"{"AttackTax":{"attacked":"Both","life":2}}"#
    )]
    #[case(
        GameRule::AttackTax {
            who: Ref::Opponents,
            attacked: Attacked::Both,
            attackers: Selector(vec![crate::refs::Pred::Type(CardType::Creature)]),
            mana: "{W/P}".parse().unwrap(),
            life: 0,
            per: Amount::Fixed(2),
        },
        r#"{"AttackTax":{"who":"Opponents","attacked":"Both","attackers":[{"Type":"Creature"}],"mana":"{W/P}","per":2}}"#
    )]
    #[case(
        GameRule::DamageCantBePrevented { source: Selector::default(), combat: None },
        r#"{"DamageCantBePrevented":{}}"#
    )]
    #[case(
        GameRule::DamageCantBePrevented { source: Selector::this(), combat: Some(true) },
        r#"{"DamageCantBePrevented":{"source":["This"],"combat":true}}"#
    )]
    #[case(GameRule::MustAttack(Ref::This), r#"{"MustAttack":"This"}"#)]
    #[case(
        GameRule::MustAttackPlayers { who: Ref::Bound("tokens".to_owned()), players: Ref::Opponents },
        r#"{"MustAttackPlayers":{"who":{"Bound":"tokens"},"players":"Opponents"}}"#
    )]
    #[case(GameRule::MustBlock(Ref::This), r#"{"MustBlock":"This"}"#)]
    #[case(
        GameRule::MustBlockAttacker { who: Ref::Target(0), attacker: Ref::This },
        r#"{"MustBlockAttacker":{"who":{"Target":0},"attacker":"This"}}"#
    )]
    #[case(
        GameRule::Combat { who: Ref::This, quirk: CombatQuirk::CantAttackAlone },
        r#"{"Combat":{"who":"This","quirk":"CantAttackAlone"}}"#
    )]
    #[case(
        GameRule::Combat { who: Ref::This, quirk: CombatQuirk::MustBeBlocked(Blocking::Every) },
        r#"{"Combat":{"who":"This","quirk":{"MustBeBlocked":"Every"}}}"#
    )]
    #[case(
        GameRule::IgnoreHexproof { by: Ref::You, what: None, players: Some(Ref::Opponents), shroud: true },
        r#"{"IgnoreHexproof":{"by":"You","players":"Opponents","shroud":true}}"#
    )]
    #[case(GameRule::CantBeCountered(Ref::This), r#"{"CantBeCountered":"This"}"#)]
    #[case(GameRule::AttackLimit(1), r#"{"AttackLimit":1}"#)]
    #[case(
        GameRule::AttackLimitAt { most: 2, at: Ref::You },
        r#"{"AttackLimitAt":{"most":2,"at":"You"}}"#
    )]
    #[case(GameRule::BlockLimit(2), r#"{"BlockLimit":2}"#)]
    #[case(
        GameRule::BlockLimitEach { most: 1, who: Ref::Opponents },
        r#"{"BlockLimitEach":{"most":1,"who":"Opponents"}}"#
    )]
    #[case(
        GameRule::CantGainLife(Ref::EachPlayer),
        r#"{"CantGainLife":"EachPlayer"}"#
    )]
    #[case(
        GameRule::NoLifeLossFromDamage(Ref::You),
        r#"{"NoLifeLossFromDamage":"You"}"#
    )]
    #[case(GameRule::CantDraw(Ref::Opponents), r#"{"CantDraw":"Opponents"}"#)]
    #[case(GameRule::CantSearch(Ref::You), r#"{"CantSearch":"You"}"#)]
    #[case(GameRule::SkipDrawStep(Ref::You), r#"{"SkipDrawStep":"You"}"#)]
    #[case(GameRule::RevealTop(Ref::You), r#"{"RevealTop":"You"}"#)]
    #[case(GameRule::LookAtTopAnyTime(Ref::You), r#"{"LookAtTopAnyTime":"You"}"#)]
    #[case(
        GameRule::NoMaxHandSize(Ref::EachPlayer),
        r#"{"NoMaxHandSize":"EachPlayer"}"#
    )]
    #[case(
        GameRule::AdditionalLandPlays { who: Ref::You, count: Amount::Fixed(1) },
        r#"{"AdditionalLandPlays":{}}"#
    )]
    #[case(
        GameRule::AdditionalLandPlays { who: Ref::EachPlayer, count: Amount::Fixed(2) },
        r#"{"AdditionalLandPlays":{"who":"EachPlayer","count":2}}"#
    )]
    #[case(GameRule::CanBeCommander, r#""CanBeCommander""#)]
    #[case(
        GameRule::GraveyardCardsUntargetable,
        r#""GraveyardCardsUntargetable""#
    )]
    #[case(
        GameRule::CantLoseGame { who: Ref::You, opponents_cant_win: true },
        r#"{"CantLoseGame":{"who":"You","opponents_cant_win":true}}"#
    )]
    #[case(
        GameRule::CantCastSpells { who: Ref::Opponents, your_turn: true },
        r#"{"CantCastSpells":{"who":"Opponents","your_turn":true}}"#
    )]
    #[case(
        GameRule::ToughnessAssignsDamage(Ref::You),
        r#"{"ToughnessAssignsDamage":"You"}"#
    )]
    #[case(GameRule::CountersRemain, r#""CountersRemain""#)]
    #[case(
        GameRule::TriggerAgain {
            cause: Some(TriggerPattern::Attacks { what: Selector::this() }),
            source: None
        },
        r#"{"TriggerAgain":{"cause":{"Attacks":{"what":["This"]}}}}"#
    )]
    #[case(GameRule::TriggerAgain { cause: None, source: None }, r#"{"TriggerAgain":{}}"#)]
    #[case(
        GameRule::Cant { who: Ref::You, action: Prohibited::PlayLand },
        r#"{"Cant":{"who":"You","action":"PlayLand"}}"#
    )]
    #[case(
        GameRule::Cant { who: Ref::Opponents, action: Prohibited::Activate(Selector::this()) },
        r#"{"Cant":{"who":"Opponents","action":{"Activate":["This"]}}}"#
    )]
    #[case(
        GameRule::Cant { who: Ref::You, action: Prohibited::SkipStep(Step::Draw) },
        r#"{"Cant":{"who":"You","action":{"SkipStep":"Draw"}}}"#
    )]
    #[case(
        GameRule::MaxHandSize { who: Ref::You, size: Amount::Fixed(3) },
        r#"{"MaxHandSize":{"who":"You","size":3}}"#
    )]
    #[case(
        GameRule::NoLegendRule(Ref::EachPlayer),
        r#"{"NoLegendRule":"EachPlayer"}"#
    )]
    #[case(
        GameRule::UntapDuring { what: Selector::this(), whose: Ref::Opponents },
        r#"{"UntapDuring":{"what":["This"],"whose":"Opponents"}}"#
    )]
    #[case(
        GameRule::TriggerAgain { cause: Some(TriggerPattern::Attacks { what: Selector::this() }), source: Some(Selector::this()) },
        r#"{"TriggerAgain":{"cause":{"Attacks":{"what":["This"]}},"source":["This"]}}"#
    )]
    fn a_rule_is_written_with_the_players_or_objects_it_names(
        #[case] rule: GameRule,
        #[case] json: &str,
    ) {
        assert_eq!(serde_json::to_string(&rule).unwrap(), json);
        assert_eq!(serde_json::from_str::<GameRule>(json).unwrap(), rule);
    }

    #[rstest]
    #[case(Duration::UntilEndOfTurn, r#""UntilEndOfTurn""#)]
    #[case(Duration::WhileYouControlSource, r#""WhileYouControlSource""#)]
    #[case(Duration::WhileSourceTapped, r#""WhileSourceTapped""#)]
    #[case(
        Duration::WhileHasCounter(Box::new(CounterKind::Other("vow".into()))),
        r#"{"WhileHasCounter":{"Other":"vow"}}"#
    )]
    #[case(
        Duration::WhileHasCounter(Box::new(CounterKind::PlusOnePlusOne)),
        r#"{"WhileHasCounter":"+1/+1"}"#
    )]
    fn a_duration_is_a_word_or_follows_a_counter_kind(
        #[case] duration: Duration,
        #[case] json: &str,
    ) {
        assert_eq!(serde_json::to_string(&duration).unwrap(), json);
        assert_eq!(serde_json::from_str::<Duration>(json).unwrap(), duration);
    }

    #[test]
    fn a_pump_is_written_with_bare_numbers() {
        let pump = Modification::ModifyPT {
            power: Amount::Fixed(3),
            toughness: Amount::Fixed(3),
        };
        assert_eq!(
            serde_json::to_string(&pump).unwrap(),
            r#"{"ModifyPT":{"power":3,"toughness":3}}"#
        );
    }
}
