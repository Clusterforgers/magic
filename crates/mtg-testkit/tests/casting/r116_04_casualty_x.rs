//! Goal: casualty X (CR 702.153, 707.9b) plays at tables of two to four seats: as Ob Nixilis, the Adversary is cast its caster may sacrifice a creature with power X or more, X announced with the sacrifice, and the cast trigger copies the spell into a token Ob Nixilis that isn't legendary and has starting loyalty X beside the original with its printed loyalty; declining the sacrifice sets X to 0 and makes no copy; no X above the power of a creature the caster controls is offered; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::card::Supertype;
use mtg_engine::choose::{CardsChoice, CostsChoice, NumberChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn table(seats: usize) -> Scenario {
    let mine = vec![
        ready(free_creature("Brute", 4, 4)),
        ready(bear()),
        ready(swamp()),
        ready(mountain()),
        ready(mountain()),
    ];
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .hand(0, [card("Ob Nixilis, the Adversary").unwrap()]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    scenario
}

/// The loyalty of each Ob Nixilis on the battlefield, and whether it is legendary.
fn walkers(driver: &Driver) -> Vec<(u32, bool)> {
    let state = driver.state();
    let mut found: Vec<(u32, bool)> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == "Ob Nixilis, the Adversary")
        .map(|object| {
            (
                object
                    .permanent()
                    .unwrap()
                    .counters_of(&CounterKind::Loyalty),
                object.card().supertypes.contains(&Supertype::Legendary),
            )
        })
        .collect();
    found.sort_unstable();
    found
}

#[rstest]
fn casualty_x_copies_with_starting_loyalty_x(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some(2), Some(4))] x: Option<u32>,
) -> Result<(), ScriptError> {
    let scenario = table(seats);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.begin_cast("Ob Nixilis, the Adversary")?;
    let mut most = None;
    let mut asked = Vec::new();
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        asked.push(decision.kind.name());
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                break;
            }
            DecisionKind::ChooseCosts(_) => {
                let chosen = if x.is_some() { vec![0] } else { vec![] };
                driver.answer(Answer::ChooseCosts(CostsChoice::Costs(chosen)))?;
            }
            DecisionKind::ChooseNumber(options) => {
                most = options.max;
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x.unwrap())))?;
            }
            DecisionKind::ChooseCards(options) => {
                let brute = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| driver.state().object(*id).unwrap().card().name == "Brute")
                    .unwrap();
                driver.answer(Answer::ChooseCards(CardsChoice(vec![brute])))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    match x {
        None => assert_eq!(
            walkers(&driver),
            vec![(3, true)],
            "no copy without casualty"
        ),
        Some(x) => {
            assert_eq!(
                most,
                Some(4),
                "X up to the greatest power the caster controls: {asked:?}"
            );
            assert_eq!(
                walkers(&driver),
                {
                    let mut both = vec![(3, true), (x, false)];
                    both.sort_unstable();
                    both
                },
                "a nonlegendary copy with starting loyalty X"
            );
            let brute_gone = driver
                .state()
                .zone(Zone::Battlefield)
                .iter()
                .all(|id| driver.state().object(*id).unwrap().card().name != "Brute");
            assert!(brute_gone, "the Brute was sacrificed");
        }
    }
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
