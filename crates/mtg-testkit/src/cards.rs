//! Synthetic cards for tests: basic lands and vanilla creatures with chosen costs and stats.

use std::sync::Arc;

use mtg_core::card::{CardDefinition, CardType};
use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaSymbol};

/// The basic land that taps for `color`.
pub fn land(color: Color) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(Some(color)))
}

/// A Forest.
pub fn forest() -> Arc<CardDefinition> {
    land(Color::Green)
}

/// A Plains.
pub fn plains() -> Arc<CardDefinition> {
    land(Color::White)
}

/// An Island.
pub fn island() -> Arc<CardDefinition> {
    land(Color::Blue)
}

/// A Swamp.
pub fn swamp() -> Arc<CardDefinition> {
    land(Color::Black)
}

/// A Mountain.
pub fn mountain() -> Arc<CardDefinition> {
    land(Color::Red)
}

/// Wastes, which taps for colorless mana.
pub fn wastes() -> Arc<CardDefinition> {
    Arc::new(CardDefinition::basic_land(None))
}

/// A mana cost of `generic` plus one colored symbol per entry of `colored`.
pub fn cost(generic: u32, colored: &[Color]) -> ManaCost {
    ManaCost::from_symbols(
        std::iter::once(ManaSymbol::Generic(generic))
            .chain(colored.iter().copied().map(ManaSymbol::Colored)),
    )
}

/// A creature with no abilities.
pub fn creature(
    name: &str,
    generic: u32,
    colored: &[Color],
    power: i32,
    toughness: i32,
) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(generic, colored))
            .with_power_toughness(power, toughness),
    )
}

/// A creature that costs nothing, handy when a test is about combat rather than mana.
pub fn free_creature(name: &str, power: i32, toughness: i32) -> Arc<CardDefinition> {
    creature(name, 0, &[], power, toughness)
}

/// A 2/2 for {1}{G}.
pub fn bear() -> Arc<CardDefinition> {
    creature("Grizzly Bears", 1, &[Color::Green], 2, 2)
}

/// A 1/1 for {G}.
pub fn elf() -> Arc<CardDefinition> {
    creature("Llanowar Elf", 0, &[Color::Green], 1, 1)
}

/// A 4/4 for {3}{G}{G}.
pub fn giant() -> Arc<CardDefinition> {
    creature("Craw Wurm", 3, &[Color::Green, Color::Green], 4, 4)
}

/// An instant with no effect, used to test instant-speed timing.
pub fn instant(name: &str, generic: u32, colored: &[Color]) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(generic, colored)),
    )
}

#[cfg(test)]
mod tests {
    //! Goal: synthetic cards have the costs, colors and types the test tables assume.
    use super::*;
    use mtg_core::mana::ManaType;
    use rstest::rstest;

    #[rstest]
    #[case(forest(), ManaType::Green)]
    #[case(plains(), ManaType::White)]
    #[case(island(), ManaType::Blue)]
    #[case(swamp(), ManaType::Black)]
    #[case(mountain(), ManaType::Red)]
    #[case(wastes(), ManaType::Colorless)]
    fn lands_tap_for_their_color(#[case] card: Arc<CardDefinition>, #[case] mana: ManaType) {
        assert_eq!(
            card.abilities,
            vec![mtg_core::card::AbilityDef::Mana(
                mtg_core::card::ManaAbility { produces: mana }
            )]
        );
    }

    #[rstest]
    #[case(bear(), 2, 2, 2)]
    #[case(elf(), 1, 1, 1)]
    #[case(giant(), 5, 4, 4)]
    #[case(free_creature("X", 3, 1), 0, 3, 1)]
    fn creatures_report_value_and_stats(
        #[case] card: Arc<CardDefinition>,
        #[case] value: u32,
        #[case] power: i32,
        #[case] toughness: i32,
    ) {
        assert_eq!(card.mana_value(), value);
        assert_eq!((card.power, card.toughness), (Some(power), Some(toughness)));
        assert!(card.is_type(CardType::Creature));
    }
}
