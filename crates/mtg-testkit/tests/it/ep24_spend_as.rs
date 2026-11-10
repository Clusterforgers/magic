//! Goal: "You may spend mana as though it were mana of any color" (CR 609.4b) lets red mana pay green symbols, for every spell or only the spells the rule names, never pays `{C}` unless the rule says any type, reaches only its controller, and the cast replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::copying::{replays, run};
use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::modify::{GameRule, SpendAs};
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An artifact with the rule.
fn orrery(spend_as: SpendAs, spells: Option<Selector>) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Orrery")
            .with_type(CardType::Artifact)
            .with_rules(Ability::Rule {
                rule: GameRule::SpendManaAs {
                    who: Ref::You,
                    spend_as,
                    spells,
                },
                condition: None,
                from: ZoneKind::Battlefield,
            }),
    )
}

/// A card of this type and mana cost.
fn spell(kind: CardType, cost: &str) -> Arc<CardDefinition> {
    let card = CardDefinition::new("Spell")
        .with_type(kind)
        .with_mana_cost(ManaCost::parse(cost).unwrap());
    Arc::new(if kind == CardType::Creature {
        card.with_power_toughness(2, 2)
    } else {
        card
    })
}

fn creatures() -> Option<Selector> {
    Some(Selector(vec![Pred::Type(CardType::Creature)]))
}

/// Whether seat 0 may cast its spell now.
fn castable(driver: &Driver) -> bool {
    let id = driver.state().zone(Zone::Hand(pid(0)))[0];
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.castable.contains(&id),
        _ => false,
    }
}

#[rstest]
#[case::no_rule(None, CardType::Creature, "{G}{G}", 0, false)]
#[case::any_color(Some((SpendAs::AnyColor, None)), CardType::Creature, "{G}{G}", 0, true)]
#[case::any_color_for_creatures(Some((SpendAs::AnyColor, creatures())), CardType::Creature, "{G}{G}", 0, true)]
#[case::creatures_only(Some((SpendAs::AnyColor, creatures())), CardType::Artifact, "{G}{G}", 0, false)]
#[case::any_color_is_no_colorless(Some((SpendAs::AnyColor, None)), CardType::Artifact, "{C}", 0, false)]
#[case::any_type_pays_colorless(Some((SpendAs::AnyType, None)), CardType::Artifact, "{C}", 0, true)]
#[case::an_opponent_s_rule(Some((SpendAs::AnyColor, None)), CardType::Creature, "{G}{G}", 1, false)]
fn mana_of_another_color_pays_where_the_rule_says(
    #[case] rule: Option<(SpendAs, Option<Selector>)>,
    #[case] kind: CardType,
    #[case] cost: &str,
    #[case] owner: usize,
    #[case] can: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        let mut mine = vec![ready(mountain()), ready(mountain())];
        let mut theirs = Vec::new();
        if let Some((spend_as, spells)) = rule.clone() {
            let held = if owner == 0 { &mut mine } else { &mut theirs };
            held.push(ready(orrery(spend_as, spells)));
        }
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .hand(0, [spell(kind, cost)])
            .battlefield(0, mine)
            .battlefield(1, theirs)
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    assert_eq!(castable(&driver), can);
    if can {
        driver.cast("Spell")?;
        run(&mut driver, 2)?;
        let id = driver.id_of("Spell")?;
        let zone = driver.state().object(id).unwrap().zone();
        assert_eq!(zone, Zone::Battlefield);
        assert_eq!(driver.state().player(pid(0)).unwrap().mana().total(), 0);
    }
    replays(&driver, &setup);
    Ok(())
}
