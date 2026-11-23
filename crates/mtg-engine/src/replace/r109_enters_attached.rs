//! An Equipment that enters attached to a permanent its controller chooses (`ReplaceBy::R109EntersAttached`, CR 614.12,
//! 301.5): "As this Equipment enters, choose a creature you control it could be attached to. If you do, it enters
//! attached to that creature."

use mtg_cards::refs::Selector;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Candidate, Item, Origin, Pass, Slot};
use crate::choose::{CardsOptions, Choice};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::resolve::Scope;
use crate::zones::Zone;

impl Pass<'_> {
    /// The permanents `to` names that `controller` controls and the entering `object` could be attached to, in id order.
    fn r109_hosts(
        &self,
        candidate: &Candidate,
        to: &Selector,
        (object, controller): (ObjectId, PlayerId),
    ) -> Result<Vec<ObjectId>, EngineError> {
        let scope = Scope::announcing(self.state, controller, candidate.object);
        let mut hosts: Vec<ObjectId> = scope
            .matching(to)?
            .into_iter()
            .map(|named| named.id)
            .filter(|id| {
                *id != object
                    && self.state.object(*id).is_some_and(|host| {
                        host.zone() == Zone::Battlefield
                            && crate::characteristics::controller_of(self.state, *id)
                                == Some(controller)
                    })
                    && crate::attach::legal_host_entering(self.state, object, *id)
            })
            .collect();
        hosts.sort_unstable();
        hosts.dedup();
        Ok(hosts)
    }

    /// The Equipment `object` entering under `controller` asks for at most one host among those `to` names; the chosen
    /// one is attached right after it enters, and with none chosen it enters unattached.
    pub(super) fn r109_enters_attached(
        &mut self,
        at: usize,
        candidate: &Candidate,
        to: &Selector,
        (object, controller): (ObjectId, PlayerId),
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let hosts = self.r109_hosts(candidate, to, (object, controller))?;
        let chosen = if hosts.is_empty() {
            None
        } else {
            match self.answer() {
                None => {
                    return Ok(Applied::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseCards(CardsOptions {
                            one_per_name: false,
                            source: candidate.object.id,
                            from: hosts,
                            min: 0,
                            max: 1,
                            text: "You may choose a creature for it to enter attached to.".into(),
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
                        && cards.iter().all(|card| hosts.contains(card)) =>
                {
                    cards.first().copied()
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the host {controller:?} chooses for {object:?}"
                    )));
                }
            }
        };
        self.slots[at].applied = applied;
        if let Some(host) = chosen {
            let entered = self.entered_at(at, object, controller);
            self.slots.insert(
                entered + 1,
                Slot {
                    item: Item::One(Event::Attached { object, to: host }),
                    applied: Vec::new(),
                    frozen: false,
                },
            );
        }
        Ok(Applied::Done)
    }
}
