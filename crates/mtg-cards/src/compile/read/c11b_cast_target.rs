//! A cast of a target card during a resolution: "You may cast target instant card from your graveyard without paying its
//! mana cost.", and the exile of that spell: "If that spell would be put into your graveyard, exile it instead."

use super::selectors::target;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// "You may cast target instant card from your graveyard[ without paying its mana cost]": the target card is cast during
/// the resolution, for nothing or for its mana cost.
pub fn c11b_cast_target(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("you may cast")?;
    let what = target(r)?;
    let free = r.words("without paying its mana cost");
    Some(vec![Effect::CastFromZone {
        who: Ref::You,
        what,
        free,
        play: false,
        way: None,
        otherwise: Vec::new(),
    }])
}

/// "If that spell would be put into your graveyard, exile it instead.", after the cast of the target: a shield that sends
/// the spell to exile when it would go from the stack to a graveyard, for as long as it is that spell.
pub fn c11b_exile_instead(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> Option<()> {
    let Some(Effect::CastFromZone {
        what: what @ Ref::Target(0),
        play: false,
        ..
    }) = effects.last()
    else {
        return None;
    };
    let shield = c11b_shield(what.clone());
    r.need("if that spell would be put into")?;
    (r.word("your") || r.word("a") || r.word("their")).then_some(())?;
    r.need("graveyard")?;
    r.punct(',').then_some(())?;
    r.need("exile it instead")?;
    r.sentence_end().then(|| effects.push(shield))
}

/// The shield over the spell cast from the card `cast` names: it is exiled instead of going to a graveyard.
pub fn c11b_shield(cast: Ref) -> Effect {
    Effect::Shield {
        when: ReplaceWhen::ZoneChange {
            what: Selector(vec![Pred::In(ZoneKind::Stack), Pred::SameCard(cast)]),
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
        who: Ref::You,
        what: Ref::Target(0),
        free,
        play: false,
        way: None,
        otherwise,
    } = effect
    else {
        return None;
    };
    otherwise.is_empty().then_some(())?;
    let free = if *free {
        " without paying its mana cost"
    } else {
        ""
    };
    Some(format!("you may cast {}{free}", named()?))
}

/// The wording of the shield over the spell cast from the target, whose graveyard the target phrase names: "if that
/// spell would be put into your graveyard, exile it instead".
pub fn c11b_exile_text(effect: &Effect, target: Option<&Targetable>) -> Option<String> {
    (*effect == c11b_shield(Ref::Target(0))).then_some(())?;
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
