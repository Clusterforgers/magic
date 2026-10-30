//! The objects an announcement is about, as they were when it began: a manual action that takes one of them away
//! (a card moved out of the hand it is cast from, a permanent moved off the battlefield, a token gone) ends the
//! announcement without a trace, since what it would put on the stack is no longer there to put.

use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;

use super::What;

/// The objects an announcement is about, each as the object it was when the announcement began.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Anchor(Vec<Option<ObjectRef>>);

impl Anchor {
    /// The objects `what` is about now: the card of a spell, the source of an activation, the attackers of a declaration.
    /// A triggered ability is about none: it goes on the stack whatever became of its source (CR 113.7a).
    pub(super) fn of(state: &GameState, what: &What) -> Anchor {
        let objects = match what {
            What::Spell(card) => vec![*card],
            What::Ability(ability) => vec![ability.object],
            What::Attack(attackers, _) => attackers.iter().map(|(object, _)| *object).collect(),
            What::Trigger(_) => Vec::new(),
        };
        Anchor(
            objects
                .into_iter()
                .map(|id| state.object(id).map(GameObject::reference))
                .collect(),
        )
    }
}
