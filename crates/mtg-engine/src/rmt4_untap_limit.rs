//! Untap limits (CR 502.3): "Players can't untap more than one land during their untap steps." While a limit would
//! hold back a permanent, the untap step asks the active player which permanents untap (with those that may stay
//! tapped), and the limits keep the first ones chosen that they allow; the rest stay tapped.

use mtg_cards::refs::Selector;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::object::ObjectRef;
use crate::resolve::Scope;
use crate::rules::untap_limits;
use crate::state::GameState;

/// An untap limit in force: whose rule it is, the players it limits, which permanents and how many.
#[derive(Debug, Clone)]
pub(crate) struct UntapLimit {
    /// The player whose rule it is.
    pub(crate) controller: PlayerId,
    /// The object that has the rule.
    pub(crate) source: ObjectRef,
    /// The players whose untap steps it limits.
    pub(crate) who: Vec<PlayerId>,
    /// The permanents counted, among those the untapping player controls.
    pub(crate) what: Selector,
    /// The most of them that untap.
    pub(crate) count: u32,
}

impl UntapLimit {
    /// Whether the limit counts the permanent `id`.
    fn picks(&self, state: &GameState, id: ObjectId) -> bool {
        state.object(id).is_some_and(|object| {
            Scope::announcing(state, self.controller, self.source)
                .selects(&self.what, object.reference())
                .unwrap_or(false)
        })
    }

    fn cap(&self) -> usize {
        usize::try_from(self.count).unwrap_or(usize::MAX)
    }
}

/// The limits on the untap step of `active`.
fn limits(state: &GameState, active: PlayerId) -> impl Iterator<Item = &UntapLimit> {
    untap_limits(state)
        .iter()
        .filter(move |limit| limit.who.contains(&active))
}

/// The permanents of `untapping` that a limit on `active`'s untap step would hold back some of: every permanent a limit
/// counts, for each limit that counts more of them than it allows. Empty when the limits let everything untap.
pub(crate) fn held_back(
    state: &GameState,
    active: PlayerId,
    untapping: &[ObjectId],
) -> Vec<ObjectId> {
    let mut found = Vec::new();
    for limit in limits(state, active) {
        let counted: Vec<ObjectId> = untapping
            .iter()
            .copied()
            .filter(|id| limit.picks(state, *id))
            .collect();
        if counted.len() > limit.cap() {
            for id in counted {
                if !found.contains(&id) {
                    found.push(id);
                }
            }
        }
    }
    found
}

/// The permanents of `chosen` that untap, in the order chosen: each one untaps while every limit that counts it has
/// room left.
pub(crate) fn allowed(state: &GameState, active: PlayerId, chosen: &[ObjectId]) -> Vec<ObjectId> {
    let in_force: Vec<&UntapLimit> = limits(state, active).collect();
    let mut used = vec![0_usize; in_force.len()];
    let mut out = Vec::new();
    for &id in chosen {
        if out.contains(&id) {
            continue;
        }
        let counting: Vec<usize> = in_force
            .iter()
            .enumerate()
            .filter(|(_, limit)| limit.picks(state, id))
            .map(|(at, _)| at)
            .collect();
        if counting.iter().all(|at| used[*at] < in_force[*at].cap()) {
            for at in counting {
                used[at] += 1;
            }
            out.push(id);
        }
    }
    out
}
