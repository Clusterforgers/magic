//! Goal: a dig that takes at most one card for each alternative of its filter ("a creature card and/or a land card from
//! among the top four") offers each alternative its own cards, never gives one card to two alternatives, may take none,
//! puts the taken cards into the hand and the rest on the bottom, and replays to the same state, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, cost, creature, forest};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// "Look at the top four cards; you may take a creature card and/or a land card; the rest go to the bottom."
const DIG: &str = r#"{"Spell":{"effects":[
    {"LookAtTop":{"as":"top","count":4}},
    {"Choose":{"as":"c","count":{"min":0,"max":1},"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Type":"Creature"}]}}},
    {"Choose":{"as":"l","count":{"min":0,"max":1},"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Type":"Land"},{"Not":{"Is":{"Bound":"c"}}}]}}},
    {"Move":{"to":"Hand","what":{"Bound":"c"}}},
    {"Move":{"to":"Hand","what":{"Bound":"l"}}},
    {"Move":{"to":{"Library":{"place":"Bottom","order":"Chosen"}},"what":{"Bound":"top"}}}
]}}"#;

fn dig() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(DIG).unwrap();
    Arc::new(
        CardDefinition::new("Dig")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

/// A creature that is also a land.
fn dryad() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Dryad")
            .with_type(CardType::Creature)
            .with_type(CardType::Land)
            .with_power_toughness(0, 1),
    )
}

fn table(top: Vec<Arc<CardDefinition>>, seats: usize) -> Scenario {
    let rest = || (0..6).map(|_| forest()).collect::<Vec<_>>();
    (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            let mut library = if seat == 0 { top.clone() } else { Vec::new() };
            library.extend(rest());
            scenario.library(seat, library)
        })
        .hand(0, vec![dig()])
}

/// Casts the dig and answers each choice with the first option, or with nothing when `take` is false.
fn run(driver: &mut Driver, take: bool) -> Result<usize, ScriptError> {
    driver.cast("Dig")?;
    let mut asked = 0;
    for _ in 0..200 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(asked);
        };
        match decision.kind {
            DecisionKind::ChooseCards(options) => {
                asked += 1;
                let pick = options
                    .from
                    .iter()
                    .copied()
                    .take(usize::from(take))
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(pick)))?;
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(asked);
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the dig".into()))
}

fn hand(driver: &Driver) -> Vec<String> {
    let mut names: Vec<String> = driver
        .state()
        .zone(Zone::Hand(PlayerId::from_index(0)))
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .map(|name| {
            if name == "Alpha" || name == "Beta" {
                "Creature".to_owned()
            } else {
                name
            }
        })
        .collect();
    names.sort();
    names
}

#[rstest]
#[case::one_of_each(true, &["Creature", "Forest"])]
#[case::none_taken(false, &[])]
fn a_card_is_taken_for_each_alternative(
    #[values(2, 3, 4)] seats: usize,
    #[case] take: bool,
    #[case] taken: &[&str],
) -> Result<(), ScriptError> {
    let top = vec![
        creature("Alpha", 1, &[], 2, 2),
        creature("Beta", 1, &[], 2, 2),
        forest(),
        forest(),
    ];
    let setup = table(top.clone(), seats).setup().unwrap();
    let mut driver = Driver::from_scenario(table(top, seats))?;
    run(&mut driver, take)?;
    assert_eq!(hand(&driver), taken);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn one_card_does_not_count_for_two_alternatives(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // The only creature and the only land is one card: it is taken once, and the land alternative has nothing left.
    let top = vec![
        dryad(),
        burn("Bolt1", 1),
        burn("Bolt2", 1),
        burn("Bolt3", 1),
    ];
    let mut driver = Driver::from_scenario(table(top, seats))?;
    let asked = run(&mut driver, true)?;
    assert_eq!((hand(&driver), asked), (vec!["Dryad".to_owned()], 1));
    Ok(())
}
