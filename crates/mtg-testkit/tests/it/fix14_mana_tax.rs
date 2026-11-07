//! Goal: an increase to the cost of every activated ability ("activated abilities of enchanted creature cost {3} more
//! to activate") taxes mana abilities too (CR 605.1a, 601.2f), at tables of two to four seats: the taxed mana creature
//! pays inside a payment only with the tax paid from other mana, is never offered as free mana, and a modifier of the
//! non-mana abilities leaves it untaxed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An enchantment whose rule raises the cost of the abilities of creatures of the `kind` by {3}.
fn rays(kind: &str) -> Arc<CardDefinition> {
    let rule = format!(
        r#"{{"CostModifier":{{"applies_to":[{{"Type":"Creature"}}],"change":"Increase","mana":"{{3}}","activating":"{kind}"}}}}"#
    );
    Arc::new(
        CardDefinition::new("Rays")
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str(&rule).unwrap()),
    )
}

fn spell() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse("{G}").unwrap()),
    )
}

#[rstest]
fn a_tax_on_every_activated_ability_taxes_mana_abilities(
    #[values(2, 3, 4)] seats: usize,
    #[values("Llanowar Elves", "Paradise Druid")] elf: &str,
    #[values(("Every", 3, true), ("Every", 2, false), ("Mana", 2, false), ("Any", 0, true))] case: (
        &str,
        usize,
        bool,
    ),
) -> Result<(), ScriptError> {
    let (kind, islands, castable) = case;
    let mut mine = vec![ready(rays(kind)), ready(card(elf).unwrap())];
    mine.extend((0..islands).map(|_| ready(island())));
    let driver =
        Driver::from_scenario(Scenario::new(seats).hand(0, [spell()]).battlefield(0, mine))?;
    let elf_id = driver.id_of(elf)?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        return Err(ScriptError::Expectation("priority was expected".into()));
    };
    assert_eq!(!options.castable.is_empty(), castable, "{kind} {islands}");
    let free = options
        .mana_abilities
        .iter()
        .any(|option| option.source.object == elf_id);
    assert_eq!(free, kind == "Any", "{kind}");
    Ok(())
}

#[rstest]
fn the_tax_is_paid_from_other_mana(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut mine = vec![ready(rays("Every")), ready(card("Llanowar Elves").unwrap())];
    mine.extend((0..3).map(|_| ready(island())));
    let mut driver =
        Driver::from_scenario(Scenario::new(seats).hand(0, [spell()]).battlefield(0, mine))?;
    driver.cast("Spell")?;
    let tapped = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .and_then(|o| o.permanent())
                .is_some_and(|p| p.tapped())
        })
        .count();
    assert_eq!(tapped, 4);
    Ok(())
}
