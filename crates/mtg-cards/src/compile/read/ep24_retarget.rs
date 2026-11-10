//! New targets for a spell or ability on the stack (`Effect::ChangeTargets`), written as the card words them: "change
//! the target of target spell or ability with a single target", "you may choose new targets for target spell", "change a
//! target of target instant or sorcery spell with a single target to ~".

use mtg_core::card::CardType;

use crate::effect::{Effect, NewTargets};
use crate::refs::{AbilityKind, Pred, Ref, TargetSpec, Targetable};

/// The words of a target spell with one target.
const SINGLE: &str = " with a single target";

/// The sentence of a change of targets of the one target `spec`, without its first capital and period; `None` for any
/// other shape.
pub(super) fn ep24_retarget_text(effect: &Effect, spec: &TargetSpec) -> Option<String> {
    let Effect::ChangeTargets {
        what: Ref::Target(0),
        to,
        onto,
    } = effect
    else {
        return None;
    };
    let target = target_text(spec)?;
    Some(match (to, onto) {
        (NewTargets::Another, None) => format!("change the target of {target}"),
        (NewTargets::Choose, None) => format!("you may choose new targets for {target}"),
        (NewTargets::One, Some(Ref::This)) if target.ends_with(SINGLE) => {
            format!("change the target of {target} to ~")
        }
        (NewTargets::One, Some(Ref::This)) => format!("change a target of {target} to ~"),
        _ => return None,
    })
}

/// "target spell or ability", "target instant or sorcery spell with a single target": a spell or ability on the stack,
/// one of the kinds this family names, and "with a single target" last.
fn target_text(spec: &TargetSpec) -> Option<String> {
    if !spec.bounds.is_one() || spec.divide.is_some() {
        return None;
    }
    let Targetable::Object(selector) = &spec.what else {
        return None;
    };
    let (preds, single) = match selector.0.as_slice() {
        [rest @ .., Pred::SingleTarget] => (rest, SINGLE),
        rest => (rest, ""),
    };
    let either = |a: Pred, b: Pred| Pred::AnyOf(vec![a, b]);
    let kind = match preds {
        [Pred::Spell] => "spell",
        [Pred::AnyOf(kinds)]
            if kinds.as_slice()
                == [
                    Pred::Spell,
                    Pred::Ability(AbilityKind::Activated),
                    Pred::Ability(AbilityKind::Triggered),
                ] =>
        {
            "spell or ability"
        }
        [Pred::Spell, kinds]
            if *kinds == either(Pred::Type(CardType::Instant), Pred::Type(CardType::Sorcery)) =>
        {
            "instant or sorcery spell"
        }
        [Pred::Spell, Pred::Type(CardType::Instant)] => "instant spell",
        _ => return None,
    };
    Some(format!("target {kind}{single}"))
}
