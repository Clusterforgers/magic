//! A prohibition on activating the abilities of a target, a bound object or the trigger's object (CR 611.2c).
//!
//! The prohibition judges its selectors whenever an ability is activated, with no choices in scope, so `Is {Target 0}`
//! would name nothing there. As the rule begins, the objects such a reference names become its subjects instead:
//! "its activated abilities can't be activated" covers the abilities of exactly that object, and of no new object it
//! becomes (CR 400.7).

use mtg_cards::modify::Prohibited;
use mtg_cards::refs::{Pred, Ref, Selector};

use crate::choose::Target;
use crate::resolve::{ResolveError, Scope};

/// The subjects and action of a `Cant` rule whose players are every player and whose sources name objects of this
/// resolution: the named objects become the subjects and leave the selector. Any other rule is kept as it is.
pub(crate) fn lower(
    scope: &Scope<'_>,
    who: Vec<Target>,
    action: &Prohibited,
) -> Result<(Vec<Target>, Prohibited), ResolveError> {
    let (Prohibited::Activate(sources) | Prohibited::ActivateKind { sources, .. }) = action else {
        return Ok((who, action.clone()));
    };
    let Some((at, named)) = sources
        .0
        .iter()
        .enumerate()
        .find_map(|(at, pred)| match pred {
            Pred::Is(named) if resolved(named) => Some((at, named)),
            _ => None,
        })
    else {
        return Ok((who, action.clone()));
    };
    if !every_player(scope, &who)? {
        return Ok((who, action.clone()));
    }
    let objects = scope
        .objects(named)?
        .into_iter()
        .map(Target::Object)
        .collect();
    let mut rest = sources.0.clone();
    rest.remove(at);
    let rest = Selector(rest);
    let action = match action {
        Prohibited::ActivateKind { kind, .. } => Prohibited::ActivateKind {
            kind: *kind,
            sources: rest,
        },
        _ => Prohibited::Activate(rest),
    };
    Ok((objects, action))
}

/// Whether the reference names what this resolution chose or met, which a later scope can no longer read.
fn resolved(named: &Ref) -> bool {
    matches!(
        named,
        Ref::Target(_)
            | Ref::Bound(_)
            | Ref::Triggering
            | Ref::TriggeringFirst
            | Ref::Dealer
            | Ref::Paid
    )
}

/// Whether the subjects are exactly the players of the game, so that the sources alone decide what is forbidden.
fn every_player(scope: &Scope<'_>, who: &[Target]) -> Result<bool, ResolveError> {
    let players = scope.players(&Ref::EachPlayer)?;
    Ok(who.iter().all(|t| matches!(t, Target::Player(_)))
        && players.iter().all(|p| who.contains(&Target::Player(*p))))
}
