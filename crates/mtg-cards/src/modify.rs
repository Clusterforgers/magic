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
    /// Names the copiable values of a copy: "except its name is Lazav, Dimir Mastermind" (layer 1); as the change of a
    /// static ability or an effect it replaces the name (layer 3): "named Legitimate Businessperson".
    SetName(String),
    /// Gives the copiable values of a copy the ability that is resolving: "except it has this ability". Only the
    /// exceptions of a `BecomeCopy` have it.
    ThisAbility,
    /// Sets the starting loyalty of a copy: "except its starting loyalty is 1". Only the exceptions of a copy have it.
    SetLoyalty(Amount),
    /// Adds a subtype (layer 4).
    AddSubtype(Subtype),
    /// Replaces the creature types with these (layer 4): "becomes a red Weird" (CR 205.1b).
    SetCreatureTypes(Vec<Subtype>),
    /// Gives each of the listed keywords that one of the objects the reference names has, with the variant it has
    /// ("hexproof from white" for hexproof) (layer 6): "If a creature card with flying was exiled with this creature's
    /// delve ability, this creature has flying. The same is true for ..." (Soulflayer).
    KeywordsOf {
        /// The objects whose keywords are read.
        from: Box<Ref>,
        /// The keywords given.
        keywords: Vec<Keyword>,
    },
    /// Replaces the creature types with those of the object the reference names (layer 4): "has the creature types of the
    /// last creature card exiled with it" (Duplicant).
    SetCreatureTypesOf(Ref),
    /// Makes the land type at this place among the basic land types the source chose as it entered the land types (layer
    /// 4): "are the second chosen type" (Illusionary Terrain).
    SetLandTypesNthChosen(u8),
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
    /// Adds the basic land type the resolution chose under this name (layer 4): "becomes the basic land type of your
    /// choice in addition to its other types". Only an effect of a spell or ability has it.
    AddLandTypeChosen(String),
    /// Changes the text (layer 3, CR 612): every instance of the word the resolution chose under `from` becomes the word it
    /// chose under `to` ("replacing all instances of one color word with another"). Only an effect of a spell or ability
    /// has it.
    ChangeTextChosen {
        /// The name of the word replaced.
        from: String,
        /// The name of the word written.
        to: String,
    },
    /// Changes the text (layer 3, CR 612): every instance of the creature type the resolution chose under `from` becomes
    /// the creature type `to` ("replacing all instances of one creature type with Vampire"). Only an effect of a spell or
    /// ability has it.
    ChangeTextChosenTo {
        /// The name of the word replaced.
        from: String,
        /// The creature type written.
        to: String,
    },
    /// The object is a copy of the first object `of` names (layer 1, CR 707.2, 613.2a), with the exceptions in its copiable
    /// values (CR 707.9b): "Enchanted creature is a copy of the chosen creature", "this creature has the full text of that
    /// card". Only a static ability has it; it is read from the card's own rules, works out again whenever what `of`
    /// names changes, and ends once it names nothing.
    R707CopyOf {
        /// The object copied.
        of: Ref,
        /// Changes the copy has.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        except: Vec<Modification>,
    },
    /// The two objects the effect changes exchange their text boxes (layer 3, CR 612.1, 613.1c): each has the rules text
    /// the other's copiable values give it, keywords and abilities, and keeps its name, types and other characteristics
    /// ("exchange the text boxes of those creatures"). Only an effect of a spell or ability has it; with any other number
    /// of objects on the battlefield it changes nothing.
    R612ExchangeTextBoxes,
    /// Adds the creature type the source chose as it entered (layer 4): "is the chosen type in addition to its
    /// other types". Only a static ability has it; with no type chosen it changes nothing.
    AddChosenSubtype,
    /// Makes the colour the source chose as it entered the only colour (layer 5): "is the chosen color". Only a static
    /// ability has it; with no colour chosen it changes nothing.
    SetChosenColor,
    /// Adds the colour the source chose as it entered to the colours it has (layer 5): "are the chosen color in addition
    /// to their other colors". Only a static ability has it; with no colour chosen it changes nothing.
    AddChosenColor,
    /// Adds each ability the source chose as it entered from among first strike, vigilance, and lifelink (layer 6):
    /// "have each of the chosen abilities". Only a static ability has it; with nothing chosen it changes nothing.
    R6001AddChosenAbilities,
    /// Sets the color to the one the card's owner chose for it before the game began as their commander (layer 5): "If ~
    /// is your commander, choose a color before the game begins. ~ is the chosen color." Only the card's own
    /// characteristic-defining static ability has it; a card nobody chose a color for keeps its colors.
    SetCommanderColor,
    /// Adds a keyword ability (layer 6).
    AddKeyword(Keyword),
    /// Adds a numbered keyword whose number is counted for each object as the layer applies (layer 6): "~ has annihilator
    /// X, where X is the number of +1/+1 counters on it". The keyword's own number is a placeholder.
    R6601KeywordX {
        /// The keyword, its number replaced by the count.
        keyword: Keyword,
        /// The number, read with the object as `Affected`.
        count: Amount,
    },
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
    /// The object has the activated abilities of the objects the reference names (layer 6, CR 613.1f): a permanent's as it has
    /// them now (granted ones and its land types' mana abilities included), a card's elsewhere as printed: "has all activated
    /// abilities of all creature cards in graveyards". A card that leaves its zone is a new object and gives none.
    GainAbilitiesOf(Ref),
    /// The object has the activated abilities of the objects the reference names that are no mana abilities (layer 6, CR
    /// 605.1a), as `GainAbilitiesOf` gives them: "has all activated abilities of lands your opponents control except mana
    /// abilities".
    R111GainNonManaAbilitiesOf(Ref),
    /// The object has the activated and triggered abilities printed on the cards the reference names, wherever they are
    /// (layer 6): "~ has all activated and triggered abilities of the last chosen card".
    R6702GainActivatedAndTriggeredOf(Ref),
    /// The object has the abilities of the ability stickers on the objects the reference names, in any zone (layer 6, CR
    /// 123): "has all abilities of ability stickers on this Equipment".
    R114StickerAbilitiesOf(Ref),
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
    /// The object can't attack its owner, the planeswalkers its owner controls or both (a restriction, no layer).
    CantAttackOwner(Attacked),
    /// The object can't attack the players the reference names as the effect begins or the planeswalkers they control
    /// (a restriction, no layer): "can't attack those players or planeswalkers they control this combat".
    R6601CantAttackPlayers(Ref),
    /// The object can't be blocked unless it is attacking its owner or a permanent its owner controls (a restriction, no
    /// layer).
    CantBeBlockedUnlessAttackingOwner,
    /// The object can't block (a restriction, no layer).
    CantBlock,
    /// The object does not untap during its controller's untap step (a restriction, no layer).
    DoesntUntap,
    /// Damage dealt to the object that would remove all the loyalty counters on it removes all but one of them instead
    /// (a restriction, no layer; CR 120.3c): the damage is dealt in full, only its result changes (Deification).
    R6700KeepsLastLoyalty,
    /// The object can't be turned face up (a restriction, no layer).
    CantBeTurnedFaceUp,
    /// The object can't be returned to its owner's hand to pay a beam me up cost (a restriction, no layer): "can't be
    /// beamed up".
    R119CantBeBeamedUp,
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
    /// The object can't be blocked unless all creatures the defending player controls block it (a restriction, no layer).
    BlockedByAll,
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
            | Modification::ThisAbility
            | Modification::SetLoyalty(_)
            | Modification::R707CopyOf { .. } => Some("1"),
            Modification::SetController(_) => Some("2"),
            Modification::ChangeTextChosen { .. }
            | Modification::ChangeTextChosenTo { .. }
            | Modification::R612ExchangeTextBoxes => Some("3"),
            Modification::AddType(_)
            | Modification::RemoveType(_)
            | Modification::SetTypes(_)
            | Modification::RemoveSupertype(_)
            | Modification::AddSubtype(_)
            | Modification::SetCreatureTypes(_)
            | Modification::SetCreatureTypesOf(_)
            | Modification::SetLandTypesNthChosen(_)
            | Modification::SetLandTypes(_)
            | Modification::SetLandTypesChosen(_)
            | Modification::SetCreatureTypesChosen(_)
            | Modification::AddCreatureTypeChosen(_)
            | Modification::AddLandTypeChosen(_)
            | Modification::AddChosenSubtype
            | Modification::AddAllCreatureTypes => Some("4"),
            Modification::SetColors(_)
            | Modification::AddColors(_)
            | Modification::SetColorsChosen(_)
            | Modification::SetChosenColor
            | Modification::AddChosenColor
            | Modification::SetCommanderColor => Some("5"),
            Modification::AddKeyword(_)
            | Modification::R6601KeywordX { .. }
            | Modification::KeywordsOf { .. }
            | Modification::R6001AddChosenAbilities
            | Modification::AddProtectionFromChosenColor
            | Modification::AddProtectionFromChosenCardType
            | Modification::LockKeyword(_)
            | Modification::RemoveKeyword(_)
            | Modification::RemoveKeywordKind(_)
            | Modification::GrantAbility(_)
            | Modification::GainAbilitiesOf(_)
            | Modification::R111GainNonManaAbilitiesOf(_)
            | Modification::R6702GainActivatedAndTriggeredOf(_)
            | Modification::R114StickerAbilitiesOf(_)
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
            | Modification::CantAttackOwner(_)
            | Modification::R6601CantAttackPlayers(_)
            | Modification::CantBeBlockedUnlessAttackingOwner
            | Modification::CantBlock
            | Modification::DoesntUntap
            | Modification::R6700KeepsLastLoyalty
            | Modification::CantBeTurnedFaceUp
            | Modification::R119CantBeBeamedUp
            | Modification::CantBeBlocked
            | Modification::CantBeBlockedBy(_)
            | Modification::CanBlockOnly(_)
            | Modification::MinBlockers(_)
            | Modification::MaxBlockers(_)
            | Modification::BlockedByAll
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
    /// In that added combat (0).
    R109ThatCombat(u32),
    /// Until the controller's next turn begins.
    UntilYourNextTurn,
    /// Until the controller's next end step begins: the one of this turn when it has not begun yet (CR 513).
    UntilYourNextEndStep,
    /// Until the controller's next upkeep begins: the one of a later turn, so one begun in their upkeep lasts until the
    /// upkeep of their next turn.
    UntilYourNextUpkeep,
    /// Until the end of the controller's next upkeep: the one of a later turn, so one begun in their upkeep lasts
    /// through the upkeep of their next turn and ends as its draw step begins.
    UntilEndOfYourNextUpkeep,
    /// Until the end of the controller's next turn.
    UntilEndOfYourNextTurn,
    /// Only during the next turn of the player who controls the permanent it applies to as it begins ("that creature can't
    /// attack during its controller's next turn"): it changes nothing before that turn and ends with it. Only a continuous
    /// effect lasts this way.
    DuringItsControllersNextTurn,
    /// Until the end of the next combat phase of the player who controls the creature the rule names as it begins
    /// ("target creature attacks during its controller's next combat phase if able"): this turn's when that player is
    /// active and has not declared attackers yet. Only a rule lasts this way.
    UntilItsControllersNextCombatEnds,
    /// Until the end of combat on the controller's next turn: it ends as that turn's postcombat main phase begins
    /// ("Until end of combat on your next turn, you may play that card").
    UntilEndOfCombatOnYourNextTurn,
    /// For as long as the source stays on the battlefield.
    WhileSourceOnBattlefield,
    /// For as long as the source stays attached to the permanent the effect changes ("for as long as this Equipment
    /// remains attached to it", CR 611.2b): only a copy effect lasts this long.
    WhileSourceAttached,
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
    /// controller's next untap step", CR 502.3), or of a player an untap limit names ("creatures don't untap during
    /// target player's next untap step"): only those rules last this long.
    NextUntapStep,
    /// For as long as every object the effect names is still on the battlefield with a counter of the kind ('Each of those
    /// creatures can't attack you for as long as it has a vow counter on it'): it ends when one of them loses its last counter
    /// of the kind or leaves, and a counter put on another permanent later changes nothing. Only a continuous effect that
    /// names its objects (a reference, not a group) lasts this way.
    WhileHasCounter(Box<CounterKind>),
    /// For as long as every object the effect names is still on the battlefield with an Aura attached ("gain control of
    /// target creature for as long as that creature is enchanted", CR 611.2b): it ends when one of them is no longer
    /// enchanted or leaves. Only a continuous effect that names its objects lasts this way.
    WhileEnchanted,
    /// For as long as every object the effect names is still on the battlefield and tapped ("for as long as it remains
    /// tapped"): it ends when one of them untaps or leaves. A continuous effect or a rule that names its objects lasts
    /// this way.
    WhileTapped,
    /// For as long as the player the reference names as the effect begins is the monarch ("for as long as they're the
    /// monarch", CR 724): it ends when that player stops being the monarch, and at once when they are not the monarch
    /// as it begins. Only a continuous effect lasts this way.
    WhileMonarch(#[cfg_attr(feature = "ts", ts(type = "unknown"))] Ref),
    /// Until the next untap step of the player who controls a permanent it applies to as it begins ("target land
    /// becomes a Swamp until its controller's next untap step"): it ends as that player's next turn begins. Only a
    /// continuous effect lasts this way.
    UntilItsControllersNextUntapStep,
    /// Until the turn it makes a player skip has been skipped ("skip your next turn", CR 614.10): only the rule that a
    /// player skips a turn lasts this long.
    UntilTurnSkipped,
    /// Until the step or combat phase its rule makes a player skip has been skipped once ("skips their next untap step",
    /// "skips their next combat phase", CR 614.10): only the rule that a player skips a step or combat lasts this long.
    UntilSkipped,
    /// As `UntilSkipped`, and it also ends with the turn ("skips their next combat phase this turn").
    UntilSkippedThisTurn,
    /// During the next turn of the player the rule names, begun after this one: it applies from that turn's start and ends
    /// with it ("skips all combat phases of their next turn", CR 614.10); only the rule that a player skips steps or combat
    /// lasts this way.
    TheirNextTurn,
    /// Until the next turn of the one player the rule names ("Until that player's next turn, they gain protection from
    /// everything"): it lasts as `UntilYourNextTurn` does for that player; only a rule lasts this way.
    UntilTheirNextTurn,
    /// Until the next end step of the one player a permission names ("Until that player's next end step, they may play
    /// that card"): it lasts as `UntilYourNextEndStep` does for that player; only a rule lasts this way.
    R113UntilTheirNextEndStep,
    /// Until the card the source became when its ability exiled it is cast from exile ("until this card is cast from
    /// exile"); it lasts for good once that card leaves exile another way.
    UntilCastFromExile,
    /// Until any player casts a creature spell after the effect began ("until a player casts a creature spell"). Only a
    /// continuous effect lasts this way.
    UntilCreatureSpellCast,
    /// Until end of turn, or until any player rolls this number on a die after the effect began, whichever comes first
    /// ("until end of turn, or until any player rolls a 1, whichever comes first"). Only a continuous effect lasts this way.
    UntilEndOfTurnOrRolled(u32),
    /// Until the end of the next turn of the player the reference names, a name the resolution bound to that player
    /// ("until the end of defending player's next turn, that player gets two rad counters whenever they cast a spell"):
    /// it ends with the first turn of theirs that ends after the turn it began in. Only a delayed trigger lasts this way.
    R6700UntilEndOfTheirNextTurn(#[cfg_attr(feature = "ts", ts(type = "unknown"))] Ref),
    /// For as long as the condition holds (CR 611.2b: "for as long as this creature remains tapped and that creature's
    /// power remains less than or equal to this creature's power"), read with each object of the effect as `Affected`:
    /// it ends the first time the condition fails and does not come back. Only a continuous effect lasts this way.
    R113While(#[cfg_attr(feature = "ts", ts(type = "unknown"))] Box<Condition>),
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
    /// With `once`, the permission works once each turn on every player's turn, not only on the controller's ("Once each
    /// turn, you may pay {0} rather than pay the mana cost for a spell you cast ...").
    #[serde(default, skip_serializing_if = "crate::effect::is_false")]
    pub any_turn: bool,
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
    /// What a spell cast through it gets: "If you cast a spell this way, that artifact enters tapped."
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub riders: Vec<CastRider>,
    /// At most this many of the objects `only` names may be cast or played through it, all told, within the turn ("Until
    /// end of turn, you may play one of those cards"); any number when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

impl Default for CastPermission {
    /// A permission for the controller to cast every card from a hand, with nothing waived, limited or added.
    fn default() -> Self {
        CastPermission {
            who: Ref::You,
            what: Selector::default(),
            from: None,
            free: false,
            flash: false,
            only: None,
            play: false,
            once: false,
            any_turn: false,
            additional: None,
            rather: None,
            spend_as: SpendAs::Normal,
            when: None,
            any_cast: false,
            riders: Vec::new(),
            limit: None,
        }
    }
}

/// What a spell cast through a permission gets ("If you cast a spell this way, ..."), applied by the engine as the spell
/// leaves the stack or enters as a permanent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CastRider {
    /// The permanent the spell becomes enters tapped: "that artifact enters tapped".
    EntersTapped,
    /// The permanent enters with one counter of the kind on it: "that creature enters with a finality counter on it".
    EntersWithCounter(CounterKind),
    /// The permanent gains the keyword until end of turn: "it gains haste until end of turn".
    GainsThisTurn(Keyword),
    /// The permanent has the subtype in addition to its other types: "is a Vampire in addition to its other types".
    AlsoSubtype(Subtype),
    /// The spell is exiled rather than put into its owner's graveyard from the stack: "If a spell cast this way would be
    /// put into your graveyard, exile it instead."
    ExiledInstead,
    /// A spell that is an Equipment gains the ability, which its permanent keeps: "When you cast an Equipment spell this
    /// way, it gains "When this Equipment enters, attach it to target creature you control."".
    R6103EquipmentGains(std::sync::Arc<crate::ability::Ability>),
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

/// Which side of the table a rule means: one chosen as its source entered or by its resolution ("left" or "right").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SideOf {
    /// The word "left" or "right" the source chose as it entered ("the chosen direction"); no side without one.
    Entered,
    /// The word "left" or "right" the resolution chose under this name ("the last chosen direction").
    ChosenAs(String),
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
    /// Each of them attacks if able once any creature the selector picks attacks, judged for its controller ("If a
    /// creature you control attacks, this creature also attacks if able."): a declaration with such a creature and
    /// without one of them that can attack for nothing is refused.
    AttackIfAnyAttacks(Selector),
    /// They can attack as though they did not have defender.
    AttackDespiteDefender,
    /// They can attack the players who attacked their controller during those players' last turn as though they did not
    /// have defender (Weathered Sentinels); planeswalkers and battles stay out of reach.
    AttackLastTurnAttackersDespiteDefender,
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
    /// While their power is negative, they assign combat damage as though it were positive (its absolute value).
    AssignsAsThoughPositive,
    /// They station permanents using their toughness rather than their power (CR 702.184a): the station ability they pay
    /// for puts charge counters equal to their toughness.
    R6500StationsByToughness,
    /// They must be blocked if able: a requirement on the block declaration (CR 509.1c).
    MustBeBlocked(Blocking),
    /// Every creature the selector picks that is able to block them does so (CR 509.1c), judged for their controller:
    /// "All Walls able to block ~ do so."
    MustBeBlockedBy(Selector),
    /// They must be blocked by at least one creature the selector picks if able (CR 509.1c), judged for their controller:
    /// "must be blocked by an Eldrazi if able"; a block by another creature does not meet it.
    MustBeBlockedByOne(Selector),
    /// They must be blocked by this many creatures or more if able (CR 509.1c): "must be blocked by two or more creatures
    /// if able"; a block by fewer meets it only while no other creature could join it.
    R6001MustBeBlockedByAtLeast(u32),
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
    /// Divides damage among defender and creatures.
    R109AssignsAmongDefenders,
    /// Unblocked: all damage to one creature.
    R109UnblockedToCreature,
    /// The defender assigns its damage.
    R109DefenderAssigns,
    /// Lethal damage counts per source.
    R109LethalFromOneSource,
}

/// The capability key of a combat damage quirk of r109 (`Combat.<variant>`), which an engine without it lacks.
pub fn r109_quirk_key(quirk: &CombatQuirk) -> Option<&'static str> {
    Some(match quirk {
        CombatQuirk::R109AssignsAmongDefenders => "Combat.R109AssignsAmongDefenders",
        CombatQuirk::R109UnblockedToCreature => "Combat.R109UnblockedToCreature",
        CombatQuirk::R109DefenderAssigns => "Combat.R109DefenderAssigns",
        CombatQuirk::R109LethalFromOneSource => "Combat.R109LethalFromOneSource",
        _ => return None,
    })
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
    /// Be the target of spells or abilities that can target only Walls: a specification whose every kind of object is a
    /// Wall.
    BeTargetedByWallsOnly,
    /// Play lands as the special action (CR 305.1).
    PlayLand,
    /// Play the land cards the selector picks, judged where they are as they would be played (CR 305.1): "You can't play
    /// lands from your hand."
    PlayLands(Selector),
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
    /// Be sacrificed or exiled by a triggered ability their controller controls while it resolves: "Triggered abilities you
    /// control can't cause you to sacrifice or exile creature tokens you control." A cost the player pays to resolve the
    /// ability (a "you may sacrifice") is caused by it too.
    R6603ByOwnTriggers,
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
    /// Become the monarch (CR 724): "You can't become the monarch this turn."; the monarch stays who it was.
    BecomeMonarch,
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
    /// Attack the planeswalkers the selector picks, judged as the rule's controller and source see them as the attack is
    /// declared (CR 508.1b): "creatures they control can't attack Jaces you control this turn". The players stay open.
    AttackPlaneswalkers(Selector),
    /// Activate abilities of one kind of the sources the selector picks (every source when empty): the kind is a loyalty
    /// ability, the equip ability or the cycling ability, or any activated ability but a mana ability.
    ActivateKind {
        /// The kind of ability.
        kind: ActivationKind,
        /// The sources; every source when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        sources: Selector,
    },
    /// Pay costs of casting spells or activating abilities with life or with the permanents `sacrifice` picks (CR 601.2h,
    /// 602.2b): a spell or ability whose cost needs it can't be cast or activated, a Phyrexian symbol is paid with mana,
    /// paying 0 life stays open, and a resolving effect still makes them pay. Mana abilities are left out with
    /// `but_mana_abilities`: "Players can't pay life or sacrifice creatures to cast spells or activate abilities."
    PayCosts {
        /// Life can't be paid.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        life: bool,
        /// The permanents that can't be sacrificed; none when sacrifices stay open.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sacrifice: Option<Selector>,
        /// The costs of mana abilities stay open ("abilities that aren't mana abilities").
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        but_mana_abilities: bool,
    },
    /// Attack a player, not the planeswalkers they control (CR 508.1b), unless the condition holds for that player ("you" in
    /// it is that player): "creatures can't attack a player unless that player cast a spell ... during their last turn".
    R111AttackPlayer {
        /// The attack is open while this holds for the defending player.
        unless: Condition,
    },
    /// Cast the spells and play the lands whose cards the selector picks (CR 601, 305.1): "spells with the chosen names
    /// can't be cast and lands with the chosen names can't be played".
    R111CastOrPlay(Selector),
    /// Attack at all (CR 508.1): "they can't attack this turn". With `you` only the player who controls the rule is out of
    /// reach, not the planeswalkers or battles (CR 508.1b): "can't attack you during their next turn".
    R118Attack {
        /// Only the rule's controller can't be attacked.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        you: bool,
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
    /// As the spell resolves, the replacement takes the place of its move into a graveyard (CR 608.2n, 614.1a): "put that
    /// card into your hand instead of into your graveyard as it resolves" (`ReplaceBy::MoveTo`).
    R6503Resolves(crate::trigger::ReplaceBy),
    /// Mana may be spent as though it were mana of any color or type to pay its mana cost only, not its additional or
    /// optional costs (CR 609.4b): "For one spell this turn, you may spend mana as though it were mana of any type to
    /// pay that spell's mana cost."
    R112SpendAs(SpendAs),
}

/// A change to the rules of the game rather than to an object. `Effect::Rule` makes it for a duration and
/// `Ability::Rule` for as long as its source has the ability; the references name the players or objects it is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameRule {
    /// Each power-up ability of the permanents the selector matches can be activated an additional time
    /// (`Limit::PowerUp`).
    PowerUpAgain(Selector),
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
    /// Each creature `who` names attacks one of the players `players` names each combat if able (CR 508.1d), the set read
    /// again as each declaration is made: "~ attacks an opponent with the most life among your opponents each combat if
    /// able". A creature that can't attack any of them may attack as it could otherwise.
    R6702MustAttackOneOf {
        /// The creatures.
        who: Ref,
        /// The players, any one of whom meets the requirement.
        players: Ref,
    },
    /// Creatures attack the planeswalker `at` names each combat if able (CR 508.1d): the creatures `who` names, and
    /// every creature the players `of` names control as attackers are declared, whenever it came under their control
    /// ("creatures that player controls attack ~ if able"). A creature that can't attack it may attack as it could
    /// otherwise.
    MustAttackAt {
        /// The creatures that attack it; none when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who: Option<Ref>,
        /// The players whose creatures attack it; none when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        of: Option<Ref>,
        /// The planeswalker they attack.
        at: Ref,
    },
    /// Each player `who` names attacks with at least one creature each combat if able (CR 508.1d): a requirement on the
    /// whole declaration, met by any creature; with `at`, by one attacking a player or planeswalker `at` names.
    MustAttackWithOne {
        /// The players who must attack.
        who: Ref,
        /// The players and planeswalkers the creature attacks; any defender when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<Box<Ref>>,
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
    /// The player `chooser` names chooses which creatures the players `whose` names control block and how they block
    /// (CR 509.1a): "you choose which creatures block this combat and how those creatures block". The declaration still
    /// obeys every restriction and requirement, and a cost to block is paid or refused by the blocker's controller.
    R6400ChoosesBlocks {
        /// Who declares the blocks instead.
        chooser: Ref,
        /// The players whose blocks they declare.
        #[serde(
            default = "Ref::each_player",
            skip_serializing_if = "Ref::is_each_player"
        )]
        whose: Ref,
    },
    /// The player `chooser` names casts every ballot of each vote, for each player (CR 701.38): "You choose how each
    /// player votes this turn."; each vote still counts as that player's.
    R6400ChoosesVotes {
        /// Who votes for everyone.
        chooser: Ref,
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
    /// "Each creature with one or more counters on it can't attack you or planeswalkers you control unless its controller
    /// pays {X}, where X is the number of counters on that creature" (CR 508.1h): each taxed creature costs the mana once
    /// for each counter on it as the attack is declared.
    R6601AttackTaxPerCounter {
        /// The players it protects.
        #[serde(default = "Ref::you", skip_serializing_if = "Ref::is_you")]
        who: Ref,
        /// Whether it protects the players, the planeswalkers they control or both.
        attacked: Attacked,
        /// The attacking creatures it taxes, judged when the attack is declared; every creature when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        attackers: Selector,
        /// The mana each counter on a taxed creature costs; no `{X}`.
        mana: ManaCostX,
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
        /// The other costs each taxed creature takes, once for each of `per` (CR 508.1g): permanents sacrificed,
        /// returned or tapped ("can't attack unless you sacrifice two Islands").
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        cost: Vec<crate::cost::CostItem>,
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
        /// The other costs each taxed blocker takes, once for each of `per` (CR 509.1d): permanents tapped, sacrificed
        /// or returned.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        cost: Vec<crate::cost::CostItem>,
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
    /// The colored mana symbols in the costs of the spells the players cast are drawn anew each round within the color
    /// identity of their commanders: each color of that identity stands for another one of it, so a cost keeps its mana
    /// value, generic and colorless mana stay as they are, and a card inside the identity stays castable (`cost_colors`).
    CostColorsDrawn(Ref),
    /// The players pay no commander tax (CR 903.8): their commanders cost no more to cast from the command zone, in mana or
    /// in life.
    NoCommanderTax(Ref),
    /// The players have a keyword, such as protection from everything (CR 702.16j), while the rule lasts.
    PlayersHave {
        /// The players.
        who: Ref,
        /// The keyword they have.
        keyword: Keyword,
    },
    /// The players can't draw cards.
    CantDraw(Ref),
    /// The players draw cards from the bottom of their libraries rather than the top ("You draw cards from the bottom of
    /// your library rather than the top").
    DrawFromBottom(Ref),
    /// The players can't search libraries.
    CantSearch(Ref),
    /// Spells and abilities the players control can't cause their controller to search their own library: "Spells and
    /// abilities your opponents control can't cause their controller to search their library."
    OwnSpellsCantSearch(Ref),
    /// The players search only the top `count` cards of any library they would search (CR 614.1a): "If an opponent
    /// would search a library, that player searches the top four cards of that library instead."
    SearchTop {
        /// Who searches that way.
        who: Ref,
        /// How many cards from the top they search.
        count: u32,
    },
    /// A permanent with a counter of this kind that would untap during its controller's untap step loses all counters
    /// of the kind instead and stays tapped (CR 614.1a): "If a permanent with a wind counter on it would untap during
    /// its controller's untap step, remove all wind counters from it instead."
    UntapRemovesCounters(CounterKind),
    /// The players may vote an additional time each time they vote: "While voting, you may vote an additional time."
    /// (CR 701.38d).
    VoteAgain(Ref),
    /// The players get an additional vote each time they vote: "While voting, you get an additional vote." (CR 701.38d).
    ExtraVote(Ref),
    /// The game's turn order runs the other way round the table (Aeon Engine); two such rules cancel out.
    ReverseTurnOrder,
    /// Each player may attack only the nearest opponent on a side of them and planeswalkers that opponent controls
    /// (Pramikon, Sky Rampart); several such rules all apply, so sides that disagree leave nobody to attack.
    AttackOnlyNearest(SideOf),
    /// A player in the group who would begin an extra turn skips that turn instead (CR 500.7).
    SkipExtraTurns(Ref),
    /// A player in the group who would flip a coin flips two instead and ignores one (CR 705.3, Krark's Thumb); each such
    /// rule doubles the coins again.
    FlipTwoCoins(Ref),
    /// A player in the group can't control more than `most` creatures with the same name: the newest beyond that are
    /// sacrificed as a state-based action (the Delves modifier Mirror Hall).
    SameNameLimit {
        /// The players.
        who: Ref,
        /// The most creatures of one name each may control.
        most: u32,
    },
    /// A card or token a player in the group owns that would be put into a graveyard makes its owner flip a coin; on a
    /// loss it is exiled instead (the Delves modifier No Rest). The flip uses the game's random stream and is logged.
    FlipOrExiled(Ref),
    /// A player in the group flips one coin in each of their untap steps; on a loss their permanents `what` matches
    /// don't untap during that step (the Delves modifier Stone Sleep).
    FlipOrStayTapped {
        /// The players.
        who: Ref,
        /// The permanents held down on a loss.
        what: Selector,
    },
    /// A player in the group who would gain life flips a coin; on a loss they gain no life (the Delves modifier No
    /// Mercy).
    FlipOrNoLifeGain(Ref),
    /// The first time each turn a player in the group flips one or more coins, those coins come up heads and the player
    /// wins those flips (Edgar, King of Figaro); every coin of one instruction is one set (the ruling of Edgar).
    R6700FirstFlipsHeads(Ref),
    /// A player in the group who would face a villainous choice faces it an additional time (CR 701.56): "If an
    /// opponent would face a villainous choice, they face that choice an additional time." Each such rule adds one.
    R6700VillainousAgain(Ref),
    /// A player in the group who would roll one or more dice rolls that many plus one instead and ignores the lowest roll
    /// (Wyll, Blade of Frontiers); each such rule adds one more die and one more ignored roll.
    ExtraDie(Ref),
    /// After a player in the group rolls one or more dice, they may pay `pay` to increase or decrease one result of that
    /// roll by `by` (CR 706.2: the modified number is the result; it may leave the die's range, the ruling of
    /// Xenosquirrels): "After you roll a die, you may pay 1 life. If you do, increase or decrease the result by 1."
    /// They see every die of the roll first and choose which to change (the ruling of Night Shift of the Living Dead).
    R6402ChangeRoll {
        /// Whose rolls.
        who: Ref,
        /// What changing a result costs.
        pay: R6402RollPay,
        /// How much the result goes up or down.
        by: u32,
        /// Only once each turn for the permanent ("Do this only once each turn").
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        once: bool,
    },
    /// A player in the group who would roll one or more dice of `sides` faces rolls them and may exchange one result with
    /// one of the numbers the permanent stored in `slot` (`Effect::R112RollStore`; CR 614.1a, 706): "If you would roll one
    /// or more six-sided dice, instead roll them and you may exchange one result with this creature's base power or base
    /// toughness." The result becomes the stored number and the stored number the result.
    R113ExchangeRoll {
        /// Whose rolls.
        who: Ref,
        /// How many faces the dice have.
        sides: u32,
        /// The memory slot of the permanent.
        slot: String,
    },
    /// "Creature spells you cast have sticker kicker {1}." (CR 123, the Wicker Picker rulings): as a player in the group
    /// casts a spell of `spells`, they may pay `cost` in addition, once for each such rule; for each payment they get a
    /// ticket and may put a sticker on the spell just before it becomes cast. A spell paid for it counts as kicked for
    /// other objects, never for its own kicker.
    R114StickerKicker {
        /// Who casts the spells.
        who: Ref,
        /// The spells.
        spells: Selector,
        /// The sticker kicker cost.
        cost: crate::cost::Cost,
    },
    /// "Note the mana value of each card as it's put into exile.": the permanent notes the number for itself as each card
    /// goes to exile, with no stack; `Amount::R113GreatestNoted` reads the notes of this turn.
    R113NoteExiledManaValue,
    /// A player in the group who would roll one or more planar dice rolls that many plus one instead and ignores one
    /// (CR 901.9): "If you would roll one or more planar dice, instead roll that many planar dice plus one and ignore
    /// one." No game the engine plays has a planar deck, so it never applies (the rulings of Fractured Powerstone).
    ExtraPlanarDie(Ref),
    /// A player in the group who would planeswalk first looks at the top two cards of their planar deck, puts one on the
    /// bottom and the other on top (CR 901.10, Susan Foreman). No game the engine plays has a planar deck, so it never
    /// applies.
    PlaneswalkLookTwo(Ref),
    /// The players play with the top card of their library revealed to every seat.
    RevealTop(Ref),
    /// The players play with their hands revealed.
    RevealHand(Ref),
    /// The player plays with the cards revealed in their hand and can't play them (CR 400.2): "that player plays with
    /// that card revealed in their hand and can't play it"; it ends for a card once it leaves the hand.
    R119PlaysRevealed {
        /// The player whose hand holds the cards.
        who: Ref,
        /// The cards.
        card: Ref,
    },
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
    /// The players may activate the loyalty abilities of planeswalkers they control twice each turn rather than only
    /// once (CR 606.3).
    LoyaltyTwice(Ref),
    /// For each planeswalker the players control, they may activate one of its loyalty abilities once more this turn
    /// as though none had been activated (CR 606.3): each such rule raises the allowance by one (The Chain Veil).
    R6600LoyaltyOnceMore(Ref),
    /// The next time the creatures adapt this turn, each adapts as though it had no +1/+1 counters on it (CR 701.46a):
    /// the rule is used up by that adapt.
    R6600AdaptsFresh(Ref),
    /// The loyalty ability that makes this rule may be activated this many more times this turn than the planeswalker's
    /// loyalty abilities otherwise may ("you may activate ~'s loyalty ability two more times this turn"); only that
    /// ability of that permanent, so a loyalty ability it gains is not covered (Comet, Stellar Pup rulings). A rule
    /// only a resolving loyalty ability makes.
    R6603LoyaltyAgain(u32),
    /// The controller may pay this cost rather than the echo cost of each permanent they control (CR 702.30, CR 118.9):
    /// "You may pay {0} rather than pay the echo cost for permanents you control." Paying it counts as paying echo.
    EchoRather(Cost),
    /// The controller reveals the first card they draw each turn as they draw it ("Reveal the first card you draw each
    /// turn."): only on their own turns with `your_turns`, and only when they choose to with `may` ("You may reveal
    /// the first card you draw each turn as you draw it."). `TriggerPattern::FirstDrawRevealed` triggers on the reveal.
    RevealFirstDraw {
        /// Whether the reveal is optional.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        may: bool,
        /// Whether it works only on the controller's turns.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        your_turns: bool,
    },
    /// Creatures the players control can boast twice during each of their turns rather than once (CR 702.142c): a boast
    /// ability is an activated ability limited to once each turn that works only if its source attacked this turn.
    BoastTwice(Ref),
    /// The players reveal each card they draw as they draw it ("the chosen player reveals each card they draw").
    R111RevealDraws(Ref),
    /// The players may activate loyalty abilities of planeswalkers they control on any player's turn any time they could
    /// cast an instant (Teferi's Talent's emblem).
    LoyaltyAnyTime(Ref),
    /// Loyalty abilities of planeswalkers the players activate cost an additional `[+plus]` to activate ("Planeswalkers'
    /// loyalty abilities you activate cost an additional [+1] to activate.").
    LoyaltyCostsMore {
        /// The players who activate.
        who: Ref,
        /// The loyalty counters the cost adds.
        plus: u32,
    },
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
    /// Any player may pay the mana, a special action any time they have priority (CR 116.2), for that player to ignore
    /// the other effects of the rule's source until end of turn; those effects leave such a player out with
    /// `Condition::PaidToIgnore` ("Any player may pay {2} for that player to ignore this effect until end of turn.").
    PayToIgnore(ManaCostX),
    /// The rule's controller may pay the mana any time they could cast an instant, a special action that does not use the
    /// stack (CR 116.2), as often as they like while the rule lasts; each payment does the effects at once, `Target(0)` in
    /// them the object or player that the spell beginning the rule targeted, fixed as it began (CR 611.2c): "Until end of
    /// turn, you may pay {1} any time you could cast an instant. If you do, prevent the next 1 damage that would be dealt
    /// to that permanent or player this turn."
    R114PayToDo {
        /// What each payment costs.
        cost: ManaCostX,
        /// What each payment does.
        effects: Vec<crate::effect::Effect>,
    },
    /// The players the reference names may pay the cost, a special action any time they have priority (CR 116.2), for
    /// that player to ignore the other effects of the rule's source until end of turn; those effects leave such a player
    /// out with `Condition::PaidToIgnoreBy` ("Its controller may exile three cards from their graveyard for that player
    /// to ignore this effect until end of turn.").
    PayToIgnoreBy {
        /// Who may pay: "its controller", "that creature's controller".
        payer: Ref,
        /// What ignoring the effects costs.
        cost: Cost,
    },
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
    /// During each of their untap steps the players choose artifact, creature or land, and can untap only permanents of
    /// the chosen type that step (CR 502.3): "each player chooses artifact, creature, or land during their untap step".
    R112UntapChosenType(Ref),
    /// The players skip their draw steps (CR 614.10): the step does not begin on their turns.
    SkipDrawStep(Ref),
    /// During their own turns, as long as they haven't activated an exhaust ability this turn, the players may activate
    /// exhaust abilities as though they haven't been activated ("During your turn, as long as you haven't activated an
    /// exhaust ability this turn, you may activate exhaust abilities as though they haven't been activated.").
    R6007ExhaustAgain(Ref),
    /// The permanents can't have more than `most` counters of the kind on them ("~ can't have more than seven dream
    /// counters on it."): a put that would pass the cap puts only enough to reach it, and none once it is reached.
    R6500CounterCap {
        /// The permanents capped.
        what: Ref,
        /// The kind of counter.
        counter: crate::value::CounterKind,
        /// The most they may have.
        most: u32,
    },
    /// Planeswalkers the players control aren't put into their owners' graveyards for having 0 loyalty: the state-based
    /// action of CR 704.5i passes them over.
    R6500ZeroLoyaltyStays(Ref),
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
    /// Rulebreaker: what a deck with the card as its commander may hold, as the card words it after the ability word (`~`
    /// for its name, no final period: "A deck with this commander can have any land cards"); it changes nothing in a game,
    /// the deck check only warns (CR 903.5), and the rule is written without `from`.
    R116Rulebreaker(String),
    /// A booster draft rule: nothing in a game; no `from`.
    Draft(DraftRule),
    /// "If this card is in a graveyard, effects from spells named Muscle Burst count it as a card named Muscle Burst.": a
    /// resolving spell of that name reads the card as named so; from the graveyard only.
    CountsAsNamed(String),
    /// "This spell can't be copied." (CR 707.10): an effect that would copy the spell makes no copy of it; no `from`.
    CantBeCopied,
    /// "Damage isn't removed from ~ during cleanup steps." (CR 514.2): the cleanup step removes the damage of every other
    /// permanent and leaves this one's; read from the permanent's abilities as the turn ends.
    KeepsDamage,
    /// "You can't spend mana to cast this spell.": the cast of the card offers no mana of the pool and no mana ability, so
    /// only what pays without mana (convoke, delve) pays its cost; no `from`.
    CantSpendMana,
    /// "Rather than pay {2} for each previous time you've cast this spell from the command zone this game, pay 2 life that
    /// many times.": the commander tax of the card is paid as this much life for each previous cast from the command zone
    /// in place of its generic mana (CR 903.8); no `from`.
    TaxAsLife(u32),
    /// "Plotting cards from your hand costs {2} less.": the plot costs of the cards in its controller's hand cost this
    /// much generic mana less (CR 702.170a).
    R6018PlotLess(u32),
    /// "You may plot nonland cards from the top of your library.": its controller may plot the top card of their library
    /// that the selector picks as they plot a card from their hand (CR 702.170a), when it has plot.
    R111PlotFromLibraryTop(Selector),
    /// "You may cast creature spells from your graveyard using their sneak abilities.": its controller may cast the cards
    /// of `from` they own that `what` picks for an alternative cost of `way` they have, which otherwise works from a hand.
    R111CastWayFrom {
        /// The way of casting.
        way: crate::value::CastWay,
        /// The zone the cards are cast from.
        from: ZoneKind,
        /// The cards.
        what: Selector,
    },
    /// "As you cast spells from your hand this turn, they gain cascade.": each spell its controller casts that `what` picks
    /// (judged as it is cast, where it is cast from) gains the keyword as it is cast, until it leaves the stack (CR 611.2c
    /// fixes the spell as it is cast, so the gained keyword triggers on its cast).
    R112SpellsGain {
        /// The cards cast.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        what: Selector,
        /// The keyword gained.
        keyword: crate::keyword::Keyword,
    },
    /// "Spend only mana produced by basic lands to cast this spell.": the cast of the card spends only mana whose source
    /// the filter holds for, from the pool or from mana abilities of such sources; no `from`.
    SpendOnlyFrom(crate::spend::SpendFilter),
    /// `SpendOnlyFrom` for one way of casting the card only: "You may pay {R}{G} rather than pay this spell's mana cost.
    /// Spend only mana produced by Treasures to cast it this way."
    R112SpendOnlyFromWay {
        /// The sources the mana must come from.
        from: crate::spend::SpendFilter,
        /// The way of casting it holds for.
        way: crate::value::CastWay,
    },
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
        /// How many times the mana changes the cost, counted as each spell is cast ("where X is the amount of life you
        /// lost this turn").
        #[serde(default = "Amount::one", skip_serializing_if = "Amount::is_one")]
        per: Amount,
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
    /// While an opponent chooses targets as they cast a spell or activate an ability, they must choose at least one
    /// Flagbearer on the battlefield if able.
    Flagbearers,
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
    /// Lethal damage dealt to each creature the players control is determined by its power rather than its toughness
    /// (CR 704.5g, 120.6): damage marked at least equal to its power destroys it.
    LethalByPower(Ref),
    /// While it applies, all damage dealt to the players is dealt as though its source had infect (CR 702.90b).
    DamageAsInfect(Ref),
    /// The players can't venture into the dungeon more than this many times each turn (CR 701.49).
    VentureLimit {
        /// Who is limited.
        who: Ref,
        /// The ventures each turn.
        per_turn: u32,
    },
    /// The players' devotion to each color and each combination of colors is greater by this much (CR 700.5).
    DevotionIncreased {
        /// Whose devotion.
        who: Ref,
        /// By how much.
        by: u32,
    },
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
    /// Spells and abilities the opponents of the players control can't cause them to discard cards or sacrifice
    /// permanents; costs they pay themselves still can.
    OpponentsCantMakeDiscardOrSacrifice(Ref),
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
        /// With `spells`, also to activate the abilities of the permanents this selects ("or to activate abilities of
        /// permanents you control but don't own"); without, only to activate them ("to activate abilities of creatures
        /// you control").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        abilities: Option<Selector>,
    },
    /// Unspent mana the players would lose as a step or phase ends becomes mana of `into` instead and stays: "If you would
    /// lose unspent mana, that mana becomes colorless instead."
    UnspentManaBecomes {
        /// The players.
        who: Ref,
        /// The type the mana becomes.
        into: ManaType,
    },
    /// Spells or activated abilities cost an additional "Sacrifice a <permanent>" (CR 601.2f, 602.2b), paid by their
    /// caster or activator with the other parts of the cost: "Spells cost an additional "Sacrifice a Swamp" to cast for
    /// each black mana symbol in their mana costs.", "Activated abilities of nontoken Rebels cost an additional "Sacrifice
    /// a land" to activate."
    R110SacrificeTax {
        /// The spells that pay it, judged as they are cast; no spell when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spells: Option<Selector>,
        /// The objects whose activated abilities, mana abilities included, pay it; no ability when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        abilities: Option<Selector>,
        /// What each sacrifice takes.
        sacrifice: Selector,
        /// Once for each mana symbol of this color in the spell's mana cost or the ability's activation cost (a hybrid
        /// or Phyrexian symbol of the color counts); once when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        per_symbol: Option<Color>,
    },
    /// The players may spend mana of one color as though it were mana of another color or of any color (CR 609.4b),
    /// whatever they pay: "You may spend white mana as though it were red mana.", "You may spend white mana as though it
    /// were mana of any color. You may spend other mana only as though it were colorless mana."
    R111SpendColorAs {
        /// The players.
        who: Ref,
        /// The color of the mana.
        mana: Color,
        /// The color it may be spent as; any color when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        as_color: Option<Color>,
        /// Mana of every other type may be spent only as though it were colorless mana: it pays no colored symbol, and
        /// it pays `{C}`.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        others_colorless: bool,
        /// Only to pay the activation costs of the abilities of the rule's source ("to pay the activation costs of ~'s
        /// abilities"); every payment when false.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        r112_own_abilities: bool,
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
    /// Reveal a drafted creature card and note its name, then turn this card face down.
    NoteCreatureName,
    /// Reveal it as drafted; the drafter's right neighbour, the drafter and the left neighbour each choose a color.
    ChooseColorsAround,
    /// Before the game, reveal it from the deck and exile an instant or sorcery card drafted that isn't in the deck.
    ExileDraftedSpell,
    /// Before the game, reveal it from the deck and exile a creature card drafted that isn't in the deck.
    ExileDraftedCreature,
    /// Before the game, reveal it from the deck and exile three or more creature cards drafted that aren't in the deck.
    ExileDraftedCreatures,
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
    /// Every mana is of one color the tapping player chooses instead of any other type, in the same amount ("it produces
    /// mana of a color of your choice instead of any other type").
    OnlyChosenColor,
    /// One mana of a color the tapping player chooses instead of any other type and amount.
    OneChosenColor,
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
    #[case(Modification::AddChosenColor, Some("5"))]
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
            cost: Vec::new(),
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
            cost: Vec::new(),
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
            cost: Vec::new(),
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
            cost: Vec::new(),
        },
        r#"{"BlockTax":{"blockers":["This"],"mana":"{2}"}}"#
    )]
    #[case(
        GameRule::AttackTax {
            who: Ref::EachPlayer,
            attacked: Attacked::Both,
            attackers: Selector::this(),
            mana: ManaCostX::default(),
            life: 0,
            per: Amount::Fixed(1),
            cost: vec![crate::cost::CostItem::Sacrifice {
                filter: Selector(vec![crate::refs::Pred::Type(CardType::Land)]),
                count: Amount::Fixed(1),
            }],
        },
        r#"{"AttackTax":{"who":"EachPlayer","attacked":"Both","attackers":["This"],"cost":[{"Sacrifice":{"filter":[{"Type":"Land"}]}}]}}"#
    )]
    #[case(
        GameRule::R6400ChoosesBlocks { chooser: Ref::You, whose: Ref::Opponents },
        r#"{"R6400ChoosesBlocks":{"chooser":"You","whose":"Opponents"}}"#
    )]
    #[case(
        GameRule::R6400ChoosesVotes { chooser: Ref::You },
        r#"{"R6400ChoosesVotes":{"chooser":"You"}}"#
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
    #[case(
        GameRule::MustAttackWithOne { who: Ref::Opponents, at: None },
        r#"{"MustAttackWithOne":{"who":"Opponents"}}"#
    )]
    #[case(
        GameRule::MustAttackWithOne { who: Ref::Opponents, at: Some(Box::new(Ref::You)) },
        r#"{"MustAttackWithOne":{"who":"Opponents","at":"You"}}"#
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
    #[case(
        GameRule::EchoRather(serde_json::from_str(r#""{0}""#).unwrap()),
        r#"{"EchoRather":"{0}"}"#
    )]
    #[case(
        GameRule::MustAttackAt { who: None, of: Some(Ref::Target(0)), at: Ref::This },
        r#"{"MustAttackAt":{"of":{"Target":0},"at":"This"}}"#
    )]
    #[case(GameRule::CanBeCommander, r#""CanBeCommander""#)]
    #[case(GameRule::AnyNumberInDeck, r#""AnyNumberInDeck""#)]
    #[case(GameRule::UpToInDeck(9), r#"{"UpToInDeck":9}"#)]
    #[case(
        GameRule::GraveyardCardsUntargetable,
        r#""GraveyardCardsUntargetable""#
    )]
    #[case(GameRule::Flagbearers, r#""Flagbearers""#)]
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

/// What changing a die's result costs (`GameRule::R6402ChangeRoll`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum R6402RollPay {
    /// This much life (CR 119.4: only a player with that much life may pay it).
    Life(u32),
    /// One counter of this kind removed from the permanent with the rule.
    RemoveCounter(CounterKind),
}
