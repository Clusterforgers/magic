//! One effect that acts between two targets: "Attach target Equipment you control to target creature you control.",
//! "Target creature you control deals damage equal to its power to another target creature."

use super::{Amount, Effect, Ref, TargetSpec, Targetable, spec_text};
use crate::value::StatKind;

/// Whether the specification asks for objects.
fn objects(spec: &TargetSpec) -> bool {
    matches!(spec.what, Targetable::Object(_))
}

/// The sentence for the one effect over the two targets, in the order the card names them.
pub(super) fn text(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [first, second] = targets else {
        return None;
    };
    (objects(first) && objects(second) && first.bounds.is_one() && second.bounds.is_one())
        .then_some(())?;
    match effects {
        [
            Effect::Attach {
                what: Ref::Target(0),
                to: Ref::Target(1),
            },
        ] => Some(format!(
            "attach {} to {}",
            spec_text(first)?,
            spec_text(second)?
        )),
        [
            Effect::DealDamage {
                source: Some(Ref::Target(0)),
                to: Ref::Target(1),
                amount:
                    Amount::Stat {
                        of: Ref::Target(0),
                        stat: StatKind::Power,
                    },
            },
        ] => Some(format!(
            "{} deals damage equal to its power to {}",
            spec_text(first)?,
            spec_text(second)?
        )),
        [
            Effect::BecomeCopy {
                what: Ref::Target(0),
                of: Ref::Target(1),
                except,
                duration,
            },
        ] if except.is_empty() => Some(format!(
            "{} becomes a copy of {}{}",
            spec_text(first)?,
            spec_text(second)?,
            crate::compile::read::copy_spells::c10_xp_copy_duration_text(duration.clone())?
        )),
        _ => None,
    }
}
