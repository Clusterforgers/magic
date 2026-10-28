//! Cost increases and reductions (CR 601.2f, 118.7): what the cost modifiers in play do to the cost of a spell, after
//! X and the additional costs are added.
//!
//! The modifiers are the `Ability::CostModifier`s of the spell itself that name it (a card that is not in play has no
//! other static ability) and of the permanents of every player, read through the ability reader so a permanent that
//! lost its abilities lost them. Each one that applies to the spell
//! changes the cost by its mana as many times as its `per` counts, and the increases are added before the reductions
//! are taken away: a reduction of generic mana never goes below zero and one of colored mana removes only a symbol of
//! that color. Hybrid and Phyrexian symbols stay as they are.

use std::borrow::Cow;
use std::sync::Arc;

use mtg_cards::ability::{Ability, CostChange};
use mtg_cards::mana::ManaCostX;
use mtg_cards::refs::{Pred, TargetSpec};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::{ManaCost, ManaSymbol};

use super::{Ctx, What, targets};
use crate::ability_reader::{abilities_of, definition_has_cost_modifier};
use crate::characteristics::controller_of_object;
use crate::choose::Choices;
use crate::object::{ObjectKind, ObjectRef};
use crate::resolve::{CastTargets, Scope};
use crate::state::GameState;
use crate::zones::Zone;

/// The most times one modifier changes a cost: a count this large is more mana than any game has.
const MOST_TIMES: i64 = 100;

/// `mana` after every cost modifier that applies to the spell has changed it; an ability costs what it costs.
pub(super) fn apply(ctx: &Ctx<'_>, choices: &Choices, mana: Option<ManaCost>) -> Option<ManaCost> {
    let What::Spell(spell) = ctx.what else {
        return mana;
    };
    if !ctx.state.possible.costs {
        return mana;
    }
    let specs = targets::specs(ctx, choices);
    modified(ctx.state, ctx.player, *spell, &specs, choices, mana, false)
}

/// The mana cost `player` pays for `spell` before anything is announced, at least: the printed cost with the
/// modifiers that need no announcement applied, which the priority decision judges against the mana they have.
pub(crate) fn cheapest<'a>(
    state: &GameState,
    player: PlayerId,
    spell: ObjectId,
    printed: &'a ManaCost,
) -> Cow<'a, ManaCost> {
    if !state.possible.costs {
        return Cow::Borrowed(printed);
    }
    let cost = modified(
        state,
        player,
        spell,
        &[],
        &Choices::default(),
        Some(printed.clone()),
        true,
    );
    Cow::Owned(cost.unwrap_or(ManaCost::ZERO))
}

/// A cost modifier in play.
struct Modifier {
    /// The object whose ability it is.
    source: ObjectRef,
    /// Who controls it: the player "you" names.
    controller: PlayerId,
    ability: Arc<Ability>,
}

/// The cost modifiers that may apply to `spell`, cast by `caster`: its own, and those of the permanents of the
/// players still in the game.
fn gathered(state: &GameState, spell: ObjectId, caster: PlayerId) -> Vec<Modifier> {
    let mut found = Vec::new();
    for id in std::iter::once(spell).chain(state.zone(Zone::Battlefield).iter().copied()) {
        let Some(object) = state
            .object(id)
            .filter(|object| object.kind() != ObjectKind::Ability)
        else {
            continue;
        };
        if !definition_has_cost_modifier(object.card()) {
            continue;
        }
        let controller = if id == spell {
            caster
        } else {
            controller_of_object(state, object)
        };
        if !state.is_alive(controller) {
            continue;
        }
        for (_, ability) in abilities_of(state, id) {
            // A card that is not in play has no static ability but the ones that name the spell it is (CR 604.3).
            let works = match &*ability {
                Ability::CostModifier { applies_to, .. } => {
                    id != spell || applies_to.0.contains(&Pred::This)
                }
                _ => false,
            };
            if works {
                found.push(Modifier {
                    source: object.reference(),
                    controller,
                    ability,
                });
            }
        }
    }
    found
}

/// `mana` with the cost modifiers of the game applied to `spell`, which `player` casts with `choices` announced: the
/// increases first, then the reductions (CR 601.2f). A spell with no mana cost has one only when something increases
/// it; no modifier makes the cost less than zero.
pub(crate) fn modified(
    state: &GameState,
    player: PlayerId,
    spell: ObjectId,
    specs: &[TargetSpec],
    choices: &Choices,
    mana: Option<ManaCost>,
    judging: bool,
) -> Option<ManaCost> {
    let Some(target) = state
        .object(spell)
        .map(crate::object::GameObject::reference)
    else {
        return mana;
    };
    let announced = Choices::default();
    let (mut more, mut less) = (ManaCost::ZERO, ManaCost::ZERO);
    for modifier in gathered(state, spell, player) {
        let Ability::CostModifier {
            applies_to,
            change,
            mana: unit,
            per,
            condition,
        } = &*modifier.ability
        else {
            continue;
        };
        // What the spell announced is what its own modifiers read ("for each target beyond the first"); another
        // object's modifier reads the game as it is.
        let own = modifier.source.id == spell;
        let scope = Scope {
            casting: Some((spell, player)),
            cast_targets: Some(CastTargets { choices, judging }),
            choices: if own { choices } else { &announced },
            specs: if own {
                specs.iter().collect()
            } else {
                Vec::new()
            },
            ..Scope::announcing(state, modifier.controller, modifier.source)
        };
        if !scope.selects(applies_to, target).unwrap_or(false)
            || condition
                .as_ref()
                .is_some_and(|condition| !scope.holds(condition).unwrap_or(false))
        {
            continue;
        }
        let times = u32::try_from(scope.amount(per).unwrap_or(0).clamp(0, MOST_TIMES)).unwrap_or(0);
        let part = repeated(unit, times);
        match change {
            CostChange::Increase => more = joined(more, &part),
            CostChange::Reduce => less = joined(less, &part),
        }
    }
    // The commander tax is an increase of the cost like any other (CR 903.8).
    let tax = crate::commanders::tax_for(state, spell);
    if tax > 0 {
        more = joined(
            more,
            &ManaCost::from_symbols(vec![ManaSymbol::Generic(tax)]),
        );
    }
    let mana = if more.symbols().is_empty() {
        mana
    } else {
        Some(joined(mana.unwrap_or(ManaCost::ZERO), &more))
    };
    match mana {
        Some(cost) if !less.symbols().is_empty() => Some(reduced(&cost, &less)),
        other => other,
    }
}

/// The mana of `unit` `times` over; an `{X}` in it has no value here and is left out.
fn repeated(unit: &ManaCostX, times: u32) -> ManaCost {
    (0..times).fold(ManaCost::ZERO, |all, _| joined(all, unit.rest()))
}

/// `cost` with every symbol of `more` added.
fn joined(cost: ManaCost, more: &ManaCost) -> ManaCost {
    more.symbols()
        .iter()
        .fold(cost, |cost, symbol| cost.with(*symbol))
}

/// `cost` less `less`: generic mana comes off the generic part, down to nothing, and a colored or colorless symbol
/// takes one such symbol from the cost, if it has one. Whatever finds nothing to reduce is lost.
pub(super) fn reduced(cost: &ManaCost, less: &ManaCost) -> ManaCost {
    let mut generic = cost.generic();
    let mut rest: Vec<ManaSymbol> = cost
        .symbols()
        .iter()
        .copied()
        .filter(|symbol| !matches!(symbol, ManaSymbol::Generic(_)))
        .collect();
    for symbol in less.symbols() {
        match symbol {
            ManaSymbol::Generic(n) => generic = generic.saturating_sub(*n),
            ManaSymbol::Colored(_) | ManaSymbol::Colorless => {
                if let Some(at) = rest.iter().position(|have| have == symbol) {
                    rest.remove(at);
                }
            }
            _ => {}
        }
    }
    ManaCost::from_symbols(rest).with(ManaSymbol::Generic(generic))
}

#[cfg(test)]
mod tests {
    //! Goal: a cost reduction takes generic mana from the generic part down to nothing and a colored symbol from a
    //! matching symbol only, an increase adds its mana beside every kind of symbol, and a modifier's mana is repeated
    //! as often as it counts.
    use rstest::rstest;

    use super::*;

    fn cost(text: &str) -> ManaCost {
        ManaCost::parse(text).unwrap()
    }

    #[rstest]
    #[case::one_generic("{2}{R}", "{1}", "{1}{R}")]
    #[case::all_generic("{2}{R}", "{2}", "{R}")]
    #[case::floor_at_zero_generic("{2}{R}", "{5}", "{R}")]
    #[case::nothing_generic_to_take("{R}{R}", "{1}", "{R}{R}")]
    #[case::to_nothing("{2}", "{2}", "")]
    #[case::the_color_it_names("{1}{R}{G}", "{R}", "{1}{G}")]
    #[case::a_color_the_cost_lacks("{2}{G}", "{R}", "{2}{G}")]
    #[case::only_one_symbol_of_a_color("{R}{R}", "{R}", "{R}")]
    #[case::colorless("{1}{C}", "{C}", "{1}")]
    #[case::generic_and_color("{3}{R}", "{1}{R}", "{2}")]
    #[case::hybrid_stays("{2}{W/U}", "{2}", "{W/U}")]
    #[case::hybrid_not_matched_by_color("{W/U}", "{W}", "{W/U}")]
    #[case::phyrexian_stays("{1}{W/P}", "{1}", "{W/P}")]
    #[case::phyrexian_not_matched_by_color("{1}{W/P}", "{W}", "{1}{W/P}")]
    #[case::mono_hybrid_stays("{2/W}{1}", "{1}", "{2/W}")]
    fn a_reduction_takes_what_it_names(
        #[case] before: &str,
        #[case] less: &str,
        #[case] after: &str,
    ) {
        assert_eq!(reduced(&cost(before), &cost(less)), cost(after));
    }

    #[rstest]
    #[case::none("{1}", 0, "")]
    #[case::once("{1}", 1, "{1}")]
    #[case::three_times("{1}", 3, "{3}")]
    #[case::a_color_three_times("{R}", 3, "{R}{R}{R}")]
    #[case::mixed_twice("{1}{U}", 2, "{2}{U}{U}")]
    fn a_modifier_changes_the_cost_as_often_as_it_counts(
        #[case] unit: &str,
        #[case] times: u32,
        #[case] expected: &str,
    ) {
        let unit: ManaCostX = unit.parse().unwrap();
        assert_eq!(repeated(&unit, times), cost(expected));
    }

    #[rstest]
    #[case::generic("{R}", "{1}", "{1}{R}")]
    #[case::beside_generic("{2}{R}", "{1}", "{3}{R}")]
    #[case::a_color("{2}{R}", "{U}", "{2}{U}{R}")]
    #[case::beside_hybrid("{W/U}", "{2}", "{2}{W/U}")]
    #[case::beside_phyrexian("{1}{G/P}", "{1}", "{2}{G/P}")]
    #[case::onto_nothing("", "{1}", "{1}")]
    fn an_increase_adds_its_mana(#[case] before: &str, #[case] more: &str, #[case] after: &str) {
        assert_eq!(joined(cost(before), &cost(more)), cost(after));
    }
}
