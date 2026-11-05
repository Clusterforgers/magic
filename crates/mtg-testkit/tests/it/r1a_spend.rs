//! Goal: a cast permission that lets mana of any type or color pay (CR 609.4b) offers a colored spell to a caster whose lands are
//! of the wrong color, and a plain permission does not; the cost the caster pays is the same total.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A permanent that lets its controller cast creature cards from a graveyard, spending mana as `spend` says.
fn spender(spend: &str) -> Arc<CardDefinition> {
    let extra = if spend.is_empty() {
        String::new()
    } else {
        format!(r#","spend_as":"{spend}""#)
    };
    Arc::new(
        CardDefinition::new("Spender")
            .with_type(CardType::Artifact)
            .with_rules(
                serde_json::from_str(&format!(
                    r#"{{"Rule":{{"rule":{{"MayCast":{{"from":"Graveyard","what":[{{"Type":"Creature"}}]{extra}}}}}}}}}"#
                ))
                .unwrap(),
            ),
    )
}

#[rstest]
#[case::mana_of_the_wrong_color_does_not_pay("", 2, false)]
#[case::mana_of_any_type_pays("AnyType", 2, true)]
#[case::mana_of_any_color_pays("AnyColor", 2, true)]
#[case::the_total_is_still_owed("AnyType", 1, false)]
fn a_permission_lets_mana_of_any_type_or_color_pay_a_colored_cost(
    #[case] spend: &str,
    #[case] forests: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut mine = lands(forests, forest);
    mine.push(ready(spender(spend)));
    let table = Scenario::new(2)
        .graveyard(0, [creature("Bear", 1, &[Color::Red], 2, 2)])
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(table)?;
    assert_eq!(driver.begin_cast("Bear").is_ok(), offered);
    Ok(())
}
