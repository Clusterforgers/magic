//! The parts of an ability's cost beyond `{T}`, mana and losing the source: life, counters removed from the source, and
//! the permanents and cards the player picks (CR 118.3, 601.2b, 601.2h).
//!
//! A cost is read into [`Part`]s as the ability is announced. The picks are asked then, with `ChooseCards`
//! (`announce/parts.rs`), and every part is paid with its events as the ability goes on the stack (`cast.rs`).

mod among;
mod behold;
mod blight;
mod bounce;
mod c11c_counter_picks;
mod counters;
mod damage;
mod discard;
mod draw;
pub(crate) mod energy;
mod evidence;
mod exile;
mod exile_hand;
mod exile_top;
mod exiled;
pub(crate) mod forage;
mod life;
pub(crate) mod mill;
mod r300_random_shots;
mod r303c_sacrifice_all;
mod random;
mod rmt5_top_shots;
pub(crate) use r300_random_shots::r300_random_shots;
pub(crate) use rmt5_top_shots::rmt5_top_shots;
mod r5518_alike;
mod reveal;
mod sacrifice;
mod source;
mod tap;
mod to_library;
mod unattach;
mod untap;

pub(crate) use counters::after_removal;

use mtg_cards::cost::{Alike, CostItem};
use mtg_cards::refs::{Pred, Selector, ZoneKind};
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::error::IllegalAnswer;
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
    /// A tapped permanent the payer controls is untapped.
    Untap,
    /// A permanent the payer controls returns to its owner's hand.
    Return,
    /// A permanent the payer controls is exiled.
    ExilePermanent,
    /// A permanent the payer controls loses a counter of the kind its filter names.
    Counter,
    /// A card in any player's graveyard is exiled.
    ExileAny,
    /// Cards in the payer's graveyard are exiled, with total mana value at least `Pick.count` (CR 701.59).
    Evidence,
    /// A creature the payer controls gets that many -1/-1 counters.
    Blight(u32),
    /// A permanent the payer controls is chosen, or a card in their hand is revealed; then exiled when the flag is set.
    Behold(bool),
    /// Cards of a zone of the payer go on top of their library, or at the bottom of it when the flag is set.
    ToLibrary(ZoneKind, bool),
    /// One Food the payer controls is sacrificed, or three cards of their graveyard are exiled.
    Forage,
    /// Permanents the payer controls and/or cards in their graveyard are exiled (craft, CR 702.167).
    Craft,
    /// A card in exile, of any owner, goes to its owner's graveyard.
    ExiledToGraveyard,
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
    /// The filter reads the X announced ("a red card with mana value X"): the part waits for X, then picks by it.
    pub(crate) reads_x: bool,
    /// What the picked cards have in common as a set ("with the same name", "that share a color").
    pub(crate) alike: Option<Alike>,
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
    /// Damage the source deals to the payer.
    Damage(u32),
    /// Each other player gains this much life.
    OthersGainLife(u32),
    /// The source stops being attached.
    Unattach,
    /// The payer discards every card in their hand ("Discard your hand"): nothing to choose, an empty hand pays it.
    DiscardHand,
    /// The payer reveals every card in their hand ("reveal your hand"): nothing to choose, an empty hand pays it.
    RevealHand,
    /// The payer sacrifices every permanent they control that matches ("sacrifice all permanents you control"): nothing
    /// to choose, none pays it.
    SacrificeAll(Selector),
    /// Cards exiled from the top of the payer's library.
    ExileTop(u32),
    /// Counters put on the source.
    PutCounters {
        /// The kind of counter.
        counter: CounterKind,
        /// How many.
        count: u32,
    },
    /// The source is exerted (CR 701.43).
    Exert,
    /// Counters removed in all from among the permanents the payer controls that match.
    Among {
        /// The kind of counter; any kind when absent.
        counter: Option<CounterKind>,
        /// The permanents the counters come from.
        filter: Selector,
        /// How many counters in all.
        count: u32,
    },
    /// Energy counters paid (CR 107.14).
    Energy(u32),
    /// Cards milled from the top of the payer's library (CR 701.17).
    Mill(u32),
    /// Cards drawn (CR 121).
    Draw(u32),
    /// Cards discarded from the payer's hand at random (CR 701.9b).
    DiscardRandom(u32),
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
        CostItem::Draw(_) => "Cost.Draw.Counted",
        CostItem::DiscardRandom(_) => "Cost.DiscardRandom.Counted",
        CostItem::DamageToYou(_) => "Cost.DamageToYou.Counted",
        CostItem::ExileTop(_) => "Cost.ExileTop.Counted",
        CostItem::Blight(_) => "Cost.Blight.Counted",
        CostItem::Behold { .. } => "Cost.Behold.Counted",
        CostItem::ExiledToGraveyard { .. } => "Cost.ExiledToGraveyard.Counted",
        CostItem::ToLibrary { .. } => "Cost.ToLibrary.Counted",
        CostItem::RemoveCounters { .. } => "Cost.RemoveCounters.Counted",
        CostItem::PutCounters { .. } => "Cost.PutCounters.Counted",
        CostItem::UntapPermanents { .. } => "Cost.UntapPermanents.Counted",
        CostItem::RemoveCountersAmong { .. } => "Cost.RemoveCountersAmong.Counted",
        CostItem::Sacrifice { .. } => "Cost.Sacrifice.Counted",
        CostItem::Discard { .. } => "Cost.Discard.Counted",
        CostItem::ExileFromGraveyard { .. } => "Cost.ExileFromGraveyard.Counted",
        CostItem::ExileFromHand { .. } => "Cost.ExileFromHand.Counted",
        CostItem::Craft { .. } => "Cost.Craft.Counted",
        CostItem::RevealFromHand { .. } => "Cost.RevealFromHand.Counted",
        CostItem::TapPermanents { .. } => "Cost.TapPermanents.Counted",
        CostItem::ReturnToHand { .. } => "Cost.ReturnToHand.Counted",
        CostItem::ExilePermanents { .. } => "Cost.ExilePermanents.Counted",
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

/// A behold, exiling what it beholds when the cost says so.
fn behold_pick(item: &CostItem) -> Result<(Part, bool), &'static str> {
    let CostItem::Behold {
        filter,
        count,
        exile,
    } = item
    else {
        return Err("Cost.Behold");
    };
    pick(Kind::Behold(*exile), filter, count, item)
}

fn pick(
    kind: Kind,
    filter: &Selector,
    count: &Amount,
    item: &CostItem,
) -> Result<(Part, bool), &'static str> {
    let (count, x) = amount(count, item)?;
    let count = usize::try_from(count).unwrap_or(usize::MAX);
    let reads_x = !x && filter.r301_reads_x();
    Ok((
        Part::Pick(Pick {
            kind,
            filter: filter.clone(),
            count,
            reads_x,
            alike: None,
        }),
        x || reads_x,
    ))
}

/// The parts of the cost items beyond the common picks: damage, unattaching, exiling the top, evidence, blight, behold,
/// a move into the library and forage.
#[allow(clippy::too_many_lines)]
fn read_more(item: &CostItem) -> Result<Option<(Part, bool)>, &'static str> {
    Ok(Some(match item {
        CostItem::DamageToYou(count) => {
            let (number, x) = amount(count, item)?;
            (Part::Damage(number), x)
        }
        CostItem::ExileTop(count) => {
            let (number, x) = amount(count, item)?;
            (Part::ExileTop(number), x)
        }
        CostItem::Draw(count) => {
            let (number, x) = amount(count, item)?;
            (Part::Draw(number), x)
        }
        CostItem::DiscardRandom(count) => {
            let (number, x) = amount(count, item)?;
            (Part::DiscardRandom(number), x)
        }
        CostItem::Unattach => (Part::Unattach, false),
        CostItem::OthersGainLife(amount) => (Part::OthersGainLife(*amount), false),
        CostItem::TapPower { filter, power } => {
            let count = usize::try_from(*power).unwrap_or(usize::MAX);
            let kind = Kind::TapPower;
            let filter = filter.clone();
            (
                Part::Pick(Pick {
                    kind,
                    filter,
                    count,
                    reads_x: false,
                    alike: None,
                }),
                false,
            )
        }
        CostItem::CollectEvidence(value) => {
            let count = usize::try_from(*value).unwrap_or(usize::MAX);
            let filter = Selector::default();
            let kind = Kind::Evidence;
            (
                Part::Pick(Pick {
                    kind,
                    filter,
                    count,
                    reads_x: false,
                    alike: None,
                }),
                false,
            )
        }
        CostItem::Blight(count) => {
            let (number, x) = amount(count, item)?;
            if x {
                return Err(counted(item));
            }
            let filter = Selector(vec![Pred::Type(CardType::Creature)]);
            let kind = Kind::Blight(number);
            (
                Part::Pick(Pick {
                    kind,
                    filter,
                    count: 1,
                    reads_x: false,
                    alike: None,
                }),
                false,
            )
        }
        CostItem::Behold { .. } => behold_pick(item)?,
        CostItem::ExiledToGraveyard { filter, count } => {
            pick(Kind::ExiledToGraveyard, filter, count, item)?
        }
        CostItem::ToLibrary {
            from,
            filter,
            count,
            bottom,
        } => {
            if !to_library::supported(*from) {
                return Err("Cost.ToLibrary.Zone");
            }
            pick(Kind::ToLibrary(*from, *bottom), filter, count, item)?
        }
        CostItem::Craft {
            filter,
            count,
            least,
        } => {
            let (mut part, x) = pick(Kind::Craft, filter, count, item)?;
            // Before X is announced the part stands for the fewest materials it may take, so a cost with fewer than
            // that is never offered (CR 702.167a).
            if x && let Part::Pick(pick) = &mut part {
                pick.count = usize::try_from(least.unwrap_or(1)).unwrap_or(1);
            }
            (part, x)
        }
        CostItem::Forage => {
            let filter = Selector::default();
            let kind = Kind::Forage;
            (
                Part::Pick(Pick {
                    kind,
                    filter,
                    count: 1,
                    reads_x: false,
                    alike: None,
                }),
                false,
            )
        }
        _ => return Ok(None),
    }))
}

impl Part {
    /// The part a cost item is, `None` for the items that are not one of these (`{T}`, mana, losing the source), or the
    /// name of the part whose amount the engine cannot count.
    pub(crate) fn read(item: &CostItem) -> Result<Option<Part>, &'static str> {
        if let Some(whole) = whole_part(item) {
            return Ok(Some(whole));
        }
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
                filter.0.push(
                    counter
                        .clone()
                        .map_or(Pred::HasAnyCounter, Pred::HasCounter),
                );
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
            CostItem::PutCounters { counter, count } => {
                let (number, x) = amount(count, item)?;
                (
                    Part::PutCounters {
                        counter: counter.clone(),
                        count: number,
                    },
                    x,
                )
            }
            CostItem::Exert => (Part::Exert, false),
            // Read off the card by the layers before any cost is paid: an unread one is a part nobody can pay.
            CostItem::OwnManaCost | CostItem::OwnManaValue => return Err(own_part(item)),
            CostItem::RemoveCountersAmong {
                counter,
                count,
                from,
            } => {
                let (number, x) = amount(count, item)?;
                let part = Part::Among {
                    counter: counter.clone(),
                    filter: from.clone(),
                    count: number,
                };
                (part, x)
            }
            CostItem::UntapPermanents { filter, count } => pick(Kind::Untap, filter, count, item)?,
            CostItem::Sacrifice { filter, count } => pick(Kind::Sacrifice, filter, count, item)?,
            CostItem::Discard { count, filter } => pick(
                Kind::Discard,
                &filter.clone().unwrap_or_default(),
                count,
                item,
            )?,
            CostItem::ExileFromGraveyard {
                filter,
                count,
                any_player,
            } => {
                let kind = if *any_player {
                    Kind::ExileAny
                } else {
                    Kind::Exile
                };
                pick(kind, filter, count, item)?
            }
            CostItem::ExileFromHand { filter, count } => {
                pick(Kind::ExileHand, filter, count, item)?
            }
            CostItem::RevealFromHand { filter, count } => pick(Kind::Reveal, filter, count, item)?,
            CostItem::AlikeFromHand {
                discard,
                filter,
                count,
                alike,
            } => {
                let kind = if *discard {
                    Kind::Discard
                } else {
                    Kind::Reveal
                };
                let (part, x) = pick(kind, filter, count, item)?;
                let part = match part {
                    Part::Pick(pick) => Part::Pick(Pick {
                        alike: Some(*alike),
                        ..pick
                    }),
                    other => other,
                };
                (part, x)
            }
            CostItem::TapPermanents { filter, count } => pick(Kind::Tap, filter, count, item)?,
            CostItem::ReturnToHand { filter, count } => pick(Kind::Return, filter, count, item)?,
            CostItem::ExilePermanents { filter, count } => {
                pick(Kind::ExilePermanent, filter, count, item)?
            }
            _ => match read_more(item)? {
                Some(found) => found,
                None => return Ok(None),
            },
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
                Part::Draw(_) => Part::Draw(x),
                Part::DiscardRandom(_) => Part::DiscardRandom(x),
                Part::Damage(_) => Part::Damage(x),
                Part::OthersGainLife(amount) => Part::OthersGainLife(*amount),
                Part::ExileTop(_) => Part::ExileTop(x),
                Part::Unattach => Part::Unattach,
                Part::DiscardHand => Part::DiscardHand,
                Part::RevealHand => Part::RevealHand,
                Part::SacrificeAll(filter) => Part::SacrificeAll(filter.clone()),
                Part::Counters { counter, .. } => Part::Counters {
                    counter: counter.clone(),
                    count: x,
                },
                Part::PutCounters { counter, .. } => Part::PutCounters {
                    counter: counter.clone(),
                    count: x,
                },
                Part::Among {
                    counter, filter, ..
                } => Part::Among {
                    counter: counter.clone(),
                    filter: filter.clone(),
                    count: x,
                },
                Part::Exert => Part::Exert,
                Part::Pick(pick) if pick.reads_x => Part::Pick(pick.r301_at(x)),
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

    /// Whether the part is life of the X announced.
    pub(crate) fn is_counted_life(&self) -> bool {
        matches!(self, Part::X(inner) if matches!(**inner, Part::Life(_)))
    }

    /// Whether the part picks by the X announced ("a red card with mana value X").
    pub(crate) fn r301_picks_by_x(&self) -> bool {
        matches!(self, Part::X(inner) if matches!(&**inner, Part::Pick(pick) if pick.reads_x))
    }

    /// Whether the part is energy of the X announced.
    pub(crate) fn is_counted_energy(&self) -> bool {
        matches!(self, Part::X(inner) if matches!(**inner, Part::Energy(_)))
    }

    /// The most X the player could pay for this part now, `None` for a part that is not counted: all their life (a player
    /// pays up to what they have, CR 119.4), the counters on the source, the energy they hold, the objects they may pick.
    pub(crate) fn most(&self, payer: &Payer<'_>) -> Option<u32> {
        let Part::X(inner) = self else { return None };
        let number = match &**inner {
            Part::Life(_) => life::most(payer),
            Part::Energy(_) => energy::most(payer),
            Part::Mill(_) => mill::most(payer),
            Part::Draw(_) => draw::most(payer),
            Part::DiscardRandom(_) => random::most(payer),
            Part::Counters { counter, .. } => counters::most(payer, counter),
            Part::PutCounters { .. } | Part::Damage(_) => u32::MAX,
            Part::Among {
                counter, filter, ..
            } => among::most(payer, filter, counter.as_ref()),
            Part::Pick(pick) if pick.reads_x => pick.r301_most(payer),
            Part::Pick(pick) => u32::try_from(pick.candidates(payer).len()).unwrap_or(u32::MAX),
            Part::ExileTop(_) => exile_top::most(payer),
            Part::Exert
            | Part::Unattach
            | Part::DiscardHand
            | Part::RevealHand
            | Part::SacrificeAll(_)
            | Part::OthersGainLife(_)
            | Part::X(_) => return None,
        };
        Some(number)
    }

    /// The name `Caps` and a refusal give the part.
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Part::Life(_) => "Cost.PayLife",
            Part::Energy(_) => "Cost.PayEnergy",
            Part::Mill(_) => "Cost.Mill",
            Part::Draw(_) => "Cost.Draw",
            Part::DiscardRandom(_) => "Cost.DiscardRandom",
            Part::Damage(_) => "Cost.DamageToYou",
            Part::OthersGainLife(_) => "Cost.OthersGainLife",
            Part::Unattach => "Cost.Unattach",
            Part::DiscardHand => "Cost.Discard",
            Part::RevealHand => "Cost.RevealFromHand",
            Part::SacrificeAll(_) => "Cost.Sacrifice",
            Part::ExileTop(_) => "Cost.ExileTop",
            Part::Counters { .. } => "Cost.RemoveCounters",
            Part::PutCounters { .. } => "Cost.PutCounters",
            Part::Exert => "Cost.Exert",
            Part::Among { .. } => "Cost.RemoveCountersAmong",
            Part::X(inner) => inner.name(),
            Part::Pick(pick) => match pick.kind {
                Kind::Sacrifice => "Cost.Sacrifice",
                Kind::Discard => "Cost.Discard",
                Kind::Exile | Kind::ExileAny => "Cost.ExileFromGraveyard",
                Kind::Evidence => "Cost.CollectEvidence",
                Kind::Blight(_) => "Cost.Blight",
                Kind::Behold(_) => "Cost.Behold",
                Kind::ToLibrary(..) => "Cost.ToLibrary",
                Kind::Forage => "Cost.Forage",
                Kind::Craft => "Cost.Craft",
                Kind::ExiledToGraveyard => "Cost.ExiledToGraveyard",
                Kind::ExileHand => "Cost.ExileFromHand",
                Kind::Reveal => "Cost.RevealFromHand",
                Kind::Tap => "Cost.TapPermanents",
                Kind::TapPower => "Cost.TapPower",
                Kind::Return => "Cost.ReturnToHand",
                Kind::ExilePermanent => "Cost.ExilePermanents",
                Kind::Untap => "Cost.UntapPermanents",
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
            Part::Damage(amount) => format!("Take {amount} damage"),
            Part::OthersGainLife(amount) => format!("Each other player gains {amount} life"),
            Part::Unattach => "Unattach this permanent".to_owned(),
            Part::DiscardHand => "Discard your hand".to_owned(),
            Part::RevealHand => "Reveal your hand".to_owned(),
            Part::SacrificeAll(_) => "Sacrifice all of them you control".to_owned(),
            Part::ExileTop(1) => "Exile the top card of your library".to_owned(),
            Part::ExileTop(amount) => format!("Exile the top {amount} cards of your library"),
            Part::Mill(1) => "Mill a card".to_owned(),
            Part::Mill(amount) => format!("Mill {amount} cards"),
            Part::Draw(1) => "Draw a card".to_owned(),
            Part::Draw(amount) => format!("Draw {amount} cards"),
            Part::DiscardRandom(1) => "Discard a card at random".to_owned(),
            Part::DiscardRandom(amount) => format!("Discard {amount} cards at random"),
            Part::Counters { counter, count } => {
                let plural = if *count == 1 { "" } else { "s" };
                format!("Remove {count} {} counter{plural}", counter.name())
            }
            Part::PutCounters { counter, count } => {
                let plural = if *count == 1 { "" } else { "s" };
                format!(
                    "Put {count} {} counter{plural} on this permanent",
                    counter.name()
                )
            }
            Part::Exert => "Exert this permanent".to_owned(),
            Part::Among { count, .. } => {
                format!("Remove {count} counters from among your permanents")
            }
            Part::Pick(pick) => pick.text().trim_end_matches('.').to_owned(),
            Part::X(inner) => match &**inner {
                Part::Life(_) => "Pay X life".to_owned(),
                Part::Energy(_) => "Pay X {E}".to_owned(),
                other => other.text(),
            },
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
            Part::Draw(amount) => draw::payable(payer, *amount).then(Vec::new),
            Part::DiscardRandom(amount) => random::payable(payer, *amount).then(Vec::new),
            Part::Damage(_) => payer.state.object(payer.source).is_some().then(Vec::new),
            Part::Unattach => unattach::payable(payer).then(Vec::new),
            Part::DiscardHand | Part::RevealHand | Part::OthersGainLife(_) => Some(Vec::new()),
            Part::SacrificeAll(filter) => r303c_sacrifice_all::r303c_all(payer, filter),
            Part::ExileTop(amount) => exile_top::payable(payer, *amount).then(Vec::new),
            Part::Pick(pick) if pick.kind == Kind::Evidence => {
                let found = pick.candidates(payer);
                (!pick.short(payer.state, &found)).then_some(found)
            }
            Part::Pick(pick) if pick.kind == Kind::Forage => {
                if let Some(food) = forage::foods(payer).first() {
                    return Some(vec![*food]);
                }
                let cards = forage::cards(payer);
                (cards.len() >= forage::CARDS)
                    .then(|| cards.into_iter().take(forage::CARDS).collect())
            }
            Part::Counters { counter, count } => {
                counters::payable(payer, counter, *count).then(Vec::new)
            }
            Part::PutCounters { .. } | Part::Exert => source::payable(payer).then(Vec::new),
            Part::Among {
                counter,
                filter,
                count,
            } => among::payable(payer, filter, counter.as_ref(), *count).then(Vec::new),
            Part::Pick(pick) if pick.kind == Kind::TapPower => {
                let power = u32::try_from(pick.count).unwrap_or(u32::MAX);
                tap::enough(payer.state, pick.candidates(payer), power, spare)
            }
            Part::Pick(pick) => {
                let mut found = pick.candidates(payer);
                found.sort_by_key(|id| spare.contains(id));
                (found.len() >= pick.count).then(|| found.into_iter().take(pick.count).collect())
            }
            // Craft takes at least the fewest materials its pick holds before X is announced.
            Part::X(inner) => match &**inner {
                Part::Pick(pick) if pick.kind == Kind::Craft => Part::Pick(pick.clone())
                    .payable(payer, spare)
                    .map(|_| Vec::new()),
                // Some X must leave the pick enough to take before X is announced.
                Part::Pick(pick) if pick.reads_x => (0..=R301_MOST_X)
                    .any(|x| Part::Pick(pick.r301_at(x)).payable(payer, spare).is_some())
                    .then(Vec::new),
                _ => Some(Vec::new()),
            },
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
            Part::Draw(amount) => draw::events(state, player, *amount),
            Part::DiscardRandom(amount) => random::events(state, player, *amount),
            Part::Damage(amount) => damage::events(state, player, source, *amount),
            Part::OthersGainLife(amount) => r286_others_gain(state, player, *amount),
            Part::Unattach => vec![unattach::event(source)],
            Part::SacrificeAll(filter) => r303c_sacrifice_all::r303c_events(
                &r303c_sacrifice_all::r303c_payer(state, player, source),
                filter,
            ),
            Part::DiscardHand => state
                .zone(crate::zones::Zone::Hand(player))
                .iter()
                .map(|object| discard::event(player, *object))
                .collect(),
            Part::RevealHand => reveal::hand_events(state, player, source),
            Part::ExileTop(amount) => exile_top::events(state, player, *amount),
            Part::Counters { counter, count } => counters::events(state, source, counter, *count),
            Part::PutCounters { counter, count } => source::put(state, source, counter, *count),
            Part::Exert => source::exert(player, source),
            Part::Among {
                counter,
                filter,
                count,
            } => among::events(state, (player, source), filter, counter.as_ref(), *count),
            Part::Pick(pick) => {
                let mut events: Vec<Event> = picked
                    .iter()
                    .flat_map(|object| pick.events(state, player, *object, kinds))
                    .collect();
                // The exile is the evidence collected (CR 701.59a); "whenever you collect evidence" waits for it.
                if pick.kind == Kind::Evidence {
                    events.push(Event::EvidenceCollected { player });
                }
                // The Food sacrificed or the cards exiled are the forage (CR 701.61a); "whenever you forage" waits for it.
                if pick.kind == Kind::Forage {
                    events.push(Event::Foraged { player });
                }
                events
            }
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
            Kind::Sacrifice
                | Kind::Discard
                | Kind::Exile
                | Kind::ExileAny
                | Kind::Evidence
                | Kind::Forage
                | Kind::Craft
                | Kind::ExiledToGraveyard
                | Kind::ExileHand
                | Kind::Reveal
                | Kind::Behold(true)
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
    /// The permanent whose static ability gave the ability being paid for (`Ref::Granter`).
    pub(crate) granter: Option<ObjectId>,
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
        let scope = Scope {
            granter: self.granter,
            ..Scope::announcing(self.state, self.player, source.reference())
        };
        from.filter(|id| !self.barred.contains(id) && keep(*id))
            .filter(|id| {
                self.state.object(*id).is_some_and(|object| {
                    scope.selects(filter, object.reference()).unwrap_or(false)
                })
            })
            .collect()
    }
}

/// The largest X a pick whose filter reads X is tried with: more than any mana value or count a game has.
const R301_MOST_X: u32 = 64;

impl Pick {
    /// The pick once X is announced as `x`: its filter reads `x` in the place of X.
    fn r301_at(&self, x: u32) -> Pick {
        Pick {
            filter: self
                .filter
                .r301_with_x(i32::try_from(x).unwrap_or(i32::MAX)),
            reads_x: false,
            alike: None,
            ..self.clone()
        }
    }

    /// The largest X that leaves the pick enough to take; 0 when none does.
    fn r301_most(&self, payer: &Payer<'_>) -> u32 {
        (0..=R301_MOST_X)
            .rev()
            .find(|x| {
                let at = self.r301_at(*x);
                !at.short(payer.state, &at.candidates(payer))
            })
            .unwrap_or(0)
    }

    /// Whether the part removes a counter of any kind, whose kind the player picks for each permanent.
    pub(crate) fn w2f_any_kind(&self) -> bool {
        self.kind == Kind::Counter && c11c_counter_picks::c11c_kind(&self.filter).is_none()
    }

    /// The objects the player may pick for this part.
    pub(crate) fn candidates(&self, payer: &Payer<'_>) -> Vec<ObjectId> {
        let mut found = match self.kind {
            Kind::Sacrifice => sacrifice::candidates(payer, &self.filter),
            Kind::Discard => discard::candidates(payer, &self.filter),
            Kind::Exile | Kind::Evidence => exile::candidates(payer, &self.filter),
            Kind::ExileHand => exile_hand::candidates(payer, &self.filter),
            Kind::Reveal => reveal::candidates(payer, &self.filter),
            Kind::Tap => tap::candidates(payer, &self.filter),
            Kind::TapPower => tap::crew_candidates(payer, &self.filter),
            Kind::Return | Kind::Blight(_) | Kind::ExilePermanent => {
                bounce::candidates(payer, &self.filter)
            }
            Kind::Untap => untap::candidates(payer, &self.filter),
            Kind::Counter => c11c_counter_picks::c11c_candidates(payer, &self.filter),
            Kind::ExileAny => exile::candidates_any(payer, &self.filter),
            Kind::Behold(_) => behold::candidates(payer, &self.filter),
            Kind::ToLibrary(from, _) => to_library::candidates(payer, from, &self.filter),
            Kind::Forage => forage::candidates(payer),
            Kind::Craft => exile::craft_candidates(payer, &self.filter),
            Kind::ExiledToGraveyard => exiled::candidates(payer, &self.filter),
        };
        // A prohibition keeps an object out of the cost it forbids (`prohibit.rs`).
        found.retain(|id| match self.kind {
            Kind::Sacrifice => crate::prohibit::may_sacrifice(payer.state, payer.player, *id),
            Kind::Discard => crate::prohibit::may_discard(payer.state, payer.player, *id),
            Kind::Exile
            | Kind::ExileHand
            | Kind::ExileAny
            | Kind::ExilePermanent
            | Kind::Craft
            | Kind::Behold(true) => crate::prohibit::may_exile(payer.state, *id),
            _ => true,
        });
        found
    }

    /// Whether `candidates` cannot pay the part: too few of them, or too little power among them.
    pub(crate) fn short(&self, state: &GameState, candidates: &[ObjectId]) -> bool {
        if let Some(alike) = self.alike {
            return !r5518_alike::r5518_has_set(state, candidates, self.count, alike);
        }
        match self.kind {
            Kind::TapPower => usize::try_from(tap::total_power(state, candidates))
                .is_ok_and(|total| total < self.count),
            Kind::Evidence => usize::try_from(evidence::total_value(state, candidates))
                .is_ok_and(|total| total < self.count),
            _ => candidates.len() < self.count,
        }
    }

    /// What the part takes without a question when the player has nothing to choose between: all of `candidates`
    /// when they are exactly as many as it takes, the one candidate for a pick by power, and nothing for 0.
    pub(crate) fn unasked(
        &self,
        state: &GameState,
        candidates: &[ObjectId],
    ) -> Option<Vec<ObjectId>> {
        let alone = match self.kind {
            Kind::TapPower => candidates.len() == 1,
            Kind::Evidence => {
                candidates.len() == 1
                    || usize::try_from(evidence::total_value(state, candidates))
                        .is_ok_and(|total| total == self.count)
            }
            Kind::Forage => forage::alone(state, candidates),
            _ => {
                candidates.len() == self.count
                    && self
                        .alike
                        .is_none_or(|alike| r5518_alike::r5518_alike(state, candidates, alike))
            }
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
        if matches!(self.kind, Kind::TapPower | Kind::Evidence) {
            return (1, u32::try_from(offered).unwrap_or(u32::MAX));
        }
        if self.kind == Kind::Forage {
            let most = u32::try_from(forage::CARDS.min(offered)).unwrap_or(u32::MAX);
            return (1, most);
        }
        let count = u32::try_from(self.count).unwrap_or(u32::MAX);
        (count, count)
    }

    /// Why `picked` cannot pay the part, when they cannot: too little power or mana value for a pick by total, or not one
    /// Food or three cards for a forage.
    pub(crate) fn weak(&self, state: &GameState, picked: &[ObjectId]) -> Option<IllegalAnswer> {
        if let Some(alike) = self.alike
            && !r5518_alike::r5518_alike(state, picked, alike)
        {
            return Some(IllegalAnswer::WrongPick(match alike {
                Alike::SameName => "cards with the same name",
                Alike::ShareColor => "cards that share a color",
            }));
        }
        let needed = u32::try_from(self.count).unwrap_or(u32::MAX);
        match self.kind {
            Kind::TapPower => {
                let got = tap::total_power(state, picked);
                (got < needed).then_some(IllegalAnswer::TooLittlePower { needed, got })
            }
            Kind::Evidence => {
                let got = evidence::total_value(state, picked);
                (got < needed).then_some(IllegalAnswer::TooLittleValue { needed, got })
            }
            Kind::Forage => (!forage::valid(state, picked)).then_some(IllegalAnswer::WrongPick(
                "one Food to sacrifice or three cards of the graveyard to exile",
            )),
            _ => None,
        }
    }

    /// What the question asks of the player, in the words of a card.
    pub(crate) fn text(&self) -> String {
        let text = self.r5518_kind_text();
        match self.alike {
            Some(alike) => format!(
                "{}{}",
                text.trim_end_matches('.'),
                r5518_alike::r5518_words(alike)
            ),
            None => text,
        }
    }

    /// What the question asks of the player for the kind of pick, before the set's words.
    fn r5518_kind_text(&self) -> String {
        match self.kind {
            Kind::Sacrifice => crate::wording::sacrifice(&self.filter, self.count),
            Kind::Discard => crate::wording::discard(self.count),
            Kind::Exile => exile::text(&self.filter, self.count),
            Kind::ExileHand => exile_hand::text(&self.filter, self.count),
            Kind::Reveal => reveal::text(&self.filter, self.count),
            Kind::Tap => tap::text(&self.filter, self.count),
            Kind::TapPower => tap::power_text(&self.filter, self.count),
            Kind::Return => bounce::text(&self.filter, self.count),
            Kind::ExilePermanent => bounce::exile_text(&self.filter, self.count),
            Kind::Untap => untap::text(&self.filter, self.count),
            Kind::Counter => c11c_counter_picks::c11c_text(&self.filter, self.count),
            Kind::ExileAny => exile::text_any(&self.filter, self.count),
            Kind::Evidence => evidence::text(self.count),
            Kind::Blight(count) => blight::text(count),
            Kind::Behold(exile) => behold::text(&self.filter, self.count, exile),
            Kind::ToLibrary(from, bottom) => {
                to_library::text(from, &self.filter, self.count, bottom)
            }
            Kind::Forage => forage::text(),
            Kind::Craft => exile::craft_text(&self.filter, self.count),
            Kind::ExiledToGraveyard => exiled::text(self.count),
        }
    }

    /// The events that pay this part with `object`, picked for it.
    fn events(
        &self,
        state: &GameState,
        player: PlayerId,
        object: ObjectId,
        kinds: &mut std::slice::Iter<'_, CounterKind>,
    ) -> Vec<Event> {
        match self.kind {
            Kind::ToLibrary(from, bottom) => to_library::events(state, from, bottom, object),
            Kind::Blight(count) => blight::events(state, object, count),
            Kind::Behold(true) => behold::exile_events(state, player, object),
            _ => self
                .event(state, player, object, kinds)
                .into_iter()
                .collect(),
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
            Kind::Exile | Kind::ExileHand | Kind::Craft | Kind::ExilePermanent => {
                exile_hand::event(state, object)
            }
            Kind::Reveal => Some(reveal::event(player, object)),
            Kind::Tap | Kind::TapPower => Some(tap::event(object)),
            Kind::Return => bounce::event(state, object),
            Kind::Untap => Some(untap::event(object)),
            Kind::Counter => c11c_counter_picks::c11c_event(state, &self.filter, object, kinds),
            Kind::ExileAny | Kind::Evidence => exile::event(state, object),
            Kind::Behold(_) => behold::event(state, player, object),
            Kind::Forage => forage::event(state, object),
            Kind::ExiledToGraveyard => exiled::event(state, object),
            Kind::Blight(_) | Kind::ToLibrary(..) => None,
        }
    }
}

#[cfg(test)]
mod tests;

/// A part that takes all of a kind the payer has, with nothing to choose: "discard your hand", "sacrifice all
/// permanents you control".
fn whole_part(item: &CostItem) -> Option<Part> {
    if discards_the_hand(item) {
        return Some(Part::DiscardHand);
    }
    if reveal::reveals_the_hand(item) {
        return Some(Part::RevealHand);
    }
    r303c_sacrifice_all::r303c_reads(item).map(Part::SacrificeAll)
}

/// "Discard your hand": a discard counted as every card of the payer's hand, with no filter.
fn discards_the_hand(item: &CostItem) -> bool {
    matches!(item, CostItem::Discard { count: Amount::Count(cards), filter: None }
        if cards.0 == [Pred::In(ZoneKind::Hand), Pred::OwnedBy(mtg_cards::refs::Ref::You)])
}

/// The name of a part of a cost read off the card by the layers: unread, nobody can pay it.
const fn own_part(item: &CostItem) -> &'static str {
    if matches!(item, CostItem::OwnManaCost) {
        "OwnManaCost"
    } else {
        "OwnManaValue"
    }
}

/// The life each other player still in the game gains as the cost is paid; one who can't gain life gains none.
fn r286_others_gain(state: &GameState, payer: PlayerId, amount: u32) -> Vec<Event> {
    if amount == 0 {
        return Vec::new();
    }
    state
        .turn_order_from(payer)
        .into_iter()
        .filter(|player| *player != payer && state.is_alive(*player))
        .filter(|player| !crate::rules::cant_gain_life(state, *player))
        .map(|player| Event::LifeGained { player, amount })
        .collect()
}
