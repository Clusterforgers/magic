//! "Spells cost an additional "Sacrifice a Swamp" to cast for each black mana symbol in their mana costs." and
//! "Activated abilities of nontoken Rebels cost an additional "Sacrifice a land" to activate." (`GameRule::R110SacrificeTax`):
//! the static abilities on the battlefield add sacrifices to the total cost of the spells and activated abilities they
//! name (CR 601.2f, 602.2b), paid by the caster or activator with the other parts of the cost (CR 601.2h).

use mtg_cards::ability::Ability;
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::modify::GameRule;
use mtg_cards::refs::Selector;
use mtg_cards::value::Amount;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;

use crate::ability_reader::abilities_of;
use crate::characteristics::controller_of_object;
use crate::parts::Part;
use crate::price::Price;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// What pays the tax: a spell being cast, or an activated ability of `source` whose activation cost holds `mana`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum R110Taxed<'a> {
    /// The spell.
    Spell(ObjectId),
    /// The source of the ability and the mana of its activation cost.
    Ability(ObjectId, Option<&'a ManaCost>),
}

/// The sacrifices the rules in play add to the cost of `taxed`, as parts of the cost: one part for each
/// rule that applies, of as many permanents as its count says.
pub(crate) fn r110_parts(state: &GameState, taxed: R110Taxed<'_>) -> Vec<Part> {
    let (object, printed) = match taxed {
        R110Taxed::Spell(spell) => (
            spell,
            state
                .object(spell)
                .and_then(|card| card.mana_cost().cloned()),
        ),
        R110Taxed::Ability(source, mana) => (source, mana.cloned()),
    };
    let Some(target) = state
        .object(object)
        .map(crate::object::GameObject::reference)
    else {
        return Vec::new();
    };
    let mut parts = Vec::new();
    for id in state.zone(Zone::Battlefield).iter().copied() {
        let Some(holder) = state.object(id) else {
            continue;
        };
        let controller = controller_of_object(state, holder);
        for (_, ability) in abilities_of(state, id) {
            let Ability::Rule {
                rule:
                    GameRule::R110SacrificeTax {
                        spells,
                        abilities,
                        sacrifice,
                        per_symbol,
                    },
                condition,
                ..
            } = &*ability
            else {
                continue;
            };
            let named = match taxed {
                R110Taxed::Spell(_) => spells.as_ref(),
                R110Taxed::Ability(..) => abilities.as_ref(),
            };
            let scope = Scope::announcing(state, controller, holder.reference());
            if !named.is_some_and(|named| scope.selects(named, target).unwrap_or(false))
                || condition
                    .as_ref()
                    .is_some_and(|held| !scope.holds(held).unwrap_or(false))
            {
                continue;
            }
            let count = per_symbol.map_or(1, |color| r110_symbols(printed.as_ref(), color));
            parts.extend(r110_sacrifices(sacrifice, count));
        }
    }
    parts
}

/// How many mana symbols of `color` the cost holds: a hybrid or Phyrexian symbol of the color counts (CR 107.4e).
pub(crate) fn r110_symbols(mana: Option<&ManaCost>, color: Color) -> u32 {
    mana.map_or(0, |mana| {
        let held = mana
            .symbols()
            .iter()
            .filter(|symbol| symbol.colors().contains(color))
            .count();
        u32::try_from(held).unwrap_or(u32::MAX)
    })
}

/// The sacrifices the rules in play add to the activation of an ability of `source` whose cost is `cost`.
pub(crate) fn r110_ability_parts(state: &GameState, source: ObjectId, cost: &Cost) -> Vec<Part> {
    let mana = cost.0.iter().find_map(|item| match item {
        CostItem::Mana(mana) => Some(mana.rest()),
        _ => None,
    });
    r110_parts(state, R110Taxed::Ability(source, mana))
}

/// The part of a cost that sacrifices `count` permanents `filter` picks; none for no permanent.
fn r110_sacrifices(filter: &Selector, count: u32) -> Vec<Part> {
    if count == 0 {
        return Vec::new();
    }
    let cost = Cost(vec![CostItem::Sacrifice {
        filter: filter.clone(),
        count: Amount::Fixed(i32::try_from(count).unwrap_or(i32::MAX)),
    }]);
    Price::announced(&cost)
        .map(|price| price.parts)
        .unwrap_or_default()
}
