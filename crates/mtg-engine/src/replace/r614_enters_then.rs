//! `ReplaceWhen::R614PutOnto` and `ReplaceBy::R614EntersThen` (CR 614.1a): "If an opponent ... would put a land onto the
//! battlefield, that player instead puts that land onto the battlefield then sacrifices a land of their choice." The
//! permanent enters as it would, and the effects follow it in the same batch, run on the game as it is once the
//! permanent is there, so no player can act in between (the land can't be tapped for mana first).

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::w5d_instead::Ran;
use super::{Applied, Candidate, Item, Origin, Pass, Unit, slots_of};
use crate::characteristics::controller_of_object;
use crate::resolve::{Inline, w5d_run_instead};
use crate::state::GameState;

/// The player who would put a permanent onto the battlefield that would enter under `controller`'s control: the controller
/// of the spell or ability resolving, else the player it enters under (who plays a land, or whose spell resolves into it).
pub(super) fn r614_putter(state: &GameState, controller: PlayerId) -> PlayerId {
    state
        .resolving()
        .and_then(|resolving| state.object(resolving.object()))
        .map_or(controller, |object| controller_of_object(state, object))
}

impl Pass<'_> {
    /// The permanent at `at` enters, then `effects` run for the player who put it there (`Ref::R115Causer`) on the game
    /// with it entered; their events follow it in the batch as new events that no replacement has applied to yet.
    pub(super) fn r614_enters_then(
        &mut self,
        at: usize,
        candidate: &Candidate,
        effects: &[Effect],
        unit: &Unit,
        applied: Vec<Origin>,
    ) -> Applied {
        let Unit::Enter { controller, .. } = unit else {
            return Applied::Done;
        };
        let putter = r614_putter(self.state, *controller);
        let mut cause = self.w5d_cause(at, unit);
        cause.r115_causer = Some(putter);
        let mut entered = self.state.clone();
        if let Item::One(event) = &self.slots[at].item
            && crate::fold::apply_event(&mut entered, event).is_err()
        {
            entered = self.state.clone();
        }
        let answers = &mut self.answers;
        let ran = match w5d_run_instead(
            &entered,
            candidate.controller,
            candidate.object,
            &cause,
            effects,
            &mut || answers.next().cloned(),
            None,
        ) {
            Ok(Inline::Events(events)) => Ran::Events(events),
            Ok(Inline::Ask(decision)) => Ran::Ask(decision),
            Ok(Inline::Declined | Inline::R115Continue { .. }) | Err(_) => Ran::Kept,
        };
        let events = match ran {
            Ran::Events(events) => events,
            Ran::Ask(decision) => return Applied::Ask(decision),
            Ran::Kept => Vec::new(),
        };
        let mut entering = self.slots[at].clone();
        entering.applied = applied;
        let mut slots = vec![entering];
        slots.extend(slots_of(events));
        self.slots.splice(at..=at, slots);
        Applied::Done
    }
}
