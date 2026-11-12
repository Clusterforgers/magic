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
    /// Adds the creature type the resolution chose under this name (layer 4): "becomes the creature type of your choice
    /// in addition to its other types". Only an effect of a spell or ability has it.
    AddCreatureTypeChosen(String),
    /// Changes the text (layer 3, CR 612): every instance of the word the resolution chose under `from` becomes the word it
    /// chose under `to` ("replacing all instances of one color word with another"). Only an effect of a spell or ability
    /// has it.
    ChangeTextChosen {
        /// The name of the word replaced.
        from: String,
        /// The name of the word written.
        to: String,
    },
    /// Adds the creature type the source chose as it entered (layer 4): "is the chosen type in addition to its
    /// other types". Only a static ability has it; with no type chosen it changes nothing.
    AddChosenSubtype,
    /// Makes the colour the source chose as it entered the only colour (layer 5): "is the chosen color". Only a static
    /// ability has it; with no colour chosen it changes nothing.
    SetChosenColor,
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
    /// Removes every keyword ability of the kind, whatever its parameter (layer 6): "loses all landwalk abilities".
    RemoveKeywordKind(crate::rmt7_kinds::KeywordKind),
    /// Removes a keyword ability and the object can't have or gain it afterwards, whichever effect is newer (layer 6):
    /// "lose flying and can't have or gain flying".
    LockKeyword(Keyword),
    /// Gives the object an activated or a triggered ability (layer 6); a keyword is `AddKeyword`.
    GrantAbility(Arc<Ability>),
    /// The object has the activated abilities printed on the cards the reference names, wherever they are (layer 6): "has all
    /// activated abilities of all creature cards in graveyards". A card that leaves its zone is a new object and gives none.
    GainAbilitiesOf(Ref),
    /// The object has each keyword of `names` that one of the objects `from` names has, with its parameter (layer 6): "As
    /// long as a card exiled with this creature has flying, this creature has flying. The same is true for ...". A name is
    /// a keyword's name ("Flying", "First strike"); "Landwalk" and "Protection" take every landwalk and protection.
    KeywordsOf {
        /// The objects whose keywords are read.
        from: Ref,
        /// The keywords given, by name, in the order printed.
        names: Vec<String>,
    },
    /// The object loses all its abilities (layer 6); abilities granted after it still count.
    LoseAbilities,
    /// The object loses all its abilities except mana abilities (layer 6, CR 605): "lose all abilities except mana
    /// abilities"; abilities granted after it still count.
    LoseAbilitiesExceptMana,
    /// The object loses the ability that made the effect, as the effect begins (layer 6): "~ loses this ability". Only an
    /// effect of an activated ability has it, never a static ability.
    LoseThisAbility,
    /// The object loses the ability printed on its card that equals this one, as the effect begins (layer 6): "this
    /// creature loses \"Prevent all damage that would be dealt to this creature.\"". Only an effect on its own source has it.
    LosePrinted(Arc<Ability>),
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
    /// The object can block an additional creature each combat for each one the amount counts, worked out as the
    /// layers are (a restriction, no layer): "for each Equipment attached to this creature".
    BlocksAdditionalPer(Amount),
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
            Modification::ChangeTextChosen { .. } => Some("3"),
            Modification::AddType(_)
            | Modification::RemoveType(_)
            | Modification::SetTypes(_)
            | Modification::RemoveSupertype(_)
            | Modification::AddSubtype(_)
            | Modification::SetCreatureTypes(_)
            | Modification::SetLandTypes(_)
            | Modification::SetLandTypesChosen(_)
            | Modification::SetCreatureTypesChosen(_)
            | Modification::AddCreatureTypeChosen(_)
            | Modification::AddChosenSubtype
            | Modification::AddAllCreatureTypes => Some("4"),
            Modification::SetColors(_)
            | Modification::AddColors(_)
            | Modification::SetColorsChosen(_)
            | Modification::SetChosenColor => Some("5"),
            Modification::AddKeyword(_)
            | Modification::AddProtectionFromChosenColor
            | Modification::AddProtectionFromChosenCardType
            | Modification::LockKeyword(_)
            | Modification::RemoveKeyword(_)
            | Modification::RemoveKeywordKind(_)
            | Modification::GrantAbility(_)
            | Modification::GainAbilitiesOf(_)
            | Modification::KeywordsOf { .. }
            | Modification::LoseThisAbility
            | Modification::LosePrinted(_)
            | Modification::LoseAbilities
            | Modification::LoseAbilitiesExceptMana => Some("6"),
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
            | Modification::BlocksAdditionalPer(_)
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
    /// Until the controller's next end step begins: the one of this turn when it has not begun yet (CR 513).
    UntilYourNextEndStep,
    /// Until the controller's next upkeep begins: the one of a later turn, so one begun in their upkeep lasts until the
    /// upkeep of their next turn.
    UntilYourNextUpkeep,
    /// Until the end of the controller's next turn.
    UntilEndOfYourNextTurn,
    /// Only during the next turn of the player who controls the permanent it applies to as it begins ("that creature can't
    /// attack during its controller's next turn"): it changes nothing before that turn and ends with it. Only a continuous
    /// effect lasts this way.
    DuringItsControllersNextTurn,
    /// Until the end of combat on the controller's next turn: it ends as that turn's postcombat main phase begins
    /// ("Until end of combat on your next turn, you may play that card").
    UntilEndOfCombatOnYourNextTurn,
    /// For as long as the source stays on the battlefield.
    WhileSourceOnBattlefield,
    /// For as long as the controller of the effect controls the source ("gain control of target creature for as long as you control ~"): it ends when the source leaves the battlefield or another player controls it (CR 611.2b).
    WhileYouControlSource,
    /// For as long as the source stays tapped ("for as long as ~ remains tapped", CR 611.2b): it ends when the source untaps, leaves
    /// the battlefield or is another object, and does not come back when it is tapped again. Only a continuous effect lasts this long.
    WhileSourceTapped,
    /// For as long as the controller of the effect controls the source and the source stays tapped ("for as long as you
    /// control ~ and ~ remains tapped", CR 611.2b): it ends when either stops. Only a continuous effect lasts this long.
    WhileYouControlSourceTapped,
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
    /// Until the step or combat phase its rule makes a player skip has been skipped once ("skips their next untap step",
    /// "skips their next combat phase", CR 614.10): only the rule that a player skips a step or combat lasts this long.
    UntilSkipped,
    /// As `UntilSkipped`, and it also ends with the turn ("skips their next combat phase this turn").
    UntilSkippedThisTurn,
    /// Until the card the source became when its ability exiled it is cast from exile ("until this card is cast from
    /// exile"); it lasts for good once that card leaves exile another way.
    UntilCastFromExile,
    /// Without end (until the object or the game is gone).
    Forever,
    /// Until the controller of the effect pays this mana as a special action, any time they have priority ("you may pay
    /// {W} to end this effect", CR 116.2c), or the object is gone. Only a continuous effect that names its objects lasts this way.
    UntilPaid(#[cfg_attr(feature = "ts", ts(type = "string"))] ManaCostX),
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
    /// What a cast through it pays rather than the mana cost (CR 118.9): "pay life equal to its mana value rather than pay
    /// its mana cost"; an amount in it counts the card cast as `This`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rather: Option<Cost>,
    /// How the mana of a cast through it may be spent: as mana of any color or of any type ("spend mana as though it were mana of any color").
    #[serde(default, skip_serializing_if = "SpendAs::is_normal")]
    pub spend_as: SpendAs,
    /// The permission is open only while this holds for the controller of the rule ("during any turn you attacked with a
    /// token, you may play that card"); always when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Condition>,
    /// It opens no zone: its free cast or `rather` cost is offered for every cast the player may make anyway, from a hand,
    /// a command zone or a zone another permission opens ("You may cast Dragon spells without paying their mana costs").
    #[serde(default, skip_serializing_if = "crate::effect::is_false")]
    pub any_cast: bool,
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
    /// They can't attack unless at least `others` other creatures the selector picks attack too (CR 508.1c): "can't
    /// attack unless at least two other creatures attack"; the selector reads the creature itself as `This`.
    AttackWithOthers {
        /// How many other attackers at least.
        others: u32,
        /// The others that count; any creature when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        among: Selector,
    },
    /// They can't block unless at least `others` other creatures the selector picks block too (CR 509.1b).
    BlockWithOthers {
        /// How many other blockers at least.
        others: u32,
        /// The others that count; any creature when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        among: Selector,
    },
    /// They can attack only alone (CR 508.1c).
    AttackAlone,
    /// If they attack, every other creature their controller controls that can attack for nothing attacks too ("If this
    /// creature attacks, all creatures you control attack if able."): a declaration with them and without such a creature is
    /// refused.
    AttackWithAll,
    /// They can attack as though they did not have defender.
    AttackDespiteDefender,
    /// They can attack as though they had haste (CR 302.6): "Enchanted creature can attack as though it had haste."
    AttackAsThoughHaste,
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
    /// Every creature the selector picks that is able to block them does so (CR 509.1c), judged for their controller:
    /// "All Walls able to block ~ do so."
    MustBeBlockedBy(Selector),
    /// They can be blocked as though they did not have the landwalk ability the selector names (CR 702.14).
    IgnoreLandwalk(Selector),
    /// They can be blocked as though they did not have any landwalk ability.
    IgnoreAllLandwalk,
    /// They can block creatures with landwalk as though those did not have it.
    BlockIgnoringLandwalk,
    /// They can block creatures with shadow as though they had shadow (CR 702.28).
    BlockAsThoughShadow,
    /// They block the creatures picked as though they had reach.
    BlockAsThoughReach(Selector),
    /// They can block creatures with shadow as though those did not have shadow (CR 702.28).
    BlockIgnoringShadow,
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
    /// Have counters of one kind removed from them (CR 122): "Stun counters can't be removed from permanents your
    /// opponents control."; a stun counter that would be removed in place of an untap stays and the permanent stays
    /// tapped.
    RemoveCounterOf(crate::value::CounterKind),
    /// Have triggered abilities of the subjects' permanents trigger from the event the pattern describes (CR 603).
    Trigger(TriggerPattern),
    /// Become attached, or have anything attached to them (CR 701.3).
    Attach,
    /// Have the attachments the selector picks attached to them: such an attachment can't become attached, and one attached
    /// already is attached illegally (CR 303.4d, 301.5c, 704.5m, 704.5n).
    BeAttachedBy(Selector),
    /// Discard cards, or be discarded (CR 701.9).
    Discard,
    /// Be exiled (CR 406).
    Exile,
    /// Transform (CR 701.28).
    Transform,
    /// Phase out (CR 702.26).
    PhaseOut,
    /// Become untapped, by an untap step, an effect or a {Q} cost.
    Untap,
    /// Crew Vehicles: be tapped to pay a crew cost (CR 702.122).
    Crew,
    /// Become suspected (CR 701.60): an effect that would suspect it does nothing to it.
    BecomeSuspected,
    /// Phase in (CR 702.26): a phased-out permanent covered stays phased out through its controller's untap step and any
    /// other moment it would phase in.
    PhaseIn,
    /// Enter the battlefield from the zone they are in (CR 400.6): the move does not happen, and the card stays where it is.
    EnterBattlefield,
    /// Skip the step on their turns (CR 614.10); the cleanup step is never skipped.
    SkipStep(Step),
    /// Skip their turns (CR 614.10).
    SkipTurn,
    /// Skip the steps of their combat phases (CR 614.10, 506.1).
    SkipCombat,
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
    /// It can't be countered (CR 101.2) for as long as it is on the stack: "the next spell you cast this turn can't be
    /// countered".
    Uncounterable,
    /// It may be cast by paying this cost rather than its mana cost (CR 118.9): "Rather than pay the mana cost of the next
    /// spell you cast this turn, you may pay life equal to that spell's mana value"; an amount in it counts the card cast
    /// as `This`.
    Rather(crate::cost::Cost),
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
    /// Creatures can't block unless their controller pays the mana or life for each of them as blockers are declared
    /// (CR 509.1d): the declaration is made only with the payment.
    BlockTax {
        /// The blocking creatures it taxes, judged when blockers are declared; every creature when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        blockers: Selector,
        /// The attackers whose blockers it taxes, judged the same way; every attacker when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        attackers: Selector,
        /// The mana each taxed creature costs, once for each of `per`; no `{X}`.
        #[serde(default, skip_serializing_if = "no_mana")]
        mana: ManaCostX,
        /// The life each taxed creature costs, once for each of `per`.
        #[serde(default, skip_serializing_if = "crate::spend::is_zero")]
        life: u32,
        /// How many times the cost is paid for each creature, counted when the rule began or the state last changed.
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
    /// The players get an additional vote each time they vote: "While voting, you get an additional vote." (CR 701.38d).
    ExtraVote(Ref),
    /// A player in the group who would begin an extra turn skips that turn instead (CR 500.7).
    SkipExtraTurns(Ref),
    /// A player in the group who would flip a coin flips two instead and ignores one (CR 705.3, Krark's Thumb); each such
    /// rule doubles the coins again.
    FlipTwoCoins(Ref),
    /// A player in the group who would roll one or more dice rolls that many plus one instead and ignores the lowest roll
    /// (Wyll, Blade of Frontiers); each such rule adds one more die and one more ignored roll.
    ExtraDie(Ref),
    /// The players play with the top card of their library revealed to every seat.
    RevealTop(Ref),
    /// The players play with their hands revealed.
    RevealHand(Ref),
    /// Players may cast cards the ordinary rules do not let them cast: from another zone, without paying their mana
    /// costs, or as though they had flash (CR 601.3).
    MayCast(CastPermission),
    /// The players may look at the top card of their own library any time; no one else sees it.
    LookAtTopAnyTime(Ref),
    /// The players may look at the cards in exile the selector picks that lie face down (CR 406.3): "You may look at the
    /// cards exiled with ~".
    LookAtExiled {
        /// The players who may look; the controller of the rule when absent.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The cards they may look at; every face-down card in exile when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// Only the objects the reference names when the rule begins, as the objects they are then (CR 400.7).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        only: Option<Ref>,
    },
    /// The players may look at the face-down permanents the selector picks any time (CR 708.5): "You may look at face-down
    /// creatures you don't control any time."
    LookAtFaceDown {
        /// The players who may look; the controller of the rule when absent.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// The permanents they may look at; every face-down permanent when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
    },
    /// The players have no maximum hand size (CR 402.2).
    NoMaxHandSize(Ref),
    /// The players' maximum hand size is `size` (CR 402.2): the least of the sizes in force.
    MaxHandSize {
        /// The players.
        who: Ref,
        /// The cards they may keep at cleanup.
        size: Amount,
    },
    /// The players' maximum hand size grows by `by` (shrinks when negative), after any size that is set (CR 402.2): "your maximum
    /// hand size is reduced by three".
    HandSizeChange {
        /// The players.
        who: Ref,
        /// The cards added to the size; removed when negative.
        by: Amount,
    },
    /// The rule's controller controls the players while they search their libraries (CR 722): "You control your
    /// opponents while they're searching their libraries."
    ControlSearches(Ref),
    /// The players exile what they find in their libraries; you may play it from exile (Opposition Agent).
    ExileFound(Ref),
    /// The legend rule (CR 704.5j) does not apply to the legendary permanents of the players.
    NoLegendRule(Ref),
    /// The legend rule (CR 704.5j) does not apply to the permanents the selector picks, judged for the rule's controller:
    /// "the legend rule doesn't apply to tokens you control".
    NoLegendRuleFor(Selector),
    /// The permanents the selector picks that the rule's controller controls untap during the untap step of each of the
    /// players (CR 502.3): "untap all creatures you control during each other player's untap step".
    UntapDuring {
        /// The permanents, among those the rule's controller controls.
        what: Selector,
        /// The players whose untap steps untap them.
        whose: Ref,
    },
    /// The players can't untap more than `count` of their permanents the selector picks during their untap steps (CR
    /// 502.3): "players can't untap more than one land during their untap steps".
    UntapLimit {
        /// The players whose untap steps are limited.
        who: Ref,
        /// The permanents counted, among those the untapping player controls.
        what: Selector,
        /// The most of them that untap.
        count: u32,
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
    /// A deck can hold any number of cards with the card's name (CR 100.2a); it changes nothing in a game, and the rule is
    /// written without `from`.
    AnyNumberInDeck,
    /// A deck can hold up to this many cards with the card's name (CR 100.2a, "A deck can have up to nine cards named
    /// ~"); it changes nothing in a game, and the rule is written without `from`.
    UpToInDeck(u32),
    /// A booster draft rule: nothing in a game; no `from`.
    Draft(DraftRule),
    /// "This spell can't be copied." (CR 707.10): an effect that would copy the spell makes no copy of it; no `from`.
    CantBeCopied,
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
    /// The spells `applies_to` selects cost `mana` more or less to cast while the rule lasts (CR 601.2f): "spells your
    /// opponents cast cost {1} more to cast until your next turn"; "you" in the selector is the rule's controller.
    SpellCost {
        /// The spells, with `Spell` first, as a cost modifier names them.
        applies_to: Selector,
        /// More or less.
        change: crate::ability::CostChange,
        /// The mana added or removed.
        mana: ManaCostX,
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
    /// All damage is dealt as though its source had wither (CR 702.80): damage to a creature is dealt as -1/-1
    /// counters.
    DamageAsWither,
    /// A player losing unspent mana loses that much life (mana burn): "A player losing unspent mana causes that player to
    /// lose that much life."
    ManaBurn,
    /// Every creature can attack the players and the planeswalkers they control as though it had haste (CR 302.6): "All
    /// creatures can attack your opponents and planeswalkers your opponents control as though those creatures had haste."
    AttackAsThoughHasteAt(Ref),
    /// The players can't lose the game (CR 104.3), and with `opponents_cant_win` their opponents can't win it.
    CantLoseGame {
        /// The players who can't lose.
        who: Ref,
        /// Their opponents can't win the game either.
        #[serde(default, skip_serializing_if = "crate::effect::is_false")]
        opponents_cant_win: bool,
        /// Only the loss for 0 or less life (CR 704.5a).
        #[serde(default, skip_serializing_if = "crate::effect::is_false")]
        life_only: bool,
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
    /// The players don't lose unspent mana of the type (of every type when absent) as steps and phases end (CR 500.4):
    /// "You don't lose unspent red mana as steps and phases end."
    KeepUnspentMana {
        /// The players.
        who: Ref,
        /// The type kept; every type when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mana: Option<ManaType>,
    },
    /// The controller may activate abilities of the creatures the selector picks as though they had haste (CR 302.6): "You
    /// may activate abilities of creatures you control as though those creatures had haste." They still can't attack.
    AbilitiesAsThoughHaste(Selector),
    /// For each mana symbol of the color in a cost, the players may pay 2 life rather than pay that mana: "For each {B} in a
    /// cost, you may pay 2 life rather than pay that mana."
    PayLifeForMana {
        /// The players.
        who: Ref,
        /// The color of the symbols.
        color: Color,
    },
    /// The players may pay 2 life as they cast a spell the selector picks to have it cost one mana symbol of the color
    /// less, and only that color: "As an additional cost to cast green permanent spells, you may pay 2 life. Those spells
    /// cost {G} less to cast if you paid life this way. This effect reduces only the amount of green mana you pay."
    PayLifeForOneSymbol {
        /// The players.
        who: Ref,
        /// The color of the symbol.
        color: Color,
        /// The spells, judged as they are cast.
        spells: Selector,
    },
    /// The players draw no more than `most` cards each turn; the draws past it don't happen: "Each opponent can't draw more
    /// than one card each turn."
    DrawLimit {
        /// The players.
        who: Ref,
        /// The cards each may draw in a turn.
        most: u32,
    },
    /// Spells and abilities the opponents of the players control can't cause them to sacrifice permanents; costs they pay
    /// themselves still can.
    OpponentsCantMakeSacrifice(Ref),
    /// The players may spend mana as though it were mana of any color or of any type (CR 609.4b): "You may spend mana as
    /// though it were mana of any color", "... to cast creature spells".
    SpendManaAs {
        /// The players.
        who: Ref,
        /// Any color or any type.
        spend_as: SpendAs,
        /// Only to cast the spells this selects, judged as they are cast; every spell and ability when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spells: Option<Selector>,
    },
    /// Unspent mana the players would lose as a step or phase ends becomes mana of `into` instead and stays: "If you would
    /// lose unspent mana, that mana becomes colorless instead."
    UnspentManaBecomes {
        /// The players.
        who: Ref,
        /// The type the mana becomes.
        into: ManaType,
    },
    /// The players may activate the loyalty abilities of each planeswalker they control twice each turn rather than once
    /// (CR 606.3): "You may activate the loyalty abilities of planeswalkers you control twice each turn rather than only
    /// once." Two such rules still allow two.
    LoyaltyTwice(Ref),
    /// The players may activate one more loyalty ability of each planeswalker they control this turn, as though none had
    /// been activated (The Chain Veil); each such rule allows one more.
    LoyaltyOnceMore(Ref),
    /// The players may activate loyalty abilities of planeswalkers they control on any player's turn, any time they could
    /// cast an instant (CR 606.3).
    LoyaltyAsInstant(Ref),
    /// The loyalty abilities the players activate cost an additional `[+plus]` (CR 606.4, Carth the Lion); the total
    /// cost is one change of counters, and such rules add up.
    LoyaltyCostMore {
        /// The players who activate.
        who: Ref,
        /// The loyalty counters added to the cost.
        plus: u32,
    },
    /// While a player in `who` chooses targets as part of casting a spell or activating an ability they control, they must
    /// choose at least one permanent `what` selects if able (Flagbearer): "While an opponent is choosing targets as part
    /// of casting a spell they control or activating an ability they control, that player must choose at least one
    /// Flagbearer on the battlefield if able." Several such rules ask for one such target in all.
    MustTarget {
        /// The players choosing.
        who: Ref,
        /// The permanents one of which must be a target.
        what: Selector,
    },
}

/// A rule of a booster draft; each variant is one printed sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum DraftRule {
    /// Draft it face up.
    FaceUp,
    /// Draft this many cards at random while face up.
    AtRandom {
        /// The cards drafted at random.
        picks: u32,
    },
    /// Look at the next pick from the pack.
    LookAtNextPick,
    /// Last cards of packs go to its drafter.
    LastCardsPassed,
    /// Trade a card after the draft.
    TradeAfterDraft,
    /// Reveal it as drafted; its drafter may then add a booster pack to the draft.
    AddBoosterPack,
    /// Reveal it as drafted and note how many cards its drafter has drafted this round, itself included.
    NoteDraftedCount,
    /// Reveal it as drafted and note the player who passed it.
    NotePasser,
    /// Reveal it as drafted, then reveal the next card drafted and note its name.
    NoteNextName,
    /// Reveal it as drafted, then guess the name of the next card drafted from its booster pack.
    GuessNextPick,
    /// Turn it face down during the draft to look at the next card a chosen player drafts.
    LookAtNextDrafted,
    /// Turn it face down during the draft to look at an unopened booster pack or one nobody looks at.
    LookAtAnyPack,
    /// Draft a whole booster pack in place of one card, then draft no more cards that round.
    DraftWholePack,
    /// Draft an additional card from a pack, putting this card into it.
    DraftAdditional,
    /// Draft an additional card from a pack, turning this card face down and passing the next pack without a pick.
    DraftAdditionalPassNext,
    /// Remove a card drafted from the draft, face down or face up.
    RemoveDrafted {
        /// It is removed face up.
        face_up: bool,
    },
    /// Reveal a drafted card and note its name, then turn this card face down.
    NoteName,
    /// Reveal a drafted creature card and note its creature types, then turn this card face down.
    NoteCreatureTypes,
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
    use crate::refs::{Pred, ZoneKind};
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
    #[case(Modification::AddChosenSubtype, Some("4"))]
    #[case(Modification::SetChosenColor, Some("5"))]
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
        GameRule::BlockTax {
            blockers: Selector::this(),
            attackers: Selector::default(),
            mana: "{2}".parse().unwrap(),
            life: 0,
            per: Amount::Fixed(1),
        },
        r#"{"BlockTax":{"blockers":["This"],"mana":"{2}"}}"#
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
    #[case(GameRule::AnyNumberInDeck, r#""AnyNumberInDeck""#)]
    #[case(GameRule::UpToInDeck(9), r#"{"UpToInDeck":9}"#)]
    #[case(
        GameRule::GraveyardCardsUntargetable,
        r#""GraveyardCardsUntargetable""#
    )]
    #[case(
        GameRule::CantLoseGame { who: Ref::You, opponents_cant_win: true, life_only: false },
        r#"{"CantLoseGame":{"who":"You","opponents_cant_win":true}}"#
    )]
    #[case(
        GameRule::CantLoseGame { who: Ref::You, opponents_cant_win: false, life_only: true },
        r#"{"CantLoseGame":{"who":"You","life_only":true}}"#
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
        GameRule::UntapLimit { who: Ref::EachPlayer, what: Selector(vec![Pred::Type(CardType::Land)]), count: 1 },
        r#"{"UntapLimit":{"who":"EachPlayer","what":[{"Type":"Land"}],"count":1}}"#
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
        Duration::WhileYouControlSourceTapped,
        r#""WhileYouControlSourceTapped""#
    )]
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
