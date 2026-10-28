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
pub(crate) struct Traits(u8);

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
        Modification::SetColors(_) => Traits::COLORS,
        Modification::AddKeyword(_) | Modification::RemoveKeyword(_) => Traits::KEYWORDS,
        Modification::LoseAbilities => Traits::KEYWORDS.or(Traits::RULES),
        Modification::SetController(_) => Traits::CONTROL,
        Modification::SetPowerToughness { .. }
        | Modification::ModifyPowerToughness { .. }
        | Modification::SwitchPowerToughness => Traits::POWER.or(Traits::TOUGHNESS),
        // An ability that is given adds effects (the pass finds them as they come) and changes none that exist.
        Modification::GrantAbility(_)
        | Modification::CantAttack
        | Modification::CantAttackYou { .. }
        | Modification::CantBlock
        | Modification::DoesntUntap
        | Modification::CantBeBlocked
        | Modification::CantBeBlockedBy { .. }
        | Modification::CanBlockOnly { .. }
        | Modification::MinBlockers(_)
        | Modification::MaxBlockers(_)
        | Modification::Goaded { .. } => Traits::NONE,
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
        Wanted::SetColors(_) => Traits::COLORS,
        Wanted::AddKeyword(_) | Wanted::AddProtectionFromChosenColor | Wanted::RemoveKeyword(_) => {
            Traits::KEYWORDS
        }
        Wanted::LoseAbilities => Traits::KEYWORDS.or(Traits::RULES),
        Wanted::SetController(_) => Traits::CONTROL,
        Wanted::SetPT { .. } | Wanted::ModifyPT { .. } | Wanted::SwitchPT => {
            Traits::POWER.or(Traits::TOUGHNESS)
        }
        Wanted::GrantAbility(_)
        | Wanted::RemoveSupertype(_)
        | Wanted::CantAttack
        | Wanted::CantAttackYou(_)
        | Wanted::CantBlock
        | Wanted::DoesntUntap
        | Wanted::CantBeBlocked
        | Wanted::CantBeBlockedBy(_)
        | Wanted::CanBlockOnly(_)
        | Wanted::MinBlockers(_)
        | Wanted::MaxBlockers(_)
        | Wanted::Goaded => Traits::NONE,
    }
}

/// What the amounts and the player of a modification of the data read when they are worked out.
pub(crate) fn reads_of_modification(wanted: &Wanted) -> Traits {
    match wanted {
        Wanted::ModifyPT { power, toughness } | Wanted::SetPT { power, toughness } => {
            reads_of_amount(power).or(reads_of_amount(toughness))
        }
        Wanted::SetController(who) => reads_of_ref(who),
        Wanted::CantBeBlockedBy(blockers) | Wanted::CanBlockOnly(blockers) => {
            reads_of_selector(blockers)
        }
        Wanted::SwitchPT
        | Wanted::AddType(_)
        | Wanted::RemoveType(_)
        | Wanted::SetTypes(_)
        | Wanted::RemoveSupertype(_)
        | Wanted::AddSubtype(_)
        | Wanted::SetCreatureTypes(_)
        | Wanted::AddAllCreatureTypes
        | Wanted::SetColors(_)
        | Wanted::AddKeyword(_)
        | Wanted::AddProtectionFromChosenColor
        | Wanted::RemoveKeyword(_)
        | Wanted::GrantAbility(_)
        | Wanted::LoseAbilities
        | Wanted::CantAttack
        | Wanted::CantAttackYou(_)
        | Wanted::CantBlock
        | Wanted::DoesntUntap
        | Wanted::CantBeBlocked
        | Wanted::MinBlockers(_)
        | Wanted::MaxBlockers(_)
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
        | Ref::Dealer
        | Ref::Attached
        | Ref::AttachedPlayer
        | Ref::Monarch
        | Ref::Chosen
        | Ref::Promised
        | Ref::Defending
        | Ref::Attacked
        | Ref::MostLife
        | Ref::Bound(_)
        | Ref::Paid => Traits::NONE,
        Ref::ControllerOf(inner) => Traits::CONTROL.or(reads_of_ref(inner)),
        Ref::OwnerOf(inner) => reads_of_ref(inner),
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
        | Pred::EnteredThisTurn
        | Pred::DamagedThisTurn
        | Pred::AttackedThisTurn
        | Pred::Attacking
        | Pred::Blocking
        | Pred::Unblocked
        | Pred::In(_)
        | Pred::CastFrom(_)
        | Pred::HasCounter(_)
        | Pred::XInCost
        | Pred::Goaded
        | Pred::HasAnyCounter
        | Pred::Named(_) => Traits::NONE,
        Pred::Is(who)
        | Pred::OwnedBy(who)
        | Pred::AttackingPlayer(who)
        | Pred::ExiledWith(who)
        | Pred::EncodedWith(who)
        | Pred::Crewed(who)
        | Pred::Saddled(who)
        | Pred::SharesName(who)
        | Pred::DamagedBy(who)
        | Pred::SameCard(who) => reads_of_ref(who),
        Pred::SharesType(who) => Traits::TYPES.or(reads_of_ref(who)),
        Pred::SharesColor(who) => Traits::COLORS.or(reads_of_ref(who)),
        Pred::Type(_) => Traits::TYPES,
        Pred::Subtype(_) | Pred::ChosenSubtype => Traits::SUBTYPES,
        Pred::SharesSubtype(who) => Traits::SUBTYPES.or(reads_of_ref(who)),
        Pred::Color(_) | Pred::Colorless | Pred::ChosenColor => Traits::COLORS,
        Pred::ControlledBy(who) => Traits::CONTROL.or(reads_of_ref(who)),
        Pred::HasKeyword(_) => Traits::KEYWORDS,
        Pred::Compare { stat, to, .. } => reads_of_stat(*stat).or(reads_of_amount(to)),
        Pred::Not(inner) => reads_of_pred(inner),
        Pred::Targeting(inner) | Pred::HasAttached(inner) => reads_of_selector(inner),
        Pred::AnyOf(any) => any
            .iter()
            .fold(Traits::NONE, |all, inner| all.or(reads_of_pred(inner))),
    }
}

fn reads_of_stat(stat: StatKind) -> Traits {
    match stat {
        StatKind::Power => Traits::POWER,
        StatKind::Toughness => Traits::TOUGHNESS,
        StatKind::Colors => Traits::COLORS,
        StatKind::ManaValue | StatKind::Loyalty => Traits::NONE,
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
        | Amount::Devoured
        | Amount::CastWith(_)
        | Amount::ColorsSpent
        | Amount::PermanentsGraved
        | Amount::LifeLostThisWay
        | Amount::Destroyed(_)
        | Amount::DieResult(_)
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
        | Amount::StartingLife(who)
        | Amount::LowestLife(who)
        | Amount::LowestHand(who)
        | Amount::HighestLife(who) => reads_of_ref(who),
        Amount::Sum(parts) => parts
            .iter()
            .fold(Traits::NONE, |all, part| all.or(reads_of_amount(part))),
        Amount::EvenShare(total) => reads_of_amount(total),
        Amount::Times { of, .. } | Amount::Half { of, .. } => reads_of_amount(of),
        Amount::Devotion(_) => Traits::CONTROL,
        Amount::Greatest { of, stat }
        | Amount::Total { of, stat }
        | Amount::Distinct { of, stat } => reads_of_selector(of).or(reads_of_stat(*stat)),
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
        Condition::Not(inner) => reads_of_condition(inner),
        Condition::All(all) | Condition::Any(all) => all
            .iter()
            .fold(Traits::NONE, |sum, inner| sum.or(reads_of_condition(inner))),
        Condition::Compare { left, right, .. } => reads_of_amount(left).or(reads_of_amount(right)),
        Condition::Matches { subject, selector } => {
            reads_of_ref(subject).or(reads_of_selector(selector))
        }
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
        | Condition::Performed => Traits::NONE,
        Condition::Monarch(who)
        | Condition::MonarchAtTurnStart(who)
        | Condition::CitysBlessing(who) => reads_of_ref(who),
        Condition::Level { of, .. } => reads_of_ref(of),
    }
}
