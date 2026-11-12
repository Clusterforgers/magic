//! `Effect::Shield`: a replacement effect that lasts a while, fixed to what its references name now (CR 615, 611.2c).

use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen, ShieldShape};
use mtg_core::ids::EffectId;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::Event;
use crate::object::Chosen;
use crate::object::GameObject;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rules::{DamageShield, Rule, RuleEffect};

/// A shield of damage prevention begins for the recipients its `to` names now, whatever their number (none named,
/// none protected: nothing begins), and lasts for its duration. It prevents all the damage or, as `PreventUpTo`, that
/// much in all; `PreventCounted` counts that much as it begins. A shield over deaths, or over leaving the battlefield,
/// covers the permanents its selector names now; a shield over the next draw or loss of the game is judged at that event. Any other shield refuses by the name of its effect (`ShieldShape`).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Shield { when, by, duration } = effect else {
        return Err(wrong_effect("Shield"));
    };
    let shape = ShieldShape::of(when, by).ok_or(ResolveError::Unsupported("Effect.Shield"))?;
    let covered = match (shape, when, by) {
        (
            ShieldShape::Named { leaves },
            ReplaceWhen::ZoneChange { what, .. },
            ReplaceBy::MoveTo(to),
        ) => Some((what, *to, None, false, leaves)),
        (ShieldShape::Countered, ReplaceWhen::Countered { what }, ReplaceBy::MoveTo(to)) => {
            Some((what, *to, None, true, false))
        }
        (
            ShieldShape::Countered,
            ReplaceWhen::Countered { what },
            ReplaceBy::IntoLibrary { place, .. },
        ) => Some((what, ZoneKind::Library, Some(*place), true, false)),
        _ => None,
    };
    // A class of events (every creature that dies this turn, every card put into a graveyard) is judged as each happens.
    if matches!(shape, ShieldShape::Class | ShieldShape::Next) {
        let Some(players) = r374_fixed_players(cx, when)? else {
            return Ok(Run::events(Vec::new()));
        };
        let rule = Rule::Replaces {
            when: when.clone(),
            by: by.clone(),
            players,
        };
        return Ok(begin(cx, rule, duration.clone()));
    }
    if let Some((what, to, place, countered, leaves)) = covered {
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
                to,
                place,
                countered,
                cast,
                leaves,
            },
            duration.clone(),
        ));
    }
    let (
        ReplaceWhen::Damage { to, source, combat },
        ReplaceBy::Prevent
        | ReplaceBy::PreventNext
        | ReplaceBy::PreventUpTo(_)
        | ReplaceBy::PreventCounted(_)
        | ReplaceBy::PreventCountedAnd { .. }
        | ReplaceBy::PreventAnd(_)
        | ReplaceBy::Multiply(_)
        | ReplaceBy::Add(_)
        | ReplaceBy::Instead(_)
        | ReplaceBy::InsteadNext(_)
        | ReplaceBy::RedirectUpTo { .. }
        | ReplaceBy::Redirect { .. },
    ) = (when, by)
    else {
        return Err(ResolveError::Unsupported("Effect.Shield"));
    };
    if let Some(run) =
        super::rmt2_shares::rmt2_divided_shields(cx, (to.as_ref(), source, *combat), by, duration)?
    {
        return Ok(run);
    }
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
pub(super) fn damage_shield(
    cx: &Cx<'_>,
    (to, source, combat): (Option<&Ref>, &Selector, Option<bool>),
    by: &ReplaceBy,
) -> Result<Option<DamageShield>, ResolveError> {
    let once = matches!(
        by,
        ReplaceBy::PreventNext | ReplaceBy::InsteadNext(_) | ReplaceBy::Redirect { next: true, .. }
    );
    let instead = match by {
        ReplaceBy::Multiply(_)
        | ReplaceBy::Add(_)
        | ReplaceBy::Instead(_)
        | ReplaceBy::RedirectUpTo { .. }
        | ReplaceBy::Redirect { .. } => Some(by.clone()),
        ReplaceBy::InsteadNext(effects) => Some(ReplaceBy::Instead(effects.clone())),
        _ => None,
    };
    let (left, then) = match by {
        ReplaceBy::PreventUpTo(most) | ReplaceBy::RedirectUpTo { most, .. } => {
            (Some(*most), Vec::new())
        }
        ReplaceBy::PreventCounted(amount) => {
            let most = u32::try_from(cx.scope.amount(amount)?.max(0)).unwrap_or(u32::MAX);
            (Some(most), Vec::new())
        }
        ReplaceBy::PreventCountedAnd { most, effects } => {
            let most = u32::try_from(cx.scope.amount(most)?.max(0)).unwrap_or(u32::MAX);
            (Some(most), effects.clone())
        }
        ReplaceBy::PreventAnd(then) => (None, then.clone()),
        _ => (None, Vec::new()),
    };
    // A kind of object ("creatures you control") is judged as each damage event is replaced (CR 611.2c, 614.6); a
    // player, a target or a bound object is fixed now.
    let (to, each) = match to {
        None => (None, None),
        Some(Ref::Each(kind)) if judged_per_event(kind) => (None, Some(kind.clone())),
        // "you and/or permanents you control": the fixed parts and at most one kind judged per event, either covered.
        Some(Ref::Union(parts)) => {
            let mut fixed = Vec::new();
            let mut each = None;
            for part in parts {
                match part {
                    Ref::Each(kind) if judged_per_event(kind) && each.is_none() => {
                        each = Some(kind.clone());
                    }
                    named => fixed.extend(cx.scope.refs(named)?),
                }
            }
            (Some(fixed), each)
        }
        Some(named) => (Some(cx.scope.refs(named)?), None),
    };
    // The new recipient of redirected damage is fixed as the shield begins (CR 611.2c); naming none, it redirects nothing.
    let redirect = match by {
        ReplaceBy::RedirectUpTo { to: named, .. } | ReplaceBy::Redirect { to: named, .. } => {
            match cx.scope.refs(named)?.first() {
                Some(target) => Some(*target),
                None => return Ok(None),
            }
        }
        _ => None,
    };
    let chosen = Selector(
        source
            .0
            .iter()
            .map(|pred| g11b_chosen_fixed(cx, pred))
            .collect(),
    );
    let (from, source) = c9_fight_fixed_sources(cx, &chosen)?;
    Ok(Some(DamageShield {
        to,
        each,
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

/// Whether a kind of protected object reads the same after the resolution: its words name no target, binding or
/// choice of the resolution, only characteristics, status and players the shield's controller can name later.
fn judged_per_event(kind: &Selector) -> bool {
    fn player(named: &Ref) -> bool {
        matches!(named, Ref::You | Ref::Opponents | Ref::EachPlayer)
    }
    fn plain(pred: &Pred) -> bool {
        match pred {
            Pred::Type(_)
            | Pred::Supertype(_)
            | Pred::Subtype(_)
            | Pred::Color(_)
            | Pred::Colorless
            | Pred::Token
            | Pred::Commander
            | Pred::Other
            | Pred::Tapped
            | Pred::Untapped
            | Pred::Attacking
            | Pred::Blocking
            | Pred::Unblocked
            | Pred::HasKeyword(_)
            | Pred::HasCounter(_)
            | Pred::HasAnyCounter
            | Pred::Named(_) => true,
            Pred::ControlledBy(named) | Pred::OwnedBy(named) => player(named),
            Pred::Not(inner) => plain(inner),
            Pred::AnyOf(parts) => parts.iter().all(plain),
            _ => false,
        }
    }
    !kind.is_empty() && kind.0.iter().all(plain)
}

/// The rule of a shield begins for its duration, unless it names nothing.
/// The players a draw shield names by a target, fixed as it begins (CR 611.2c): empty for a shield whose event names
/// them by class, `None` when the reference names no player now.
fn r374_fixed_players(
    cx: &Cx<'_>,
    when: &ReplaceWhen,
) -> Result<Option<Vec<mtg_core::ids::PlayerId>>, ResolveError> {
    match when {
        ReplaceWhen::Draw { who, .. }
            if !matches!(who, Ref::You | Ref::Opponents | Ref::EachPlayer) =>
        {
            let players = cx.scope.players(who)?;
            Ok((!players.is_empty()).then_some(players))
        }
        _ => Ok(Some(Vec::new())),
    }
}

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

/// A source named by the value the resolution chose ("sources of the color of your choice") as that value: the shield
/// judges its sources after the resolution and its choices are gone. A choice not made, or one that is no
/// characteristic, stays as it is and matches nothing then.
fn g11b_chosen_fixed(cx: &Cx<'_>, pred: &Pred) -> Pred {
    match pred {
        Pred::ChosenAs(name) => match cx.scope.chosen_value(name) {
            Some(Chosen::Color(color)) => Pred::Color(*color),
            Some(Chosen::Colors(colors)) => {
                Pred::AnyOf(colors.iter().map(|color| Pred::Color(*color)).collect())
            }
            Some(Chosen::CreatureType(subtype) | Chosen::LandType(subtype)) => {
                Pred::Subtype(subtype.clone())
            }
            Some(Chosen::CardType(kind)) => Pred::Type(*kind),
            Some(Chosen::Name(name)) => Pred::Named(name.clone()),
            _ => pred.clone(),
        },
        Pred::Not(inner) => Pred::Not(Box::new(g11b_chosen_fixed(cx, inner))),
        Pred::AnyOf(parts) => Pred::AnyOf(
            parts
                .iter()
                .map(|part| g11b_chosen_fixed(cx, part))
                .collect(),
        ),
        _ => pred.clone(),
    }
}
