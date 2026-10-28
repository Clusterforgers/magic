//! The parts of an ability's cost beyond `{T}`, mana and losing the source: life, counters removed from the source, and
//! the permanents and cards the player picks (CR 118.3, 601.2b, 601.2h).
//!
//! A cost is read into [`Part`]s as the ability is announced. The picks are asked then, with `ChooseCards`
//! (`announce/parts.rs`), and every part is paid with its events as the ability goes on the stack (`cast.rs`).

mod bounce;
mod c11c_counter_picks;
mod counters;
mod discard;
pub(crate) mod energy;
mod exile;
mod exile_hand;
mod life;
pub(crate) mod mill;
mod reveal;
mod sacrifice;
mod tap;

pub(crate) use counters::after_removal;

use mtg_cards::cost::CostItem;
use mtg_cards::refs::{Pred, Selector};
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::events::Event;
use crate::resolve::Scope;
use crate::snapshot::{Snapshot, snapshot};
use crate::state::GameState;

/// What kind of object a pick takes and what paying does to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A permanent the payer controls is sacrificed.
    Sacrifice,
    /// A card in the payer's hand is discarded.
    Discard,
    /// A card in the payer's graveyard is exiled.
    Exile,
    /// A card in the payer's hand is exiled.
    ExileHand,
    /// A card in the payer's hand is shown to every player and stays there.
    Reveal,
    /// An untapped permanent the payer controls is tapped.
    Tap,
    /// Any number of untapped permanents the payer controls are tapped, with total power at least `Pick.count`.
    TapPower,
    /// A permanent the payer controls returns to its owner's hand.
    Return,
    /// A permanent the payer controls loses a counter of the kind its filter names.
    Counter,
}

/// A part of the cost that takes objects the player picks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pick {
    /// What is taken.
    pub(crate) kind: Kind,
    /// Which objects may be picked.
    pub(crate) filter: Selector,
    /// How many; the least total power for a pick of [`Kind::TapPower`].
    pub(crate) count: usize,
}

/// One part of a cost the engine pays besides `{T}`, mana and the loss of the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Part {
    /// Life paid (CR 119.4).
    Life(u32),
    /// Counters removed from the source.
    Counters {
        /// The kind of counter.
        counter: CounterKind,
        /// How many.
        count: u32,
    },
    /// Energy counters paid (CR 107.14).
    Energy(u32),
    /// Cards milled from the top of the payer's library (CR 701.17).
    Mill(u32),
    /// Objects picked at the announcement.
    Pick(Pick),
    /// A part whose amount is the X announced with the ability, held here as it stands for X = 0; `Part::at` counts it.
    X(Box<Part>),
}

/// The name `Caps` gives a part whose amount is neither a fixed number nor X, which nothing counts yet.
const fn counted(item: &CostItem) -> &'static str {
    match item {
        CostItem::PayLife(_) => "Cost.PayLife.Counted",
        CostItem::PayEnergy(_) => "Cost.PayEnergy.Counted",
        CostItem::Mill(_) => "Cost.Mill.Counted",
        CostItem::RemoveCounters { .. } => "Cost.RemoveCounters.Counted",
        CostItem::Sacrifice { .. } => "Cost.Sacrifice.Counted",
        CostItem::Discard { .. } => "Cost.Discard.Counted",
        CostItem::ExileFromGraveyard { .. } => "Cost.ExileFromGraveyard.Counted",
        CostItem::ExileFromHand { .. } => "Cost.ExileFromHand.Counted",
        CostItem::RevealFromHand { .. } => "Cost.RevealFromHand.Counted",
        CostItem::TapPermanents { .. } => "Cost.TapPermanents.Counted",
        CostItem::ReturnToHand { .. } => "Cost.ReturnToHand.Counted",
        _ => "Cost",
    }
}

/// A fixed amount, or X (`true`, counted as 0 until it is announced); the name of the part otherwise.
fn amount(amount: &Amount, item: &CostItem) -> Result<(u32, bool), &'static str> {
    match amount {
        Amount::Fixed(number) => u32::try_from(*number)
            .map(|number| (number, false))
            .map_err(|_| counted(item)),
        Amount::X => Ok((0, true)),
        _ => Err(counted(item)),
    }
}

fn pick(
    kind: Kind,
    filter: &Selector,
    count: &Amount,
    item: &CostItem,
) -> Result<(Part, bool), &'static str> {
    let (count, x) = amount(count, item)?;
    let count = usize::try_from(count).unwrap_or(usize::MAX);
    Ok((
        Part::Pick(Pick {
            kind,
            filter: filter.clone(),
            count,
        }),
        x,
    ))
}

impl Part {
    /// The part a cost item is, `None` for the items that are not one of these (`{T}`, mana, losing the source), or the
    /// name of the part whose amount the engine cannot count.
    pub(crate) fn read(item: &CostItem) -> Result<Option<Part>, &'static str> {
        let (part, x) = match item {
            CostItem::PayLife(count) => {
                let (number, x) = amount(count, item)?;
                (Part::Life(number), x)
            }
            CostItem::PayEnergy(count) => {
                let (number, x) = amount(count, item)?;
                (Part::Energy(number), x)
            }
            CostItem::Mill(count) => {
                let (number, x) = amount(count, item)?;
                (Part::Mill(number), x)
            }
            CostItem::RemoveCounters {
                counter,
                count,
                from: Some(filter),
            } => {
                let mut filter = filter.clone();
                filter.0.push(counter.clone().map_or(Pred::HasAnyCounter, Pred::HasCounter));
                pick(Kind::Counter, &filter, count, item)?
            }
            CostItem::RemoveCounters { counter, count, .. } => {
                let (number, x) = amount(count, item)?;
                let counter = counter.clone().ok_or("Cost.RemoveCounters.AnyKind")?;
                (
                    Part::Counters {
                        counter,
                        count: number,
                    },
                    x,
                )
            }
            CostItem::Sacrifice { filter, count } => pick(Kind::Sacrifice, filter, count, item)?,
            CostItem::Discard { count, filter } => pick(
                Kind::Discard,
                &filter.clone().unwrap_or_default(),
                count,
                item,
            )?,
            CostItem::ExileFromGraveyard { filter, count } => {
                pick(Kind::Exile, filter, count, item)?
            }
            CostItem::ExileFromHand { filter, count } => {
                pick(Kind::ExileHand, filter, count, item)?
            }
            CostItem::RevealFromHand { filter, count } => pick(Kind::Reveal, filter, count, item)?,
            CostItem::TapPermanents { filter, count } => pick(Kind::Tap, filter, count, item)?,
            CostItem::TapPower { filter, power } => (
                Part::Pick(Pick {
                    kind: Kind::TapPower,
                    filter: filter.clone(),
                    count: usize::try_from(*power).unwrap_or(usize::MAX),
                }),
                false,
            ),
            CostItem::ReturnToHand { filter, count } => pick(Kind::Return, filter, count, item)?,
            _ => return Ok(None),
        };
        Ok(Some(if x { Part::X(Box::new(part)) } else { part }))
    }

    /// The part for the X announced: a counted part takes X, any other stays as it is.
    pub(crate) fn at(&self, x: u32) -> Part {
        match self {
            Part::X(inner) => match &**inner {
                Part::Life(_) => Part::Life(x),
                Part::Energy(_) => Part::Energy(x),
                Part::Mill(_) => Part::Mill(x),
                Part::Counters { counter, .. } => Part::Counters {
                    counter: counter.clone(),
                    count: x,
                },
                Part::Pick(pick) => Part::Pick(Pick {
                    count: usize::try_from(x).unwrap_or(usize::MAX),
                    ..pick.clone()
                }),
                Part::X(_) => self.clone(),
            },
            other => other.clone(),
        }
    }

    /// Whether the amount of the part is the X announced with the ability.
    pub(crate) const fn is_counted(&self) -> bool {
        matches!(self, Part::X(_))
    }

    /// The most X the player could pay for this part now, `None` for a part that is not counted: all their life (a player
    /// pays up to what they have, CR 119.4), the counters on the source, the energy they hold, the objects they may pick.
    pub(crate) fn most(&self, payer: &Payer<'_>) -> Option<u32> {
        let Part::X(inner) = self else { return None };
        let number = match &**inner {
            Part::Life(_) => life::most(payer),
            Part::Energy(_) => energy::most(payer),
            Part::Mill(_) => mill::most(payer),
            Part::Counters { counter, .. } => counters::most(payer, counter),
            Part::Pick(pick) => u32::try_from(pick.candidates(payer).len()).unwrap_or(u32::MAX),
            Part::X(_) => return None,
        };
        Some(number)
    }

    /// The name `Caps` and a refusal give the part.
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Part::Life(_) => "Cost.PayLife",
            Part::Energy(_) => "Cost.PayEnergy",
            Part::Mill(_) => "Cost.Mill",
            Part::Counters { .. } => "Cost.RemoveCounters",
            Part::X(inner) => inner.name(),
            Part::Pick(pick) => match pick.kind {
                Kind::Sacrifice => "Cost.Sacrifice",
                Kind::Discard => "Cost.Discard",
                Kind::Exile => "Cost.ExileFromGraveyard",
                Kind::ExileHand => "Cost.ExileFromHand",
                Kind::Reveal => "Cost.RevealFromHand",
                Kind::Tap => "Cost.TapPermanents",
                Kind::TapPower => "Cost.TapPower",
                Kind::Return => "Cost.ReturnToHand",
                Kind::Counter => "Cost.RemoveCounters",
            },
        }
    }

    /// The part in the words of a card, for the question an effect asks: "Pay 2 life", "Pay {E}{E}", "Discard a card".
    pub(crate) fn text(&self) -> String {
        match self {
            Part::Life(amount) => format!("Pay {amount} life"),
            Part::Energy(amount) => {
                format!(
                    "Pay {}",
                    "{E}".repeat(usize::try_from(*amount).unwrap_or(0))
                )
            }
            Part::Mill(1) => "Mill a card".to_owned(),
            Part::Mill(amount) => format!("Mill {amount} cards"),
            Part::Counters { counter, count } => {
                let plural = if *count == 1 { "" } else { "s" };
                format!("Remove {count} {} counter{plural}", counter.name())
            }
            Part::Pick(pick) => pick.text().trim_end_matches('.').to_owned(),
            Part::X(inner) => inner.text(),
        }
    }

    /// Whether the part taps permanents.
    pub(crate) const fn taps(&self) -> bool {
        match self {
            Part::Pick(pick) => matches!(pick.kind, Kind::Tap | Kind::TapPower),
            Part::X(inner) => inner.taps(),
            _ => false,
        }
    }

    /// Whether the player could pay the part now with `barred` left alone, and the objects it would take, leaving the
    /// `spare` ones for the mana as far as it can (the player picks the real ones; this is what the check claims). A
    /// counted part is payable: X can be 0.
    pub(crate) fn payable(&self, payer: &Payer<'_>, spare: &[ObjectId]) -> Option<Vec<ObjectId>> {
        match self {
            Part::Life(amount) => life::payable(payer, *amount).then(Vec::new),
            Part::Energy(amount) => energy::payable(payer, *amount).then(Vec::new),
            Part::Mill(amount) => mill::payable(payer, *amount).then(Vec::new),
            Part::Counters { counter, count } => {
                counters::payable(payer, counter, *count).then(Vec::new)
            }
            Part::Pick(pick) if pick.kind == Kind::TapPower => {
                let power = u32::try_from(pick.count).unwrap_or(u32::MAX);
                tap::enough(payer.state, pick.candidates(payer), power, spare)
            }
            Part::Pick(pick) => {
                let mut found = pick.candidates(payer);
                found.sort_by_key(|id| spare.contains(id));
                (found.len() >= pick.count).then(|| found.into_iter().take(pick.count).collect())
            }
            Part::X(_) => Some(Vec::new()),
        }
    }

    /// The events that pay the part; `picked` are the objects the player picked for it.
    pub(crate) fn events(
        &self,
        state: &GameState,
        player: PlayerId,
        source: ObjectId,
        picked: &[ObjectId],
    ) -> Vec<Event> {
        self.w2f_events(state, (player, source), picked, &mut [].iter())
    }

    /// The events that pay the part, a counter of any kind taking its kind from `kinds` (`Choices::cost_kinds`), in turn.
    pub(crate) fn w2f_events(
        &self,
        state: &GameState,
        (player, source): (PlayerId, ObjectId),
        picked: &[ObjectId],
        kinds: &mut std::slice::Iter<'_, CounterKind>,
    ) -> Vec<Event> {
        match self {
            Part::Life(amount) => life::events(player, *amount),
            Part::Energy(amount) => energy::events(player, *amount),
            Part::Mill(amount) => mill::events(state, player, *amount),
            Part::Counters { counter, count } => counters::events(state, source, counter, *count),
            Part::Pick(pick) => picked
                .iter()
                .filter_map(|object| pick.event(state, player, *object, kinds))
                .collect(),
            Part::X(inner) => inner.w2f_events(state, (player, source), picked, kinds),
        }
    }
}

/// The objects the picks of `parts` take out of their zones (a sacrifice, a discard, an exile from a graveyard or a
/// hand), each as it is before the cost is paid: what "the sacrificed creature's power" reads. A tapped permanent stays
/// where it is, and a returned one goes to a hand where a face-down permanent must stay hidden, so neither is photographed.
pub(crate) fn photographed(
    state: &GameState,
    parts: &[Part],
    picks: &[Vec<ObjectId>],
) -> Vec<Snapshot> {
    let mut picks = picks.iter();
    let mut shots = Vec::new();
    for part in parts {
        let Part::Pick(pick) = part else { continue };
        let Some(picked) = picks.next() else { break };
        if matches!(
            pick.kind,
            Kind::Sacrifice | Kind::Discard | Kind::Exile | Kind::ExileHand | Kind::Reveal
        ) {
            shots.extend(picked.iter().filter_map(|id| snapshot(state, *id)));
        }
    }
    shots
}

/// The events that pay `parts` for `source` in the order of the cost, with the objects picked for the parts that take
/// some (`picks`, one list for each such part); `None` when a pick is missing.
pub(crate) fn pay(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    parts: &[Part],
    picks: &[Vec<ObjectId>],
    kinds: &[CounterKind],
) -> Option<Vec<Event>> {
    let mut picks = picks.iter();
    let mut kinds = kinds.iter();
    let mut events = Vec::new();
    for part in parts {
        let picked = if matches!(part, Part::Pick(_)) {
            picks.next()?.as_slice()
        } else {
            &[]
        };
        events.extend(part.w2f_events(state, (player, source), picked, &mut kinds));
    }
    Some(events)
}

/// Who pays and for what: the player, the object whose ability it is, and the objects that cannot be picked because
/// the cost already takes them (the source, which the price taps or loses, and what earlier parts picked).
pub(crate) struct Payer<'a> {
    /// The game.
    pub(crate) state: &'a GameState,
    /// The player who pays.
    pub(crate) player: PlayerId,
    /// The object whose ability is activated.
    pub(crate) source: ObjectId,
    /// What cannot be picked.
    pub(crate) barred: &'a [ObjectId],
}

impl Payer<'_> {
    /// The objects among `from` that match `filter`, are not barred and that `keep` lets through.
    fn matching(
        &self,
        from: impl Iterator<Item = ObjectId>,
        filter: &Selector,
        keep: impl Fn(ObjectId) -> bool,
    ) -> Vec<ObjectId> {
        let Some(source) = self.state.object(self.source) else {
            return Vec::new();
        };
        let scope = Scope::announcing(self.state, self.player, source.reference());
        from.filter(|id| !self.barred.contains(id) && keep(*id))
            .filter(|id| {
                self.state.object(*id).is_some_and(|object| {
                    scope.selects(filter, object.reference()).unwrap_or(false)
                })
            })
            .collect()
    }
}

impl Pick {
    /// Whether the part removes a counter of any kind, whose kind the player picks for each permanent.
    pub(crate) fn w2f_any_kind(&self) -> bool {
        self.kind == Kind::Counter && c11c_counter_picks::c11c_kind(&self.filter).is_none()
    }

    /// The objects the player may pick for this part.
    pub(crate) fn candidates(&self, payer: &Payer<'_>) -> Vec<ObjectId> {
        match self.kind {
            Kind::Sacrifice => sacrifice::candidates(payer, &self.filter),
            Kind::Discard => discard::candidates(payer, &self.filter),
            Kind::Exile => exile::candidates(payer, &self.filter),
            Kind::ExileHand => exile_hand::candidates(payer, &self.filter),
            Kind::Reveal => reveal::candidates(payer, &self.filter),
            Kind::Tap | Kind::TapPower => tap::candidates(payer, &self.filter),
            Kind::Return => bounce::candidates(payer, &self.filter),
            Kind::Counter => c11c_counter_picks::c11c_candidates(payer, &self.filter),
        }
    }

    /// Whether `candidates` cannot pay the part: too few of them, or too little power among them.
    pub(crate) fn short(&self, state: &GameState, candidates: &[ObjectId]) -> bool {
        match self.kind {
            Kind::TapPower => usize::try_from(tap::total_power(state, candidates))
                .is_ok_and(|total| total < self.count),
            _ => candidates.len() < self.count,
        }
    }

    /// What the part takes without a question when the player has nothing to choose between: all of `candidates`
    /// when they are exactly as many as it takes, the one candidate for a pick by power, and nothing for 0.
    pub(crate) fn unasked(&self, candidates: &[ObjectId]) -> Option<Vec<ObjectId>> {
        let alone = match self.kind {
            Kind::TapPower => candidates.len() == 1,
            _ => candidates.len() == self.count,
        };
        if alone {
            Some(candidates.to_vec())
        } else if self.count == 0 {
            Some(Vec::new())
        } else {
            None
        }
    }

    /// The fewest and the most objects the question takes from `offered` candidates.
    pub(crate) fn bounds(&self, offered: usize) -> (u32, u32) {
        if self.kind == Kind::TapPower {
            return (1, u32::try_from(offered).unwrap_or(u32::MAX));
        }
        let count = u32::try_from(self.count).unwrap_or(u32::MAX);
        (count, count)
    }

    /// Whether `picked` are too weak for the part, and the power they and the part have: a pick by power only.
    pub(crate) fn weak(&self, state: &GameState, picked: &[ObjectId]) -> Option<(u32, u32)> {
        if self.kind != Kind::TapPower {
            return None;
        }
        let got = tap::total_power(state, picked);
        let needed = u32::try_from(self.count).unwrap_or(u32::MAX);
        (got < needed).then_some((needed, got))
    }

    /// What the question asks of the player, in the words of a card.
    pub(crate) fn text(&self) -> String {
        match self.kind {
            Kind::Sacrifice => crate::wording::sacrifice(&self.filter, self.count),
            Kind::Discard => crate::wording::discard(self.count),
            Kind::Exile => exile::text(&self.filter, self.count),
            Kind::ExileHand => exile_hand::text(&self.filter, self.count),
            Kind::Reveal => reveal::text(&self.filter, self.count),
            Kind::Tap => tap::text(&self.filter, self.count),
            Kind::TapPower => tap::power_text(&self.filter, self.count),
            Kind::Return => bounce::text(&self.filter, self.count),
            Kind::Counter => c11c_counter_picks::c11c_text(&self.filter, self.count),
        }
    }

    /// The event that pays this part with `object`, picked for it.
    fn event(
        &self,
        state: &GameState,
        player: PlayerId,
        object: ObjectId,
        kinds: &mut std::slice::Iter<'_, CounterKind>,
    ) -> Option<Event> {
        match self.kind {
            Kind::Sacrifice => sacrifice::event(state, object),
            Kind::Discard => Some(discard::event(player, object)),
            Kind::Exile => exile::event(state, object),
            Kind::ExileHand => exile_hand::event(state, object),
            Kind::Reveal => Some(reveal::event(player, object)),
            Kind::Tap | Kind::TapPower => Some(tap::event(object)),
            Kind::Return => bounce::event(state, object),
            Kind::Counter => c11c_counter_picks::c11c_event(state, &self.filter, object, kinds),
        }
    }
}

#[cfg(test)]
mod tests;
