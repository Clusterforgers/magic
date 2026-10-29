//! One batch, one trigger: the events of a batch are tried as two groups, those that look back and the rest, so that
//! each is judged against the state the rules say. A batch ability that both groups fit ("one or more other cards are
//! put into exile from anywhere": a permanent and a graveyard card exiled together) triggers once for the whole batch.

use super::Triggered;
use crate::choose::Target;

/// Whether a cause is about no player, as the causes of a batch of moves are.
fn w15b_no_players(trigger: &Triggered) -> bool {
    !trigger
        .cause
        .subjects
        .iter()
        .any(|subject| matches!(subject, Target::Player(_)))
}

/// Folds the triggers that the second group found (`found[forward..]`) into the triggers of the same ability that the
/// first group found (`found[back..forward]`): the subjects and snapshots of both, and the counts added up.
pub(super) fn w15b_join_passes(found: &mut Vec<Triggered>, back: usize, forward: usize) {
    if forward > found.len() || back > forward {
        return;
    }
    let later = found.split_off(forward);
    for trigger in later {
        let same = found[back..].iter_mut().find(|earlier| {
            earlier.source == trigger.source
                && earlier.ability == trigger.ability
                && earlier.origin == trigger.origin
                && earlier.controller == trigger.controller
                && earlier.delayed.is_none()
                && trigger.delayed.is_none()
                && w15b_no_players(earlier)
                && w15b_no_players(&trigger)
        });
        let Some(earlier) = same else {
            found.push(trigger);
            continue;
        };
        for subject in trigger.cause.subjects {
            if !earlier.cause.subjects.contains(&subject) {
                earlier.cause.subjects.push(subject);
            }
        }
        for shot in trigger.cause.snapshots {
            if !earlier.cause.snapshots.contains(&shot) {
                earlier.cause.snapshots.push(shot);
            }
        }
        earlier.cause.amount = earlier.cause.amount.saturating_add(trigger.cause.amount);
    }
}
