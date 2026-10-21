//! The optional costs paid in addition to the cost (CR 601.2b, 118.8, 702.33): kicker and multikicker.
//!
//! A cost that may be paid once is offered with the others in `ChooseCosts`; one that may be paid any number of times
//! (multikicker) is asked afterwards with `ChooseNumber`. Only what the player can pay for with the rest of the total
//! cost is offered. The costs chosen are kept in `Choices.optional_costs`, a repeatable cost once for each payment,
//! their mana joins the total cost, and the spell, the permanent it becomes and their abilities remember them.

use mtg_cards::value::CastWay;
use mtg_core::mana::ManaCost;

use super::{Ctx, Next, What, Work, total};
use crate::choose::{Choices, CostsOptions, NumberOptions, OptionalCost};
use crate::decision::DecisionKind;
use crate::keywords::{self, keywords_of};
use crate::object::{GameObject, ObjectRef};
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
    /// Whether it may be paid any number of times, and not only once.
    pub(crate) repeatable: bool,
}

/// The optional costs the keywords of a spell give it, in the order the card prints them. A cost with a part other
/// than mana is not offered: nothing pays it yet.
pub(super) fn read(state: &GameState, what: &What) -> Vec<Optional> {
    let What::Spell(spell) = what else {
        return Vec::new();
    };
    keywords_of(state, *spell)
        .into_iter()
        .filter_map(|keyword| {
            let extra = keywords::rules(keyword).additional_cost(keyword)?;
            let price = Price::of(&extra.cost).ok()?;
            if price.tap || price.moves_source() {
                return None;
            }
            let mana = price.mana?;
            Some(Optional {
                label: format!("{} {mana}", extra.name),
                mana,
                repeatable: extra.repeatable,
            })
        })
        .collect()
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
    ctx.needs
        .optional
        .iter()
        .enumerate()
        .filter(|(index, cost)| !cost.repeatable && affordable(ctx, choices, &[*index]))
        .map(|(index, cost)| OptionalCost {
            index,
            label: cost.label.clone(),
        })
        .collect()
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
        What::Spell(_) => None,
    }
}

/// The optional costs paid for the source of an ability, which the permanent remembers from its spell.
fn paid_for(ctx: &Ctx<'_>) -> Vec<usize> {
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
