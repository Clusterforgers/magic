//! Goal: "the greatest mana value among instant and sorcery spells you've cast this turn" reads the turn's casts as they
//! were cast at two to four seats: Rootha makes no Elemental before any such spell (a creature spell is a near miss),
//! a 1/1 after Lightning Bolt and a 3/3 after Lightning Bolt and Compulsive Research, with flying and haste, and the log
//! replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls Rootha, three Islands and three Mountains, and holds the spells; every library holds bears.
fn r379_table(seats: usize, spells: &[&str]) -> Scenario {
    let mut ours = vec![card("Rootha, Mastering the Moment").unwrap()];
    ours.extend((0..3).map(|_| island()));
    ours.extend((0..3).map(|_| mountain()));
    let hand: Vec<_> = spells
        .iter()
        .map(|name| {
            if *name == "Sprite" {
                free_creature("Sprite", 1, 1)
            } else {
                card(name).unwrap()
            }
        })
        .collect();
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, ours.into_iter().map(ready).collect::<Vec<_>>());
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| bear()));
    }
    scenario
}

/// Answers until the stack is empty, targeting seat 1 with Lightning Bolt and seat 0 with any other spell.
fn r379_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let bolt = driver
                    .state()
                    .stack()
                    .iter()
                    .chain(driver.state().zone(Zone::Hand(decision.player)).iter())
                    .any(|id| driver.state().object(*id).unwrap().card().name == "Lightning Bolt");
                let seat = usize::from(bolt);
                let target = driver.target_player(seat);
                driver.choose_targets(&[target])?;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision))?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

#[rstest]
#[case::nothing_cast(&[], None)]
#[case::near_miss_a_creature_spell(&["Sprite"], None)]
#[case::one_bolt(&["Lightning Bolt"], Some(1))]
#[case::bolt_and_research(&["Lightning Bolt", "Compulsive Research"], Some(3))]
fn r379_rootha_makes_the_greatest_elemental(
    #[values(2, 3, 4)] seats: usize,
    #[case] spells: &[&str],
    #[case] size: Option<i32>,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r379_table(seats, spells))?;
    for spell in spells {
        driver.begin_cast(spell)?;
        r379_settle(&mut driver)?;
    }
    driver.pass_until(Step::DeclareAttackers)?;
    let state = driver.state();
    let elementals: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).unwrap().card().name == "Elemental")
        .copied()
        .collect();
    match size {
        None => assert!(
            elementals.is_empty(),
            "no instant or sorcery spell was cast"
        ),
        Some(size) => {
            assert_eq!(elementals.len(), 1);
            let seen = characteristics(state, elementals[0]).unwrap();
            assert_eq!((seen.power, seen.toughness), (Some(size), Some(size)));
            for keyword in [Keyword::Flying, Keyword::Haste] {
                assert!(has_keyword(state, elementals[0], &keyword), "{keyword:?}");
            }
        }
    }
    let setup = r379_table(seats, spells).setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
