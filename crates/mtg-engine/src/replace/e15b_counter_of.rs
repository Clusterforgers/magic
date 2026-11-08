//! A counter of a kind chosen as the permanent enters ("enters with your choice of a deathtouch counter or a lifelink
//! counter on it", CR 614.1c): the controller picks the kind, and the counter comes in with the events that bring the
//! permanent in, so a replacement of counters reaches it.

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Item, Origin, Pass, Slot};
use crate::choose::{Choice, ChoiceOption, OptionOptions, Target};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::object::ObjectRef;

impl Pass<'_> {
    /// `controller` picks one of `kinds` for the permanent `object` becomes, which enters with one counter of it.
    pub(super) fn e15b_counter_of(
        &mut self,
        at: usize,
        (object, controller): (ObjectId, PlayerId),
        kinds: &[CounterKind],
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let picked = match (kinds, self.answer()) {
            ([], _) => None,
            ([only], _) => Some(only.clone()),
            (_, None) => {
                return Ok(Applied::Ask(Decision {
                    player: controller,
                    kind: DecisionKind::ChooseOption(OptionOptions {
                        source: object,
                        options: kinds
                            .iter()
                            .map(|kind| ChoiceOption::Label(format!("{} counter", kind.name())))
                            .collect(),
                        text: "Choose the counter it enters with.".to_owned(),
                    }),
                }));
            }
            (_, Some((who, Choice::Option(index))))
                if *who == controller && *index < kinds.len() =>
            {
                Some(kinds[*index].clone())
            }
            (_, Some(other)) => {
                return Err(Self::invalid(format!(
                    "{other:?} is no answer to the choice of a counter of {controller:?}"
                )));
            }
        };
        self.slots[at].applied.clone_from(&applied);
        let (Some(counter), Some(card)) = (picked, self.state.object(object)) else {
            return Ok(Applied::Done);
        };
        // The counter is for the permanent the card becomes (CR 400.7).
        let permanent = ObjectRef {
            id: object,
            counter: card.counter() + 1,
        };
        let mut inherited = applied;
        inherited.dedup();
        self.slots.insert(
            at + 1,
            Slot {
                item: Item::One(Event::CountersChanged(CountersChanged {
                    on: Target::Object(permanent),
                    counter,
                    delta: 1,
                })),
                applied: inherited,
                frozen: false,
            },
        );
        Ok(Applied::Done)
    }
}
