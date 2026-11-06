//! `Effect::Shield`: a replacement effect that lasts a while, fixed to what its references name now (CR 615, 611.2c).

use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_core::ids::EffectId;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::Event;
use crate::object::GameObject;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rules::{DamageShield, Rule, RuleEffect};

/// A shield of damage prevention begins for the recipients its `to` names now, whatever their number (none named,
/// none protected: nothing begins), and lasts for its duration. It prevents all the damage or, as `PreventUpTo`, that
/// much in all. A shield over deaths covers the permanents its selector names now. Any other shield refuses by the name
/// of its effect.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Shield { when, by, duration } = effect else {
        return Err(wrong_effect("Shield"));
    };
    let covered = match (when, by) {
        (
            ReplaceWhen::ZoneChange {
                what,
                from: Some(ZoneKind::Battlefield | ZoneKind::Stack),
                to: Some(ZoneKind::Graveyard),
            },
            ReplaceBy::MoveTo(to),
        ) if names_objects(what) => Some((what, to, false)),
        (ReplaceWhen::Countered { what }, ReplaceBy::MoveTo(to)) => Some((what, to, true)),
        _ => None,
    };
    // A class of events (every creature that dies this turn, every card put into a graveyard) is judged as each happens.
    if covered.is_none() && is_class_shield(when, by) {
        let rule = Rule::Replaces {
            when: when.clone(),
            by: by.clone(),
        };
        return Ok(begin(cx, rule, duration.clone()));
    }
    if let Some((what, to, countered)) = covered {
        let cast = matches!(
            when,
            ReplaceWhen::ZoneChange {
                from: Some(ZoneKind::Stack),
                ..
            }
        );
        // A selector looks on the battlefield or the stack, so a card named from its graveyard or exile is read by its name.
        let named = match what.0.as_slice() {
            [Pred::Is(named)] => named.clone(),
            _ => Ref::Each(what.clone()),
        };
        let objects = cx
            .scope
            .live_objects(&named)?
            .into_iter()
            .map(GameObject::reference)
            .collect();
        return Ok(begin(
            cx,
            Rule::DiesInstead {
                objects,
                to: *to,
                countered,
                cast,
            },
            duration.clone(),
        ));
    }
    let (
        ReplaceWhen::Damage { to, source, combat },
        ReplaceBy::Prevent
        | ReplaceBy::PreventNext
        | ReplaceBy::PreventUpTo(_)
        | ReplaceBy::PreventAnd(_)
        | ReplaceBy::Multiply(_)
        | ReplaceBy::Add(_)
        | ReplaceBy::Instead(_)
        | ReplaceBy::InsteadNext(_)
        | ReplaceBy::RedirectUpTo { .. },
    ) = (when, by)
    else {
        return Err(ResolveError::Unsupported("Effect.Shield"));
    };
    match damage_shield(cx, (to.as_ref(), source, *combat), by)? {
        Some(shield) if shield.left != Some(0) => Ok(begin(
            cx,
            Rule::PreventDamage(Box::new(shield)),
            duration.clone(),
        )),
        _ => Ok(Run::events(Vec::new())),
    }
}

/// The shield over damage to the recipients `to` names now from the sources of the selector; none when it would
/// redirect to nothing.
fn damage_shield(
    cx: &Cx<'_>,
    (to, source, combat): (Option<&Ref>, &Selector, Option<bool>),
    by: &ReplaceBy,
) -> Result<Option<DamageShield>, ResolveError> {
    let once = matches!(by, ReplaceBy::PreventNext | ReplaceBy::InsteadNext(_));
    let instead = match by {
        ReplaceBy::Multiply(_)
        | ReplaceBy::Add(_)
        | ReplaceBy::Instead(_)
        | ReplaceBy::RedirectUpTo { .. } => Some(by.clone()),
        ReplaceBy::InsteadNext(effects) => Some(ReplaceBy::Instead(effects.clone())),
        _ => None,
    };
    let (left, then) = match by {
        ReplaceBy::PreventUpTo(most) | ReplaceBy::RedirectUpTo { most, .. } => {
            (Some(*most), Vec::new())
        }
        ReplaceBy::PreventAnd(then) => (None, then.clone()),
        _ => (None, Vec::new()),
    };
    let to = match to {
        None => None,
        Some(named) => Some(cx.scope.refs(named)?),
    };
    // The new recipient of redirected damage is fixed as the shield begins (CR 611.2c); naming none, it redirects nothing.
    let redirect = match by {
        ReplaceBy::RedirectUpTo { to: named, .. } => match cx.scope.refs(named)?.first() {
            Some(target) => Some(*target),
            None => return Ok(None),
        },
        _ => None,
    };
    let (from, source) = c9_fight_fixed_sources(cx, source)?;
    Ok(Some(DamageShield {
        to,
        source,
        from,
        combat,
        left,
        then,
        once,
        instead,
        redirect,
    }))
}

/// Whether the selector names particular objects now (a target, what a spell damaged) rather than a kind of object.
fn names_objects(what: &Selector) -> bool {
    what.0
        .iter()
        .any(|pred| matches!(pred, Pred::Is(_) | Pred::SameCard(_) | Pred::DamagedBy(_)))
}

/// Whether the shield replaces a class of zone changes or entries, judged when each happens.
fn is_class_shield(when: &ReplaceWhen, by: &ReplaceBy) -> bool {
    match (when, by) {
        (ReplaceWhen::ZoneChange { what, .. }, ReplaceBy::MoveTo(_))
        | (ReplaceWhen::Enters { what }, ReplaceBy::Tapped) => !names_objects(what),
        _ => false,
    }
}

/// The rule of a shield begins for its duration, unless it names nothing.
fn begin(cx: &Cx<'_>, rule: Rule, duration: Duration) -> Run {
    if crate::rules::names_nothing(&rule) {
        return Run::events(Vec::new());
    }
    let state = cx.state();
    Run::events(vec![Event::RuleCreated {
        effect: Box::new(RuleEffect {
            id: EffectId(state.next_effect),
            rule,
            duration,
            source: Some(cx.scope.source),
            controller: cx.controller(),
            turn: state.turn().number(),
        }),
    }])
}

/// The sources a shield names as particular objects ("damage target instant or sorcery spell would deal"), fixed now
/// as the objects they are (CR 611.2c), and the rest of its selector, judged when the damage would be dealt.
fn c9_fight_fixed_sources(
    cx: &Cx<'_>,
    source: &Selector,
) -> Result<(Option<Vec<Target>>, Selector), ResolveError> {
    let mut fixed: Option<Vec<Target>> = None;
    let mut rest = Vec::new();
    for pred in &source.0 {
        let Pred::Is(named) = pred else {
            rest.push(pred.clone());
            continue;
        };
        let now = cx.scope.refs(named)?;
        fixed = Some(match fixed {
            None => now,
            Some(before) => before.into_iter().filter(|t| now.contains(t)).collect(),
        });
    }
    Ok((fixed, Selector(rest)))
}
