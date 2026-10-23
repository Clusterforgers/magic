//! The optional costs paid in addition to the cost (CR 601.2b, 118.8, 702.27, 702.33, 702.42): kicker, multikicker,
//! buyback, entwine and "As an additional cost to cast this spell, you may ...".
//!
//! A cost that may be paid once is offered with the others in `ChooseCosts`; one that may be paid any number of times
//! (multikicker) is asked afterwards with `ChooseNumber`. Only what the player can pay for with the rest of the total
//! cost is offered: the mana, the life and the objects the parts take. The costs chosen are kept in
//! `Choices.optional_costs`, a repeatable cost once for each payment, their mana joins the total cost, their parts are
//! picked and paid with the others (`parts.rs`), and the spell, the permanent it becomes and their abilities remember
//! them. A spell with entwine asks its costs before its modes, since paying entwine chooses them all.

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::value::CastWay;
use mtg_core::mana::ManaCost;

use super::{Ctx, Next, What, Work, targets, total};
use crate::ability_reader;
use crate::choose::{
    Choices, CostsOptions, NumberOptions, OptionalCost, every_mode_is_legal, modal_of, options_for,
};
use crate::decision::DecisionKind;
use crate::keywords::{self, OptionalKind, gift};
use crate::object::{GameObject, ObjectRef};
use crate::parts::Part;
use crate::price::Price;
use crate::state::GameState;

/// The most times a cost is offered to be paid: more mana than any game has.
const MOST_TIMES: usize = 100;

/// A cost the player may pay in addition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Optional {
    /// The cost as the card prints it.
    pub(crate) label: String,
    /// The mana it adds to the total cost.
    pub(crate) mana: ManaCost,
    /// The other parts it pays: life and the objects the player picks.
    pub(crate) parts: Vec<Part>,
    /// Whether it may be paid any number of times, and not only once.
    pub(crate) repeatable: bool,
    /// What paying it does.
    pub(crate) kind: OptionalKind,
    /// Whether the engine pays it. A cost with a part it does not pay stays in the list, so the indices that name a cost
    /// are the same wherever they are read, and is never offered.
    pub(crate) offerable: bool,
}

impl Optional {
    /// A kicker-like cost of mana alone.
    #[cfg(test)]
    pub(crate) fn mana_only(label: &str, mana: &str, repeatable: bool) -> Self {
        Optional {
            label: label.to_owned(),
            mana: ManaCost::parse(mana).unwrap(),
            parts: Vec::new(),
            repeatable,
            kind: OptionalKind::Kicker,
            offerable: true,
        }
    }
}

/// One optional cost a card prints, before the engine reads what it pays.
struct Printed {
    kind: OptionalKind,
    name: &'static str,
    cost: Cost,
    repeatable: bool,
}

/// The optional costs a card prints, in the order it prints them: the keywords that give one and the abilities that say
/// "As an additional cost to cast this spell, you may ...".
fn printed(card: &CardDefinition) -> impl Iterator<Item = Printed> + '_ {
    ability_reader::printed_rules(card).filter_map(|rule| match &**rule {
        Ability::Keyword(keyword) => {
            let extra = keywords::rules(keyword).additional_cost(keyword)?;
            Some(Printed {
                kind: extra.kind,
                name: extra.name,
                cost: extra.cost,
                repeatable: extra.repeatable,
            })
        }
        Ability::AdditionalCost {
            cost,
            required: false,
        } => Some(Printed {
            kind: OptionalKind::Additional,
            name: "",
            cost: cost.clone(),
            repeatable: false,
        }),
        _ => None,
    })
}

/// What `printed` costs: the mana, the parts and whether the engine pays them as a spell is announced.
fn read_cost(printed: Printed) -> Optional {
    let Printed {
        kind,
        name,
        cost,
        repeatable,
    } = printed;
    let none = |label: String| Optional {
        label,
        mana: ManaCost::ZERO,
        parts: Vec::new(),
        repeatable,
        kind,
        offerable: false,
    };
    // A promise costs nothing and is always offered; who gets the gift is chosen with it (`offered_once`).
    if kind == OptionalKind::Gift {
        return Optional {
            offerable: true,
            ..none(name.to_owned())
        };
    }
    let Ok(price) = Price::announced(&cost) else {
        return none(name.to_owned());
    };
    let mana = price.mana.clone().unwrap_or_default();
    let mut words: Vec<String> = Vec::new();
    if price.mana.is_some() {
        words.push(mana.to_string());
    }
    words.extend(price.parts.iter().map(Part::text));
    let label = if name.is_empty() {
        words.join(", ")
    } else {
        format!("{name} {}", words.join(", "))
    };
    // A part that needs a source to lose, tap or hold counters, or an X, is not paid for a spell; a repeated cost has no
    // picks to ask for each payment.
    let payable = !price.tap
        && !price.moves_source()
        && price.x == 0
        && (price.mana.is_some() || !price.parts.is_empty())
        && price
            .parts
            .iter()
            .all(|part| !part.is_counted() && !matches!(part, Part::Counters { .. }))
        && (!repeatable || price.parts.is_empty());
    Optional {
        label,
        mana,
        parts: price.parts,
        repeatable,
        kind,
        offerable: payable,
    }
}

/// The optional costs of a card, by index.
pub(crate) fn listed(card: &CardDefinition) -> Vec<Optional> {
    printed(card).map(read_cost).collect()
}

/// How many times the optional costs of `kind` were paid among those `paid` names, by index into the costs of `card`.
pub(crate) fn payments(card: &CardDefinition, paid: &[usize], kind: OptionalKind) -> usize {
    if paid.is_empty() {
        return 0;
    }
    let kinds: Vec<OptionalKind> = printed(card).map(|cost| cost.kind).collect();
    // A number from `gift::SEAT_BASE` up is a gift promised to a seat.
    let kind_of = |index: usize| {
        if index >= gift::SEAT_BASE {
            Some(OptionalKind::Gift)
        } else {
            kinds.get(index).copied()
        }
    };
    paid.iter()
        .filter(|index| kind_of(**index) == Some(kind))
        .count()
}

/// The optional costs of a spell, in the order the card prints them.
pub(super) fn read(state: &GameState, what: &What) -> Vec<Optional> {
    let What::Spell(spell) = what else {
        return Vec::new();
    };
    state
        .object(*spell)
        .map(|spell| listed(spell.card()))
        .unwrap_or_default()
}

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    if work.done.optional {
        return None;
    }
    // An ability is not cast: it remembers what was paid for its source.
    if let What::Ability(_) | What::Trigger(_) = ctx.what {
        work.choices.optional_costs = paid_for(ctx);
        work.choices.way = cast_for(ctx);
        work.done.optional = true;
        return None;
    }
    if work.done.repeated.is_none() {
        let offered = offered_once(ctx, work.choices);
        if offered.is_empty() {
            work.done.repeated = Some(0);
        } else {
            return Some(ctx.ask(DecisionKind::ChooseCosts(CostsOptions {
                source: ctx.source,
                optional: offered,
                can_cancel: ctx.can_cancel(),
            })));
        }
    }
    let mut settled = work.done.repeated.unwrap_or(0);
    loop {
        work.done.repeated = Some(settled);
        let Some(index) = ctx.needs.repeatable().nth(settled) else {
            work.done.optional = true;
            return None;
        };
        let most = most_times(ctx, work.choices, index);
        if most == 0 {
            settled += 1;
            continue;
        }
        let label = ctx.needs.optional.get(index).map_or("", |cost| &cost.label);
        return Some(ctx.ask(DecisionKind::ChooseNumber(NumberOptions {
            source: ctx.source,
            min: 0,
            max: u32::try_from(most).ok(),
            except: Vec::new(),
            text: format!("Choose how many times to pay {label}."),
            can_cancel: ctx.can_cancel(),
        })));
    }
}

/// Whether the spell has entwine, which is paid before its modes are chosen.
pub(super) fn entwines(ctx: &Ctx<'_>) -> bool {
    ctx.needs
        .optional
        .iter()
        .any(|cost| cost.offerable && cost.kind == OptionalKind::Entwine)
}

/// Whether the player paid entwine, so every mode is chosen.
pub(super) fn entwined(ctx: &Ctx<'_>, choices: &Choices) -> bool {
    choices.optional_costs.iter().any(|index| {
        ctx.needs
            .optional
            .get(*index)
            .is_some_and(|cost| cost.kind == OptionalKind::Entwine)
    })
}

/// `mana` with the mana of the optional costs chosen added, a cost once for each time it was paid.
pub(super) fn add(ctx: &Ctx<'_>, choices: &Choices, mana: Option<ManaCost>) -> Option<ManaCost> {
    let paid: Vec<&ManaCost> = choices
        .optional_costs
        .iter()
        .filter_map(|index| ctx.needs.optional.get(*index))
        .map(|cost| &cost.mana)
        .collect();
    if paid.is_empty() {
        return mana;
    }
    let symbols = mana
        .iter()
        .chain(paid)
        .flat_map(|cost| cost.symbols().iter().copied());
    Some(ManaCost::from_symbols(symbols))
}

/// The costs that may be paid once and that the player can pay for with the rest of the total cost, by index.
fn offered_once(ctx: &Ctx<'_>, choices: &Choices) -> Vec<OptionalCost> {
    let mut offered = Vec::new();
    for (index, cost) in ctx.needs.optional.iter().enumerate() {
        if !cost.offerable || cost.repeatable {
            continue;
        }
        if cost.kind == OptionalKind::Gift {
            offered.extend(gifts(ctx, choices, cost));
        } else if (cost.kind != OptionalKind::Entwine || every_mode_legal(ctx))
            && affordable(ctx, choices, &[index])
        {
            offered.push(OptionalCost {
                index,
                label: cost.label.clone(),
            });
        }
    }
    offered
}

/// The promise of a gift to each opponent still in the game, as the spell could still be cast with it: the targets the
/// spell needs with the gift promised must exist. It is offered as `gift::promise(opponent)`; one promise at most is
/// chosen (`choose::costs`).
fn gifts(ctx: &Ctx<'_>, choices: &Choices, cost: &Optional) -> Vec<OptionalCost> {
    if choices
        .optional_costs
        .iter()
        .any(|paid| *paid >= gift::SEAT_BASE)
    {
        return Vec::new();
    }
    ctx.state
        .turn_order_from(ctx.player)
        .into_iter()
        .filter(|opponent| *opponent != ctx.player)
        .filter(|opponent| {
            let mut trial = choices.clone();
            trial.optional_costs.push(gift::promise(*opponent));
            targets_remain(ctx, &trial)
        })
        .map(|opponent| OptionalCost {
            index: gift::promise(opponent),
            label: format!("{} to player {}", cost.label, opponent.index() + 1),
        })
        .collect()
}

/// Whether every target specification of the spell can still be met with the costs `choices` pays.
fn targets_remain(ctx: &Ctx<'_>, choices: &Choices) -> bool {
    targets::specs(ctx, choices)
        .iter()
        .enumerate()
        .all(|(index, spec)| {
            options_for(
                ctx.state,
                ctx.source,
                ctx.player,
                index,
                spec,
                false,
                ctx.cause(),
                choices.x,
                &choices.optional_costs,
                choices.way,
            )
            .is_some()
        })
}

/// Whether the spell is modal and every one of its modes has the targets it needs, which entwine asks for.
fn every_mode_legal(ctx: &Ctx<'_>) -> bool {
    ctx.ability.and_then(modal_of).is_some_and(|modal| {
        every_mode_is_legal(ctx.state, ctx.source, ctx.player, &modal, ctx.cause())
    })
}

/// The most times the player can pay the repeatable cost `index` on top of what is chosen.
fn most_times(ctx: &Ctx<'_>, choices: &Choices, index: usize) -> usize {
    (1..=MOST_TIMES)
        .take_while(|times| affordable(ctx, choices, &vec![index; *times]))
        .count()
}

/// Whether the total cost with `more` optional costs paid on top of `choices` can be paid.
fn affordable(ctx: &Ctx<'_>, choices: &Choices, more: &[usize]) -> bool {
    let mut trial = choices.clone();
    trial.optional_costs.extend_from_slice(more);
    total::of(ctx, &trial).is_some_and(|total| total::affordable(ctx, &total))
}

/// The source of an ability being announced.
fn source_of(ctx: &Ctx<'_>) -> Option<ObjectRef> {
    match ctx.what {
        What::Trigger(waiting) => Some(waiting.trigger.source),
        What::Ability(ability) => ctx.state.object(ability.object).map(GameObject::reference),
        What::Spell(_) | What::Attack(_) => None,
    }
}

/// The optional costs paid for the source of an ability, which the permanent remembers from its spell. A triggered
/// ability remembers them from the snapshot taken as it triggered, so it still reads them after its source has left.
fn paid_for(ctx: &Ctx<'_>) -> Vec<usize> {
    if let What::Trigger(waiting) = ctx.what
        && !waiting.trigger.snapshot.paid.is_empty()
    {
        return waiting.trigger.snapshot.paid.clone();
    }
    source_of(ctx).map_or_else(Vec::new, |source| paid_by(ctx.state, source))
}

/// The alternative cost the source of an ability was cast for, which the permanent remembers from its spell.
fn cast_for(ctx: &Ctx<'_>) -> Option<CastWay> {
    let source = source_of(ctx)?;
    ctx.state
        .object(source.id)
        .filter(|object| object.counter() == source.counter)
        .and_then(GameObject::cast_way)
}

/// The optional costs paid as `source` was cast, while it is the object that was named.
fn paid_by(state: &GameState, source: ObjectRef) -> Vec<usize> {
    state
        .object(source.id)
        .filter(|object| object.counter() == source.counter)
        .map(|object| object.paid_costs().to_vec())
        .unwrap_or_default()
}

/// The parts the paid optional costs of a spell pay, in the order of the costs.
pub(super) fn parts_paid(optional: &[Optional], choices: &Choices) -> Vec<Part> {
    let mut paid: Vec<usize> = choices.optional_costs.clone();
    paid.dedup();
    paid.iter()
        .filter_map(|index| optional.get(*index))
        .flat_map(|cost| cost.parts.iter().cloned())
        .collect()
}
