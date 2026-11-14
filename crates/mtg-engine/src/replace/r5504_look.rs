//! `ReplaceBy::ChooseAfter` (CR 614.1c, 614.12): as a permanent enters, effects happen first and then its controller
//! makes the choice it keeps ("As this artifact enters, look at an opponent's hand, then choose any card name."). The
//! effects' events come first in the batch, the choice is remembered with the permanent's entering.

use mtg_cards::effect::Effect;
use mtg_cards::trigger::EntersChoice;

use super::w5d_instead::Ran;
use super::{Applied, Asked, Candidate, Origin, Pass, Unit, slots_of};
use crate::error::EngineError;

impl Pass<'_> {
    /// Runs `first` for the permanent entering at `at`, then asks its controller for `choice`, which the permanent
    /// remembers as it enters.
    pub(super) fn r5504_choose_after(
        &mut self,
        at: usize,
        candidate: &Candidate,
        (first, choice): (&[Effect], EntersChoice),
        unit: &Unit,
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let Unit::Enter { object, controller } = *unit else {
            return Ok(Applied::Done);
        };
        let events = match self.w5d_run(at, candidate, first, unit) {
            Ran::Events(events) => events,
            Ran::Ask(decision) => return Ok(Applied::Ask(decision)),
            Ran::Kept => Vec::new(),
        };
        // The name is asked with the cards just looked at, so the player names having seen them.
        let looked: Vec<String> = events
            .iter()
            .filter_map(|event| match event {
                crate::events::Event::HandLooked(looked) if looked.player == controller => {
                    Some(looked)
                }
                _ => None,
            })
            .flat_map(|looked| looked.cards.iter())
            .filter_map(|id| self.state.object(*id))
            .map(|card| card.card().name.clone())
            .collect();
        let asked = if matches!(
            choice,
            EntersChoice::CardName | EntersChoice::NonlandCardName
        ) {
            self.r4702_chooses_name(candidate.object.id, controller, choice, looked)?
        } else {
            self.chooses(candidate.object.id, controller, choice)?
        };
        let chosen = match asked {
            Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
            Asked::Got(chosen) => chosen,
        };
        let events = self.w9c_fresh_tokens(events);
        let ran = events.len();
        let mut entering = self.slots[at].clone();
        entering.applied = applied;
        let mut slots = slots_of(events);
        slots.push(entering);
        self.slots.splice(at..=at, slots);
        self.w2b_remember(at + ran, object, controller, chosen);
        Ok(Applied::Done)
    }
}
