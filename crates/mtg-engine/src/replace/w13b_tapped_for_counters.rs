//! A permanent its controller may have enter tapped for counters ("You may have this land enter tapped. If you do, you
//! get two rad counters."): asked yes or no as it enters.

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Asked, Candidate, Item, Origin, Pass, Slot, entering};
use crate::choose::{Choice, Target, YesNoOptions};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;

impl Pass<'_> {
    /// Whether `controller` has the permanent enter tapped.
    fn w13b_enters_tapped(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
    ) -> Result<Asked<bool>, EngineError> {
        match self.answer() {
            None => Ok(Asked::Ask(Decision {
                player: controller,
                kind: DecisionKind::YesNo(YesNoOptions::new(
                    source,
                    "Have it enter tapped?".to_owned(),
                )),
            })),
            Some((who, Choice::YesNo(yes))) if *who == controller => Ok(Asked::Got(*yes)),
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to the tapped entry of {controller:?}"
            ))),
        }
    }

    /// The permanent enters tapped and its controller gets `count` counters of `counter`, when they say so.
    pub(super) fn w13b_tapped_for_counters(
        &mut self,
        at: usize,
        candidate: &Candidate,
        (counter, count): (&CounterKind, u32),
        (object, controller): (ObjectId, PlayerId),
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let yes = match self.w13b_enters_tapped(candidate.object.id, controller)? {
            Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
            Asked::Got(yes) => yes,
        };
        if yes {
            let entered = self.entered_at(at, object, controller);
            if let Item::One(Event::Entered { tapped, .. }) = &mut self.slots[entered].item {
                *tapped = true;
            }
        }
        if let Some(slot) = self.slots.get(at)
            && matches!(&slot.item, Item::One(event) if entering(self.state, event).is_some())
        {
            self.slots[at].applied = applied;
        }
        if let (true, Ok(delta)) = (yes, i32::try_from(count)) {
            let got = Slot {
                item: Item::One(Event::CountersChanged(CountersChanged {
                    on: Target::Player(controller),
                    counter: counter.clone(),
                    delta,
                })),
                applied: Vec::new(),
                frozen: false,
            };
            self.slots.insert(at + 1, got);
        }
        Ok(Applied::Done)
    }
}
