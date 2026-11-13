//! Counters of different kinds chosen as the permanent enters ("enters with your choice of two different counters on it
//! from among menace, deathtouch, and lifelink", CR 614.1c): the controller picks a set of kinds, and one counter of each
//! comes in with the events that bring the permanent in.

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Item, Origin, Pass, Slot};
use crate::choose::{Choice, ChoiceOption, OptionOptions, Target};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::object::ObjectRef;

/// Every set of `count` different indices below `kinds`, in order.
pub(crate) fn r403_sets(kinds: usize, count: usize) -> Vec<Vec<usize>> {
    fn grow(
        from: usize,
        kinds: usize,
        left: usize,
        set: &mut Vec<usize>,
        all: &mut Vec<Vec<usize>>,
    ) {
        if left == 0 {
            all.push(set.clone());
            return;
        }
        for index in from..kinds {
            set.push(index);
            grow(index + 1, kinds, left - 1, set, all);
            set.pop();
        }
    }
    let mut all = Vec::new();
    grow(0, kinds, count, &mut Vec::new(), &mut all);
    all
}

impl Pass<'_> {
    /// `controller` picks `count` different kinds of `kinds` for the permanent `object` becomes, which enters with one
    /// counter of each.
    pub(super) fn r403_counters_of(
        &mut self,
        at: usize,
        (object, controller): (ObjectId, PlayerId),
        (kinds, count): (&[CounterKind], u32),
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let sets = r403_sets(kinds.len(), usize::try_from(count).unwrap_or(0));
        let picked = match (sets.as_slice(), self.answer()) {
            ([], _) => None,
            ([only], _) => Some(only.clone()),
            (_, None) => {
                let label = |set: &Vec<usize>| {
                    let names: Vec<&str> = set.iter().map(|index| kinds[*index].name()).collect();
                    format!("{} counters", names.join(" and "))
                };
                return Ok(Applied::Ask(Decision {
                    player: controller,
                    kind: DecisionKind::ChooseOption(OptionOptions {
                        can_cancel: false,
                        source: object,
                        options: sets
                            .iter()
                            .map(|set| ChoiceOption::Label(label(set)))
                            .collect(),
                        text: "Choose the counters it enters with.".to_owned(),
                    }),
                }));
            }
            (_, Some((who, Choice::Option(index))))
                if *who == controller && *index < sets.len() =>
            {
                Some(sets[*index].clone())
            }
            (_, Some(other)) => {
                return Err(Self::invalid(format!(
                    "{other:?} is no answer to the choice of counters of {controller:?}"
                )));
            }
        };
        self.slots[at].applied.clone_from(&applied);
        let (Some(set), Some(card)) = (picked, self.state.object(object)) else {
            return Ok(Applied::Done);
        };
        // The counters are for the permanent the card becomes (CR 400.7).
        let permanent = ObjectRef {
            id: object,
            counter: card.counter() + 1,
        };
        let mut inherited = applied;
        inherited.dedup();
        for index in set.into_iter().rev() {
            self.slots.insert(
                at + 1,
                Slot {
                    item: Item::One(Event::CountersChanged(CountersChanged {
                        on: Target::Object(permanent),
                        counter: kinds[index].clone(),
                        delta: 1,
                    })),
                    applied: inherited.clone(),
                    frozen: false,
                },
            );
        }
        Ok(Applied::Done)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the sets of different kinds offered are every combination of the count asked, each once.
    use super::r403_sets;
    use rstest::rstest;

    #[rstest]
    #[case(3, 2, vec![vec![0, 1], vec![0, 2], vec![1, 2]])]
    #[case(4, 3, vec![vec![0, 1, 2], vec![0, 1, 3], vec![0, 2, 3], vec![1, 2, 3]])]
    #[case(2, 2, vec![vec![0, 1]])]
    fn r403_every_set_of_different_kinds(
        #[case] kinds: usize,
        #[case] count: usize,
        #[case] sets: Vec<Vec<usize>>,
    ) {
        assert_eq!(r403_sets(kinds, count), sets);
    }
}
