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
        Modification::AddSubtype(_)
        | Modification::SetCreatureTypes(_)
        | Modification::AddAllCreatureTypes => Traits::SUBTYPES,
        // The land loses its abilities with its land types.
        Modification::SetLandTypes(_) => Traits::SUBTYPES.or(Traits::KEYWORDS).or(Traits::RULES),
        Modification::SetColors(_) | Modification::AddColors(_) => Traits::COLORS,
        Modification::AddKeyword(_)
        | Modification::RemoveKeyword(_)
        | Modification::LockKeyword(_) => Traits::KEYWORDS,
        Modification::LoseAbilities => Traits::KEYWORDS.or(Traits::RULES),
        Modification::SetController(_) => Traits::CONTROL,
        Modification::SetPowerToughness { .. }
        | Modification::SetPower(_)
        | Modification::SetToughness(_)
        | Modification::ModifyPowerToughness { .. }
        | Modification::SwitchPowerToughness => Traits::POWER.or(Traits::TOUGHNESS),
        // An ability that is given adds effects (the pass finds them as they come) and changes none that exist.
        Modification::GrantAbility(_)
        | Modification::GainAbilities(_)
        | Modification::CantAttack
        | Modification::CantAttackYou { .. }
        | Modification::CantBlock
        | Modification::DoesntUntap
        | Modification::CantBeTurnedFaceUp
        | Modification::CantBeBlocked
        | Modification::CantBeBlockedBy { .. }
        | Modification::CanBlockOnly { .. }
        | Modification::MinBlockers(_)
        | Modification::MaxBlockers(_)
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
        Wanted::AddSubtype(_) | Wanted::SetCreatureTypes(_) | Wanted::AddAllCreatureTypes => {
            Traits::SUBTYPES
        }
        Wanted::SetLandTypes(_) | Wanted::SetLandTypesChosen(_) => {
            Traits::SUBTYPES.or(Traits::KEYWORDS).or(Traits::RULES)
        }
        Wanted::SetCreatureTypesChosen(_) => Traits::SUBTYPES,
        Wanted::SetColors(_) | Wanted::AddColors(_) | Wanted::SetColorsChosen(_) => Traits::COLORS,
        Wanted::AddKeyword(_)
        | Wanted::AddProtectionFromChosenColor
        | Wanted::AddProtectionFromChosenCardType
        | Wanted::RemoveKeyword(_)
        | Wanted::LockKeyword(_) => Traits::KEYWORDS,
        Wanted::LoseAbilities => Traits::KEYWORDS.or(Traits::RULES),
        Wanted::SetController(_) => Traits::CONTROL,
        Wanted::SetPT { .. }
        | Wanted::SetPower(_)
        | Wanted::SetToughness(_)
        | Wanted::ModifyPT { .. }
        | Wanted::SwitchPT => Traits::POWER.or(Traits::TOUGHNESS),
        Wanted::GrantAbility(_)
        | Wanted::GainAbilitiesOf(_)
        | Wanted::RemoveSupertype(_)
        | Wanted::AddSupertype(_)
        | Wanted::SetName(_)
        | Wanted::ThisAbility
        | Wanted::CantAttack
        | Wanted::CantAttackYou(_)
        | Wanted::CantBlock
        | Wanted::DoesntUntap
        | Wanted::CantBeTurnedFaceUp
        | Wanted::CantBeBlocked
        | Wanted::CantBeBlockedBy(_)
        | Wanted::CanBlockOnly(_)
        | Wanted::MinBlockers(_)
        | Wanted::MaxBlockers(_)
        | Wanted::BlocksAdditional(_) => Traits::NONE,
        Wanted::Goaded => Traits::GOADED,
    }
}

/// What the amounts and the player of a modification of the data read when they are worked out.
pub(crate) fn reads_of_modification(wanted: &Wanted) -> Traits {
    match wanted {
        Wanted::ModifyPT { power, toughness } | Wanted::SetPT { power, toughness } => {
            reads_of_amount(power).or(reads_of_amount(toughness))
        }
        Wanted::SetPower(value) | Wanted::SetToughness(value) => reads_of_amount(value),
        Wanted::SetController(who) | Wanted::GainAbilitiesOf(who) => reads_of_ref(who),
        Wanted::CantBeBlockedBy(blockers) | Wanted::CanBlockOnly(blockers) => {
            reads_of_selector(blockers)
        }
        Wanted::SwitchPT
        | Wanted::AddType(_)
        | Wanted::RemoveType(_)
        | Wanted::SetTypes(_)
        | Wanted::RemoveSupertype(_)
        | Wanted::AddSupertype(_)
        | Wanted::SetName(_)
        | Wanted::ThisAbility
        | Wanted::AddSubtype(_)
        | Wanted::SetCreatureTypes(_)
        | Wanted::SetLandTypes(_)
        | Wanted::SetLandTypesChosen(_)
        | Wanted::SetCreatureTypesChosen(_)
        | Wanted::SetColorsChosen(_)
        | Wanted::AddAllCreatureTypes
        | Wanted::SetColors(_)
        | Wanted::AddColors(_)
        | Wanted::AddKeyword(_)
        | Wanted::AddProtectionFromChosenColor
        | Wanted::AddProtectionFromChosenCardType
        | Wanted::RemoveKeyword(_)
        | Wanted::LockKeyword(_)
        | Wanted::GrantAbility(_)
        | Wanted::LoseAbilities
        | Wanted::CantAttack
        | Wanted::CantAttackYou(_)
        | Wanted::CantBlock
        | Wanted::DoesntUntap
        | Wanted::CantBeTurnedFaceUp
        | Wanted::CantBeBlocked
        | Wanted::MinBlockers(_)
        | Wanted::MaxBlockers(_)
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
        | Ref::Chosen
        | Ref::Promised
        | Ref::Defending
        | Ref::Attacked
        | Ref::MostLife
        | Ref::ActivePlayer
        | Ref::Bound(_)
        | Ref::Affected
        | Ref::Candidate
        | Ref::Paid => Traits::NONE,
        Ref::Remembered { on, .. } => reads_of_ref(on),
        Ref::ControllerOf(inner) => Traits::CONTROL.or(reads_of_ref(inner)),
        Ref::OwnerOf(inner) => reads_of_ref(inner),
        Ref::OrController(inner) => Traits::CONTROL.or(reads_of_ref(inner)),
        Ref::Arrived { of, .. } | Ref::Departed(of) => reads_of_ref(of),
        Ref::Each(selector) => reads_of_selector(selector),
    }
}

/// What a selector reads of each object it tests.
pub(crate) fn reads_of_selector(selector: &Selector) -> Traits {
    selector
        .0
        .iter()
        .fold(Traits::NONE, |all, pred| all.or(reads_of_pred(pred)))
}

fn reads_of_pred(pred: &Pred) -> Traits {
    match pred {
        Pred::This
        | Pred::Other
        | Pred::Supertype(_)
        | Pred::Token
        | Pred::Commander
        | Pred::Spell
        | Pred::Ability(_)
        | Pred::Tapped
        | Pred::Untapped
        | Pred::Monstrous
        | Pred::Renowned
        | Pred::Paired
        | Pred::EnteredThisTurn
        | Pred::SummoningSick
        | Pred::MilledThisTurn
        | Pred::HasAdventure
        | Pred::DamagedThisTurn
        | Pred::AttackedThisTurn
        | Pred::Attacking
        | Pred::Blocking
        | Pred::Unblocked
        | Pred::In(_)
        | Pred::CastFrom(_)
        | Pred::HasCounter(_)
        | Pred::CounterCount { .. }
        | Pred::CanEnchantSource
        | Pred::XInCost
        | Pred::GenericCost(_)
        | Pred::SingleTarget
        | Pred::HasAnyCounter
        | Pred::Named(_)
        | Pred::ChosenName
        | Pred::FaceDown
        | Pred::GravedThisTurn
        | Pred::ArrivedFrom(_)
        | Pred::CastWith(_)
        | Pred::TopOfLibrary
        | Pred::BottomOfLibrary
        | Pred::Kicked
        | Pred::AdditionalCostPaid
        | Pred::Suspended => Traits::NONE,
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
        | Pred::Blocks(who)
        | Pred::TargetedBy(who)
        | Pred::TargetsPlayer(who)
        | Pred::OwnedBy(who)
        | Pred::AttackingPlayer(who)
        | Pred::ExiledWith(who)
        | Pred::EncodedWith(who)
        | Pred::Crewed(who)
        | Pred::Saddled(who)
        | Pred::SharesName(who)
        | Pred::DamagedBy(who)
        | Pred::AttachedTo(who)
        | Pred::PairedWith(who)
        | Pred::SameCard(who) => reads_of_ref(who),
        Pred::SharesType(who) => Traits::TYPES.or(reads_of_ref(who)),
        Pred::SharesColor(who) => Traits::COLORS.or(reads_of_ref(who)),
        Pred::Type(_) => Traits::TYPES,
        Pred::Subtype(_) | Pred::ChosenSubtype => Traits::SUBTYPES,
        Pred::ChosenAs(_) => Traits::COLORS.or(Traits::TYPES).or(Traits::SUBTYPES),
        Pred::SharesSubtype(who) => Traits::SUBTYPES.or(reads_of_ref(who)),
        Pred::Color(_) | Pred::Colorless | Pred::ChosenColor => Traits::COLORS,
        Pred::ControlledBy(who) => Traits::CONTROL.or(reads_of_ref(who)),
        Pred::HasKeyword(_) | Pred::HasKeywordNamed(_) => Traits::KEYWORDS,
        Pred::Compare { stat, to, .. } => reads_of_stat(*stat).or(reads_of_amount(to)),
        Pred::Test { left, right, .. } => reads_of_amount(left).or(reads_of_amount(right)),
        Pred::Not(inner) => reads_of_pred(inner),
        Pred::Targeting(inner) | Pred::HasAttached(inner) => reads_of_selector(inner),
        Pred::AnyOf(any) => any
            .iter()
            .fold(Traits::NONE, |all, inner| all.or(reads_of_pred(inner))),
    }
}

fn reads_of_stat(stat: StatKind) -> Traits {
    match stat {
        // The base after the effects that set it still depends on the layers that set power and toughness.
        StatKind::Power | StatKind::BasePower => Traits::POWER,
        StatKind::Toughness | StatKind::BaseToughness => Traits::TOUGHNESS,
        StatKind::Colors => Traits::COLORS,
        StatKind::BasicLandTypes | StatKind::CreatureType => Traits::SUBTYPES,
        StatKind::ManaValue | StatKind::Loyalty | StatKind::ManaSpent | StatKind::Name => {
            Traits::NONE
        }
    }
}

/// What an amount reads when it is worked out.
pub(crate) fn reads_of_amount(amount: &Amount) -> Traits {
    match amount {
        Amount::Fixed(_)
        | Amount::X
        | Amount::Divided
        | Amount::TargetCount(_)
        | Amount::Kicks
        | Amount::Promised
        | Amount::ResolvedThisTurn
        | Amount::Devoured
        | Amount::Bound(_)
        | Amount::CastWith(_)
        | Amount::ColorsSpent
        | Amount::PermanentsGraved
        | Amount::LifeLostThisWay
        | Amount::Destroyed(_)
        | Amount::Exiled(_)
        | Amount::PhasedOut(_)
        | Amount::DieResult(_)
        | Amount::TurnNumber
        | Amount::ManaSpent { .. }
        | Amount::ManaColors(_)
        | Amount::ManaInPool(_)
        | Amount::Random { .. }
        | Amount::TriggeringTargets
        | Amount::Chosen
        | Amount::ChosenAs(_)
        | Amount::Triggering => Traits::NONE,
        Amount::Count(selector) => reads_of_selector(selector),
        Amount::Stat { of, stat } => reads_of_ref(of).or(reads_of_stat(*stat)),
        Amount::Life(who)
        | Amount::CommanderCasts(who)
        | Amount::Counters { on: who, .. }
        | Amount::AnyCounters(who)
        | Amount::Mutations(who)
        | Amount::Players(who)
        | Amount::Tallied { who, .. }
        | Amount::CommanderColors(who)
        | Amount::StartingLife(who)
        | Amount::CastLastTurn(who)
        | Amount::HighestCounters { who, .. }
        | Amount::LowestLife(who)
        | Amount::LowestHand(who)
        | Amount::HighestLife(who) => reads_of_ref(who),
        Amount::Sum(parts) | Amount::Product(parts) => parts
            .iter()
            .fold(Traits::NONE, |all, part| all.or(reads_of_amount(part))),
        Amount::CountOf { of, filter } => reads_of_ref(of).or(reads_of_selector(filter)),
        Amount::EvenShare(total) => reads_of_amount(total),
        Amount::Times { of, .. } | Amount::Half { of, .. } => reads_of_amount(of),
        Amount::Devotion(_) => Traits::CONTROL,
        Amount::Greatest { of, stat }
        | Amount::Total { of, stat }
        | Amount::Distinct { of, stat }
        | Amount::MostCommon { of, stat } => reads_of_selector(of).or(reads_of_stat(*stat)),
        Amount::Doors(of) => reads_of_selector(of),
        Amount::When {
            condition,
            then,
            otherwise,
        } => reads_of_condition(condition)
            .or(reads_of_amount(then))
            .or(reads_of_amount(otherwise)),
        Amount::Tally { who, filter, .. } => reads_of_ref(who).or(reads_of_selector(filter)),
        Amount::Remembered { on, .. } => reads_of_ref(on),
        Amount::CardTypes(of) => reads_of_selector(of).or(Traits::TYPES),
        Amount::Largest { who, of } => reads_of_ref(who).or(reads_of_selector(of)),
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
        Condition::YourTurn => Traits::CONTROL,
        Condition::NewlyControlled(what) => reads_of_ref(what).or(Traits::CONTROL),
        Condition::Kicked
        | Condition::KickedWith(_)
        | Condition::AdditionalCostPaid
        | Condition::CastWith(_)
        | Condition::ManaSpent(_)
        | Condition::IsDay
        | Condition::IsNight
        | Condition::Step(_)
        | Condition::BeforeAttackers
        | Condition::InCombat
        | Condition::FirstCombat
        | Condition::NeitherDayNorNight
        | Condition::ChosenWord { .. }
        | Condition::Performed => Traits::NONE,
        Condition::Monarch(who)
        | Condition::MonarchAtTurnStart(who)
        | Condition::CompletedDungeon(who)
        | Condition::Initiative(who)
        | Condition::CitysBlessing(who)
        | Condition::StartingPlayer(who)
        | Condition::Searched { who, .. } => reads_of_ref(who),
        Condition::Level { of, .. } => reads_of_ref(of),
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
