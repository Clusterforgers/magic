//! "it enters under your control instead" and "is created under your control instead" (`ReplaceBy::UnderYourControl`):
//! a permanent or the tokens an opponent of the replacement's controller would get go to that controller (Gather
//! Specimens, Crafty Cutpurse). A replacement of control applies before the others of the event (CR 616.1b), so the
//! new controller makes the choices as it enters (Gather Specimens rulings).

use mtg_core::ids::{ObjectId, PlayerId};

use super::{Item, Pass, Unit};
use crate::events::Event;

impl Pass<'_> {
    /// Whether the permanent or tokens of `unit` would come under an opponent of `taker`; any unit
    /// a control replacement does not cover is no.
    pub(super) fn r5517_from_opponent(&self, taker: PlayerId, unit: &Unit) -> bool {
        let would = match unit {
            Unit::Tokens { controller } => *controller,
            Unit::Enter { object, controller } => self.r5517_would_control(*object, *controller),
            _ => return false,
        };
        self.state.is_opponent(taker, would)
    }

    /// Who `object` would enter under: the controller its `Entered` event names, else the unit's.
    fn r5517_would_control(&self, object: ObjectId, fallback: PlayerId) -> PlayerId {
        self.slots
            .iter()
            .find_map(|slot| match &slot.item {
                Item::One(Event::Entered {
                    object: o,
                    controller,
                    ..
                }) if *o == object => Some(*controller),
                _ => None,
            })
            .unwrap_or(fallback)
    }

    /// Puts the tokens or the entering permanent of slot `at` under `controller`.
    pub(super) fn r5517_under_you(&mut self, at: usize, controller: PlayerId, unit: &Unit) {
        match unit {
            Unit::Tokens { .. } => {
                if let Item::Tokens(run) = &mut self.slots[at].item {
                    for token in run.iter_mut() {
                        token.controller = controller;
                    }
                }
            }
            Unit::Enter {
                object,
                controller: was,
            } => {
                let entered = self.entered_at(at, *object, *was);
                if let Item::One(Event::Entered {
                    controller: under, ..
                }) = &mut self.slots[entered].item
                {
                    *under = controller;
                }
            }
            _ => {}
        }
    }
}
