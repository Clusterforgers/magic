//! A cast of a target card during a resolution: "You may cast target instant card from your graveyard without paying its
//! mana cost.", and the exile of that spell: "If that spell would be put into your graveyard, exile it instead."; a cast
//! that any type of mana pays for ends ", and mana of any type can be spent to cast that spell".

use super::selectors::target;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::{Duration, SpendAs};
use crate::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// The name the spell cast from the target is bound to: "that spell".
pub const C11B_CAST: &str = "cast";

/// "You may cast target instant card from your graveyard[ without paying its mana cost]": the target card is cast during
/// the resolution, for nothing or for its mana cost.
pub fn c11b_cast_target(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("you may cast")?;
    let what = target(r)?;
    let free = r.words("without paying its mana cost");
    let mut cast = c11b_cast(what, free, None);
    // "..., and mana of any type can be spent to cast that spell" (Tinybones).
    if !free
        && r.attempt(|r| {
            r.punct(',').then_some(())?;
            r.need(R4701_ANY_TYPE)
        })
        .is_some()
        && let Effect::CastFromZone { spend_as, .. } = &mut cast
    {
        *spend_as = SpendAs::AnyType;
    }
    Some(vec![cast])
}

/// The words after the cast of a target whose spell any type of mana can pay for.
const R4701_ANY_TYPE: &str = "and mana of any type can be spent to cast that spell";

/// The cast of the card `what` names, bound as `bind` when it is cast.
fn c11b_cast(what: Ref, free: bool, bind: Option<String>) -> Effect {
    Effect::CastFromZone {
        rather: None,
        spend_as: crate::modify::SpendAs::default(),
        who: Ref::You,
        what,
        free,
        play: false,
        way: None,
        transformed: false,
        bind,
        otherwise: Vec::new(),
    }
}

/// "If that spell would be put into your graveyard, exile it instead.", after the cast of the target: the cast binds the
/// spell, and a shield sends that spell to exile when it would go from the stack to a graveyard.
pub fn c11b_exile_instead(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let Some(&Effect::CastFromZone {
        what: Ref::Target(0),
        free,
        ..
    }) = effects.last()
    else {
        return None;
    };
    let cast = c11b_cast(Ref::Target(0), free, None);
    (effects.last() == Some(&cast)).then_some(())?;
    r.need("if that spell would be put into")?;
    (r.word("your") || r.word("a") || r.word("their")).then_some(())?;
    r.need("graveyard")?;
    r.punct(',').then_some(())?;
    r.need("exile it instead")?;
    r.sentence_end().then_some(())?;
    effects.pop();
    effects.push(c11b_cast(Ref::Target(0), free, Some(C11B_CAST.to_owned())));
    effects.push(c11b_shield());
    Some(())
}

/// The shield over the spell bound as [`C11B_CAST`]: it is exiled instead of going from the stack to a graveyard.
pub fn c11b_shield() -> Effect {
    Effect::Shield {
        when: ReplaceWhen::ZoneChange {
            what: Selector(vec![
                Pred::In(ZoneKind::Stack),
                Pred::Is(Ref::Bound(C11B_CAST.to_owned())),
            ]),
            from: Some(ZoneKind::Stack),
            to: Some(ZoneKind::Graveyard),
        },
        by: ReplaceBy::MoveTo(ZoneKind::Exile),
        duration: Duration::Forever,
    }
}

/// The wording of the cast of the target, given the target's name: "you may cast target instant card from your
/// graveyard without paying its mana cost".
pub fn c11b_cast_text(effect: &Effect, named: impl FnOnce() -> Option<String>) -> Option<String> {
    let Effect::CastFromZone {
        what: Ref::Target(0),
        free,
        bind,
        ..
    } = effect
    else {
        return None;
    };
    let bound = bind.as_ref().filter(|name| *name == C11B_CAST).cloned();
    let mut plain = c11b_cast(Ref::Target(0), *free, bound.clone());
    let any_type = !*free
        && bound.is_none()
        && matches!(
            effect,
            Effect::CastFromZone {
                spend_as: SpendAs::AnyType,
                ..
            }
        );
    if any_type && let Effect::CastFromZone { spend_as, .. } = &mut plain {
        *spend_as = SpendAs::AnyType;
    }
    (*effect == plain).then_some(())?;
    let free = if *free {
        " without paying its mana cost"
    } else {
        ""
    };
    let spend = if any_type {
        format!(", {R4701_ANY_TYPE}")
    } else {
        String::new()
    };
    Some(format!("you may cast {}{free}{spend}", named()?))
}

/// The wording of the shield over the spell cast from the target, whose graveyard the target phrase names: "if that
/// spell would be put into your graveyard, exile it instead".
pub fn c11b_exile_text(effect: &Effect, target: Option<&Targetable>) -> Option<String> {
    (*effect == c11b_shield()).then_some(())?;
    let Some(Targetable::Object(selector)) = target else {
        return None;
    };
    let owner = selector.0.iter().find_map(|pred| match pred {
        Pred::OwnedBy(owner) => Some(owner),
        _ => None,
    });
    let whose = match owner {
        Some(Ref::You) => "your",
        Some(Ref::Opponents) => "their",
        _ => "a",
    };
    Some(format!(
        "if that spell would be put into {whose} graveyard, exile it instead"
    ))
}
