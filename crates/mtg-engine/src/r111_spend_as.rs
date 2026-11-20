//! "You may spend white mana as though it were red mana." and "You may spend white mana as though it were mana of any
//! color. You may spend other mana only as though it were colorless mana." (`GameRule::R111SpendColorAs`, CR 609.4b):
//! the rules in play say which types of mana pay each colored symbol and `{C}` for a player, a table the payment
//! carries (`Spend::r111_pays`). Each plain way to pay a cost (`ManaCost::resolutions`) becomes every way the table
//! allows, each symbol asked as the type of mana that pays it, so the payment functions match mana to symbols as before.

use std::collections::HashSet;

use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::{ManaCost, ManaSymbol, ManaType, Resolution};

use crate::ability_reader::abilities_of;
use crate::characteristics::controller_of_object;
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// For each colored symbol and `{C}` (by `ManaType::index`), the types of mana that pay it, one bit per type.
pub type R111Pays = [u8; 6];

/// The table where each symbol is paid by its own type only.
const R111_PLAIN: R111Pays = [1, 2, 4, 8, 16, 32];

/// The bit of a type of mana.
const fn bit(mana: ManaType) -> u8 {
    1 << mana.index()
}

/// The table the rules in play give `player`, or `None` when every symbol is paid by its own type only.
pub(crate) fn r111_pays(state: &GameState, player: PlayerId) -> Option<R111Pays> {
    r112_pays_for(state, player, None)
}

/// The table the rules in play give `player` paying the activation cost of an ability of `ability_of`: a rule that
/// holds only for its source's abilities (`r112_own_abilities`, CR 609.4b) counts when that source is the holder.
pub(crate) fn r112_pays_for(
    state: &GameState,
    player: PlayerId,
    ability_of: Option<mtg_core::ids::ObjectId>,
) -> Option<R111Pays> {
    let mut rules = Vec::new();
    for id in state.zone(Zone::Battlefield).iter().copied() {
        let Some(holder) = state.object(id) else {
            continue;
        };
        for (_, ability) in abilities_of(state, id) {
            let Ability::Rule {
                rule:
                    GameRule::R111SpendColorAs {
                        who,
                        mana,
                        as_color,
                        others_colorless,
                        r112_own_abilities,
                    },
                condition,
                ..
            } = &*ability
            else {
                continue;
            };
            if *r112_own_abilities && ability_of != Some(id) {
                continue;
            }
            let scope = Scope::announcing(
                state,
                controller_of_object(state, holder),
                holder.reference(),
            );
            if condition
                .as_ref()
                .is_some_and(|held| !scope.holds(held).unwrap_or(false))
                || !scope
                    .players(who)
                    .is_ok_and(|players| players.contains(&player))
            {
                continue;
            }
            rules.push((*mana, *as_color, *others_colorless));
        }
    }
    (!rules.is_empty()).then(|| r111_table(&rules))
}

/// The table of the rules `(mana, as_color, others_colorless)`: the restrictions first, then what each rule allows, so
/// the color a rule names keeps what it may pay.
pub(crate) fn r111_table(rules: &[(Color, Option<Color>, bool)]) -> R111Pays {
    let mut pays = R111_PLAIN;
    let colorless = ManaType::Colorless.index();
    for &(mana, _, others) in rules {
        if others {
            let own = bit(ManaType::from(mana));
            for (at, slot) in pays.iter_mut().enumerate() {
                if at == colorless {
                    // Any mana but the named color pays {C} as though it were colorless.
                    *slot |= !own & 0b11_1111;
                } else {
                    *slot &= own;
                }
            }
        }
    }
    for &(mana, as_color, _) in rules {
        let own = bit(ManaType::from(mana));
        match as_color {
            Some(color) => pays[ManaType::from(color).index()] |= own,
            None => {
                for slot in &mut pays[..colorless] {
                    *slot |= own;
                }
            }
        }
    }
    pays
}

/// Every way to pay `cost` when `pays` says which mana pays each symbol: each plain way to pay it with every colored
/// symbol and `{C}` asked as a type of mana that pays it, the symbol's own type first; no repeats.
pub fn r111_resolutions(cost: &ManaCost, pays: Option<&R111Pays>) -> Vec<Resolution> {
    let plain = cost.resolutions();
    let Some(pays) = pays.filter(|pays| **pays != R111_PLAIN) else {
        return plain;
    };
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for resolution in plain {
        let mut ways: Vec<Vec<ManaSymbol>> = vec![Vec::new()];
        for symbol in resolution.cost.symbols() {
            let asked: Vec<ManaSymbol> = match *symbol {
                ManaSymbol::Colored(color) => as_symbols(pays[ManaType::from(color).index()]),
                ManaSymbol::Colorless => as_symbols(pays[ManaType::Colorless.index()]),
                other => vec![other],
            };
            ways = ways
                .into_iter()
                .flat_map(|way| {
                    asked.iter().map(move |symbol| {
                        let mut next = way.clone();
                        next.push(*symbol);
                        next
                    })
                })
                .collect();
        }
        for way in ways {
            let cost = ManaCost::from_symbols(way);
            if seen.insert((cost.clone(), resolution.life)) {
                found.push(Resolution {
                    cost,
                    life: resolution.life,
                });
            }
        }
    }
    found
}

/// The symbols that ask for each type of mana of `mask`, in WUBRG then colorless order.
fn as_symbols(mask: u8) -> Vec<ManaSymbol> {
    ManaType::ALL
        .into_iter()
        .filter(|mana| mask & bit(*mana) != 0)
        .map(|mana| match mana {
            ManaType::White => ManaSymbol::Colored(Color::White),
            ManaType::Blue => ManaSymbol::Colored(Color::Blue),
            ManaType::Black => ManaSymbol::Colored(Color::Black),
            ManaType::Red => ManaSymbol::Colored(Color::Red),
            ManaType::Green => ManaSymbol::Colored(Color::Green),
            ManaType::Colorless => ManaSymbol::Colorless,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: the table follows each rule (one color as another, as any color, other mana only as colorless) and every
    //! symbol of a cost, hybrid, mono-hybrid and Phyrexian ones included, is paid by exactly the mana the table allows.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use rstest::rstest;

    fn ways(cost: &str, rules: &[(Color, Option<Color>, bool)]) -> Vec<String> {
        let table = r111_table(rules);
        let mut found: Vec<String> =
            r111_resolutions(&ManaCost::parse(cost).unwrap(), Some(&table))
                .into_iter()
                .map(|r| format!("{}+{}", r.cost, r.life))
                .collect();
        found.sort();
        found
    }

    const SUNGLASSES: (Color, Option<Color>, bool) = (Color::White, Some(Color::Red), false);
    const DAWN: (Color, Option<Color>, bool) = (Color::White, None, true);

    #[rstest]
    #[case::red("{1}{R}", &[SUNGLASSES], &["{1}{R}+0", "{1}{W}+0"])]
    #[case::not_green("{G}", &[SUNGLASSES], &["{G}+0"])]
    #[case::red_hybrid("{R/G}", &[SUNGLASSES], &["{G}+0", "{R}+0", "{W}+0"])]
    #[case::mono_hybrid("{2/R}", &[SUNGLASSES], &["{2}+0", "{R}+0", "{W}+0"])]
    #[case::phyrexian("{R/P}", &[SUNGLASSES], &["{R}+0", "{W}+0", "{0}+2"])]
    #[case::two_red("{R}{R}", &[SUNGLASSES], &["{R}{R}+0", "{W}{R}+0", "{W}{W}+0"])]
    #[case::dawn_colored("{1}{U}{B}", &[DAWN], &["{1}{W}{W}+0"])]
    #[case::dawn_colorless("{C}", &[DAWN], &["{B}+0", "{C}+0", "{G}+0", "{R}+0", "{U}+0"])]
    #[case::dawn_generic("{3}", &[DAWN], &["{3}+0"])]
    #[case::both("{R}", &[DAWN, SUNGLASSES], &["{W}+0"])]
    fn r111_symbols_are_paid_by_the_mana_the_rules_allow(
        #[case] cost: &str,
        #[case] rules: &[(Color, Option<Color>, bool)],
        #[case] expected: &[&str],
    ) {
        let mut expected: Vec<String> = expected.iter().map(|s| (*s).to_owned()).collect();
        expected.sort();
        assert_eq!(ways(cost, rules), expected);
    }

    #[test]
    fn r111_no_rule_leaves_the_resolutions() {
        let cost = ManaCost::parse("{2}{W/U}{B/P}").unwrap();
        assert_eq!(r111_resolutions(&cost, None), cost.resolutions());
        assert_eq!(
            r111_resolutions(&cost, Some(&R111_PLAIN)),
            cost.resolutions()
        );
    }
}
