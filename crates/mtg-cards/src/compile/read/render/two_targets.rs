//! One effect that acts between two targets: "Attach target Equipment you control to target creature you control.",
//! "Target creature you control deals damage equal to its power to another target creature."

use super::{Amount, Effect, Ref, TargetSpec, Targetable, spec_text};
use crate::value::StatKind;

/// Whether the specification asks for objects.
fn objects(spec: &TargetSpec) -> bool {
    matches!(spec.what, Targetable::Object(_))
}

/// The target, with "other than ~" after its words when it may not be the source ("target creature you control with a
/// power and toughness sticker on it other than ~").
fn other_than_this(spec: &TargetSpec) -> Option<String> {
    let Targetable::Object(selector) = &spec.what else {
        return None;
    };
    if !selector.0.contains(&crate::refs::Pred::Other) {
        return spec_text(spec);
    }
    let mut rest = spec.clone();
    rest.what = Targetable::Object(crate::refs::Selector(
        selector
            .0
            .iter()
            .filter(|pred| **pred != crate::refs::Pred::Other)
            .cloned()
            .collect(),
    ));
    Some(format!("{} other than ~", spec_text(&rest)?))
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
                bind: None,
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
                unpreventable: false,
                ..
            },
        ] => Some(format!(
            "{} deals damage equal to its power to {}",
            spec_text(first)?,
            spec_text(second)?
        )),
        // "Target creature you don't control blocks target creature you control ... other than ~ this turn if able."
        [
            Effect::Rule {
                rule:
                    crate::modify::GameRule::MustBlockAttacker {
                        who: Ref::Target(0),
                        attacker: Ref::Target(1),
                    },
                duration: crate::modify::Duration::UntilEndOfTurn,
            },
        ] => Some(format!(
            "{} blocks {} this turn if able",
            spec_text(first)?,
            other_than_this(second)?
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
            crate::compile::read::copy_spells::c10_xp_copy_duration_text(duration)?
        )),
        [
            Effect::MoveCounters {
                from: Ref::Target(0),
                to: Ref::Target(1),
                counter,
                count,
                up_to,
            },
        ] => {
            let onto = spec_text(second)?;
            // A second target other than the first is "onto a second target permanent", which the reader reads
            // without the filter (a second target is another object, CR 115.3).
            let onto = match onto.strip_prefix("another ") {
                Some(rest) => format!("a second {rest}"),
                None => format!("a second {onto}"),
            };
            // "Move any number of counters": the controller may stop.
            let counters = if *up_to {
                crate::compile::read::counters::ep30_any_number_text(
                    counter.as_ref(),
                    count,
                    &Ref::Target(0),
                )?
            } else {
                crate::compile::read::counters::ep21_move_text(
                    counter.as_ref(),
                    count,
                    &Ref::Target(0),
                )?
            };
            Some(format!(
                "move {counters} from {} onto {onto}",
                spec_text(first)?,
            ))
        }
        _ => None,
    }
}
