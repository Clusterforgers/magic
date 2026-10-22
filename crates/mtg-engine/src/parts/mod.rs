//! The parts of an ability's cost beyond `{T}`, mana and losing the source: life, counters removed from the source, and
//! the permanents and cards the player picks (CR 118.3, 601.2b, 601.2h).
//!
//! A cost is read into [`Part`]s as the ability is announced. The picks are asked then, with `ChooseCards`
//! (`announce/parts.rs`), and every part is paid with its events as the ability goes on the stack (`cast.rs`).

mod bounce;
mod counters;
mod discard;
mod energy;
mod exile;
mod exile_hand;
mod life;
mod sacrifice;
mod tap;

pub(crate) use counters::after_removal;

use mtg_cards::cost::CostItem;
use mtg_cards::refs::Selector;
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::events::Event;
use crate::resolve::Scope;
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
    /// An untapped permanent the payer controls is tapped.
    Tap,
    /// A permanent the payer controls returns to its owner's hand.
    Return,
}

/// A part of the cost that takes objects the player picks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pick {
    /// What is taken.
    pub(crate) kind: Kind,
    /// Which objects may be picked.
    pub(crate) filter: Selector,
    /// How many.
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
        CostItem::RemoveCounters { .. } => "Cost.RemoveCounters.Counted",
        CostItem::Sacrifice { .. } => "Cost.Sacrifice.Counted",
        CostItem::Discard { .. } => "Cost.Discard.Counted",
        CostItem::ExileFromGraveyard { .. } => "Cost.ExileFromGraveyard.Counted",
        CostItem::ExileFromHand { .. } => "Cost.ExileFromHand.Counted",
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
            CostItem::RemoveCounters { counter, count } => {
                let (number, x) = amount(count, item)?;
                let counter = counter.clone();
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
            CostItem::TapPermanents { filter, count } => pick(Kind::Tap, filter, count, item)?,
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
            Part::Counters { .. } => "Cost.RemoveCounters",
            Part::X(inner) => inner.name(),
            Part::Pick(pick) => match pick.kind {
                Kind::Sacrifice => "Cost.Sacrifice",
                Kind::Discard => "Cost.Discard",
                Kind::Exile => "Cost.ExileFromGraveyard",
                Kind::ExileHand => "Cost.ExileFromHand",
                Kind::Tap => "Cost.TapPermanents",
                Kind::Return => "Cost.ReturnToHand",
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
            Part::Pick(pick) => matches!(pick.kind, Kind::Tap),
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
            Part::Counters { counter, count } => {
                counters::payable(payer, counter, *count).then(Vec::new)
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
        match self {
            Part::Life(amount) => life::events(player, *amount),
            Part::Energy(amount) => energy::events(player, *amount),
            Part::Counters { counter, count } => counters::events(state, source, counter, *count),
            Part::Pick(pick) => picked
                .iter()
                .filter_map(|object| pick.event(state, player, *object))
                .collect(),
            Part::X(inner) => inner.events(state, player, source, picked),
        }
    }
}

/// The events that pay `parts` for `source` in the order of the cost, with the objects picked for the parts that take
/// some (`picks`, one list for each such part); `None` when a pick is missing.
pub(crate) fn pay(
    state: &GameState,
    player: PlayerId,
    source: ObjectId,
    parts: &[Part],
    picks: &[Vec<ObjectId>],
) -> Option<Vec<Event>> {
    let mut picks = picks.iter();
    let mut events = Vec::new();
    for part in parts {
        let picked = if matches!(part, Part::Pick(_)) {
            picks.next()?.as_slice()
        } else {
            &[]
        };
        events.extend(part.events(state, player, source, picked));
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
    /// The objects the player may pick for this part.
    pub(crate) fn candidates(&self, payer: &Payer<'_>) -> Vec<ObjectId> {
        match self.kind {
            Kind::Sacrifice => sacrifice::candidates(payer, &self.filter),
            Kind::Discard => discard::candidates(payer, &self.filter),
            Kind::Exile => exile::candidates(payer, &self.filter),
            Kind::ExileHand => exile_hand::candidates(payer, &self.filter),
            Kind::Tap => tap::candidates(payer, &self.filter),
            Kind::Return => bounce::candidates(payer, &self.filter),
        }
    }

    /// What the question asks of the player, in the words of a card.
    pub(crate) fn text(&self) -> String {
        match self.kind {
            Kind::Sacrifice => crate::wording::sacrifice(&self.filter, self.count),
            Kind::Discard => crate::wording::discard(self.count),
            Kind::Exile => exile::text(&self.filter, self.count),
            Kind::ExileHand => exile_hand::text(&self.filter, self.count),
            Kind::Tap => tap::text(&self.filter, self.count),
            Kind::Return => bounce::text(&self.filter, self.count),
        }
    }

    /// The event that pays this part with `object`, picked for it.
    fn event(&self, state: &GameState, player: PlayerId, object: ObjectId) -> Option<Event> {
        match self.kind {
            Kind::Sacrifice => sacrifice::event(state, object),
            Kind::Discard => Some(discard::event(player, object)),
            Kind::Exile => exile::event(state, object),
            Kind::ExileHand => exile_hand::event(state, object),
            Kind::Tap => Some(tap::event(object)),
            Kind::Return => bounce::event(state, object),
        }
    }
}

#[cfg(test)]
mod tests;
