//! Goal: the cards that cast a target card during a resolution play as printed at tables of two, three and four seats:
//! Torrential Gearhulk and Impulsivity cast the target instant for nothing when their controller says yes, the spell is
//! exiled instead of going to its owner's graveyard, a no leaves the card where it was, and the log replays to the same
//! state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, gain_life, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::names;

/// An instant for `{3}` that gains 3 life: too dear to cast from the lands left, so only the free cast casts it.
fn c11b_brew() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Brew")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(3, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(3)],
            }),
    )
}

/// Answers until the stack is empty, saying `yes` to every question.
fn c11b_settle(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let state = driver.state();
        if state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 has the card in hand and seven lands; Brew lies in the graveyard of seat `owner` and nowhere else.
fn c11b_table(seats: usize, name: &str, owner: usize) -> Scenario {
    let lands: Vec<_> = (0..7)
        .map(|_| {
            ready(if name == "Impulsivity" {
                mountain()
            } else {
                island()
            })
        })
        .collect();
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(name).unwrap()])
        .battlefield(0, lands)
        .graveyard(owner, [c11b_brew()]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(island())]);
    }
    scenario
}

#[rstest]
fn c11b_the_target_instant_is_cast_for_nothing_and_exiled(
    #[values(2, 3, 4)] seats: usize,
    #[values("Torrential Gearhulk", "Impulsivity")] name: &str,
    #[values(false, true)] yes: bool,
) -> Result<(), ScriptError> {
    // Torrential Gearhulk takes an instant from its controller's graveyard, Impulsivity from any graveyard.
    let owner = if name == "Impulsivity" { seats - 1 } else { 0 };
    let scenario = c11b_table(seats, name, owner);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(PlayerId(0)).unwrap().life();
    driver.cast(name)?;
    c11b_settle(&mut driver, yes)?;
    let owner = PlayerId(u64::try_from(owner).unwrap());
    let state = driver.state();
    let gained = if yes { 3 } else { 0 };
    assert_eq!(state.player(PlayerId(0)).unwrap().life(), life + gained);
    let (graveyard, exile) = if yes {
        (Vec::new(), vec!["Brew".to_owned()])
    } else {
        (vec!["Brew".to_owned()], Vec::new())
    };
    assert_eq!(names(state, Zone::Graveyard(owner)), graveyard);
    assert_eq!(names(state, Zone::Exile), exile);
    replays(&driver, &setup);
    Ok(())
}
