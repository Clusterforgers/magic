//! Riot (CR 702.136) in the replacement step: the choice a permanent makes as it enters, and the static abilities that
//! give a creature riot before it is in play.

use mtg_cards::ability::Ability;
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_cards::value::CounterKind;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use super::{Applied, Item, Origin, Pass, Slot, Source};
use crate::abilities::{AbilityRef, AbilitySource};
use crate::choose::{Choice, ChoiceOption, OptionOptions, Target};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::effects::{ContinuousEffect, Modification as Change, Selector as Affected};
use crate::error::EngineError;
use crate::events::Event;
use crate::keywords;
use crate::object::ObjectRef;

/// The options of the choice, in the order the answer counts them.
const COUNTER: &str = "+1/+1 counter";
const HASTE: &str = "Haste";

/// The enters replacements that the static ability at `address` gives to the objects it affects by giving them a
/// keyword (riot): the replacement the keyword adds, worded for the group the static ability names. A creature that
/// is about to enter has them although it has no abilities in play (CR 614.12).
pub(super) fn given_to_entering(
    address: AbilityRef,
    rules: &Ability,
    controller: PlayerId,
    object: ObjectRef,
) -> Vec<Source> {
    let Ability::Static {
        affected: Ref::Each(group),
        mods,
        condition,
        from: ZoneKind::Battlefield,
        ..
    } = rules
    else {
        return Vec::new();
    };
    if address.source != AbilitySource::Printed {
        return Vec::new();
    }
    mods.iter()
        .filter_map(|modification| match modification {
            Modification::AddKeyword(keyword) => Some(keyword),
            _ => None,
        })
        .flat_map(|keyword| keywords::rules(keyword).adds_abilities(keyword))
        .filter_map(|added| match added {
            Ability::Replacement {
                when: ReplaceWhen::Enters { what },
                by: by @ ReplaceBy::Riot,
                ..
            } if what == Selector::this() => Some(Source {
                address,
                ability: std::sync::Arc::new(Ability::Replacement {
                    when: ReplaceWhen::Enters {
                        what: group.clone(),
                    },
                    by,
                    condition: condition.clone(),
                    from: ZoneKind::Battlefield,
                }),
                controller,
                object,
                copied: false,
            }),
            _ => None,
        })
        .collect()
}

impl Pass<'_> {
    /// Riot: `controller` picks a +1/+1 counter or haste for the permanent `object` becomes. The counter is proposed
    /// with the events that bring it in, so a replacement of counters reaches it; haste is an effect that lasts while
    /// the permanent stays on the battlefield.
    pub(super) fn riot(
        &mut self,
        at: usize,
        object: ObjectId,
        controller: PlayerId,
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let haste = match self.answer() {
            None => {
                return Ok(Applied::Ask(Decision {
                    player: controller,
                    kind: DecisionKind::ChooseOption(OptionOptions {
                        source: object,
                        options: vec![
                            ChoiceOption::Label(COUNTER.to_owned()),
                            ChoiceOption::Label(HASTE.to_owned()),
                        ],
                        text: "Choose a +1/+1 counter or haste.".to_owned(),
                    }),
                }));
            }
            Some((who, Choice::Option(index))) if *who == controller && *index < 2 => *index == 1,
            Some(other) => {
                return Err(Self::invalid(format!(
                    "{other:?} is no answer to the riot choice of {controller:?}"
                )));
            }
        };
        self.slots[at].applied.clone_from(&applied);
        let Some(card) = self.state.object(object) else {
            return Ok(Applied::Done);
        };
        // The choice is for the permanent the card becomes (CR 400.7).
        let permanent = ObjectRef {
            id: object,
            counter: card.counter() + 1,
        };
        let follow = if haste {
            let (id, timestamp) = self.fresh_effect();
            Event::EffectCreated {
                effect: Box::new(
                    ContinuousEffect::new(
                        id,
                        Affected::Objects(vec![permanent]),
                        Change::AddKeyword(mtg_cards::keyword::Keyword::Haste),
                        Duration::WhileSourceOnBattlefield,
                        timestamp,
                    )
                    .from_source(permanent)
                    .begun(controller, self.state.turn().number()),
                ),
            }
        } else {
            Event::CountersChanged(CountersChanged {
                on: Target::Object(permanent),
                counter: CounterKind::PlusOnePlusOne,
                delta: 1,
            })
        };
        let mut inherited = applied;
        inherited.dedup();
        // Effects of one batch stay in the order of their ids.
        let place = self
            .slots
            .iter()
            .rposition(|slot| matches!(&slot.item, Item::One(Event::EffectCreated { .. })))
            .map_or(at + 1, |last| (last + 1).max(at + 1));
        self.slots.insert(
            place,
            Slot {
                item: Item::One(follow),
                applied: inherited,
                frozen: false,
            },
        );
        Ok(Applied::Done)
    }

    /// The next free effect id and timestamp, counting the effects this batch has made.
    fn fresh_effect(&self) -> (EffectId, u64) {
        let (id, timestamp) = self
            .slots
            .iter()
            .filter_map(|slot| match &slot.item {
                Item::One(Event::EffectCreated { effect }) => Some((effect.id.0, effect.timestamp)),
                _ => None,
            })
            .fold(
                (self.state.next_effect, self.state.next_timestamp),
                |(id, stamp), (made, at)| (id.max(made + 1), stamp.max(at + 1)),
            );
        (EffectId(id), timestamp)
    }
}
