//! A draw inside the steps of a replacement effect passes through the other replacement effects before the steps after it
//! run (CR 614.6, 121.6): "instead draw a card, then reveal it" whose draw another effect replaces ("reveal the top card
//! ... put it into the graveyard") reveals and judges only a card that was drawn. The draw's events go through a pass of
//! their own, the effects already applied to the event left out, and the events that pass makes are settled: the batch's
//! pass does not replace them again.

use std::cell::RefCell;

use mtg_core::ids::PlayerId;

use super::{Origin, Pass, Run, slots_of};
use crate::choose::Choice;
use crate::events::Event;
use crate::resolve::{R114Replaced, ResolveError};
use crate::state::GameState;

/// What a pass over the draws of an inner step shares with the step that runs it.
pub(super) struct Inner<'s, 'a> {
    /// The answers of the batch's journal not taken yet.
    pub(super) answers: &'s RefCell<std::slice::Iter<'a, (PlayerId, Choice)>>,
    /// The effects applied to the event the steps replace, the running one included.
    pub(super) applied: Vec<Origin>,
    /// The events the inner passes made, which the batch's pass leaves as they are.
    pub(super) settled: &'s RefCell<Vec<Event>>,
    /// The players whose draws an inner pass replaced, once for each draw.
    pub(super) replaced: &'s RefCell<Vec<PlayerId>>,
}

impl Pass<'_> {
    /// Passes the events of an inner draw step through the replacement effects not yet applied, in `state` (the state the
    /// steps before it leave), taking the batch's answers in order.
    pub(super) fn r114_inner(
        &self,
        state: &GameState,
        events: Vec<Event>,
        inner: &Inner<'_, '_>,
    ) -> Result<R114Replaced, ResolveError> {
        let answers = inner.answers.borrow().clone();
        let before = answers.len();
        let mut slots = slots_of(events);
        for slot in &mut slots {
            slot.applied.clone_from(&inner.applied);
        }
        let mut pass = Pass {
            state,
            sources: self.sources.clone(),
            shields: self.shields.clone(),
            left: self.left.clone(),
            touched: Vec::new(),
            spent: std::collections::BTreeSet::new(),
            answers,
            next_id: self.next_id,
            slots,
            draws_changed: false,
            pulled: self.pulled.clone(),
            r109_replaced: Vec::new(),
            activated: self.activated.clone(),
            putter: self.putter,
            r109_tokens: self.r109_tokens.clone(),
            r114_settled: Vec::new(),
        };
        let ran = pass
            .run()
            .map_err(|error| ResolveError::Invalid(error.to_string()))?;
        let taken = before - pass.answers.len();
        if taken > 0 {
            inner.answers.borrow_mut().nth(taken - 1);
        }
        match ran {
            Run::Ask(decision) => Ok(R114Replaced::Ask(decision)),
            Run::R109Restart(_) => Err(ResolveError::Invalid(
                "a draw inside a replacement made tokens the batch does not see".into(),
            )),
            Run::Done => {
                inner.replaced.borrow_mut().extend(pass.r109_replaced);
                let made: Vec<Event> = pass
                    .slots
                    .into_iter()
                    .flat_map(|slot| match slot.item {
                        super::Item::One(event) => vec![event],
                        super::Item::Tokens(run) => {
                            run.into_iter().map(Event::TokenCreated).collect()
                        }
                    })
                    .collect();
                inner.settled.borrow_mut().extend(made.iter().cloned());
                Ok(R114Replaced::Events(made))
            }
        }
    }

    /// Marks the slots from `at` on whose events an inner pass settled as frozen, each settled event once.
    pub(super) fn r114_freeze_settled(&mut self, at: usize, count: usize) {
        let mut settled = std::mem::take(&mut self.r114_settled);
        for slot in self.slots.iter_mut().skip(at).take(count) {
            if let super::Item::One(event) = &slot.item
                && let Some(found) = settled.iter().position(|held| held == event)
            {
                settled.swap_remove(found);
                slot.frozen = true;
            }
        }
    }
}
