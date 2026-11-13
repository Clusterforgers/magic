//! `ReplaceBy::PreventCountedAnd` (CR 615): a shield prevents up to what it has left of its counted total and runs its
//! effects with the damage it prevented from the event as the event's amount ("prevent the next 3 damage ... You gain
//! life equal to the damage prevented this way", "for each 1 damage prevented this way, put a +1/+1 counter on that
//! creature", "if damage is prevented this way, ~ deals that much damage to any target").

use mtg_cards::effect::Effect;
use mtg_cards::trigger::ReplaceBy;

use super::{Candidate, Item, Origin, Pass, Slot, Unit, slots_of};
use crate::decision::Decision;
use crate::events::Event;
use crate::resolve::{Inline, r370_run_instead_bound};
use crate::rules::Rule;

impl Pass<'_> {
    /// The targets the shield behind the candidate fixed as it began; none for any other replacement.
    fn r370_shield_bound(
        &self,
        candidate: &Candidate,
    ) -> Vec<(String, Vec<crate::choose::Target>)> {
        let Origin::Shield(id) = candidate.origin else {
            return Vec::new();
        };
        self.shields
            .iter()
            .find(|effect| effect.id == id)
            .and_then(|effect| match &effect.rule {
                Rule::PreventDamage(shield) => Some(shield.bound.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Prevents up to `most` of the damage at `at` and adds the events of `effects`, read with the damage prevented.
    pub(super) fn r283_prevent_counted_and(
        &mut self,
        at: usize,
        candidate: &Candidate,
        most: u32,
        effects: &[Effect],
        unit: &Unit,
        applied: &[Origin],
    ) -> Option<Decision> {
        let amount = match &self.slots[at].item {
            Item::One(Event::DamageDealt { amount, .. }) => *amount,
            _ => 0,
        };
        let prevented = most.min(amount);
        let mut cause = self.w5d_cause(at, unit);
        cause.amount = prevented;
        let events = if prevented == 0 {
            Vec::new()
        } else {
            let state = self.state;
            let bound = self.r370_shield_bound(candidate);
            let answers = &mut self.answers;
            match r370_run_instead_bound(
                state,
                (candidate.controller, candidate.object),
                &cause,
                effects,
                &bound,
                &mut || answers.next().cloned(),
            ) {
                Ok(Inline::Events(events)) => events,
                Ok(Inline::Ask(decision)) => return Some(decision),
                Ok(Inline::Declined) | Err(_) => Vec::new(),
            }
        };
        let events = self.w9c_fresh_tokens(events);
        self.prevent(
            at,
            candidate,
            &ReplaceBy::PreventUpTo(most),
            applied.to_vec(),
        );
        let slots: Vec<Slot> = slots_of(events)
            .into_iter()
            .map(|mut slot| {
                slot.applied = applied.to_vec();
                slot
            })
            .collect();
        let at = at.min(self.slots.len());
        self.slots.splice(at..at, slots);
        None
    }
}
