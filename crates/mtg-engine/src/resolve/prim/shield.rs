//! `Effect::Shield`: a replacement effect that lasts a while, fixed to what its references name now (CR 615, 611.2c).

use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_core::ids::EffectId;

use super::wrong_effect;
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
    if let (
        ReplaceWhen::ZoneChange {
            what,
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        },
        ReplaceBy::MoveTo(to),
    ) = (when, by)
    {
        let objects = cx
            .scope
            .live_objects(&Ref::Each(what.clone()))?
            .into_iter()
            .map(GameObject::reference)
            .collect();
        return Ok(begin(cx, Rule::DiesInstead { objects, to: *to }, *duration));
    }
    let (
        ReplaceWhen::Damage { to, source, combat },
        ReplaceBy::Prevent | ReplaceBy::PreventUpTo(_),
    ) = (when, by)
    else {
        return Err(ResolveError::Unsupported("Effect.Shield"));
    };
    let left = match by {
        ReplaceBy::PreventUpTo(most) => Some(*most),
        _ => None,
    };
    let to = match to {
        None => None,
        Some(named) => Some(cx.scope.refs(named)?),
    };
    let rule = Rule::PreventDamage(DamageShield {
        to,
        source: source.clone(),
        combat: *combat,
        left,
    });
    if left == Some(0) {
        return Ok(Run::events(Vec::new()));
    }
    Ok(begin(cx, rule, *duration))
}

/// The rule of a shield begins for its duration, unless it names nothing.
fn begin(cx: &Cx<'_>, rule: Rule, duration: Duration) -> Run {
    if crate::rules::names_nothing(&rule) {
        return Run::events(Vec::new());
    }
    let state = cx.state();
    Run::events(vec![Event::RuleCreated {
        effect: RuleEffect {
            id: EffectId(state.next_effect),
            rule,
            duration,
            source: Some(cx.scope.source),
            controller: cx.controller(),
            turn: state.turn().number(),
        },
    }])
}
