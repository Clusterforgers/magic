//! Goal: a cost change for a time that counts ("Spells you cast this turn that are black and/or red cost {X} less to
//! cast, where X is the amount of life you lost this turn") counts as each spell is cast, life lost after the ability
//! resolved included, reduces only spells of its colors, and never the colored mana, at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_testkit::cards::{cost, creature, forest, mountain, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// A free sorcery with the effects `effects` (JSON).
fn sorcery(name: &str, effects: &str) -> Arc<CardDefinition> {
    let spell: Ability =
        serde_json::from_str(&format!(r#"{{"Spell":{{"effects":{effects}}}}}"#)).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(spell),
    )
}

/// Rowan's rule: black and/or red spells you cast this turn cost {1} less for each life you lost this turn.
fn rowan_rule() -> Arc<CardDefinition> {
    sorcery(
        "Prelude",
        r#"[{"Rule":{"rule":{"SpellCost":{"applies_to":["Spell",{"ControlledBy":"You"},{"AnyOf":[{"Color":"Black"},{"Color":"Red"}]}],"change":"Reduce","mana":"{1}","per":{"Tallied":{"who":"You","what":"LifeLost"}}}},"duration":"UntilEndOfTurn"}}]"#,
    )
}

/// A free sorcery: its caster loses `amount` life.
fn bleed(name: &str, amount: u32) -> Arc<CardDefinition> {
    sorcery(name, &format!(r#"[{{"LoseLife":{{"amount":{amount}}}}}]"#))
}

fn pass_all(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 loses `before` life, casts the rule, loses `after` life and starts casting a 4-generic creature of `color`.
fn wurm_cost(
    seats: usize,
    (before, after): (u32, u32),
    color: Color,
) -> Result<String, ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [lands(4, swamp), lands(4, mountain), lands(4, forest)].concat(),
        )
        .hand(
            0,
            vec![
                bleed("Before", before),
                rowan_rule(),
                bleed("After", after),
                creature("Wurm", 4, &[color], 6, 4),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    for name in ["Before", "Prelude", "After"] {
        driver.cast(name)?;
        pass_all(&mut driver, seats)?;
    }
    driver.begin_cast("Wurm")?;
    Ok(driver.pay_options()?.cost.to_string())
}

fn written(text: &str) -> String {
    ManaCost::parse(text).unwrap().to_string()
}

#[rstest]
#[case::no_life_lost((0, 0), Color::Black, "{4}{B}")]
#[case::lost_before((2, 0), Color::Black, "{2}{B}")]
#[case::lost_after_counts((0, 3), Color::Red, "{1}{R}")]
#[case::both((1, 2), Color::Black, "{1}{B}")]
#[case::never_the_colored_mana((5, 4), Color::Red, "{R}")]
#[case::other_colors_pay_in_full((2, 2), Color::Green, "{4}{G}")]
fn the_count_is_read_as_each_spell_is_cast(
    #[values(2, 3, 4)] seats: usize,
    #[case] lost: (u32, u32),
    #[case] color: Color,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    assert_eq!(wurm_cost(seats, lost, color)?, written(expected));
    Ok(())
}
