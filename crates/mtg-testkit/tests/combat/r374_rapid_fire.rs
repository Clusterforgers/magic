//! Goal: a spell cast only before blockers are declared is offered in the first main phase and the declare attackers step
//! and not after combat, and its target gains first strike and, when it has no rampage, rampage 2, which pumps it for each
//! blocker beyond the first, at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::{has_keyword, keywords_of};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{bear, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

/// Seat 0's turn at `step`: it holds Rapid Fire and controls a Bear and four Plains.
fn table(seats: usize, step: Step) -> Scenario {
    Scenario::new(seats)
        .at(2, 0, step)
        .hand(0, vec![corpus::card("Rapid Fire").unwrap()])
        .battlefield(0, (0..4).map(|_| ready(plains())).chain([ready(bear())]))
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        _ => false,
    }
}

#[rstest]
#[case::first_main(Step::Main1, true)]
#[case::beginning_of_combat(Step::BeginCombat, true)]
#[case::second_main(Step::Main2, false)]
#[case::end_step(Step::End, false)]
fn offered_only_before_blockers(
    #[case] step: Step,
    #[case] offered: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(seats, step))?;
    assert_eq!(castable(&driver), offered, "{step:?}");
    Ok(())
}

#[rstest]
fn the_bear_gains_first_strike_and_rampage_two(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, Step::Main1))?;
    driver.begin_cast("Rapid Fire")?;
    let bear = driver.target_named("Grizzly Bears")?;
    driver.choose_targets(&[bear])?;
    driver.pay_tapping(&["Plains", "Plains", "Plains", "Plains"], 0)?;
    while !driver
        .state()
        .zone(mtg_engine::zones::Zone::Stack)
        .is_empty()
    {
        driver.pass()?;
    }
    let id = driver.id_of("Grizzly Bears")?;
    let state = driver.state();
    assert!(has_keyword(state, id, &Keyword::FirstStrike));
    let rampage: Vec<&Keyword> = keywords_of(state, id)
        .into_iter()
        .filter(|keyword| matches!(keyword, Keyword::Macro { name, .. } if name == "rampage"))
        .collect();
    assert_eq!(
        rampage,
        vec![&Keyword::Macro {
            name: "rampage".to_owned(),
            amount: 2
        }]
    );
    Ok(())
}
