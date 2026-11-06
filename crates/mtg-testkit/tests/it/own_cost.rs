//! Goal: a keyword a static ability gives with a cost equal to the card's own mana cost (Underworld Breach's escape, a Djinn's replicate) reads the mana cost off each card it reaches: the card is offered exactly when that cost plus the rest of the cost is payable, and a land card is never offered.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::name_of;

/// An enchantment: "Each nonland card in your graveyard has escape. The escape cost is equal to the card's mana cost plus
/// exile three other cards from your graveyard."
fn breach() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Static":{"affected":{"Each":[{"OwnedBy":"You"},{"Not":{"Type":"Land"}},{"In":"Graveyard"}]},"mods":[{"AddKeyword":{"Escape":["OwnManaCost",{"ExileFromGraveyard":{"count":3,"filter":["Other"]}}]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Breach")
            .with_type(CardType::Enchantment)
            .with_rules(rules),
    )
}

fn castable(driver: &Driver, name: &str) -> bool {
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        return false;
    };
    options
        .castable
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

const FODDER: [&str; 3] = [
    "Dregscape Zombie",
    "Kathari Screecher",
    "Reconstructed Thopter",
];

#[rstest]
#[case::the_mana_and_the_cards("Staggershock", 3, 3, true)]
#[case::a_land_short("Staggershock", 2, 3, false)]
#[case::a_card_short("Staggershock", 3, 2, false)]
#[case::a_cheaper_card_needs_less_mana("Lightning Bolt", 1, 3, true)]
#[case::a_land_card_has_no_escape("Forest", 5, 3, false)]
fn the_cost_of_a_given_escape_is_the_mana_cost_of_the_card_in_the_graveyard(
    #[case] escaping: &str,
    #[case] lands: usize,
    #[case] fodder: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .graveyard_named(
                0,
                std::iter::once(escaping).chain(FODDER[..fodder].iter().copied()),
            )
            .battlefield(
                0,
                std::iter::once(ready(breach())).chain((0..lands).map(|_| ready(mountain()))),
            ),
    )?;
    assert_eq!(castable(&driver, escaping), offered);
    Ok(())
}

/// A creature: "Each creature card in your graveyard has `keyword`. Its cost is equal to its mana cost."
fn giver(keyword: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"OwnedBy":"You"}},{{"In":"Graveyard"}}]}},"mods":[{{"AddKeyword":{{"{keyword}":["OwnManaCost"]}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Giver")
            .with_type(CardType::Creature)
            .with_rules(rules),
    )
}

#[rstest]
#[case::scavenge_with_the_mana("Scavenge", 3, true)]
#[case::scavenge_a_land_short("Scavenge", 2, false)]
#[case::encore_with_the_mana("Encore", 3, true)]
#[case::encore_a_land_short("Encore", 2, false)]
fn a_given_graveyard_ability_costs_the_mana_cost_of_its_card(
    #[case] keyword: &str,
    #[case] swamps: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .graveyard_named(0, ["Kathari Screecher"])
            .battlefield(
                0,
                std::iter::once(ready(giver(keyword)))
                    .chain(std::iter::once(ready(mtg_testkit::cards::bear())))
                    .chain((0..swamps).map(|_| ready(mtg_testkit::cards::island()))),
            ),
    )?;
    let found = driver.offered_abilities("Kathari Screecher")?;
    assert_eq!(!found.is_empty(), offered, "{found:?}");
    Ok(())
}

/// A creature: "Each instant and sorcery spell you cast has replicate. The replicate cost is equal to its mana cost."
fn djinn() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Static":{"affected":{"Each":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"ControlledBy":"You"},{"WasCast":{}},{"In":"Stack"}]},"mods":[{"AddKeyword":{"Replicate":["OwnManaCost"]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Djinn")
            .with_type(CardType::Creature)
            .with_rules(rules),
    )
}

/// A sorcery for `{1}{U}` whose only effect is that its controller gains 2 life.
fn plain() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Plain")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(1, &[Color::Blue]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::GainLife {
                    who: Ref::You,
                    amount: Amount::Fixed(2),
                }],
            }),
    )
}

#[rstest]
#[case::the_cost_only(2, None)]
#[case::one_spare_cost(4, Some(1))]
#[case::two_spare_costs(6, Some(2))]
fn a_given_replicate_costs_the_mana_cost_of_the_spell_cast(
    #[case] islands: usize,
    #[case] most: Option<u32>,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .hand(0, vec![plain()])
            .battlefield(
                0,
                std::iter::once(ready(djinn())).chain((0..islands).map(|_| ready(island()))),
            ),
    )?;
    driver.begin_cast("Plain")?;
    // Plain costs {1}{U}: every full cost left over is one more payment.
    match (most, &driver.decision()?.kind) {
        (Some(most), DecisionKind::ChooseNumber(options)) => assert_eq!(options.max, Some(most)),
        (None, kind) => assert!(
            !matches!(kind, DecisionKind::ChooseNumber(_)),
            "{}",
            kind.name()
        ),
        (Some(_), kind) => panic!("the payments are asked: {}", kind.name()),
    }
    Ok(())
}
