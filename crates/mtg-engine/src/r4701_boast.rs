//! Boast twice (CR 702.142c): "Creatures you control can boast twice during each of your turns rather than once." A boast
//! ability is an activated ability limited to once each turn that works only if its source attacked this turn; under the
//! rule, during its controller's turn, a creature of theirs may use each such ability twice.

use mtg_cards::ability::{Ability, Limit};
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::Condition;
use mtg_core::ids::ObjectId;

use crate::state::GameState;

/// Whether the ability is a boast ability: once each turn, only if its source attacked this turn.
fn r4701_is_boast(rules: &Ability) -> bool {
    let Ability::Activated {
        condition: Some(Condition::Matches { subject, selector }),
        limit: Some(Limit::OncePerTurn),
        ..
    } = rules
    else {
        return false;
    };
    *subject == Ref::This && *selector == Selector(vec![Pred::AttackedThisTurn])
}

/// How many times this turn the boast ability `rules` of `source` may be activated when a rule raises it above once: two
/// for a creature whose controller is the active player and has the rule; `None` otherwise.
pub(crate) fn r4701_boast_most(
    state: &GameState,
    source: ObjectId,
    rules: &Ability,
) -> Option<u32> {
    if !r4701_is_boast(rules) {
        return None;
    }
    let object = state.object(source)?;
    let controller = crate::characteristics::controller_of_object(state, object);
    let creature = crate::characteristics::characteristics(state, source)
        .is_some_and(|held| held.is_creature());
    (creature
        && state.active_player() == Some(controller)
        && state.rules().boast_twice.contains(&controller))
    .then_some(2)
}

impl GameState {
    /// How many times the ability `key` names was activated this turn as a boast that a rule allows twice, the use of its
    /// plain once-each-turn limit included.
    fn r4701_boasts(
        &self,
        key: (
            crate::object::ObjectRef,
            crate::abilities::AbilitySource,
            usize,
        ),
    ) -> u32 {
        u32::from(self.limits_spent.contains(&key))
            + self.tallies.limit_uses.get(&key).copied().unwrap_or(0)
    }

    /// Whether the boast ability has been activated `most` times this turn.
    pub(crate) fn r4701_boast_spent(
        &self,
        most: u32,
        source: crate::object::ObjectRef,
        origin: crate::abilities::AbilitySource,
        ability: usize,
    ) -> bool {
        self.r4701_boasts((source, origin, ability)) >= most
    }

    /// Records a boast of an ability a rule allows `most` times this turn; `false` when they were all used. The first use
    /// spends the plain limit, so the ability stays spent once the rule ends.
    pub(crate) fn r4701_spend_boast(
        &mut self,
        most: u32,
        source: crate::object::ObjectRef,
        origin: crate::abilities::AbilitySource,
        ability: usize,
    ) -> bool {
        let key = (source, origin, ability);
        if self.r4701_boasts(key) >= most {
            return false;
        }
        if !self.limits_spent.insert(key) {
            *self.tallies.limit_uses.entry(key).or_insert(0) += 1;
        }
        true
    }
}
