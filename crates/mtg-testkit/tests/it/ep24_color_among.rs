//! Goal: "Add one mana of any color among legendary permanents you control" offers one mana of each colour the chosen objects have now: a red and green legend offers red and green, an opponent's legend and a nonlegendary permanent offer nothing, a colourless legend offers nothing, and the mana added replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::copying::replays;
use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Activators, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::{Effect, Produce};
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::spend::ManaLasts;
use mtg_cards::value::Amount;
use mtg_core::card::{CardType, Supertype};
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A land with "{T}: Add one mana of any color among legendary permanents you control."
fn plaza() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Plaza")
            .with_type(CardType::Land)
            .with_rules(Ability::Activated {
                cost: Cost(vec![CostItem::Tap]),
                timing: Timing::Instant,
                condition: None,
                limit: None,
                by: Activators::Controller,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: vec![Effect::AddMana {
                    who: Ref::You,
                    mana: Produce::AnyColorAmong(Selector(vec![
                        Pred::Supertype(Supertype::Legendary),
                        Pred::ControlledBy(Ref::You),
                    ])),
                    times: Amount::one(),
                    only: Vec::new(),
                    lasts: ManaLasts::Step,
                    when_spent: None,
                }],
            }),
    )
}

/// A creature with this mana cost, legendary or not.
fn creature(name: &str, cost: &str, legendary: bool) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2)
        .with_mana_cost(ManaCost::parse(cost).unwrap());
    if legendary {
        card = card.with_supertype(Supertype::Legendary);
    }
    Arc::new(card)
}

/// The types the plaza offers, in the order of the wheel.
fn offered(driver: &Driver) -> Vec<ManaType> {
    let mut found: Vec<ManaType> = driver
        .decision()
        .ok()
        .and_then(|decision| match &decision.kind {
            mtg_engine::decision::DecisionKind::Priority(options) => Some(
                options
                    .mana_abilities
                    .iter()
                    .filter(|m| {
                        driver
                            .state()
                            .object(m.source.object)
                            .is_some_and(|o| o.card().name == "Plaza")
                    })
                    .map(|m| m.produces)
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    found.sort();
    found
}

#[rstest]
#[case::a_red_and_green_legend(creature("Legend", "{R}{G}", true), 0, vec![ManaType::Red, ManaType::Green])]
#[case::an_opponent_s_legend(creature("Legend", "{R}{G}", true), 1, vec![])]
#[case::a_nonlegendary_creature(creature("Bear", "{R}{G}", false), 0, vec![])]
#[case::a_colorless_legend(creature("Golem", "{3}", true), 0, vec![])]
fn the_plaza_offers_the_colors_of_the_legends_you_control(
    #[case] other: Arc<CardDefinition>,
    #[case] owner: usize,
    #[case] colors: Vec<ManaType>,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut mine = vec![ready(plaza())];
        let mut theirs = Vec::new();
        let held = if owner == 0 { &mut mine } else { &mut theirs };
        held.push(ready(other.clone()));
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .battlefield(0, mine)
            .battlefield(1, theirs)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    assert_eq!(offered(&driver), colors);
    if let Some(first) = colors.first() {
        driver.tap_for_mana_of("Plaza", *first)?;
        let pool = driver.state().player(pid(0)).unwrap().mana().clone();
        assert_eq!((pool.get(*first), pool.total()), (1, 1));
    }
    replays(&driver, &setup);
    Ok(())
}
