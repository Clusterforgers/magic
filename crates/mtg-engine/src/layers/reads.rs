//! What an effect reads of the game and what a modification writes: the cheap test that keeps the dependency pass of
//! the layers from trying effects that cannot affect each other.
//!
//! A dependency (CR 613.8a) exists only when applying one effect changes what the other applies to or does, so an
//! effect that writes nothing the other reads is independent of it. Each name here is a set of the characteristics
//! the layers change; a selector, a condition and an amount say which they read, a modification which it writes.

use mtg_cards::modify::Modification as Wanted;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::{Amount, Condition, StatKind};

use crate::effects::Modification;

/// A set of the things the layers change in an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Traits(u16);

impl Traits {
    pub(crate) const NONE: Traits = Traits(0);
    pub(crate) const TYPES: Traits = Traits(1);
    pub(crate) const SUBTYPES: Traits = Traits(1 << 1);
    pub(crate) const COLORS: Traits = Traits(1 << 2);
    pub(crate) const POWER: Traits = Traits(1 << 3);
    pub(crate) const TOUGHNESS: Traits = Traits(1 << 4);
    pub(crate) const CONTROL: Traits = Traits(1 << 5);
    /// The keywords it has.
    pub(crate) const KEYWORDS: Traits = Traits(1 << 6);
    /// The other abilities it has, whether it has its static abilities among them.
    pub(crate) const RULES: Traits = Traits(1 << 7);
    /// Whether it is goaded (CR 701.15), which a selector of goaded creatures reads.
    pub(crate) const GOADED: Traits = Traits(1 << 8);
    /// The supertypes effects gave it (CR 205.4).
    pub(crate) const SUPERTYPES: Traits = Traits(1 << 9);
    /// The activated abilities it has, which an object that has "all activated abilities of" it reads (CR 613.1f).
    pub(crate) const ACTIVATED: Traits = Traits(1 << 10);

    /// Both sets together.
    pub(crate) const fn or(self, other: Traits) -> Traits {
        Traits(self.0 | other.0)
    }

    /// Whether the two sets share something.
    pub(crate) const fn meets(self, other: Traits) -> bool {
        self.0 & other.0 != 0
    }
}

/// What a modification changes in the object it applies to.
pub(crate) fn writes(modification: &Modification) -> Traits {
    match modification {
        Modification::AddType(_) | Modification::RemoveType(_) | Modification::SetTypes(_) => {
            Traits::TYPES.or(Traits::SUBTYPES)
        }
        Modification::AddSupertype(_) | Modification::RemoveSupertype(_) => Traits::SUPERTYPES,
        Modification::AddSubtype(_)
        | Modification::SetCreatureTypes(_)
        | Modification::AddAllCreatureTypes => Traits::SUBTYPES,
        // The land loses its abilities with its land types; a text change rewrites the rules, the keywords and the
        // subtypes.
        Modification::SetLandTypes(_)
        | Modification::ChangeText { .. }
        | Modification::R612ExchangeTextBoxes
        | Modification::R612TextBoxOf(_) => Traits::SUBTYPES
            .or(Traits::KEYWORDS)
            .or(Traits::RULES)
            .or(Traits::ACTIVATED),
        Modification::SetColors(_) | Modification::AddColors(_) => Traits::COLORS,
        Modification::AddKeyword(_)
        | Modification::AddKeywords(_)
        | Modification::RemoveKeyword(_)
        | Modification::RemoveKeywordKind(_)
        | Modification::LockKeyword(_)
        // A sticker's ability may be a keyword.
        | Modification::R114StickerAbilities(_) => Traits::KEYWORDS.or(Traits::ACTIVATED),
        Modification::LoseAbilities
        | Modification::LoseAbilitiesExceptMana
        | Modification::LoseAbility(_) => Traits::KEYWORDS.or(Traits::RULES).or(Traits::ACTIVATED),
        // What an object gains are activated abilities another object that has its abilities reads.
        Modification::GrantAbility(_) | Modification::GainAbilities(_) => Traits::ACTIVATED,
        Modification::SetController(_) => Traits::CONTROL,
        Modification::SetPowerToughness { .. }
        | Modification::SetPower(_)
        | Modification::SetToughness(_)
        | Modification::ModifyPowerToughness { .. }
        | Modification::SwitchPowerToughness => Traits::POWER.or(Traits::TOUGHNESS),
        // An ability that is given adds effects (the pass finds them as they come) and changes none that exist.
        // A name read by no other effect of the engine's: words added to it change nothing another effect reads.
        Modification::AddNameWords(_)
        | Modification::SetName(_)
        | Modification::GrantText(_)
        | Modification::CantAttack
        | Modification::CantAttackYou { .. }
        | Modification::R6601CantAttackPlayers { .. }
        | Modification::CantAttackOwner { .. }
        | Modification::CantBeBlockedUnlessAttackingOwner
        | Modification::CantBlock
        | Modification::DoesntUntap
        | Modification::R6700KeepsLastLoyalty
        | Modification::CantBeTurnedFaceUp
        | Modification::CantBeBlocked
        | Modification::CantBeBlockedBy { .. }
        | Modification::CanBlockOnly { .. }
        | Modification::MinBlockers(_)
        | Modification::MaxBlockers(_)
        | Modification::BlockedByAll
        | Modification::BlocksAdditional(_) => Traits::NONE,
        Modification::Goaded { .. } => Traits::GOADED,
    }
}

/// What a modification of the data changes in the object it applies to.
pub(crate) fn writes_of_wanted(wanted: &Wanted) -> Traits {
    match wanted {
        Wanted::AddType(_) | Wanted::RemoveType(_) | Wanted::SetTypes(_) => {
            Traits::TYPES.or(Traits::SUBTYPES)
        }
        Wanted::AddSubtype(_)
        | Wanted::SetCreatureTypes(_)
        | Wanted::SetCreatureTypesOf(_)
        | Wanted::AddAllCreatureTypes
        | Wanted::SetCreatureTypesChosen(_)
        | Wanted::AddCreatureTypeChosen(_)
        | Wanted::AddChosenSubtype => Traits::SUBTYPES,
        Wanted::SetLandTypes(_)
        | Wanted::SetLandTypesChosen(_)
        | Wanted::SetLandTypesNthChosen(_)
        | Wanted::AddLandTypeChosen(_)
        | Wanted::ChangeTextChosen { .. }
        | Wanted::ChangeTextChosenTo { .. }
        | Wanted::R612ExchangeTextBoxes => Traits::SUBTYPES
            .or(Traits::KEYWORDS)
            .or(Traits::RULES)
            .or(Traits::ACTIVATED),
        Wanted::SetColors(_)
        | Wanted::AddColors(_)
        | Wanted::SetColorsChosen(_)
        | Wanted::SetChosenColor
        | Wanted::AddChosenColor
        | Wanted::SetCommanderColor => Traits::COLORS,
        Wanted::AddKeyword(_)
        | Wanted::R6601KeywordX { .. }
        | Wanted::R6001AddChosenAbilities
        | Wanted::KeywordsOf { .. }
        | Wanted::AddProtectionFromChosenColor
        | Wanted::AddProtectionFromChosenCardType
        | Wanted::RemoveKeyword(_)
        | Wanted::RemoveKeywordKind(_)
        | Wanted::LockKeyword(_)
        // A sticker's ability may be a keyword.
        | Wanted::R114StickerAbilitiesOf(_) => Traits::KEYWORDS.or(Traits::ACTIVATED),
        Wanted::LoseAbilities
        | Wanted::LoseAbilitiesExceptMana
        | Wanted::LoseThisAbility
        | Wanted::LosePrinted(_) => Traits::KEYWORDS.or(Traits::RULES).or(Traits::ACTIVATED),
        Wanted::GrantAbility(_)
        | Wanted::GainAbilitiesOf(_)
        | Wanted::R111GainNonManaAbilitiesOf(_)
        | Wanted::R6702GainActivatedAndTriggeredOf(_) => Traits::ACTIVATED,
        Wanted::SetController(_) => Traits::CONTROL,
        Wanted::SetPT { .. }
        | Wanted::SetPower(_)
        | Wanted::SetToughness(_)
        | Wanted::ModifyPT { .. }
        | Wanted::SwitchPT => Traits::POWER.or(Traits::TOUGHNESS),
        // A static copy is no part of the layers: housekeeping begins and ends it (`r707_static_copies`).
        Wanted::R707CopyOf { .. }
        | Wanted::SetName(_)
        | Wanted::ThisAbility
        | Wanted::SetLoyalty(_)
        | Wanted::CantAttack
        | Wanted::CantAttackYou(_)
        | Wanted::R6601CantAttackPlayers(_)
        | Wanted::CantAttackOwner(_)
        | Wanted::CantBeBlockedUnlessAttackingOwner
        | Wanted::CantBlock
        | Wanted::DoesntUntap
        | Wanted::R6700KeepsLastLoyalty
        | Wanted::CantBeTurnedFaceUp
        | Wanted::CantBeBlocked
        | Wanted::CantBeBlockedBy(_)
        | Wanted::CanBlockOnly(_)
        | Wanted::MinBlockers(_)
        | Wanted::MaxBlockers(_)
        | Wanted::BlockedByAll
        | Wanted::BlocksAdditional(_)
        | Wanted::BlocksAdditionalPer(_) => Traits::NONE,
        Wanted::Goaded => Traits::GOADED,
        Wanted::AddSupertype(_) | Wanted::RemoveSupertype(_) => Traits::SUPERTYPES,
    }
}

/// What the amounts and the player of a modification of the data read when they are worked out.
pub(crate) fn reads_of_modification(wanted: &Wanted) -> Traits {
    match wanted {
        Wanted::ModifyPT { power, toughness } | Wanted::SetPT { power, toughness } => {
            reads_of_amount(power).or(reads_of_amount(toughness))
        }
        Wanted::SetPower(value)
        | Wanted::SetToughness(value)
        | Wanted::BlocksAdditionalPer(value)
        | Wanted::R6601KeywordX { count: value, .. } => reads_of_amount(value),
        Wanted::SetController(who)
        | Wanted::GainAbilitiesOf(who)
        | Wanted::R111GainNonManaAbilitiesOf(who)
        | Wanted::R6702GainActivatedAndTriggeredOf(who) => reads_of_ref(who).or(Traits::ACTIVATED),
        // Only which objects the reference names is read: the stickers on them are no characteristic.
        Wanted::R114StickerAbilitiesOf(who) => reads_of_ref(who),
        // The keywords of the other objects are read.
        Wanted::KeywordsOf { from, .. } => Traits::KEYWORDS.or(reads_of_ref(from)),
        // The creature types of the other object are read.
        Wanted::SetCreatureTypesOf(from) => Traits::SUBTYPES.or(reads_of_ref(from)),
        Wanted::CantBeBlockedBy(blockers) | Wanted::CanBlockOnly(blockers) => {
            reads_of_selector(blockers)
        }
        Wanted::R707CopyOf { .. }
        | Wanted::SwitchPT
        | Wanted::AddType(_)
        | Wanted::RemoveType(_)
        | Wanted::SetTypes(_)
        | Wanted::RemoveSupertype(_)
        | Wanted::AddSupertype(_)
        | Wanted::SetName(_)
        | Wanted::ThisAbility
        | Wanted::SetLoyalty(_)
        | Wanted::AddSubtype(_)
        | Wanted::SetCreatureTypes(_)
        | Wanted::SetLandTypes(_)
        | Wanted::SetLandTypesChosen(_)
        | Wanted::SetLandTypesNthChosen(_)
        | Wanted::SetCreatureTypesChosen(_)
        | Wanted::AddCreatureTypeChosen(_)
        | Wanted::AddLandTypeChosen(_)
        | Wanted::ChangeTextChosen { .. }
        | Wanted::ChangeTextChosenTo { .. }
        | Wanted::R612ExchangeTextBoxes
        | Wanted::SetColorsChosen(_)
        | Wanted::AddChosenSubtype
        | Wanted::SetChosenColor
        | Wanted::AddChosenColor
        | Wanted::R6001AddChosenAbilities
        | Wanted::SetCommanderColor
        | Wanted::AddAllCreatureTypes
        | Wanted::SetColors(_)
        | Wanted::AddColors(_)
        | Wanted::AddKeyword(_)
        | Wanted::AddProtectionFromChosenColor
        | Wanted::AddProtectionFromChosenCardType
        | Wanted::RemoveKeyword(_)
        | Wanted::RemoveKeywordKind(_)
        | Wanted::LockKeyword(_)
        | Wanted::LoseThisAbility
        | Wanted::LosePrinted(_)
        | Wanted::GrantAbility(_)
        | Wanted::LoseAbilities
        | Wanted::LoseAbilitiesExceptMana
        | Wanted::CantAttack
        | Wanted::CantAttackYou(_)
        | Wanted::R6601CantAttackPlayers(_)
        | Wanted::CantAttackOwner(_)
        | Wanted::CantBeBlockedUnlessAttackingOwner
        | Wanted::CantBlock
        | Wanted::DoesntUntap
        | Wanted::R6700KeepsLastLoyalty
        | Wanted::CantBeTurnedFaceUp
        | Wanted::CantBeBlocked
        | Wanted::MinBlockers(_)
        | Wanted::MaxBlockers(_)
        | Wanted::BlockedByAll
        | Wanted::BlocksAdditional(_)
        | Wanted::Goaded => Traits::NONE,
    }
}

/// What a reference reads to name its objects and players.
pub(crate) fn reads_of_ref(reference: &Ref) -> Traits {
    match reference {
        Ref::You | Ref::Opponents => Traits::CONTROL,
        Ref::EachPlayer
        | Ref::This
        | Ref::Target(_)
        | Ref::Triggering
        | Ref::TriggeringPlayer
        | Ref::TriggeringFirst
        | Ref::Dealer
        | Ref::Against
        | Ref::Attached
        | Ref::Granter
        | Ref::AttachedPlayer
        | Ref::Monarch
        | Ref::Spotlit
        | Ref::Chosen
        | Ref::R110Seat(_)
        | Ref::R111LastDamagedYou(_)
        | Ref::R111BlockedController { .. }
        | Ref::R111Object(_)
        | Ref::R115Causer
        | Ref::R4700OtherChosen
        | Ref::Promised
        | Ref::Defending
        | Ref::Attacked
        | Ref::AttackedThisTurn
        | Ref::R6601HighestRollers
        | Ref::MostLife
        | Ref::Teammates
        | Ref::R6501PlayersFromYou
        | Ref::ActivePlayer
        | Ref::Chooser
        | Ref::Asker
        | Ref::ModesChooser
        | Ref::Bound(_)
        | Ref::Affected
        | Ref::Candidate
        | Ref::Paid
        | Ref::Convoked
        | Ref::Delved
        | Ref::R114ChosenObject
        | Ref::R6702CraftedWith
        | Ref::R6602FromYou => Traits::NONE,
        Ref::Remembered { on, .. } => reads_of_ref(on),
        Ref::LastExiledWith { on, filter } => reads_of_ref(on).or(reads_of_selector(filter)),
        Ref::ControllerOf(inner) | Ref::OrController(inner) => {
            Traits::CONTROL.or(reads_of_ref(inner))
        }
        Ref::OwnerOf(inner) | Ref::Seated { of: inner, .. } | Ref::OpponentsOf(inner) => {
            reads_of_ref(inner)
        }
        Ref::Arrived { of, .. } | Ref::Departed(of) | Ref::SourceOf(of) => reads_of_ref(of),
        Ref::Each(selector) => reads_of_selector(selector),
        Ref::Union(parts) => parts
            .iter()
            .fold(Traits::NONE, |traits, part| traits.or(reads_of_ref(part))),
        Ref::PlayersWhere { who, test } => reads_of_ref(who).or(reads_of_condition(test)),
    }
}

/// What a selector reads of each object it tests.
pub(crate) fn reads_of_selector(selector: &Selector) -> Traits {
    selector
        .0
        .iter()
        .fold(Traits::NONE, |all, pred| all.or(reads_of_pred(pred)))
}

#[allow(clippy::too_many_lines)]
fn reads_of_pred(pred: &Pred) -> Traits {
    match pred {
        Pred::Supertype(_) => Traits::SUPERTYPES,
        Pred::Drafted(_)
        | Pred::This
        | Pred::Other
        | Pred::Apart(_)
        | Pred::Token
        | Pred::Commander
        | Pred::Spell
        | Pred::Dungeon
        | Pred::Emblem
        | Pred::Ability(_)
        | Pred::Tapped
        | Pred::Untapped
        | Pred::Monstrous
        | Pred::Renowned
        | Pred::Prepared
        | Pred::Suspected
        | Pred::Solved
        | Pred::Stickered
        | Pred::WithSticker(_)
        | Pred::Harnessed
        | Pred::FoughtSinceYourLastUpkeep { .. }
        | Pred::DoubleFaced
        | Pred::Transformed
        | Pred::Paired
        | Pred::EnteredThisTurn
        | Pred::SummoningSick
        | Pred::R114Enlisted
        | Pred::MilledThisTurn
        | Pred::DrawnThisTurn
        | Pred::LastDrawnThisTurn
        | Pred::HasAdventure
        | Pred::DamagedThisTurn
        | Pred::AttackedThisTurn
        | Pred::AttackedLastTurn
        | Pred::DealtDamageYet
        | Pred::DealtCombatDamageYet
        | Pred::NthCastThisTurn(_)
        | Pred::Attacking
        | Pred::AttackingAlone
        | Pred::AttackingBattle
        | Pred::Blocking
        | Pred::Unblocked
        | Pred::Blocked
        | Pred::In(_)
        | Pred::CastFrom(_)
        | Pred::HasCounter(_)
        | Pred::CounterCount { .. }
        | Pred::CanEnchantSource
        | Pred::XInCost
        | Pred::PhyrexianInCost
        | Pred::GenericCost(_)
        | Pred::SingleTarget
        | Pred::HasAnyCounter
        | Pred::Named(_)
        | Pred::OriginallyPrintedIn(_)
        | Pred::ChosenName
        | Pred::FaceDown
        | Pred::SpentFrom { .. }
        | Pred::R113SnowOfOwnColor
        | Pred::GravedThisTurn
        | Pred::GravedThisCombat
        | Pred::R6500StartedTurnUntapped
        | Pred::SharesColorWithLastCast
        | Pred::CouldAttackThisTurn
        | Pred::ArrivedThisTurn
        | Pred::ArrivedFrom(_)
        | Pred::CastWith(_)
        | Pred::TopOfLibrary
        | Pred::BottomOfLibrary
        | Pred::TopOfGraveyard
        | Pred::TopCreatureOfGraveyard
        | Pred::R4700TopOfGraveyard { .. }
        | Pred::Kicked
        | Pred::R112Modal
        | Pred::AdditionalCostPaid
        | Pred::Suspended
        | Pred::Foretold
        | Pred::HasName
        | Pred::BlockedThisTurn(_)
        | Pred::DealtDamageThisTurn(_)
        | Pred::R6100DamagedThisGameBy(_)
        | Pred::EnteredSinceLastTurn(_) => Traits::NONE,
        Pred::WasCast { by, .. } => by.as_ref().map_or(Traits::NONE, reads_of_ref),
        Pred::DidThisTurn { by, to, .. } => by
            .iter()
            .chain(to)
            .fold(Traits::NONE, |read, who| read.or(reads_of_ref(who))),
        Pred::GreatestAmong { stat, group } | Pred::LeastAmong { stat, group } => {
            reads_of_stat(*stat).or(reads_of_selector(group))
        }
        Pred::Modified | Pred::RingBearer => Traits::CONTROL,
        Pred::Goaded => Traits::GOADED,
        Pred::Is(who)
        | Pred::BlockedBy(who)
        | Pred::BlockedThisTurnBy(who)
        | Pred::CastBefore(who)
        | Pred::BandedWith(who)
        | Pred::Blocks(who)
        | Pred::TargetedBy(who)
        | Pred::R113CopyOf(who)
        | Pred::R114CouldTarget(who)
        | Pred::TargetsPlayer(who)
        | Pred::OwnedBy(who)
        | Pred::ProtectedBy(who)
        | Pred::AttackingPlayer(who)
        | Pred::AttackingPlaneswalkerOf(who)
        | Pred::AttackingPlayerOrPlaneswalker(who)
        | Pred::ExiledWith(who)
        | Pred::R114ExiledByThem(who)
        | Pred::R114EnlistedBy(who)
        | Pred::R117SurveilledBy(who)
        | Pred::R117SharesBasicLandType(who)
        | Pred::EncodedWith(who)
        | Pred::HauntedBy(who)
        | Pred::Crewed(who)
        | Pred::Saddled(who)
        | Pred::SharesName(who)
        | Pred::DamagedBy(who)
        | Pred::DamagedThisTurnBy(who)
        | Pred::AttachedTo(who)
        | Pred::EnchantableBy(who)
        | Pred::ManaFrom(who)
        | Pred::PairedWith(who)
        | Pred::SameCard(who)
        | Pred::SourceOf(who)
        | Pred::SharesManaValue(who) => reads_of_ref(who),
        Pred::SharesType(who) | Pred::SharesTypeOf { with: who, .. } => {
            Traits::TYPES.or(reads_of_ref(who))
        }
        Pred::SharesColor(who) => Traits::COLORS.or(reads_of_ref(who)),
        Pred::Type(_) => Traits::TYPES,
        Pred::Subtype(_) => Traits::SUBTYPES,
        Pred::ChosenSubtype | Pred::NthChosenLandType(_) => Traits::SUBTYPES.or(Traits::TYPES),
        Pred::ChosenAs(_) => Traits::COLORS.or(Traits::TYPES).or(Traits::SUBTYPES),
        Pred::R6600OnlyControllersColor(_) => Traits::COLORS.or(Traits::CONTROL),
        Pred::SharesSubtype(who) | Pred::SharesCreatureTypeWithEach(who) => {
            Traits::SUBTYPES.or(reads_of_ref(who))
        }
        Pred::Color(_)
        | Pred::Colorless
        | Pred::ChosenColor
        | Pred::R112SharesColorWithSpent
        | Pred::OutsideCommanderIdentity => Traits::COLORS,
        Pred::ControlledBy(who) => Traits::CONTROL.or(reads_of_ref(who)),
        Pred::HasKeyword(_) | Pred::HasKeywordNamed(_) | Pred::R113HasWay(_) => Traits::KEYWORDS,
        // What a permanent could produce is read from its mana abilities.
        Pred::HasAbility(_) | Pred::CouldProduce(_) => Traits::KEYWORDS.or(Traits::RULES),
        Pred::Compare { stat, to, .. } => reads_of_stat(*stat).or(reads_of_amount(to)),
        Pred::R113Parity { stat, .. } => reads_of_stat(*stat),
        Pred::TotalAtMost { stat, most } => reads_of_stat(*stat).or(reads_of_amount(most)),
        Pred::Test { left, right, .. } => reads_of_amount(left).or(reads_of_amount(right)),
        Pred::Not(inner) => reads_of_pred(inner),
        Pred::Targeting(inner)
        | Pred::R6104TargetsOnly(inner)
        | Pred::R114TargetsOnlyOne(inner)
        | Pred::HasAttached(inner)
        | Pred::FromSource(inner)
        | Pred::AbilityFrom(inner) => reads_of_selector(inner),
        Pred::AnyOf(any) | Pred::R117AllOf(any) => any
            .iter()
            .fold(Traits::NONE, |all, inner| all.or(reads_of_pred(inner))),
    }
}

fn reads_of_stat(stat: StatKind) -> Traits {
    match stat {
        // The base after the effects that set it still depends on the layers that set power and toughness.
        StatKind::Power | StatKind::BasePower => Traits::POWER,
        StatKind::Toughness | StatKind::BaseToughness => Traits::TOUGHNESS,
        StatKind::Colors | StatKind::R374ColorSet | StatKind::R4700ChosenColors => Traits::COLORS,
        StatKind::BasicLandTypes | StatKind::CreatureType | StatKind::Subtypes => Traits::SUBTYPES,
        StatKind::R6018PermanentTypes => Traits::TYPES,
        StatKind::R6500StationValue => Traits::POWER.or(Traits::TOUGHNESS),
        StatKind::R374Types => Traits::TYPES.or(Traits::SUBTYPES).or(Traits::SUPERTYPES),
        StatKind::R374CreatureTypes => Traits::TYPES.or(Traits::SUBTYPES),
        StatKind::R374Bushido | StatKind::R374Toxic => Traits::KEYWORDS,
        StatKind::ManaValue
        | StatKind::Loyalty
        | StatKind::R6104LifePaid
        | StatKind::ManaSpent
        | StatKind::ColorsSpent
        | StatKind::TreasureManaSpent
        | StatKind::ArtifactManaSpent
        | StatKind::ColoredManaSymbols
        | StatKind::Kicks
        | StatKind::Name
        | StatKind::CounterKinds
        | StatKind::R374ManaCost
        | StatKind::R112ModesChosen => Traits::NONE,
    }
}

/// What an amount reads when it is worked out.
#[allow(clippy::too_many_lines)] // one arm for each kind of amount
pub(crate) fn reads_of_amount(amount: &Amount) -> Traits {
    match amount {
        Amount::Fixed(_)
        | Amount::X
        | Amount::Divided
        | Amount::TargetCount(_)
        | Amount::Kicks
        | Amount::AdditionalPaid
        | Amount::R117AdditionalPaidWith(_)
        | Amount::Promised
        | Amount::ResolvedThisTurn
        | Amount::Devoured
        | Amount::R6500DevouredMatching(_)
        | Amount::RemovedThisWay(_)
        | Amount::Bound(_)
        | Amount::CastWith(_)
        | Amount::ColorsSpent
        | Amount::PermanentsGraved
        | Amount::LifeLostThisWay
        | Amount::R6702FlipStakes
        | Amount::R112GroupedThisTurn
        | Amount::DamageDealtThisWay
        | Amount::Destroyed(_)
        | Amount::Exiled(_)
        | Amount::PhasedOut(_)
        | Amount::DieResult(_)
        | Amount::TurnNumber
        | Amount::Stickers { .. }
        | Amount::StickerVowels
        | Amount::NameStickers { .. }
        | Amount::StickerLetters { .. }
        | Amount::StickerStats { .. }
        | Amount::TallyLastTurn { .. }
        | Amount::CastSameNameThisGame(_)
        | Amount::CycledSameNameThisGame(_)
        | Amount::TallyTotal { .. }
        | Amount::TallyTypes { .. }
        | Amount::ManaSpent { .. }
        | Amount::ManaColors(_)
        | Amount::ManaInPool(_)
        | Amount::ManaInPoolOf { .. }
        | Amount::Random { .. }
        | Amount::TriggeringTargets
        | Amount::PlayersLost
        | Amount::CastTargets
        | Amount::CastTargetsOf(_)
        | Amount::Chosen
        | Amount::ChosenAs(_)
        | Amount::Triggering
        | Amount::R110MostDamageToOne
        | Amount::R110RemovedThisWay(_)
        | Amount::Drafted(_) => Traits::NONE,
        Amount::Count(selector) => reads_of_selector(selector),
        Amount::Stat { of, stat } => reads_of_ref(of).or(reads_of_stat(*stat)),
        Amount::Life(who)
        | Amount::SourcesDealtDamage(who)
        | Amount::MostDamageBySource(who)
        | Amount::GreatestCast { who, .. }
        | Amount::CommanderCasts(who)
        | Amount::CastsFromCommand(who)
        | Amount::Speed(who)
        | Amount::Counters { on: who, .. }
        | Amount::AnyCounters(who)
        | Amount::Mutations(who)
        | Amount::Players(who)
        | Amount::Tallied { who, .. }
        | Amount::CommanderColors(who)
        | Amount::StartingLife(who)
        | Amount::TurnsTaken(who)
        | Amount::LifeAsTurnBegan(who)
        | Amount::CastLastTurn(who)
        | Amount::TalliedLastTurn { who, .. }
        | Amount::HighestCounters { who, .. }
        | Amount::LowestLife(who)
        | Amount::LowestHand(who)
        | Amount::HighestLife(who) => reads_of_ref(who),
        Amount::Sum(parts) | Amount::Product(parts) => parts
            .iter()
            .fold(Traits::NONE, |all, part| all.or(reads_of_amount(part))),
        Amount::DamageDealtByTo { by, to } => reads_of_ref(by).or(reads_of_ref(to)),
        Amount::DamageToFromPlayers { to, players } => reads_of_ref(to).or(reads_of_ref(players)),
        Amount::CountOf { of, filter } => reads_of_ref(of).or(reads_of_selector(filter)),
        // A copy that was not cast reads the inner amount now.
        Amount::EvenShare(total) | Amount::CountedAsCast(total) => reads_of_amount(total),
        Amount::Times { of, .. }
        | Amount::Half { of, .. }
        | Amount::TwoToThe(of)
        | Amount::AtMost { of, .. } => reads_of_amount(of),
        Amount::Quotient { of, by, .. } | Amount::Difference(of, by) => {
            reads_of_amount(of).or(reads_of_amount(by))
        }
        Amount::R374AtMost(inner, _) => reads_of_amount(inner),
        Amount::Devotion(_) => Traits::CONTROL,
        Amount::ManaSymbols { of, .. } => reads_of_ref(of),
        Amount::Greatest { of, stat }
        | Amount::Total { of, stat }
        | Amount::Distinct { of, stat }
        | Amount::MostCommon { of, stat } => reads_of_selector(of).or(reads_of_stat(*stat)),
        Amount::Doors(of) => reads_of_selector(of),
        Amount::CardsAbove { filter, .. } => reads_of_selector(filter),
        Amount::When {
            condition,
            then,
            otherwise,
        } => reads_of_condition(condition)
            .or(reads_of_amount(then))
            .or(reads_of_amount(otherwise)),
        Amount::Tally { who, filter, .. } => reads_of_ref(who).or(reads_of_selector(filter)),
        Amount::Remembered { on, .. }
        | Amount::R112MostAlike { on, .. }
        | Amount::R113Stored { on, .. }
        | Amount::R113SharesWithAnother { of: on, .. }
        | Amount::R117GreatestOf { of: on, .. }
        | Amount::R113GreatestNoted { on }
        | Amount::R113NotedWords { on } => reads_of_ref(on),
        Amount::R112NumberChosen { by, .. } => reads_of_ref(by),
        Amount::CardTypes(of) => reads_of_selector(of).or(Traits::TYPES),
        Amount::R6702CardTypesOf(of) => reads_of_ref(of).or(Traits::TYPES),
        Amount::Largest { who, of } | Amount::Smallest { who, of } => {
            reads_of_ref(who).or(reads_of_selector(of))
        }
        Amount::Party(who) => reads_of_ref(who)
            .or(Traits::SUBTYPES)
            .or(Traits::CONTROL)
            .or(Traits::TYPES),
    }
}

/// What a condition reads when it is checked.
pub(crate) fn reads_of_condition(condition: &Condition) -> Traits {
    match condition {
        Condition::Not(inner) | Condition::AtTrigger(inner) => reads_of_condition(inner),
        Condition::All(all) | Condition::Any(all) => all
            .iter()
            .fold(Traits::NONE, |sum, inner| sum.or(reads_of_condition(inner))),
        Condition::Compare { left, right, .. } => reads_of_amount(left).or(reads_of_amount(right)),
        Condition::Matches { subject, selector } => {
            reads_of_ref(subject).or(reads_of_selector(selector))
        }
        Condition::CountOf {
            subject,
            selector,
            right,
            ..
        } => reads_of_ref(subject)
            .or(reads_of_selector(selector))
            .or(reads_of_amount(right)),
        Condition::YourTurn | Condition::PaidToIgnore => Traits::CONTROL,
        Condition::PaidToIgnoreBy(who) => reads_of_ref(who).or(Traits::CONTROL),
        Condition::NewlyControlled(what) => reads_of_ref(what).or(Traits::CONTROL),
        Condition::AttackedThisTurnBy { player, .. } => reads_of_ref(player),
        Condition::Exploited { by, what } => reads_of_ref(by).or(reads_of_ref(what)),
        Condition::DamagedThisGameBy { .. }
        | Condition::Drafted(_)
        | Condition::Kicked
        | Condition::KickedWith(_)
        | Condition::AdditionalCostPaid
        | Condition::CastWith(_)
        | Condition::R112CastByOwnAbility
        | Condition::R113SpentFromTriggering { .. }
        | Condition::CastOffSorceryTiming
        | Condition::AsCast(_)
        | Condition::R401OnCast(_)
        | Condition::R401Targets(_)
        | Condition::ManaSpent(_)
        | Condition::IsDay
        | Condition::IsNight
        | Condition::Step(_)
        | Condition::BeforeAttackers
        | Condition::AfterBlockers
        | Condition::InCombat
        | Condition::FirstCombat
        | Condition::FirstStep
        | Condition::SorceryTiming
        | Condition::AfterCombat
        | Condition::TriggeringSticker(_)
        | Condition::NeitherDayNorNight
        | Condition::R111CastOrPutLastTurn
        | Condition::R117FirstSpellThisGame
        | Condition::R111DamagedThisTurnBy(_)
        | Condition::ChosenWord { .. }
        | Condition::EnteredAs(_)
        | Condition::Performed
        | Condition::ExtraTurn => Traits::NONE,
        Condition::Monarch(who)
        | Condition::R118BegunExtraTurn(who)
        | Condition::MonarchAtTurnStart(who)
        | Condition::AttackedYouLastTurn(who)
        | Condition::CompletedDungeon(who)
        | Condition::Initiative(who)
        | Condition::CitysBlessing(who)
        | Condition::EnduringStory(who)
        | Condition::MaxSpeed(who)
        | Condition::RingTempted { who, .. }
        | Condition::FastestPlayer(who)
        | Condition::CompletedNamedDungeon { who, .. }
        | Condition::Among(who)
        | Condition::StartingPlayer(who)
        | Condition::Searched { who, .. }
        | Condition::VotedAs { with: who, .. }
        | Condition::CombatDamagedSinceLastTurn(who)
        | Condition::R111AttackedLastCombat(who)
        | Condition::AttackedThisCombat(who) => reads_of_ref(who),
        Condition::Level { of, .. } => reads_of_ref(of),
        Condition::R115MostCommonColor(among) => reads_of_selector(among).or(Traits::COLORS),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a `Pred::Test` reports to the layers what its amounts read, so a filter that compares power to base power waits for the effects that set them.
    use mtg_cards::refs::{Pred, Selector};

    use super::*;

    #[test]
    fn a_test_reads_the_stats_it_compares() {
        let pred: Pred = serde_json::from_str(
            r#"{"Test":{"left":{"Stat":{"of":"Candidate","stat":"Power"}},"op":"Gt","right":{"Stat":{"of":"Candidate","stat":"BasePower"}}}}"#,
        )
        .unwrap();
        let power: Pred =
            serde_json::from_str(r#"{"Compare":{"stat":"Power","op":"Gt","to":0}}"#).unwrap();
        let reads = reads_of_selector(&Selector(vec![pred]));
        assert_ne!(reads, Traits::NONE);
        assert_eq!(
            reads,
            reads_of_selector(&Selector(vec![power])).or(reads_of_stat(StatKind::BasePower))
        );
    }
}
