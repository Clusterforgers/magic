//! Manifest dread (CR 701.62): the look at the top two cards, the choice of one, the other put into the graveyard and
//! the manifest of the one left on top, written as "manifest dread" or "its controller manifests dread", and the step
//! on "that creature" after it: "manifest dread, then attach ~ to that creature", "manifest dread, then put three +1/+1
//! counters on that creature".

use super::{Effect, Ref, Writer};
use crate::compile::read::counters;
use crate::effect::Destination;
use crate::refs::Bounds;
use crate::value::Amount;

/// The player who manifests dread and the name of the manifested creature, when the effects open with the four steps
/// of manifest dread, all by that one player.
fn dread(effects: &[Effect]) -> Option<(&Ref, Option<&str>)> {
    let [
        Effect::LookAtTop {
            who,
            count: Amount::Fixed(2),
            bind: looked,
        },
        Effect::Choose {
            who: chooser,
            from: Ref::Bound(from),
            count,
            bind: _,
            rest_as: Some(rest),
        },
        Effect::Move {
            what: Ref::Bound(moved),
            to: Destination::Graveyard,
            bind: None,
        },
        Effect::Manifest {
            who: manifests,
            count: manifested,
            from: None,
            bind,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    let same = chooser == who && manifests == who && from == looked && moved == rest;
    (same && Bounds::is_one(count) && *manifested == Amount::one()).then_some(())?;
    Some((who, bind.as_deref()))
}

impl Writer<'_> {
    /// Manifest dread and the step on the creature it made, as one sentence, and the number of effects they cover.
    pub(super) fn r9w_dread(effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let (who, name) = dread(effects)?;
        let head = match who {
            Ref::You => "manifest dread",
            Ref::ControllerOf(of) if **of == Ref::Target(0) => "its controller manifests dread",
            _ => return None,
        };
        let Some(name) = name else {
            return Some((vec![head.to_owned()], 4));
        };
        let it = Ref::Bound(name.to_owned());
        (*who == Ref::You).then_some(())?;
        let then = match effects.get(4)? {
            Effect::Attach {
                what: Ref::This,
                to,
            } if *to == it => "attach ~ to that creature".to_owned(),
            Effect::PutCounters { on, counter, count } if *on == it => {
                format!("put {} on that creature", counters::text(counter, count)?)
            }
            _ => return None,
        };
        Some((vec![format!("{head}, then {then}")], 5))
    }
}
