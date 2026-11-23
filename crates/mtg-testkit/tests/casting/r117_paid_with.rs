//! Goal: an amount that counts the payments of one "you may" additional cost among several (`Amount::R117AdditionalPaidWith`, Primitive Justice) sets the targets and the life of a spell at tables of two to four seats: each {1}{R} paid adds one artifact target, each {1}{G} one more and 1 life, every artifact named is another one, and each payment is counted for its own cost only.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{NumberChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn relic(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(1, &[])),
    )
}

const RELICS: [&str; 4] = ["Relic A", "Relic B", "Relic C", "Relic D"];

/// Seat 0 holds the spell with four Mountains and four Forests; the last seat controls four artifacts.
fn table(seats: usize) -> Scenario {
    let mut lands: Vec<_> = (0..4).map(|_| ready(mountain())).collect();
    lands.extend((0..4).map(|_| ready(forest())));
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, lands)
        .battlefield(seats - 1, RELICS.map(|name| ready(relic(name))))
        .hand(0, [card("Primitive Justice").unwrap()])
}

/// Casts the spell paying {1}{R} `red` and {1}{G} `green` more times; each target specification takes the next
/// artifacts in order. The sizes of the target specifications come back.
fn cast(driver: &mut Driver, red: u32, green: u32) -> Result<Vec<u32>, ScriptError> {
    driver.begin_cast("Primitive Justice")?;
    let mut times = vec![red, green].into_iter();
    let mut asked = Vec::new();
    let mut next = 0;
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(
                    times.next().unwrap(),
                )))?;
            }
            DecisionKind::ChooseTargets(options) => {
                asked.push(options.min);
                let chosen: Vec<Target> = RELICS[next..next + options.min as usize]
                    .iter()
                    .map(|name| driver.target_named(name).unwrap())
                    .collect();
                for target in &chosen {
                    assert!(options.legal.contains(target));
                }
                if next > 0 {
                    let earlier = driver.target_named(RELICS[0]).unwrap();
                    assert!(!options.legal.contains(&earlier), "another target artifact");
                }
                next += chosen.len();
                driver.choose_targets(&chosen)?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the spell did not resolve: {asked:?}");
}

#[rstest]
#[case::none(0, 0)]
#[case::one_red(1, 0)]
#[case::one_green(0, 1)]
#[case::both(1, 2)]
fn each_payment_adds_its_own_target(
    #[values(2, 3, 4)] seats: usize,
    #[case] red: u32,
    #[case] green: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats))?;
    let sizes = cast(&mut driver, red, green)?;
    assert_eq!(sizes.first(), Some(&1));
    let destroyed = 1 + red + green;
    for (at, name) in RELICS.iter().enumerate() {
        assert_eq!(
            driver.id_of(name).is_err(),
            at < destroyed as usize,
            "{name}"
        );
    }
    let life = driver
        .state()
        .player(PlayerId::from_index(0))
        .unwrap()
        .life();
    assert_eq!(life, 20 + i32::try_from(green).unwrap());
    let lands_tapped = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .unwrap()
                .permanent()
                .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count();
    assert_eq!(
        lands_tapped,
        2 + 2 * (red + green) as usize,
        "each payment is {{1}} and one color"
    );
    Ok(())
}
