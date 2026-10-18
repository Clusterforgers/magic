//! Synthetic cards for tests: basic lands and vanilla creatures with chosen costs and stats.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
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

/// A creature whose cost is made of exactly these symbols, hybrid and Phyrexian ones included.
pub fn creature_costing(
    name: &str,
    symbols: &[ManaSymbol],
    power: i32,
    toughness: i32,
) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::from_symbols(symbols.iter().copied()))
            .with_power_toughness(power, toughness),
    )
}

/// A 2/2 for `{G/W}{G/W}`: two-colour hybrid.
pub fn hybrid_bear() -> Arc<CardDefinition> {
    let hybrid = ManaSymbol::Hybrid(Color::Green, Color::White);
    creature_costing("Hybrid Bear", &[hybrid, hybrid], 2, 2)
}

/// A 2/2 for `{2/G}`: monocolored hybrid.
pub fn mono_hybrid_bear() -> Arc<CardDefinition> {
    creature_costing(
        "Mono Hybrid Bear",
        &[ManaSymbol::MonoHybrid(Color::Green)],
        2,
        2,
    )
}

/// A 2/2 for `{1}{G/P}`: Phyrexian.
pub fn phyrexian_bear() -> Arc<CardDefinition> {
    creature_costing(
        "Phyrexian Bear",
        &[ManaSymbol::Generic(1), ManaSymbol::Phyrexian(Color::Green)],
        2,
        2,
    )
}

/// A 3/3 for `{1}{B/P}{B/P}`: two Phyrexian symbols and a generic one, like Dismember.
pub fn double_phyrexian_ogre() -> Arc<CardDefinition> {
    let phyrexian = ManaSymbol::Phyrexian(Color::Black);
    creature_costing(
        "Double Phyrexian Ogre",
        &[ManaSymbol::Generic(1), phyrexian, phyrexian],
        3,
        3,
    )
}

/// A 1/1 for `{G/U/P}`: hybrid Phyrexian.
pub fn hybrid_phyrexian_elf() -> Arc<CardDefinition> {
    creature_costing(
        "Hybrid Phyrexian Elf",
        &[ManaSymbol::HybridPhyrexian(Color::Green, Color::Blue)],
        1,
        1,
    )
}

/// A 1/1 for `{C/G}`: colorless hybrid.
pub fn colorless_hybrid_elf() -> Arc<CardDefinition> {
    creature_costing(
        "Colorless Hybrid Elf",
        &[ManaSymbol::ColorlessHybrid(Color::Green)],
        1,
        1,
    )
}

/// A 1/1 for `{C/P}`: colorless Phyrexian.
pub fn colorless_phyrexian_elf() -> Arc<CardDefinition> {
    creature_costing(
        "Colorless Phyrexian Elf",
        &[ManaSymbol::ColorlessPhyrexian],
        1,
        1,
    )
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
            vec![mtg_cards::AbilityDef::Mana(mtg_cards::ManaAbility {
                produces: mana
            })]
        );
    }

    #[rstest]
    #[case(hybrid_bear(), "{G/W}{G/W}", 2, "GW")]
    #[case(mono_hybrid_bear(), "{2/G}", 2, "G")]
    #[case(phyrexian_bear(), "{1}{G/P}", 2, "G")]
    #[case(double_phyrexian_ogre(), "{1}{B/P}{B/P}", 3, "B")]
    #[case(hybrid_phyrexian_elf(), "{G/U/P}", 1, "GU")]
    #[case(colorless_hybrid_elf(), "{C/G}", 1, "G")]
    #[case(colorless_phyrexian_elf(), "{C/P}", 1, "")]
    fn symbol_creatures_cost_what_their_names_say(
        #[case] card: Arc<CardDefinition>,
        #[case] cost: &str,
        #[case] value: u32,
        #[case] colors: &str,
    ) {
        assert_eq!(card.mana_cost, Some(ManaCost::parse(cost).unwrap()));
        assert_eq!(card.mana_value(), value);
        let expected: mtg_core::color::ColorSet =
            colors.chars().filter_map(Color::from_symbol).collect();
        assert_eq!(card.colors(), expected);
        assert!(card.is_type(CardType::Creature));
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
