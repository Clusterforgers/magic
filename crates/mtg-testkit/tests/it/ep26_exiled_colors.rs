//! Goal: "Add one mana of any of the exiled card's colors" offers the colours of the card the source exiled and nothing else: an imprinted red and green card offers red and green, no imprint (or a colourless card) offers nothing, a card the source exiled from a graveyard counts for "the exiled cards' colors" while one left there does not, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::copying::{replays, run};
use crate::common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

const IMPRINT: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Choose":{"as":"picked","count":{"max":1,"min":0},"from":{"Each":[{"Not":{"Type":"Land"}},{"In":"Hand"},{"OwnedBy":"You"}]}}},{"Exile":{"as":"exiled2","what":{"Bound":"picked"}}},{"Remember":{"slot":"imprinted","what":{"Bound":"exiled2"}}}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
const IMPRINTED_MANA: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColorAmong":[{"Is":{"Remembered":{"on":"This","slot":"imprinted"}}},{"In":"Exile"}]}}}]}}"#;
const PIT_EXILE: &str = r#"{"Triggered":{"effects":[{"Exile":{"what":{"Target":0}}}],"targets":[{"bounds":{"max":3,"min":0},"what":{"Object":[{"In":"Graveyard"}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
const EXILED_WITH_MANA: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColorAmong":[{"ExiledWith":"This"},{"In":"Exile"}]}}}]}}"#;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// An artifact for {0} with an entering exile and a mana ability.
fn source(name: &str, enters: &str, mana: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(rules(enters))
            .with_rules(rules(mana)),
    )
}

/// A sorcery card of this mana cost.
fn card(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// The types the named source offers at priority, sorted.
fn offered(driver: &Driver, name: &str) -> Vec<ManaType> {
    let mut found: Vec<ManaType> = match driver.decision().map(|d| &d.kind) {
        Ok(DecisionKind::Priority(options)) => options
            .mana_abilities
            .iter()
            .filter(|m| {
                driver
                    .state()
                    .object(m.source.object)
                    .is_some_and(|o| o.card().name == name)
            })
            .map(|m| m.produces)
            .collect(),
        _ => Vec::new(),
    };
    found.sort();
    found
}

/// Answers the questions of the entering trigger: yes to the may, and the named card (or none) for the picks and targets.
fn answer(driver: &mut Driver, pick: Option<&str>) -> Result<(), ScriptError> {
    for _ in 0..20 {
        let kind = driver.decision()?.kind.clone();
        match kind {
            DecisionKind::YesNo(_) => driver.answer(Answer::YesNo(YesNoChoice(pick.is_some())))?,
            DecisionKind::ChooseCards(options) => {
                let ids = options
                    .from
                    .iter()
                    .copied()
                    .filter(|id| pick.is_some_and(|name| name_of(driver.state(), *id) == name))
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(ids)))?
            }
            DecisionKind::ChooseTargets(options) => {
                let targets: Vec<_> = options
                    .legal
                    .iter()
                    .copied()
                    .filter(|t| {
                        t.object().is_some_and(|o| {
                            pick.is_some_and(|name| name_of(driver.state(), o.id) == name)
                        })
                    })
                    .collect();
                driver.choose_targets(&targets)?
            }
            DecisionKind::Priority(_) => {
                if driver.state().stack().is_empty() {
                    return Ok(());
                }
                run(driver, 2)?;
                continue;
            }
            other => panic!("unexpected {}", other.name()),
        };
    }
    Ok(())
}

#[rstest]
#[case::a_red_and_green_card(Some("{R}{G}"), vec![ManaType::Red, ManaType::Green])]
#[case::a_colorless_card(Some("{3}"), vec![])]
#[case::nothing_imprinted(None, vec![])]
fn the_imprinted_card_gives_its_colors(
    #[case] cost: Option<&str>,
    #[case] colors: Vec<ManaType>,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2).at(1, 0, Step::Main1).hand(
            0,
            [
                source("Mox", IMPRINT, IMPRINTED_MANA),
                card("Gem", cost.unwrap_or("{U}")),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Mox")?;
    run(&mut driver, 2)?;
    answer(&mut driver, cost.map(|_| "Gem"))?;
    assert_eq!(offered(&driver, "Mox"), colors);
    if let Some(first) = colors.first() {
        driver.tap_for_mana_of("Mox", *first)?;
        let pool = driver.state().player(pid(0)).unwrap().mana().clone();
        assert_eq!((pool.get(*first), pool.total()), (1, 1));
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::exiled_with_the_pit(true, vec![ManaType::Black])]
#[case::left_in_the_graveyard(false, vec![])]
fn the_cards_exiled_with_the_source_give_their_colors(
    #[case] exile: bool,
    #[case] colors: Vec<ManaType>,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(2)
            .at(1, 0, Step::Main1)
            .hand(0, [source("Pit", PIT_EXILE, EXILED_WITH_MANA)])
            .graveyard(1, [card("Grave", "{B}")])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Pit")?;
    run(&mut driver, 2)?;
    answer(&mut driver, exile.then_some("Grave"))?;
    assert_eq!(offered(&driver, "Pit"), colors);
    replays(&driver, &setup);
    Ok(())
}
