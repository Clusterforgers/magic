//! Synthetic cards for tests: basic lands and vanilla creatures with chosen costs and stats, and small spells.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_cards::value::Amount;
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

/// A creature that costs nothing and has these keywords.
pub fn keyworded(
    name: &str,
    power: i32,
    toughness: i32,
    keywords: &[Keyword],
) -> Arc<CardDefinition> {
    with_keywords(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(power, toughness),
        keywords,
    )
}

/// A creature with a cost and these keywords.
pub fn keyworded_creature(
    name: &str,
    generic: u32,
    colored: &[Color],
    power: i32,
    toughness: i32,
    keywords: &[Keyword],
) -> Arc<CardDefinition> {
    with_keywords(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(generic, colored))
            .with_power_toughness(power, toughness),
        keywords,
    )
}

fn with_keywords(card: CardDefinition, keywords: &[Keyword]) -> Arc<CardDefinition> {
    Arc::new(keywords.iter().fold(card, |card, keyword| {
        card.with_rules(Ability::Keyword(keyword.clone()))
    }))
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

/// A target specification for exactly one target of the kind `what`.
pub fn one_target(what: Targetable) -> TargetSpec {
    TargetSpec {
        what,
        bounds: Bounds::default(),
        divide: None,
    }
}

/// An effect that deals `amount` damage to what `to` names.
pub fn damage(to: Ref, amount: i32) -> Effect {
    Effect::DealDamage {
        source: None,
        to,
        amount: Amount::Fixed(amount),
    }
}

/// An effect that makes its controller gain `amount` life.
pub fn gain_life(amount: i32) -> Effect {
    Effect::GainLife {
        who: Ref::You,
        amount: Amount::Fixed(amount),
    }
}

/// An effect that makes its controller draw `count` cards.
pub fn draw(count: i32) -> Effect {
    Effect::Draw {
        who: Ref::You,
        count: Amount::Fixed(count),
    }
}

/// An ability a player activates by tapping the permanent: `{T}: effects`.
fn tap_ability(timing: Timing, targets: Vec<TargetSpec>, effects: Vec<Effect>) -> Ability {
    Ability::Activated {
        cost: Cost(vec![CostItem::Tap]),
        timing,
        limit: None,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    }
}

/// An instant for `{R}` that deals `amount` damage to any target, like Lightning Bolt.
pub fn burn(name: &str, amount: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::AnyTarget)],
                effects: vec![damage(Ref::Target(0), amount)],
            }),
    )
}

/// An instant for `{1}{G}` with three steps: 2 damage to any target, its controller gains 2 life and draws a card.
pub fn salve(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(1, &[Color::Green]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::AnyTarget)],
                effects: vec![damage(Ref::Target(0), 2), gain_life(2), draw(1)],
            }),
    )
}

/// An instant for `{1}{R}{R}` with two target specifications: 2 damage to one target and 2 damage to another.
pub fn twin_burn(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(1, &[Color::Red, Color::Red]))
            .with_rules(Ability::Spell {
                targets: vec![
                    one_target(Targetable::AnyTarget),
                    one_target(Targetable::AnyTarget),
                ],
                effects: vec![damage(Ref::Target(0), 2), damage(Ref::Target(1), 2)],
            }),
    )
}

/// An instant for `{R}`: 1 damage to target creature its controller controls, so it is cast only with one.
pub fn burn_own_creature(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Object(Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::ControlledBy(Ref::You),
                ])))],
                effects: vec![damage(Ref::Target(0), 1)],
            }),
    )
}

/// A sorcery for `{B}{B}`: target opponent loses 2 life and its controller gains 2.
pub fn drain(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Black, Color::Black]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Opponent)],
                effects: vec![
                    Effect::LoseLife {
                        who: Ref::Target(0),
                        amount: Amount::Fixed(2),
                    },
                    gain_life(2),
                ],
            }),
    )
}

/// A sorcery for `{U}` with no target: its controller draws two cards.
pub fn study(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Blue]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![draw(2)],
            }),
    )
}

/// A sorcery for `{1}{B}`: target opponent sacrifices a creature of their choice.
pub fn edict(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(1, &[Color::Black]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Opponent)],
                effects: vec![Effect::Sacrifice {
                    who: Ref::Target(0),
                    filter: Selector(vec![Pred::Type(CardType::Creature)]),
                    count: Amount::Fixed(1),
                }],
            }),
    )
}

/// A sorcery for `{B}{B}`: target opponent discards two cards of their choice.
pub fn mind_rot(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Black, Color::Black]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Opponent)],
                effects: vec![Effect::Discard {
                    who: Ref::Target(0),
                    count: Amount::Fixed(2),
                    random: false,
                }],
            }),
    )
}

/// A planeswalker for `{generic}` with no abilities, which is a permanent a player can be made to sacrifice.
pub fn walker(name: &str, generic: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Planeswalker)
            .with_mana_cost(cost(generic, &[]))
            .with_loyalty(3),
    )
}

/// A 1/1 creature for `{generic}{R}` with "{T}: deals 1 damage to any target".
pub fn pinger(name: &str, generic: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(cost(generic, &[Color::Red]))
            .with_power_toughness(1, 1)
            .with_rules(tap_ability(
                Timing::Instant,
                vec![one_target(Targetable::AnyTarget)],
                vec![damage(Ref::Target(0), 1)],
            )),
    )
}

/// An artifact for `{generic}` with "{T}: Draw a card. Activate only as a sorcery."
pub fn library_of_cards(name: &str, generic: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(generic, &[]))
            .with_rules(tap_ability(Timing::Sorcery, Vec::new(), vec![draw(1)])),
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
    #[case::edict(edict("Edict"), CardType::Sorcery, "{1}{B}")]
    #[case::mind_rot(mind_rot("Rot"), CardType::Sorcery, "{B}{B}")]
    #[case::walker(walker("Walker", 2), CardType::Planeswalker, "{2}")]
    fn the_small_spells_have_the_types_and_costs_the_tables_assume(
        #[case] card: Arc<CardDefinition>,
        #[case] card_type: CardType,
        #[case] cost: &str,
    ) {
        assert!(card.is_type(card_type));
        assert_eq!(card.mana_cost, Some(ManaCost::parse(cost).unwrap()));
    }

    #[test]
    fn the_edict_and_the_discard_spell_aim_at_an_opponent() {
        for card in [edict("Edict"), mind_rot("Rot")] {
            let Some(Ability::Spell { targets, effects }) = card.rules.first().map(|rule| &**rule)
            else {
                panic!("a spell");
            };
            assert_eq!(targets, &[one_target(Targetable::Opponent)]);
            assert_eq!(effects.len(), 1);
        }
    }

    #[test]
    fn keyword_creatures_carry_their_keywords_in_order() {
        let free = keyworded("Free", 3, 2, &[Keyword::Flying, Keyword::Trample]);
        assert_eq!((free.power, free.toughness), (Some(3), Some(2)));
        assert_eq!(free.mana_value(), 0);
        let rules: Vec<&Ability> = free.rules.iter().map(|rule| &**rule).collect();
        assert_eq!(
            rules,
            [
                &Ability::Keyword(Keyword::Flying),
                &Ability::Keyword(Keyword::Trample)
            ]
        );
        let paid = keyworded_creature("Paid", 1, &[Color::Red], 1, 1, &[Keyword::Haste]);
        assert_eq!(paid.mana_value(), 2);
        assert_eq!(paid.rules.len(), 1);
        assert!(paid.is_type(CardType::Creature));
        assert_eq!(keyworded("Plain", 1, 1, &[]).rules.len(), 0);
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
