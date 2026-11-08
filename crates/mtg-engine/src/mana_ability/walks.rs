//! Goal: the mana options walk the battlefield for cost modifiers once for each permanent with a mana ability, and not at all in a game without a cost modifier, so offering mana stays linear in the permanents however many tokens without mana abilities there are.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use rstest::rstest;

use super::*;
use crate::activation_kind::WALKS;
use crate::fixtures::state;

/// An artifact with no abilities: untapped and able to tap, but with no mana to offer.
fn trinket(n: usize) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(format!("Trinket {n}")).with_type(CardType::Artifact))
}

#[rstest]
fn the_walks_for_cost_modifiers_count_the_mana_sources_only(
    #[values(0, 1, 50)] trinkets: usize,
    #[values(1, 3)] lands: usize,
    #[values(false, true)] costs: bool,
) {
    let mut permanents: Vec<Arc<CardDefinition>> = (0..lands)
        .map(|_| Arc::new(CardDefinition::basic_land(Some(Color::Green))))
        .collect();
    permanents.extend((0..trinkets).map(trinket));
    let mut game = state(vec![vec![], vec![]], vec![permanents, vec![]]);
    game.possible.costs = costs;
    WALKS.with(|walks| walks.set(0));
    let offered = options(&game, PlayerId(0));
    assert_eq!(offered.len(), lands, "each land offers its mana");
    let walked = WALKS.with(std::cell::Cell::get);
    assert_eq!(walked, if costs { lands } else { 0 });
}
