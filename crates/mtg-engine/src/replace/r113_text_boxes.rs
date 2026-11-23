//! A permanent that may exchange its text box with another as it enters (`ReplaceBy::R612MayExchangeTextBoxes`, CR
//! 612.1, 614.1c): "As Deadpool enters, you may exchange his text box and another creature's." The exchange is a
//! continuous effect for good on the permanent the card becomes (CR 400.7) and the chosen one, made with the entering.

use mtg_cards::modify::Duration;
use mtg_cards::refs::Selector;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Candidate, Item, Origin, Pass, Slot};
use crate::choose::{CardsOptions, Choice};
use crate::decision::{Decision, DecisionKind};
use crate::effects::{ContinuousEffect, Modification, Selector as Affected};
use crate::error::EngineError;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::Scope;
use crate::zones::Zone;

impl Pass<'_> {
    /// The entering `object` under `controller` asks for at most one other permanent `with` names; with one chosen, the two
    /// exchange their text boxes from the moment it is on the battlefield.
    pub(super) fn r612_may_exchange(
        &mut self,
        at: usize,
        candidate: &Candidate,
        with: &Selector,
        (object, controller): (ObjectId, PlayerId),
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let scope = Scope::announcing(self.state, controller, candidate.object);
        let mut others: Vec<ObjectId> = scope
            .matching(with)?
            .into_iter()
            .map(|named| named.id)
            .filter(|id| {
                *id != object
                    && self
                        .state
                        .object(*id)
                        .is_some_and(|other| other.zone() == Zone::Battlefield)
            })
            .collect();
        others.sort_unstable();
        others.dedup();
        let chosen = if others.is_empty() {
            None
        } else {
            match self.answer() {
                None => {
                    return Ok(Applied::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseCards(CardsOptions {
                            one_per_name: false,
                            source: candidate.object.id,
                            from: others,
                            min: 0,
                            max: 1,
                            text: "You may choose a creature to exchange text boxes with.".into(),
                            can_cancel: false,
                            total: None,
                            groups: Vec::new(),
                            targeted: Box::default(),
                        }),
                    }));
                }
                Some((who, Choice::Cards(cards)))
                    if *who == controller
                        && cards.len() <= 1
                        && cards.iter().all(|card| others.contains(card)) =>
                {
                    cards.first().copied()
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the exchange {controller:?} chooses for {object:?}"
                    )));
                }
            }
        };
        self.slots[at].applied.clone_from(&applied);
        let (Some(other), Some(card)) = (
            chosen.and_then(|id| self.state.object(id)),
            self.state.object(object),
        ) else {
            return Ok(Applied::Done);
        };
        // The exchange is with the permanent the card becomes (CR 400.7).
        let permanent = ObjectRef {
            id: object,
            counter: card.counter() + 1,
        };
        let (id, timestamp) = self.fresh_effect();
        let effect = ContinuousEffect::new(
            id,
            Affected::Objects(vec![permanent, other.reference()]),
            Modification::R612ExchangeTextBoxes,
            Duration::Forever,
            timestamp,
        )
        .from_source(permanent)
        .begun(controller, self.state.turn().number());
        let mut inherited = applied;
        inherited.dedup();
        let entered = self.entered_at(at, object, controller);
        self.slots.insert(
            entered + 1,
            Slot {
                item: Item::One(Event::EffectCreated {
                    effect: Box::new(effect),
                }),
                applied: inherited,
                frozen: false,
            },
        );
        Ok(Applied::Done)
    }
}
