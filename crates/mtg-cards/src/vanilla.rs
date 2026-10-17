//! Plain cards with no abilities, built from data: basic lands and vanilla creatures.

use std::sync::Arc;

use mtg_core::card::{CardDefinition, CardType};
use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaSymbol};

/// The basic land that taps for `color`.
pub fn basic_land(color: Color) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(Some(color)))
}

/// A creature with no abilities that costs `generic` plus one symbol per entry of `colored`.
pub fn creature(
    name: &str,
    generic: u32,
    colored: &[Color],
    power: i32,
    toughness: i32,
) -> Arc<CardDefinition> {
    let symbols = std::iter::once(ManaSymbol::Generic(generic))
        .chain(colored.iter().copied().map(ManaSymbol::Colored));
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::from_symbols(symbols))
            .with_power_toughness(power, toughness),
    )
}

#[cfg(test)]
mod tests {
    //! Goal: vanilla creatures carry exactly the cost, stats and type they were built from, and lands are basic.
    use super::*;
    use mtg_core::card::Supertype;
    use rstest::rstest;

    #[rstest]
    #[case("Grizzly Bears", 1, &[Color::Green], 2, 2, "{1}{G}")]
    #[case("Savannah Lions", 0, &[Color::White], 2, 1, "{W}")]
    #[case("Craw Wurm", 4, &[Color::Green, Color::Green], 6, 4, "{4}{G}{G}")]
    #[case("Free Spirit", 0, &[], 3, 3, "")]
    fn creatures_match_their_data(
        #[case] name: &str,
        #[case] generic: u32,
        #[case] colored: &[Color],
        #[case] power: i32,
        #[case] toughness: i32,
        #[case] cost: &str,
    ) {
        let card = creature(name, generic, colored, power, toughness);
        assert_eq!(card.name, name);
        assert!(card.is_type(CardType::Creature));
        assert_eq!(card.abilities.len(), 0);
        assert_eq!((card.power, card.toughness), (Some(power), Some(toughness)));
        assert_eq!(card.mana_cost, Some(ManaCost::parse(cost).unwrap()));
    }

    #[rstest]
    #[case(Color::White, "Plains")]
    #[case(Color::Blue, "Island")]
    #[case(Color::Black, "Swamp")]
    #[case(Color::Red, "Mountain")]
    #[case(Color::Green, "Forest")]
    fn lands_are_basic(#[case] color: Color, #[case] name: &str) {
        let land = basic_land(color);
        assert_eq!(land.name, name);
        assert!(land.supertypes.contains(&Supertype::Basic));
        assert!(land.is_type(CardType::Land));
    }
}
