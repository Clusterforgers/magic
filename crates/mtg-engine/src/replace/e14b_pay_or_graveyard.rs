//! A permanent that enters only if its controller pays a cost as it would enter, and goes to its owner's graveyard
//! otherwise ("If ~ would enter, sacrifice two untapped lands instead. If you do, put ~ onto the battlefield. If you
//! don't, put it into its owner's graveyard."; Mox Diamond's "you may discard a land card instead").
//!
//! The cost is one pick (a sacrifice or a discard). An optional one is asked yes or no first, with No the only answer
//! when it cannot be paid, so the pace does not show the hand. The picked objects go before the entering; a land played
//! that is not paid for goes to the graveyard and still uses the land play (`ZoneCause::LandPlayed`).

use std::collections::BTreeSet;

use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Asked, Candidate, Item, Origin, Pass, Slot};
use crate::choose::{CardsOptions, Choice, ChoiceOption, OptionOptions, Target, YesNoOptions};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::object::{GameObject, ObjectRef};
use crate::parts::{Part, Payer, Pick};
use crate::price::Price;
use crate::zones::Zone;

impl Pass<'_> {
    /// Applies "pay `cost` or it goes to the graveyard" to `object` entering under `controller` (the event at `at`).
    pub(super) fn e14b_pay_or_graveyard(
        &mut self,
        at: usize,
        candidate: &Candidate,
        (cost, optional, counter_each): (&Cost, bool, &[CounterKind]),
        (object, controller): (ObjectId, PlayerId),
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        // "Exile X creature cards": X is the X the spell was cast with, which the card remembers (CR 107.3m).
        let x = self
            .state
            .object(object)
            .and_then(GameObject::announced_x)
            .unwrap_or(0);
        let cost = &r118_with_x(cost, x);
        if r118_takes_nothing(cost) {
            self.slots[at].applied = applied;
            return Ok(Applied::Done);
        }
        let pick = Price::announced(cost)
            .ok()
            .and_then(|price| match &price.parts[..] {
                [Part::Pick(pick)] if price.mana.is_none() && !price.tap => Some(pick.clone()),
                _ => None,
            });
        let Some(pick) = pick else {
            self.slots[at].applied = applied;
            return Ok(Applied::Done);
        };
        let source = candidate.object.id;
        let barred = [object];
        let options = pick.candidates(&Payer {
            state: self.state,
            player: controller,
            source: object,
            barred: &barred,
            granter: None,
            pays: crate::r4707_pay_costs::PaysFor::Effect,
        });
        let possible = !pick.short(self.state, &options);
        let pays = if optional {
            match self.e14b_pays(source, controller, possible)? {
                Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
                Asked::Got(pays) => pays,
            }
        } else {
            possible
        };
        if !pays {
            self.e14b_to_graveyard(at, object, &applied);
            return Ok(Applied::Done);
        }
        let picked = match self.e14b_picked(source, controller, &pick, &options)? {
            Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
            Asked::Got(picked) => picked,
        };
        let mut kinds = Vec::new();
        if !counter_each.is_empty() {
            for _ in &picked {
                match self.r118_counter_kind(object, controller, counter_each)? {
                    Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
                    Asked::Got(kind) => kinds.push(kind),
                }
            }
        }
        let permanent = self.state.object(object).map(|card| ObjectRef {
            id: object,
            counter: card.counter() + 1,
        });
        let paid: Vec<Slot> = Part::Pick(pick)
            .events(self.state, controller, object, &picked)
            .into_iter()
            .map(|event| Slot::new(Item::One(event)))
            .collect();
        let made = paid.len();
        self.slots.splice(at..at, paid);
        // The enter event moved to make room for the payment.
        if let Some(slot) = self.slots.get_mut(at + made) {
            slot.applied.clone_from(&applied);
        }
        // The counters come in with the events that bring the permanent in, so a replacement of counters reaches them.
        if let Some(permanent) = permanent {
            let mut inherited = applied;
            inherited.dedup();
            for (offset, counter) in kinds.into_iter().enumerate() {
                self.slots.insert(
                    at + made + 1 + offset,
                    Slot {
                        item: Item::One(Event::CountersChanged(CountersChanged {
                            on: Target::Object(permanent),
                            counter,
                            delta: 1,
                        })),
                        applied: inherited.clone(),
                        frozen: false,
                    },
                );
            }
        }
        Ok(Applied::Done)
    }

    /// The kind of one counter the permanent `object` enters with for a card paid, chosen by `controller` among `kinds`.
    fn r118_counter_kind(
        &mut self,
        object: ObjectId,
        controller: PlayerId,
        kinds: &[CounterKind],
    ) -> Result<Asked<CounterKind>, EngineError> {
        if let [only] = kinds {
            return Ok(Asked::Got(only.clone()));
        }
        match self.answer() {
            None => Ok(Asked::Ask(Decision {
                player: controller,
                kind: DecisionKind::ChooseOption(OptionOptions {
                    can_cancel: false,
                    source: object,
                    options: kinds
                        .iter()
                        .map(|kind| ChoiceOption::Label(format!("{} counter", kind.name())))
                        .collect(),
                    text: "Choose a counter it enters with for a card exiled.".to_owned(),
                }),
            })),
            Some((who, Choice::Option(index))) if *who == controller && *index < kinds.len() => {
                Ok(Asked::Got(kinds[*index].clone()))
            }
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to the choice of a counter of {controller:?}"
            ))),
        }
    }

    /// Whether `controller` pays the optional cost; No is the only answer when they cannot.
    fn e14b_pays(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        possible: bool,
    ) -> Result<Asked<bool>, EngineError> {
        match self.answer() {
            None => Ok(Asked::Ask(Decision {
                player: controller,
                kind: DecisionKind::YesNo(YesNoOptions {
                    only_no: !possible,
                    ..YesNoOptions::new(source, "Pay its cost so it enters?".to_owned())
                }),
            })),
            Some((who, Choice::YesNo(yes))) if *who == controller && (!*yes || possible) => {
                Ok(Asked::Got(*yes))
            }
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to the payment of {controller:?} for an entering"
            ))),
        }
    }

    /// The objects that pay the pick, asked unless there is no choice between them.
    fn e14b_picked(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        pick: &Pick,
        options: &[ObjectId],
    ) -> Result<Asked<Vec<ObjectId>>, EngineError> {
        if let Some(taken) = pick.unasked(self.state, options) {
            return Ok(Asked::Got(taken));
        }
        let (min, max) = pick.bounds(options.len());
        match self.answer() {
            None => Ok(Asked::Ask(Decision {
                player: controller,
                kind: DecisionKind::ChooseCards(CardsOptions {
                    one_per_name: false,
                    source,
                    from: options.to_vec(),
                    min,
                    max,
                    text: pick.text(),
                    can_cancel: false,
                    total: None,
                    groups: Vec::new(),
                    targeted: Box::default(),
                }),
            })),
            Some((who, Choice::Cards(cards)))
                if *who == controller
                    && cards.iter().all(|card| options.contains(card))
                    && cards.iter().collect::<BTreeSet<_>>().len() == cards.len()
                    && u32::try_from(cards.len()).is_ok_and(|n| (min..=max).contains(&n)) =>
            {
                Ok(Asked::Got(cards.clone()))
            }
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to the choice of what {controller:?} pays for an entering"
            ))),
        }
    }

    /// Puts `object` into its owner's graveyard in place of the entering at `at`: a land played still uses the land play.
    fn e14b_to_graveyard(&mut self, at: usize, object: ObjectId, applied: &[Origin]) {
        let Some(card) = self.state.object(object) else {
            self.slots[at].applied = applied.to_vec();
            return;
        };
        let (owner, from) = (card.owner(), card.zone());
        let grave = Zone::Graveyard(owner);
        self.slots.retain(|slot| {
            !matches!(&slot.item, Item::One(Event::Entered { object: o, .. }) if *o == object)
        });
        match &self.slots[at].item {
            Item::One(Event::LandPlayed { .. }) => {
                self.slots[at] = Slot {
                    item: Item::One(Event::ZoneChanged {
                        object,
                        from,
                        to: grave,
                        cause: ZoneCause::LandPlayed,
                    }),
                    applied: applied.to_vec(),
                    frozen: false,
                };
            }
            Item::One(Event::ZoneChanged { from, .. }) => {
                let from = *from;
                self.redirect(at, object, from, grave, applied);
            }
            _ => self.redirect(at, object, from, grave, applied),
        }
    }
}

/// `cost` with the X of an exile of cards from the graveyard fixed at `x`.
fn r118_with_x(cost: &Cost, x: u32) -> Cost {
    let fixed = Amount::Fixed(i32::try_from(x).unwrap_or(i32::MAX));
    Cost(
        cost.0
            .iter()
            .map(|item| match item {
                CostItem::ExileFromGraveyard {
                    filter,
                    any_player,
                    count: Amount::X,
                } => CostItem::ExileFromGraveyard {
                    filter: filter.clone(),
                    any_player: *any_player,
                    count: fixed.clone(),
                },
                other => other.clone(),
            })
            .collect(),
    )
}

/// Whether the cost takes no card: an exile of 0 cards, paid by doing nothing (CR 107.3, 601.2h).
fn r118_takes_nothing(cost: &Cost) -> bool {
    matches!(
        &cost.0[..],
        [CostItem::ExileFromGraveyard {
            count: Amount::Fixed(0),
            ..
        }]
    )
}
